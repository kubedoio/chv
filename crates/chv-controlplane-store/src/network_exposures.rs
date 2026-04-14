use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::ResourceId;

const UPSERT_SQL: &str = r#"
INSERT INTO network_exposures (
    network_id, service_name, protocol, listen_address, listen_port,
    target_address, target_port, exposure_policy, updated_at
)
VALUES ($1, $2, $3, $4::inet, $5, $6::inet, $7, $8, to_timestamp($9 / 1000.0))
ON CONFLICT (network_id, service_name) DO UPDATE SET
    protocol = EXCLUDED.protocol,
    listen_address = EXCLUDED.listen_address,
    listen_port = EXCLUDED.listen_port,
    target_address = EXCLUDED.target_address,
    target_port = EXCLUDED.target_port,
    exposure_policy = EXCLUDED.exposure_policy,
    updated_at = EXCLUDED.updated_at
"#;

#[derive(Clone)]
pub struct NetworkExposureRepository {
    pool: StorePool,
}

impl NetworkExposureRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, input: &NetworkExposureInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_SQL)
            .bind(input.network_id.as_str())
            .bind(&input.service_name)
            .bind(&input.protocol)
            .bind(&input.listen_address)
            .bind(input.listen_port)
            .bind(&input.target_address)
            .bind(input.target_port)
            .bind(&input.exposure_policy)
            .bind(input.updated_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct NetworkExposureInput {
    pub network_id: ResourceId,
    pub service_name: String,
    pub protocol: String,
    pub listen_address: Option<String>,
    pub listen_port: Option<i32>,
    pub target_address: Option<String>,
    pub target_port: Option<i32>,
    pub exposure_policy: Option<String>,
    pub updated_unix_ms: i64,
}
