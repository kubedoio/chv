use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::router::AppState;
use crate::BffError;

pub async fn login(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let username = payload
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing username".into()))?;

    let password = payload
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing password".into()))?;

    // Dev-mode authentication: accept admin/admin
    if username != "admin" || password != "admin" {
        return Err(BffError::Unauthorized("Invalid username or password".into()));
    }

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 24 * 60 * 60;

    let claims = crate::auth::Claims {
        sub: "admin".to_string(),
        username: "admin".to_string(),
        role: "admin".to_string(),
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
            "username": "admin",
            "role": "admin"
        }
    })))
}
