use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{EventSeverity, NodeId, ResourceKind};

const CREATE_ALERT_SQL: &str = r#"
INSERT INTO alerts (
    alert_type,
    severity,
    resource_kind,
    resource_id,
    node_id,
    status,
    message,
    operation_id,
    opened_at
)
VALUES (
    $1,
    $2::event_severity,
    $3::resource_kind,
    $4,
    $5,
    $6,
    $7,
    $8,
    to_timestamp($9 / 1000.0)
)
RETURNING alert_id
"#;

#[derive(Clone)]
pub struct AlertRepository {
    pool: StorePool,
}

impl AlertRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: &AlertCreateInput) -> Result<uuid::Uuid, StoreError> {
        let row: (uuid::Uuid,) = sqlx::query_as(CREATE_ALERT_SQL)
            .bind(&input.alert_type)
            .bind(input.severity.as_str())
            .bind(input.resource_kind.as_ref().map(|k| k.as_str()))
            .bind(&input.resource_id)
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(&input.status)
            .bind(&input.message)
            .bind(&input.operation_id)
            .bind(input.opened_unix_ms)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

pub struct AlertCreateInput {
    pub alert_type: String,
    pub severity: EventSeverity,
    pub resource_kind: Option<ResourceKind>,
    pub resource_id: Option<String>,
    pub node_id: Option<NodeId>,
    pub status: String,
    pub message: String,
    pub operation_id: Option<String>,
    pub opened_unix_ms: i64,
}
