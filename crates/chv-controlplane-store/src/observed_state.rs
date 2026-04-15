use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{Generation, NodeId, NodeState, ResourceId};

const UPSERT_NODE_OBSERVED_STATE_SQL: &str = r#"
INSERT INTO node_observed_state (
    node_id,
    observed_generation,
    observed_state,
    health_status,
    runtime_status,
    state_reason,
    entered_at,
    observed_at,
    updated_at
)
VALUES (
    $1,
    $2,
    $3::node_state,
    $4,
    $5,
    $6,
    CASE WHEN $7 IS NULL THEN NULL ELSE to_timestamp($7 / 1000.0) END,
    to_timestamp($8 / 1000.0),
    to_timestamp($8 / 1000.0)
)
ON CONFLICT (node_id) DO UPDATE SET
    observed_generation = EXCLUDED.observed_generation,
    observed_state = EXCLUDED.observed_state,
    health_status = EXCLUDED.health_status,
    runtime_status = EXCLUDED.runtime_status,
    state_reason = EXCLUDED.state_reason,
    entered_at = EXCLUDED.entered_at,
    observed_at = EXCLUDED.observed_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_VM_OBSERVED_STATE_SQL: &str = r#"
INSERT INTO vm_observed_state (
    vm_id,
    observed_generation,
    runtime_status,
    health_status,
    node_id,
    cloud_hypervisor_pid,
    api_socket_path,
    last_transition_at,
    last_error,
    observed_at,
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
    CASE WHEN $8 IS NULL THEN NULL ELSE to_timestamp($8 / 1000.0) END,
    $9,
    to_timestamp($10 / 1000.0),
    to_timestamp($10 / 1000.0)
)
ON CONFLICT (vm_id) DO UPDATE SET
    observed_generation = EXCLUDED.observed_generation,
    runtime_status = EXCLUDED.runtime_status,
    health_status = EXCLUDED.health_status,
    node_id = EXCLUDED.node_id,
    cloud_hypervisor_pid = EXCLUDED.cloud_hypervisor_pid,
    api_socket_path = EXCLUDED.api_socket_path,
    last_transition_at = EXCLUDED.last_transition_at,
    last_error = EXCLUDED.last_error,
    observed_at = EXCLUDED.observed_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_VOLUME_OBSERVED_STATE_SQL: &str = r#"
INSERT INTO volume_observed_state (
    volume_id,
    observed_generation,
    runtime_status,
    health_status,
    attached_vm_id,
    device_path,
    export_path,
    last_transition_at,
    observed_at,
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
    CASE WHEN $8 IS NULL THEN NULL ELSE to_timestamp($8 / 1000.0) END,
    to_timestamp($9 / 1000.0),
    to_timestamp($9 / 1000.0)
)
ON CONFLICT (volume_id) DO UPDATE SET
    observed_generation = EXCLUDED.observed_generation,
    runtime_status = EXCLUDED.runtime_status,
    health_status = EXCLUDED.health_status,
    attached_vm_id = EXCLUDED.attached_vm_id,
    device_path = EXCLUDED.device_path,
    export_path = EXCLUDED.export_path,
    last_transition_at = EXCLUDED.last_transition_at,
    observed_at = EXCLUDED.observed_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_NETWORK_OBSERVED_STATE_SQL: &str = r#"
INSERT INTO network_observed_state (
    network_id,
    observed_generation,
    runtime_status,
    health_status,
    exposure_status,
    applied_at,
    observed_at,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    CASE WHEN $6 IS NULL THEN NULL ELSE to_timestamp($6 / 1000.0) END,
    to_timestamp($7 / 1000.0),
    to_timestamp($7 / 1000.0)
)
ON CONFLICT (network_id) DO UPDATE SET
    observed_generation = EXCLUDED.observed_generation,
    runtime_status = EXCLUDED.runtime_status,
    health_status = EXCLUDED.health_status,
    exposure_status = EXCLUDED.exposure_status,
    applied_at = EXCLUDED.applied_at,
    observed_at = EXCLUDED.observed_at,
    updated_at = EXCLUDED.updated_at
"#;

#[derive(Clone)]
pub struct ObservedStateRepository {
    pool: StorePool,
}

