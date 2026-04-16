use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};

use std::sync::LazyLock;

static JWT_SECRET: LazyLock<String> = LazyLock::new(jwt_secret);

pub fn jwt_secret() -> String {
    std::env::var("CHV_JWT_SECRET").unwrap_or_else(|_| {
        // Temporary fallback for release builds until ops sets CHV_JWT_SECRET.
        tracing::error!("CHV_JWT_SECRET not set; using hardcoded dev fallback. Set CHV_JWT_SECRET in production!");
        "chv-dev-secret-change-in-production".to_string()
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: u64,
}

pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = JWT_SECRET.as_str();
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_aud = false;
    let token_data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

pub struct BearerToken(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "missing authorization header"))?;

        if !auth.to_ascii_lowercase().starts_with("bearer ") {
            return Err((StatusCode::UNAUTHORIZED, "invalid authorization scheme"));
        }
        let token = &auth[7..];

        let claims = validate_token(token)
            .map_err(|e| {
                tracing::warn!(error = %e, "token validation failed");
                (StatusCode::UNAUTHORIZED, "invalid or expired token")
            })?;

        Ok(BearerToken(claims))
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
        let token = encode_claims(&claims, &jwt_secret());
        let result = validate_token(&token);
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
        let token = encode_claims(&claims, &jwt_secret());
        let result = validate_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn empty_token_is_rejected() {
        // We test validate_token directly with an empty string to simulate missing logic.
        let result = validate_token("");
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
        let result = validate_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn malformed_token_is_rejected() {
        let result = validate_token("not-a-valid-jwt");
        assert!(result.is_err());
    }
}
