use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::router::AppState;
use crate::BffError;

/// Dummy bcrypt hash used when the username is not found, so that bcrypt::verify
/// always runs and response time is constant regardless of whether the user exists.
const DUMMY_HASH: &str = "$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m";

const MAX_LOGIN_ATTEMPTS: u32 = 10;
const RATE_WINDOW_SECS: u64 = 60;

static LOGIN_ATTEMPTS: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

async fn check_rate_limit(username: &str) -> Result<(), BffError> {
    let mut attempts = LOGIN_ATTEMPTS.lock().await;
    let now = Instant::now();
    let window = std::time::Duration::from_secs(RATE_WINDOW_SECS);

    let entry = attempts.entry(username.to_string()).or_default();
    entry.retain(|t| now.duration_since(*t) < window);

    if entry.len() >= MAX_LOGIN_ATTEMPTS as usize {
        return Err(BffError::TooManyRequests(
            "too many login attempts, try again later".into(),
        ));
    }
    entry.push(now);
    Ok(())
}

#[derive(sqlx::FromRow)]
struct UserRow {
    user_id: String,
    username: String,
    password_hash: String,
    role: String,
}

pub async fn login(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let username = payload
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing username".into()))?;

    check_rate_limit(username).await?;

    let password = payload
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing password".into()))?;

    let user = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, username, password_hash, role FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "login db query failed");
        BffError::Internal("authentication service unavailable".into())
    })?;

    let (user_row, hash_to_check) = match user {
        Some(u) => {
            let hash = u.password_hash.clone();
            (Some(u), hash)
        }
        None => (None, DUMMY_HASH.to_string()),
    };

    let valid = bcrypt::verify(password, &hash_to_check).map_err(|e| {
        tracing::error!(error = %e, "bcrypt verification failed");
        BffError::Internal("authentication service unavailable".into())
    })?;

    let user = match user_row {
        Some(u) if valid => u,
        _ => {
            return Err(BffError::Unauthorized(
                "Invalid username or password".into(),
            ))
        }
    };

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 24 * 60 * 60;

    let claims = crate::auth::Claims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
        exp,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| BffError::Internal(format!("failed to encode token: {}", e)))?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "username": user.username,
            "role": user.role
        }
    })))
}

#[cfg(test)]
mod tests {
    /// Verify that bcrypt::verify works with the hash seeded in migration 0008.
    /// Hash: $2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m
    /// Password: "admin"
    #[test]
    fn bcrypt_verify_known_admin_hash() {
        let hash = "$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m";
        let result = bcrypt::verify("admin", hash).expect("bcrypt::verify should not error");
        assert!(
            result,
            "bcrypt::verify should return true for admin/known-hash"
        );
    }

    #[test]
    fn bcrypt_verify_wrong_password_fails() {
        let hash = "$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m";
        let result = bcrypt::verify("wrong", hash).expect("bcrypt::verify should not error");
        assert!(
            !result,
            "bcrypt::verify should return false for wrong password"
        );
    }

    /// Verify that the timing-attack mitigation path works: when a user is not found we
    /// run bcrypt::verify against DUMMY_HASH so the response time is constant.
    /// A valid password checked against a hash of a *different* password must return false.
    #[test]
    fn bcrypt_verify_valid_password_against_dummy_hash_returns_false() {
        // DUMMY_HASH is a bcrypt hash of "admin".  Verifying a *different* valid password
        // ("hunter2") against it must return false, confirming the dummy path rejects correctly.
        let result = bcrypt::verify("hunter2", super::DUMMY_HASH)
            .expect("bcrypt::verify should not error against dummy hash");
        assert!(
            !result,
            "bcrypt::verify should return false when the password does not match the dummy hash"
        );
    }
}
