use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{
    ActorId, EventSeverity, EventType, NodeId, OperationId, ResourceId, ResourceKind,
};

const INSERT_EVENT_SQL: &str = r#"
INSERT INTO events (
    occurred_at,
    event_type,
    severity,
    resource_kind,
    resource_id,
    node_id,
    operation_id,
    actor_id,
    requested_by,
    correlation_id,
    message,
    details
)
VALUES (
    strftime('%Y-%m-%dT%H:%M:%SZ', $1 / 1000.0, 'unixepoch'),
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
    $12
)
"#;

#[derive(Clone)]
pub struct EventRepository {
    pool: StorePool,
}

impl EventRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn append(&self, input: &EventAppendInput) -> Result<(), StoreError> {
        sqlx::query(INSERT_EVENT_SQL)
            .bind(input.occurred_unix_ms)
            .bind(input.event_type.as_str())
            .bind(input.severity.as_str())
            .bind(input.resource_kind.map(ResourceKind::as_str))
            .bind(input.resource_id.as_ref().map(ResourceId::as_str))
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(input.operation_id.as_ref().map(OperationId::as_str))
            .bind(input.actor_id.as_ref().map(ActorId::as_str))
            .bind(&input.requested_by)
            .bind(&input.correlation_id)
            .bind(&input.message)
            .bind(&input.details)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct EventAppendInput {
    pub occurred_unix_ms: i64,
    pub event_type: EventType,
    pub severity: EventSeverity,
    pub resource_kind: Option<ResourceKind>,
    pub resource_id: Option<ResourceId>,
    pub node_id: Option<NodeId>,
    pub operation_id: Option<OperationId>,
    pub actor_id: Option<ActorId>,
    pub requested_by: Option<String>,
    pub correlation_id: Option<String>,
    pub message: String,
    pub details: Option<String>,
}
