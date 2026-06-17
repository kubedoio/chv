use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Request},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::BffError;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: u64,
    /// True if the user must rotate their password before performing any
    /// other action. Marked `serde(default)` so JWTs issued before this
    /// field existed deserialize cleanly with `false` — no migration of
    /// outstanding tokens required.
    #[serde(default)]
    pub must_change_password: bool,
}

/// Paths a user with `must_change_password=true` is permitted to hit. Every
/// other path must be rejected with 403 until the password is rotated and a
/// fresh JWT is issued by re-login.
fn is_password_rotation_safe_path(path: &str) -> bool {
    matches!(path, "/v1/auth/change-password" | "/v1/auth/logout")
}

#[derive(Clone, Copy, Debug)]
pub enum Role {
    Viewer,
    Operator,
    Admin,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Role::Admin),
            "operator" => Some(Role::Operator),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            Role::Viewer => 0,
            Role::Operator => 1,
            Role::Admin => 2,
        }
    }

    pub fn meets(&self, required: Role) -> bool {
        self.rank() >= required.rank()
    }
}

/// True if `role` carries the implicit `architecture:apply` permission used
/// by the drift heuristic in `chv-architecture-reconcile`. Admin and Operator
/// hold the permission; Viewer does not.
///
/// Centralised so all drift call sites in `handlers::architectures` agree on
/// the mapping. Co-located with `Role` so any future role addition has to
/// make a deliberate decision here.
pub fn has_apply_permission(role: &Role) -> bool {
    match role {
        Role::Admin | Role::Operator => true,
        Role::Viewer => false,
    }
}

fn forbidden_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({
            "message": "insufficient permissions",
            "code": "FORBIDDEN"
        })),
    )
        .into_response()
}

fn password_change_required_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({
            "message": "password change required before any other action",
            "code": "PASSWORD_CHANGE_REQUIRED",
            "error": "password_change_required"
        })),
    )
        .into_response()
}

async fn role_check(claims: &Claims, required: Role, req: Request, next: Next) -> Response {
    // Industrial-grade enforcement of the must_change_password promise made
    // by install.sh: a user flagged for rotation can ONLY hit the change-
    // password endpoint (or log out). Every other request is short-circuited
    // with 403 PASSWORD_CHANGE_REQUIRED so the UI knows to route to the
    // forced-change screen.
    if claims.must_change_password && !is_password_rotation_safe_path(req.uri().path()) {
        return password_change_required_response();
    }
    let user_role = Role::parse(&claims.role).unwrap_or(Role::Viewer);
    if !user_role.meets(required) {
        return forbidden_response();
    }
    next.run(req).await
}

pub async fn viewer_middleware(
    BearerToken(claims): BearerToken,
    req: Request,
    next: Next,
) -> Response {
    role_check(&claims, Role::Viewer, req, next).await
}

pub async fn operator_middleware(
    BearerToken(claims): BearerToken,
    req: Request,
    next: Next,
) -> Response {
    role_check(&claims, Role::Operator, req, next).await
}

pub async fn admin_middleware(
    BearerToken(claims): BearerToken,
    req: Request,
    next: Next,
) -> Response {
    role_check(&claims, Role::Admin, req, next).await
}

pub fn require_operator_or_admin(claims: &Claims) -> Result<(), BffError> {
    if claims.role == "admin" || claims.role == "operator" {
        Ok(())
    } else {
        Err(BffError::Forbidden(
            "operator or admin role required".into(),
        ))
    }
}

