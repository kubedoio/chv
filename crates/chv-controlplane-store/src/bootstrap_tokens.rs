use crate::{StoreError, StorePool};
use chrono::Utc;

const VALIDATE_TOKEN_SQL: &str = r#"
SELECT token_hash, one_time_use, used_at, expires_at
FROM bootstrap_tokens
WHERE token_hash = $1
"#;

const MARK_USED_SQL: &str = r#"
UPDATE bootstrap_tokens
SET used_at = strftime('%Y-%m-%dT%H:%M:%SZ','now'), updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
WHERE token_hash = $1 AND used_at IS NULL
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
        let row = sqlx::query_as::<_, BootstrapTokenRow>(VALIDATE_TOKEN_SQL)
            .bind(&token_hash)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            None => Ok(BootstrapTokenValidation::Invalid),
            Some(row) => {
                if let Some(expires_at) = row.expires_at {
                    if expires_at < Utc::now() {
                        return Ok(BootstrapTokenValidation::Expired);
                    }
                }
                if row.one_time_use {
                    let result = sqlx::query(MARK_USED_SQL)
                        .bind(&token_hash)
                        .execute(&self.pool)
                        .await?;
                    if result.rows_affected() == 0 {
                        return Ok(BootstrapTokenValidation::AlreadyUsed);
                    }
                }
                Ok(BootstrapTokenValidation::Valid)
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
