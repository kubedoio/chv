use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};

pub const JWT_SECRET: &str = "chv-dev-secret-change-in-production";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(JWT_SECRET.as_bytes());
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

        let token = auth
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "invalid authorization scheme"))?;

        let claims = validate_token(token)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired token"))?;

        Ok(BearerToken(claims))
    }
}