impl ObservedStateRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn upsert_node(&self, input: &NodeObservedStateInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_NODE_OBSERVED_STATE_SQL)
            .bind(input.node_id.as_str())
            .bind(generation_to_i64(input.observed_generation)?)
            .bind(input.observed_state.as_str())
            .bind(&input.health_status)
            .bind(&input.runtime_status)
            .bind(&input.state_reason)
            .bind(input.entered_unix_ms)
            .bind(input.observed_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_vm(&self, input: &VmObservedStateInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_VM_OBSERVED_STATE_SQL)
            .bind(input.vm_id.as_str())
            .bind(generation_to_i64(input.observed_generation)?)
            .bind(&input.runtime_status)
            .bind(&input.health_status)
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(input.cloud_hypervisor_pid)
            .bind(&input.api_socket_path)
            .bind(input.last_transition_unix_ms)
            .bind(&input.last_error)
            .bind(input.observed_unix_ms)
            .execute(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                    let (entity, id) = match db_err.constraint() {
                        Some(c) if c.ends_with("_node_id_fkey") => (
                            "node",
                            input
                                .node_id
                                .as_ref()
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ),
                        _ => ("vm", input.vm_id.to_string()),
                    };
                    StoreError::NotFound { entity, id }
                }
                _ => StoreError::from(e),
            })?;
        Ok(())
    }

    pub async fn upsert_volume(&self, input: &VolumeObservedStateInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_VOLUME_OBSERVED_STATE_SQL)
            .bind(input.volume_id.as_str())
            .bind(generation_to_i64(input.observed_generation)?)
            .bind(&input.runtime_status)
            .bind(&input.health_status)
            .bind(input.attached_vm_id.as_ref().map(ResourceId::as_str))
            .bind(&input.device_path)
            .bind(&input.export_path)
            .bind(input.last_transition_unix_ms)
            .bind(input.observed_unix_ms)
            .execute(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                    let (entity, id) = match db_err.constraint() {
                        Some(c) if c.ends_with("_attached_vm_id_fkey") => (
                            "vm",
                            input
                                .attached_vm_id
                                .as_ref()
                                .map(|v| v.to_string())
                                .unwrap_or_default(),
                        ),
                        _ => ("volume", input.volume_id.to_string()),
                    };
                    StoreError::NotFound { entity, id }
                }
                _ => StoreError::from(e),
            })?;
        Ok(())
    }

    pub async fn upsert_network(
        &self,
        input: &NetworkObservedStateInput,
    ) -> Result<(), StoreError> {
        sqlx::query(UPSERT_NETWORK_OBSERVED_STATE_SQL)
            .bind(input.network_id.as_str())
            .bind(generation_to_i64(input.observed_generation)?)
            .bind(&input.runtime_status)
            .bind(&input.health_status)
            .bind(&input.exposure_status)
            .bind(input.applied_unix_ms)
            .bind(input.observed_unix_ms)
            .execute(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                    StoreError::NotFound {
                        entity: "network",
                        id: input.network_id.to_string(),
                    }
                }
                _ => StoreError::from(e),
            })?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct NodeObservedStateInput {
    pub node_id: NodeId,
    pub observed_generation: Generation,
    pub observed_state: NodeState,
    pub health_status: Option<String>,
    pub runtime_status: Option<String>,
    pub state_reason: Option<String>,
    pub entered_unix_ms: Option<i64>,
    pub observed_unix_ms: i64,
}

#[derive(Clone)]
pub struct VmObservedStateInput {
    pub vm_id: ResourceId,
    pub observed_generation: Generation,
    pub runtime_status: String,
    pub health_status: Option<String>,
    pub node_id: Option<NodeId>,
    pub cloud_hypervisor_pid: Option<i32>,
    pub api_socket_path: Option<String>,
    pub last_error: Option<String>,
    pub last_transition_unix_ms: Option<i64>,
    pub observed_unix_ms: i64,
}

#[derive(Clone)]
pub struct VolumeObservedStateInput {
    pub volume_id: ResourceId,
    pub observed_generation: Generation,
    pub runtime_status: String,
    pub health_status: Option<String>,
    pub attached_vm_id: Option<ResourceId>,
    pub device_path: Option<String>,
    pub export_path: Option<String>,
    pub last_transition_unix_ms: Option<i64>,
    pub observed_unix_ms: i64,
}

#[derive(Clone)]
pub struct NetworkObservedStateInput {
    pub network_id: ResourceId,
    pub observed_generation: Generation,
    pub runtime_status: String,
    pub health_status: Option<String>,
    pub exposure_status: Option<String>,
    pub applied_unix_ms: Option<i64>,
    pub observed_unix_ms: i64,
}

fn generation_to_i64(generation: Generation) -> Result<i64, StoreError> {
    i64::try_from(generation.get()).map_err(|source| StoreError::InvalidConfiguration {
        reason: format!("generation out of range for bigint column: {source}"),
    })
}
