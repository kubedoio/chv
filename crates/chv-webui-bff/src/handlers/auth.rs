use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::auth::BearerToken;
use crate::router::AppState;
use crate::BffError;

/// Dummy bcrypt hash used when the username is not found, so that bcrypt::verify
/// always runs and response time is constant regardless of whether the user exists.
const DUMMY_HASH: &str = "$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m";

const MAX_LOGIN_ATTEMPTS: u32 = 10;
const RATE_WINDOW_SECS: u64 = 60;

/// Minimum length for a rotated password. install.sh promises operators a
/// real rotation; an industrial-grade floor here keeps anyone from picking
/// "admin" again the moment they're forced to change.
const MIN_NEW_PASSWORD_LEN: usize = 12;

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
    /// Mirrors the `must_change_password` column added in migration 0044.
    /// Stored as INTEGER in SQLite, which sqlx maps to i64 by default.
    /// install.sh sets this to 1 on the seeded admin row so the operator
    /// is forced to rotate the credential on first login.
    must_change_password: i64,
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
        "SELECT user_id, username, password_hash, role, must_change_password \
         FROM users WHERE username = ?",
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

    let valid = bcrypt::verify(password, &hash_to_check).unwrap_or(false);

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

    let must_change_password = user.must_change_password != 0;

    let claims = crate::auth::Claims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
        exp,
        must_change_password,
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
        },
        "must_change_password": must_change_password
    })))
}

