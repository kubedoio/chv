use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{
    Generation, OperationId, OperationStatus, ResourceId, ResourceKind,
};

const INSERT_OPERATION_SQL: &str = r#"
INSERT INTO operations (
    operation_id,
    idempotency_key,
    resource_kind,
    resource_id,
    operation_type,
    status,
    requested_by,
    updated_by,
    desired_generation,
    observed_generation,
    correlation_id,
    requested_at,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    $7,
    $8,
    $9,
    $10,
    $11,
    strftime('%Y-%m-%dT%H:%M:%SZ', $12 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $12 / 1000.0, 'unixepoch')
)
ON CONFLICT (idempotency_key) DO NOTHING
"#;

const SELECT_OPERATION_SQL: &str = r#"
SELECT
    operation_id,
    status
FROM operations
WHERE idempotency_key = $1
"#;

const UPDATE_OPERATION_STATUS_SQL: &str = r#"
UPDATE operations
SET
    status = $2,
    error_code = $3,
    error_message = $4,
    observed_generation = $5,
    updated_by = $6,
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch'),
    completed_at = CASE
        WHEN $2 IN ('Succeeded', 'Failed', 'Rejected', 'Stale', 'Conflict')
        THEN strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch')
        ELSE completed_at
    END
WHERE operation_id = $1
"#;

#[derive(Clone)]
pub struct OperationRepository {
    pool: StorePool,
}

impl OperationRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create_or_get(
        &self,
        input: &OperationCreateInput,
    ) -> Result<OperationReceipt, StoreError> {
        sqlx::query(INSERT_OPERATION_SQL)
            .bind(input.operation_id.as_str())
            .bind(&input.idempotency_key)
            .bind(input.resource_kind.as_str())
            .bind(input.resource_id.as_ref().map(ResourceId::as_str))
            .bind(&input.operation_type)
            .bind(input.status.as_str())
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(
                input
                    .desired_generation
                    .map(generation_to_i64)
                    .transpose()?,
            )
            .bind(
                input
                    .observed_generation
                    .map(generation_to_i64)
                    .transpose()?,
            )
            .bind(&input.correlation_id)
            .bind(input.requested_unix_ms)
            .execute(&self.pool)
            .await?;

        let row = sqlx::query_as::<_, (String, String)>(SELECT_OPERATION_SQL)
            .bind(&input.idempotency_key)
            .fetch_one(&self.pool)
            .await?;

        Ok(OperationReceipt {
            operation_id: OperationId::new(row.0).map_err(|e| {
                StoreError::InvalidConfiguration {
                    reason: e.to_string(),
                }
            })?,
            status: row
                .1
                .parse()
                .map_err(|e: _| StoreError::InvalidConfiguration {
                    reason: format!("invalid operation status in database: {e}"),
                })?,
        })
    }

    pub async fn update_status(
        &self,
        input: &OperationStatusUpdateInput,
    ) -> Result<(), StoreError> {
        sqlx::query(UPDATE_OPERATION_STATUS_SQL)
            .bind(input.operation_id.as_str())
            .bind(input.status.as_str())
            .bind(&input.error_code)
            .bind(&input.error_message)
            .bind(
                input
                    .observed_generation
                    .map(generation_to_i64)
                    .transpose()?,
            )
            .bind(&input.updated_by)
            .bind(input.updated_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct OperationCreateInput {
    pub operation_id: OperationId,
    pub idempotency_key: String,
    pub resource_kind: ResourceKind,
    pub resource_id: Option<ResourceId>,
    pub operation_type: String,
    pub status: OperationStatus,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub desired_generation: Option<Generation>,
    pub observed_generation: Option<Generation>,
    pub correlation_id: Option<String>,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct OperationStatusUpdateInput {
    pub operation_id: OperationId,
    pub status: OperationStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub observed_generation: Option<Generation>,
    pub updated_by: Option<String>,
    pub updated_unix_ms: i64,
}

#[derive(Clone)]
pub struct OperationReceipt {
    pub operation_id: OperationId,
    pub status: OperationStatus,
}

fn generation_to_i64(generation: Generation) -> Result<i64, StoreError> {
    i64::try_from(generation.get()).map_err(|e| StoreError::InvalidConfiguration {
        reason: format!("generation out of range for bigint column: {e}"),
    })
}