pub fn require_admin(claims: &Claims) -> Result<(), BffError> {
    if claims.role == "admin" {
        Ok(())
    } else {
        Err(BffError::Forbidden("admin role required".into()))
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
                Json(serde_json::json!({ "message": msg, "code": "UNAUTHORIZED" })),
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
                        // API tokens bypass the must-change-password gate: they are
                        // service credentials minted explicitly by an authenticated
                        // user, not the seeded bootstrap admin row.
                        must_change_password: false,
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
            must_change_password: false,
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
            must_change_password: false,
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
            must_change_password: false,
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

    fn future_exp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX epoch")
            .as_secs()
            + 3600
    }

    // Forge a token by splicing the payload from `forged_claims` (signed with `secret`)
    // onto the signature of `original_claims` (also signed with `secret`). Both tokens
    // share an identical HS256 header, so concatenating the foreign payload with the
    // original signature produces a token whose recomputed HMAC will not match.
    #[test]
    fn tampered_payload_is_rejected() {
        // Attack class: payload forgery (e.g., privilege escalation by changing role).
        let secret = test_secret();
        let exp = future_exp();
        let original = Claims {
            sub: "user-1".into(),
            username: "alice".into(),
            role: "viewer".into(),
            exp,
            must_change_password: false,
        };
        let forged = Claims {
            sub: "user-1".into(),
            username: "alice".into(),
            role: "admin".into(), // privilege escalation attempt
            exp,
            must_change_password: false,
        };
        let original_token = encode_claims(&original, &secret);
        let forged_token = encode_claims(&forged, &secret);

        let mut original_parts = original_token.split('.');
        let original_header = original_parts.next().expect("header segment");
        let _original_payload = original_parts.next().expect("payload segment");
        let original_sig = original_parts.next().expect("signature segment");

        let mut forged_parts = forged_token.split('.');
        let _forged_header = forged_parts.next().expect("header segment");
        let forged_payload = forged_parts.next().expect("payload segment");

        // Splice: header.forged_payload.original_sig -> signature mismatch on verify.
        let tampered = format!("{}.{}.{}", original_header, forged_payload, original_sig);
        let result = validate_token(&tampered, &secret);
        assert!(
            result.is_err(),
            "tampered payload must be rejected (signature mismatch)"
        );
    }

    #[test]
    fn tampered_signature_is_rejected() {
        // Attack class: blind signature mutation.
        let secret = test_secret();
        let claims = Claims {
            sub: "user-1".into(),
            username: "alice".into(),
            role: "admin".into(),
            exp: future_exp(),
            must_change_password: false,
        };
        let token = encode_claims(&claims, &secret);

        // Flip the last character of the signature segment to a different valid
        // base64url char so the structure still parses but the HMAC fails.
        let mut bytes = token.into_bytes();
        let last = bytes.last_mut().expect("non-empty token");
        *last = if *last == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).expect("ascii only");

        let result = validate_token(&tampered, &secret);
        assert!(result.is_err(), "byte-flipped signature must be rejected");
    }

    #[test]
    fn hs512_token_against_hs256_validator_is_rejected() {
        // Attack class: algorithm confusion / downgrade — a token signed with a
        // different (even if stronger) algorithm must not validate against the
        // pinned HS256 validator.
        let secret = test_secret();
        let claims = Claims {
            sub: "user-1".into(),
            username: "alice".into(),
            role: "admin".into(),
            exp: future_exp(),
            must_change_password: false,
        };
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS512);
        let token = jsonwebtoken::encode(
            &header,
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("HS512 encoding should succeed");

        let result = validate_token(&token, &secret);
        assert!(
            result.is_err(),
            "HS512-signed token must be rejected by HS256-pinned validator"
        );
    }

    #[test]
    fn token_past_exp_leeway_is_rejected() {
        // Attack class: clock-skew exploitation — a token expired well past the
        // default jsonwebtoken leeway (60s) must be rejected. We use 5 minutes
        // to leave no doubt: any reasonable leeway tweak still rejects this.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX epoch")
            .as_secs();
        let claims = Claims {
            sub: "user-1".into(),
            username: "alice".into(),
            role: "admin".into(),
            exp: now - 300,
            must_change_password: false,
        };
        let token = encode_claims(&claims, &test_secret());
        let result = validate_token(&token, &test_secret());
        assert!(
            result.is_err(),
            "token expired 5 minutes ago must be rejected (beyond default 60s leeway)"
        );
    }

    #[test]
    fn token_with_far_future_exp_is_accepted() {
        // Mirrors the API-token codepath which uses `exp: u64::MAX / 2`.
        // Confirms an extremely-large exp does not overflow validation.
        let claims = Claims {
            sub: "api-token-user".into(),
            username: "svc".into(),
            role: "operator".into(),
            exp: u64::MAX / 2,
            must_change_password: false,
        };
        let token = encode_claims(&claims, &test_secret());
        let result = validate_token(&token, &test_secret());
        let validated = result.expect("far-future exp must validate");
        assert_eq!(validated.exp, u64::MAX / 2);
        assert_eq!(validated.role, "operator");
    }

    #[test]
    fn token_with_extra_unknown_fields_is_accepted() {
        // Forward-compat: extra payload fields (e.g., `tenant_id`, `iss`) added by
        // future issuers must not break deserialization into `Claims`.
        let secret = test_secret();
        let payload = serde_json::json!({
            "sub": "user-1",
            "username": "alice",
            "role": "admin",
            "exp": future_exp(),
            "iss": "chv-test-issuer",
            "tenant_id": "tenant-42",
            "scope": ["read", "write"],
        });
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        let token = jsonwebtoken::encode(
            &header,
            &payload,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encoding json value should succeed");
        let validated = validate_token(&token, &secret).expect("unknown fields must be ignored");
        assert_eq!(validated.sub, "user-1");
        assert_eq!(validated.role, "admin");
    }

    #[test]
    fn missing_required_claim_is_rejected() {
        // Attack class: malformed/incomplete payload — a token missing the `role`
        // field must fail to deserialize into `Claims` (no defaulting to viewer).
        let secret = test_secret();
        let payload = serde_json::json!({
            "sub": "user-1",
            "username": "alice",
            // role intentionally omitted
            "exp": future_exp(),
        });
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        let token = jsonwebtoken::encode(
            &header,
            &payload,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encoding json value should succeed");
        let result = validate_token(&token, &secret);
        assert!(
            result.is_err(),
            "token missing required claim must be rejected"
        );
    }
}