/// Validate password policy. Centralised so the rules are visible at one site
/// and tests can pin them. Returns a human-readable reason on failure that is
/// safe to surface in 400 responses (no secrets, no PII).
fn validate_new_password_policy(password: &str) -> Result<(), &'static str> {
    if password.len() < MIN_NEW_PASSWORD_LEN {
        return Err("new password must be at least 12 characters");
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PasswordRow {
    password_hash: String,
}

/// `POST /v1/auth/change-password`
///
/// Changes the authenticated caller's password and clears the
/// `must_change_password` flag. The caller's identity comes from `claims.sub`
/// in the bearer token — a user can only change their OWN password through
/// this endpoint.
///
/// On success the existing JWT is left in place; the caller must re-login to
/// obtain a token whose `must_change_password` claim is false. This keeps the
/// flow simple and forces the user to confirm the new credential works before
/// regaining full access.
pub async fn change_password(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let current_password = payload
        .get("current_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing current_password".into()))?;
    let new_password = payload
        .get("new_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing new_password".into()))?;

    // Validate the new password BEFORE doing the DB lookup. Cheap rejection
    // for the most common mistake (too short) and avoids hashing work.
    validate_new_password_policy(new_password)
        .map_err(|reason| BffError::BadRequest(reason.into()))?;

    let row = sqlx::query_as::<_, PasswordRow>("SELECT password_hash FROM users WHERE user_id = ?")
        .bind(&claims.sub)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "change_password db query failed");
            BffError::Internal("authentication service unavailable".into())
        })?;

    // The bearer token decoded successfully so the user existed at issue time.
    // If they've been deleted since, treat the token as no longer valid.
    let row = row.ok_or_else(|| BffError::Unauthorized("invalid current password".into()))?;

    // Don't leak whether the user exists vs. whether the password was wrong:
    // both paths return the same Unauthorized response.
    let valid = bcrypt::verify(current_password, &row.password_hash).unwrap_or(false);
    if !valid {
        return Err(BffError::Unauthorized("invalid current password".into()));
    }

    let new_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST).map_err(|e| {
        tracing::error!(error = %e, "bcrypt hash failed during change_password");
        BffError::Internal("failed to hash password".into())
    })?;

    let updated = sqlx::query(
        "UPDATE users \
         SET password_hash = ?, \
             must_change_password = 0, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') \
         WHERE user_id = ?",
    )
    .bind(&new_hash)
    .bind(&claims.sub)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "change_password db update failed");
        BffError::Internal("failed to update password".into())
    })?;

    if updated.rows_affected() != 1 {
        return Err(BffError::Internal("password update affected 0 rows".into()));
    }

    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Claims;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;

    /// Verify that bcrypt::verify works with a known hash.
    /// Uses the historical default hash that migration 0008 once shipped;
    /// the test only exercises bcrypt verification, not production credentials.
    /// Hash: $2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m
    /// Password: "admin"
    #[test]
    fn bcrypt_verify_known_admin_hash() {
        let hash = "$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m";
        let result = bcrypt::verify("admin", hash).expect("bcrypt::verify should not error");
        assert!(result, "bcrypt::verify should return true for known-hash");
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

    #[test]
    fn validate_new_password_rejects_short() {
        // 11 chars — one below the 12-char floor.
        assert!(validate_new_password_policy("short_pwd_1").is_err());
    }

    #[test]
    fn validate_new_password_accepts_at_floor() {
        // 12 chars — exactly at the floor.
        assert!(validate_new_password_policy("twelve_charz").is_ok());
    }

    // ------------------------------------------------------------------
    // Integration tests against a real in-memory SQLite pool.
    // Mirrors the pattern used in handlers/vms.rs (`build_test_pool`).
    // ------------------------------------------------------------------

    /// Mutation service stub — none of these auth tests trigger any
    /// mutation, so every method panics if called. Keeping the surface
    /// minimal here: if a future test needs real mutations, swap in
    /// `chv_controlplane_service::ControlPlaneMutationService`.
    struct NoopMutations;
    #[async_trait::async_trait]
    impl crate::mutations::MutationService for NoopMutations {
        async fn mutate_vm(
            &self,
            _vm_id: String,
            _action: String,
            _force: bool,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
            unreachable!("mutate_vm not used in auth tests")
        }
        async fn migrate_vm(
            &self,
            _vm_id: String,
            _target_node_id: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
            unreachable!("migrate_vm not used in auth tests")
        }
        async fn snapshot_vm(
            &self,
            _vm_id: String,
            _destination: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
            unreachable!("snapshot_vm not used in auth tests")
        }
        async fn restore_snapshot(
            &self,
            _vm_id: String,
            _source: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
            unreachable!("restore_snapshot not used in auth tests")
        }
        async fn mutate_node(
            &self,
            _node_id: String,
            _action: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse, BffError> {
            unreachable!("mutate_node not used in auth tests")
        }
        async fn mutate_volume(
            &self,
            _volume_id: String,
            _action: String,
            _force: bool,
            _resize_bytes: Option<u64>,
            _vm_id: Option<String>,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
            unreachable!("mutate_volume not used in auth tests")
        }
        async fn snapshot_volume(
            &self,
            _volume_id: String,
            _snapshot_name: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
            unreachable!("snapshot_volume not used in auth tests")
        }
        async fn restore_volume_snapshot(
            &self,
            _volume_id: String,
            _snapshot_name: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
            unreachable!("restore_volume_snapshot not used in auth tests")
        }
        async fn delete_volume_snapshot(
            &self,
            _volume_id: String,
            _snapshot_name: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
            unreachable!("delete_volume_snapshot not used in auth tests")
        }
        async fn clone_volume(
            &self,
            _source_volume_id: String,
            _target_volume_id: String,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
            unreachable!("clone_volume not used in auth tests")
        }
        async fn mutate_network(
            &self,
            _network_id: String,
            _action: String,
            _force: bool,
            _requested_by: String,
        ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse, BffError> {
            unreachable!("mutate_network not used in auth tests")
        }
    }

    async fn build_test_state() -> AppState {
        // Single-connection in-memory pool so all queries hit the same DB.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        chv_controlplane_store::run_migrations(&pool, None)
            .await
            .expect("run migrations");

        use chv_controlplane_store::{
            AlertRepository, BackupRepository, DesiredStateRepository, EventRepository,
            NodeRepository, ObservedStateRepository, OperationRepository, TopologyRepository,
        };
        AppState {
            pool: pool.clone(),
            node_repo: NodeRepository::new(pool.clone()),
            operation_repo: OperationRepository::new(pool.clone()),
            event_repo: EventRepository::new(pool.clone()),
            alert_repo: AlertRepository::new(pool.clone()),
            desired_state_repo: DesiredStateRepository::new(pool.clone()),
            observed_state_repo: ObservedStateRepository::new(pool.clone()),
            backup_repo: BackupRepository::new(pool.clone()),
            topology_repo: TopologyRepository::new(pool.clone()),
            mutations: Arc::new(NoopMutations),
            jwt_secret: "test-secret".to_string(),
            agent_runtime_dir: std::path::PathBuf::from("/var/lib/chv/agent"),
            cache: crate::cache::BffCache::new(5),
        }
    }

    async fn seed_user(
        pool: &sqlx::SqlitePool,
        user_id: &str,
        username: &str,
        password: &str,
        must_change: bool,
    ) {
        // Use a low cost so test runtime stays under a second; bcrypt
        // semantics are identical at any cost.
        let hash = bcrypt::hash(password, 4).expect("bcrypt hash");
        sqlx::query(
            "INSERT INTO users (user_id, username, password_hash, role, must_change_password) \
             VALUES (?, ?, ?, 'admin', ?)",
        )
        .bind(user_id)
        .bind(username)
        .bind(&hash)
        .bind(if must_change { 1 } else { 0 })
        .execute(pool)
        .await
        .expect("seed user");
    }

    fn admin_token_for(state: &AppState, user_id: &str, must_change: bool) -> String {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let claims = Claims {
            sub: user_id.to_string(),
            username: "admin".to_string(),
            role: "admin".to_string(),
            exp,
            must_change_password: must_change,
        };
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        jsonwebtoken::encode(
            &header,
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
        )
        .expect("encode test token")
    }

    /// Map a `BffError` variant to its HTTP status code so tests can pin
    /// the response without round-tripping through `IntoResponse`.
    fn err_status(e: &BffError) -> u16 {
        match e {
            BffError::BadRequest(_) => 400,
            BffError::Unauthorized(_) => 401,
            BffError::Forbidden(_) => 403,
            BffError::NotFound(_) => 404,
            BffError::Conflict(_) => 409,
            BffError::TooManyRequests(_) => 429,
            BffError::Internal(_) => 500,
            BffError::NotImplemented(_) => 501,
            BffError::QuotaExceeded { .. } => 422,
            BffError::GraphEmpty => 422,
        }
    }

    #[tokio::test]
    async fn login_returns_must_change_password_flag_when_set() {
        let state = build_test_state().await;
        seed_user(&state.pool, "u-must-1", "alice", "correctpassword", true).await;

        let body = serde_json::json!({"username": "alice", "password": "correctpassword"});
        let resp = login(axum::extract::State(state.clone()), axum::Json(body))
            .await
            .expect("login should succeed");
        let v = resp.0;
        assert_eq!(
            v.get("must_change_password").and_then(|x| x.as_bool()),
            Some(true),
            "response JSON must include must_change_password=true"
        );
        assert!(v.get("token").and_then(|x| x.as_str()).is_some());

        // The token itself must carry the flag so middleware can enforce it.
        let token = v["token"].as_str().unwrap();
        let claims = crate::auth::validate_token(token, &state.jwt_secret).expect("decode token");
        assert!(claims.must_change_password);
    }

    #[tokio::test]
    async fn login_returns_must_change_password_false_when_clear() {
        let state = build_test_state().await;
        seed_user(&state.pool, "u-clear-1", "bob", "correctpassword", false).await;

        let body = serde_json::json!({"username": "bob", "password": "correctpassword"});
        let resp = login(axum::extract::State(state.clone()), axum::Json(body))
            .await
            .expect("login should succeed");
        let v = resp.0;
        assert_eq!(
            v.get("must_change_password").and_then(|x| x.as_bool()),
            Some(false)
        );
        let token = v["token"].as_str().unwrap();
        let claims = crate::auth::validate_token(token, &state.jwt_secret).expect("decode token");
        assert!(!claims.must_change_password);
    }

    #[tokio::test]
    async fn change_password_succeeds_with_valid_current() {
        let state = build_test_state().await;
        seed_user(&state.pool, "u-cp-ok", "carol", "oldpassword12", true).await;

        let token = admin_token_for(&state, "u-cp-ok", true);
        let claims = crate::auth::validate_token(&token, &state.jwt_secret).unwrap();

        let body = serde_json::json!({
            "current_password": "oldpassword12",
            "new_password": "BrandNewPassword!42",
        });
        let resp = change_password(
            BearerToken(claims),
            axum::extract::State(state.clone()),
            axum::Json(body),
        )
        .await
        .expect("change_password should succeed");
        assert_eq!(resp.0.get("ok").and_then(|v| v.as_bool()), Some(true));

        // DB row: new hash verifies, must_change_password cleared.
        let row: (String, i64) = sqlx::query_as(
            "SELECT password_hash, must_change_password FROM users WHERE user_id = ?",
        )
        .bind("u-cp-ok")
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(bcrypt::verify("BrandNewPassword!42", &row.0).unwrap());
        assert_eq!(row.1, 0, "must_change_password must be cleared");
    }

    #[tokio::test]
    async fn change_password_fails_with_wrong_current() {
        let state = build_test_state().await;
        seed_user(&state.pool, "u-cp-bad", "dan", "rightpassword12", true).await;

        let token = admin_token_for(&state, "u-cp-bad", true);
        let claims = crate::auth::validate_token(&token, &state.jwt_secret).unwrap();

        let body = serde_json::json!({
            "current_password": "wrong-password",
            "new_password": "AnotherStrongPassword!1",
        });
        let result = change_password(
            BearerToken(claims),
            axum::extract::State(state.clone()),
            axum::Json(body),
        )
        .await;
        let err = result.expect_err("must reject wrong current password");
        assert_eq!(
            err_status(&err),
            401,
            "wrong current password must return 401"
        );

        // DB hash unchanged, flag unchanged.
        let row: (String, i64) = sqlx::query_as(
            "SELECT password_hash, must_change_password FROM users WHERE user_id = ?",
        )
        .bind("u-cp-bad")
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(bcrypt::verify("rightpassword12", &row.0).unwrap());
        assert_eq!(row.1, 1, "must_change_password must still be set");
    }

    #[tokio::test]
    async fn change_password_rejects_weak_new_password() {
        let state = build_test_state().await;
        seed_user(&state.pool, "u-cp-weak", "eve", "rightpassword12", true).await;

        let token = admin_token_for(&state, "u-cp-weak", true);
        let claims = crate::auth::validate_token(&token, &state.jwt_secret).unwrap();

        let body = serde_json::json!({
            "current_password": "rightpassword12",
            "new_password": "shortpw", // 7 chars, well below the 12-char floor
        });
        let result = change_password(
            BearerToken(claims),
            axum::extract::State(state.clone()),
            axum::Json(body),
        )
        .await;
        let err = result.expect_err("must reject weak new password");
        assert_eq!(err_status(&err), 400, "weak new password must return 400");

        // Hash unchanged, flag unchanged — policy fails fast before any UPDATE.
        let row: (String, i64) = sqlx::query_as(
            "SELECT password_hash, must_change_password FROM users WHERE user_id = ?",
        )
        .bind("u-cp-weak")
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(bcrypt::verify("rightpassword12", &row.0).unwrap());
        assert_eq!(row.1, 1);
    }

    /// End-to-end via the real `bff_router`: a request with a JWT carrying
    /// `must_change_password=true` must be rejected on a protected route
    /// (here `/v1/overview`) but accepted on `/v1/auth/change-password`.
    #[tokio::test]
    async fn protected_route_blocked_when_must_change_password_set() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let state = build_test_state().await;
        seed_user(&state.pool, "u-block-1", "frank", "currentpassword12", true).await;

        let token = admin_token_for(&state, "u-block-1", true);

        let app = crate::bff_router(state.clone()).with_state(state.clone());

        // Protected route: must be 403 with PASSWORD_CHANGE_REQUIRED.
        let req = Request::builder()
            .method("POST")
            .uri("/v1/overview")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "protected route must reject must_change_password caller"
        );
        let body_bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json["code"], "PASSWORD_CHANGE_REQUIRED");

        // Change-password route: must be allowed (and succeed since current pw matches).
        let req2 = Request::builder()
            .method("POST")
            .uri("/v1/auth/change-password")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "current_password": "currentpassword12",
                    "new_password": "VeryStrongNewPassword!1",
                })
                .to_string(),
            ))
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(
            resp2.status(),
            StatusCode::OK,
            "change-password route must be reachable while flag is set"
        );
    }
}
