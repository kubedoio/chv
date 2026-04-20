use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::BffError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: u64,
}

pub fn require_operator_or_admin(claims: &Claims) -> Result<(), BffError> {
    if claims.role == "admin" || claims.role == "operator" {
        Ok(())
    } else {
        Err(BffError::Unauthorized(
            "operator or admin role required".into(),
        ))
    }
}

pub fn require_admin(claims: &Claims) -> Result<(), BffError> {
    if claims.role == "admin" {
        Ok(())
    } else {
        Err(BffError::Unauthorized("admin role required".into()))
    }
}

pub fn validate_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_aud = false;
    let token_data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

pub struct BearerToken(pub Claims);

#[async_trait]
impl FromRequestParts<crate::router::AppState> for BearerToken {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::router::AppState,
    ) -> Result<Self, Self::Rejection> {
        let reject = |msg: &'static str| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "message": msg, "code": 401 })),
            )
        };

        let auth = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| reject("missing authorization header"))?;

        if !auth.to_ascii_lowercase().starts_with("bearer ") {
            return Err(reject("invalid authorization scheme"));
        }
        let token = &auth[7..];

        // Try JWT first
        match validate_token(token, &state.jwt_secret) {
            Ok(claims) => return Ok(BearerToken(claims)),
            Err(e) => {
                tracing::debug!(error = %e, "JWT validation failed, checking API token");
            }
        }

        // Try API token (chv_ prefix)
        if token.starts_with("chv_") {
            let token_hash = chv_common::sha256_hex(token);

            #[derive(sqlx::FromRow)]
            struct ApiTokenUser {
                user_id: String,
                username: String,
                role: String,
            }

            let result = sqlx::query_as::<_, ApiTokenUser>(
                "SELECT u.user_id, u.username, u.role \
                 FROM api_tokens t \
                 JOIN users u ON t.user_id = u.user_id \
                 WHERE t.token_hash = ? \
                 AND (t.expires_at IS NULL OR t.expires_at > strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            )
            .bind(&token_hash)
            .fetch_optional(&state.pool)
            .await;

            match result {
                Ok(Some(row)) => {
                    // Update last_used_at in the background (best effort)
                    let pool = state.pool.clone();
                    let hash = token_hash.clone();
                    tokio::spawn(async move {
                        let _ = sqlx::query(
                            "UPDATE api_tokens SET last_used_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE token_hash = ?",
                        )
                        .bind(&hash)
                        .execute(&pool)
                        .await;
                    });

                    let claims = Claims {
                        sub: row.user_id,
                        username: row.username,
                        role: row.role,
                        // Far future expiry for API tokens — their expiry is managed by expires_at in DB
                        exp: u64::MAX / 2,
                    };
                    return Ok(BearerToken(claims));
                }
                Ok(None) => {
                    tracing::warn!("API token not found or expired");
                }
                Err(e) => {
                    tracing::error!(error = %e, "API token DB lookup failed");
                }
            }
        }

        Err(reject("invalid or expired token"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn encode_claims(claims: &Claims, secret: &str) -> String {
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        jsonwebtoken::encode(
            &header,
            claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encoding should succeed in tests")
    }

    fn test_secret() -> String {
        "test-secret-do-not-use-in-production".to_string()
    }

    #[test]
    fn valid_token_passes_validation() {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let claims = Claims {
            sub: "user-1".to_string(),
            username: "admin".to_string(),
            role: "admin".to_string(),
            exp,
        };
        let token = encode_claims(&claims, &test_secret());
        let result = validate_token(&token, &test_secret());
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.sub, "user-1");
        assert_eq!(validated.username, "admin");
        assert_eq!(validated.role, "admin");
    }

    #[test]
    fn expired_token_is_rejected() {
        let claims = Claims {
            sub: "user-1".to_string(),
            username: "admin".to_string(),
            role: "admin".to_string(),
            exp: 1, // expired in 1970
        };
        let token = encode_claims(&claims, &test_secret());
        let result = validate_token(&token, &test_secret());
        assert!(result.is_err());
    }

    #[test]
    fn empty_token_is_rejected() {
        let result = validate_token("", &test_secret());
        assert!(result.is_err());
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let claims = Claims {
            sub: "user-1".to_string(),
            username: "admin".to_string(),
            role: "admin".to_string(),
            exp,
        };
        let token = encode_claims(&claims, "wrong-secret");
        let result = validate_token(&token, &test_secret());
        assert!(result.is_err());
    }

    #[test]
    fn malformed_token_is_rejected() {
        let result = validate_token("not-a-valid-jwt", &test_secret());
        assert!(result.is_err());
    }

    #[test]
    fn sha256_hex_is_correct_length() {
        let hash = chv_common::sha256_hex("chv_test_token");
        assert_eq!(hash.len(), 64);
    }
}


