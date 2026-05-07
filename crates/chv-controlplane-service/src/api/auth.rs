use axum::{extract::State, http::StatusCode, response::Json, Json as AxumJson};
use chv_webui_bff::auth::Claims;
use chv_webui_bff::AppState;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn login_handler(
    State(state): State<AppState>,
    AxumJson(payload): AxumJson<Value>,
) -> Result<axum::response::Json<serde_json::Value>, chv_webui_bff::BffError> {
    let username = match payload.get("username").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => {
            return Err(chv_webui_bff::BffError::BadRequest(
                "missing username".into(),
            ));
        }
    };

    let password = match payload.get("password").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            return Err(chv_webui_bff::BffError::BadRequest(
                "missing password".into(),
            ));
        }
    };

    // Look up user by username
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, password_hash, role FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            return Err(chv_webui_bff::BffError::Unauthorized(
                "Invalid credentials".into(),
            ));
        }
        Err(e) => {
            tracing::error!(error = %e, "db error during login");
            return Err(chv_webui_bff::BffError::Internal("Internal error".into()));
        }
    };

    let password_ok = match bcrypt::verify(password, &row.password_hash) {
        Ok(ok) => ok,
        Err(e) => {
            tracing::error!(error = %e, "bcrypt verify error");
            return Err(chv_webui_bff::BffError::Internal("Internal error".into()));
        }
    };
    if !password_ok {
        return Err(chv_webui_bff::BffError::Unauthorized(
            "Invalid credentials".into(),
        ));
    }

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 24 * 60 * 60; // 24 hours

    let claims = Claims {
        sub: row.user_id.clone(),
        username: username.to_string(),
        role: row.role.clone(),
        exp,
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let token = match jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to encode jwt token");
            return Err(chv_webui_bff::BffError::Internal(
                "failed to generate token".into(),
            ));
        }
    };

    Ok(axum::response::Json(serde_json::json!({
        "token": token,
        "user": {
            "id": row.user_id,
            "username": username,
            "role": row.role,
        }
    })))
}

pub async fn me_handler(
    chv_webui_bff::auth::BearerToken(claims): chv_webui_bff::auth::BearerToken,
) -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": claims.sub,
            "username": claims.username,
            "role": claims.role
        })),
    )
}

pub async fn logout_handler() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

#[derive(sqlx::FromRow)]
struct UserRow {
    user_id: String,
    password_hash: String,
    role: String,
}
