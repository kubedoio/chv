use crate::{StoreError, StorePool};

/// Atomically validate and consume a one-time-use bootstrap token in a single UPDATE.
/// All checks (existence, expiry, already-used) are pushed into the WHERE clause to
/// eliminate the TOCTOU race between SELECT and UPDATE.
const ATOMIC_CONSUME_SQL: &str = r#"
UPDATE bootstrap_tokens
SET used_at = strftime('%Y-%m-%dT%H:%M:%SZ','now'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
WHERE token_hash = $1
  AND one_time_use = 1
  AND used_at IS NULL
  AND (expires_at IS NULL OR expires_at >= strftime('%Y-%m-%dT%H:%M:%SZ','now'))
RETURNING token_hash
"#;

/// Check token validity for multi-use tokens (no consumption needed).
const VALIDATE_TOKEN_SQL: &str = r#"
SELECT token_hash, one_time_use, used_at, expires_at
FROM bootstrap_tokens
WHERE token_hash = $1
"#;

#[derive(Clone)]
pub struct BootstrapTokenRepository {
    pool: StorePool,
}

impl BootstrapTokenRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub async fn validate_and_consume(
        &self,
        token: &str,
    ) -> Result<BootstrapTokenValidation, StoreError> {
        let token_hash = sha256(token);

        // First, look up the token to determine its type and state.
        let row = sqlx::query_as::<_, BootstrapTokenRow>(VALIDATE_TOKEN_SQL)
            .bind(&token_hash)
            .fetch_optional(&self.pool)
            .await?;

        let row = match row {
            None => return Ok(BootstrapTokenValidation::Invalid),
            Some(r) => r,
        };

        // For multi-use tokens, just check expiry — no consumption needed.
        if !row.one_time_use {
            if let Some(expires_at) = row.expires_at {
                if expires_at < chrono::Utc::now() {
                    return Ok(BootstrapTokenValidation::Expired);
                }
            }
            return Ok(BootstrapTokenValidation::Valid);
        }

        // For one-time-use tokens, atomically consume via a single UPDATE … RETURNING.
        // The WHERE clause checks: exists, not yet used, and not expired.
        // This eliminates the TOCTOU race entirely.
        let consumed: Option<(String,)> =
            sqlx::query_as(ATOMIC_CONSUME_SQL)
                .bind(&token_hash)
                .fetch_optional(&self.pool)
                .await?;

        match consumed {
            Some(_) => Ok(BootstrapTokenValidation::Valid),
            None => {
                // The atomic UPDATE didn't match. Determine why for a helpful response.
                if row.used_at.is_some() {
                    Ok(BootstrapTokenValidation::AlreadyUsed)
                } else if row.expires_at.is_some_and(|e| e < chrono::Utc::now()) {
                    Ok(BootstrapTokenValidation::Expired)
                } else {
                    // Raced with another consumer — token was used between our SELECT and UPDATE.
                    Ok(BootstrapTokenValidation::AlreadyUsed)
                }
            }
        }
    }
}

fn sha256(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(sqlx::FromRow)]
struct BootstrapTokenRow {
    #[allow(dead_code)]
    token_hash: String,
    one_time_use: bool,
    #[allow(dead_code)]
    used_at: Option<chrono::DateTime<chrono::Utc>>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapTokenValidation {
    Valid,
    Invalid,
    Expired,
    AlreadyUsed,
}
