use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{Generation, NodeId, NodeState};

const UPSERT_NODE_SQL: &str = r#"
INSERT INTO nodes (
    node_id,
    hostname,
    display_name,
    certificate_serial,
    agent_version,
    control_plane_version,
    enrolled_at,
    last_seen_at,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $8 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $8 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    hostname = EXCLUDED.hostname,
    display_name = EXCLUDED.display_name,
    certificate_serial = EXCLUDED.certificate_serial,
    agent_version = EXCLUDED.agent_version,
    control_plane_version = EXCLUDED.control_plane_version,
    enrolled_at = COALESCE(EXCLUDED.enrolled_at, nodes.enrolled_at),
    last_seen_at = EXCLUDED.last_seen_at,
    updated_at = EXCLUDED.updated_at
"#;

const ENSURE_NODE_RECORD_SQL: &str = r#"
INSERT INTO nodes (
    node_id,
    hostname,
    display_name,
    enrolled_at,
    last_seen_at,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    strftime('%Y-%m-%dT%H:%M:%SZ', $4 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $4 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $4 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    hostname = CASE
        WHEN nodes.hostname = nodes.node_id OR nodes.hostname = '' THEN EXCLUDED.hostname
        ELSE nodes.hostname
    END,
    display_name = CASE
        WHEN nodes.display_name = nodes.node_id OR nodes.display_name = '' THEN EXCLUDED.display_name
        ELSE nodes.display_name
    END,
    last_seen_at = EXCLUDED.last_seen_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_NODE_INVENTORY_SQL: &str = r#"
INSERT INTO node_inventory (
    node_id,
    architecture,
    kernel_version,
    os_release,
    cpu_count,
    memory_bytes,
    disk_bytes,
    cloud_hypervisor_version,
    chv_agent_version,
    chv_stord_version,
    chv_nwd_version,
    host_bundle_version,
    inventory_status,
    storage_classes,
    network_capabilities,
    labels,
    hypervisor_capabilities,
    last_reported_at,
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
    $12,
    $13,
    $14,
    $15,
    $16,
    $17,
    strftime('%Y-%m-%dT%H:%M:%SZ', $18 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $18 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    architecture = EXCLUDED.architecture,
    kernel_version = EXCLUDED.kernel_version,
    os_release = EXCLUDED.os_release,
    cpu_count = EXCLUDED.cpu_count,
    memory_bytes = EXCLUDED.memory_bytes,
    disk_bytes = EXCLUDED.disk_bytes,
    cloud_hypervisor_version = COALESCE(EXCLUDED.cloud_hypervisor_version, node_inventory.cloud_hypervisor_version),
    chv_agent_version = COALESCE(EXCLUDED.chv_agent_version, node_inventory.chv_agent_version),
    chv_stord_version = COALESCE(EXCLUDED.chv_stord_version, node_inventory.chv_stord_version),
    chv_nwd_version = COALESCE(EXCLUDED.chv_nwd_version, node_inventory.chv_nwd_version),
    host_bundle_version = COALESCE(EXCLUDED.host_bundle_version, node_inventory.host_bundle_version),
    inventory_status = EXCLUDED.inventory_status,
    storage_classes = EXCLUDED.storage_classes,
    network_capabilities = EXCLUDED.network_capabilities,
    labels = EXCLUDED.labels,
    hypervisor_capabilities = EXCLUDED.hypervisor_capabilities,
    last_reported_at = EXCLUDED.last_reported_at,
    updated_at = EXCLUDED.updated_at
"#;

const INSERT_NODE_VERSION_SQL: &str = r#"
INSERT INTO node_versions (
    node_id,
    component_name,
    version,
    source,
    reported_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    strftime('%Y-%m-%dT%H:%M:%SZ', $5 / 1000.0, 'unixepoch')
)
"#;

const UPSERT_NODE_DESIRED_STATE_SQL: &str = r#"
INSERT INTO node_desired_state (
    node_id,
    desired_generation,
    desired_state,
    requested_by,
    updated_by,
    state_reason,
    scheduling_paused,
    allow_workload_stop,
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
    strftime('%Y-%m-%dT%H:%M:%SZ', $9 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $9 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    desired_generation = EXCLUDED.desired_generation,
    desired_state = EXCLUDED.desired_state,
    requested_by = EXCLUDED.requested_by,
    updated_by = EXCLUDED.updated_by,
    state_reason = EXCLUDED.state_reason,
    scheduling_paused = EXCLUDED.scheduling_paused,
    allow_workload_stop = EXCLUDED.allow_workload_stop,
    requested_at = EXCLUDED.requested_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPDATE_NODE_CERTIFICATE_SQL: &str = r#"
UPDATE nodes SET
    certificate_serial = $2,
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
WHERE node_id = $1
"#;

const UPSERT_BOOTSTRAP_RESULT_SQL: &str = r#"
INSERT INTO node_bootstrap_results (
    node_id,
    operation_id,
    success,
    error_message,
    details,
    started_at,
    completed_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    strftime('%Y-%m-%dT%H:%M:%SZ', $6 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    operation_id = EXCLUDED.operation_id,
    success = EXCLUDED.success,
    error_message = EXCLUDED.error_message,
    details = EXCLUDED.details,
    started_at = EXCLUDED.started_at,
    completed_at = EXCLUDED.completed_at,
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
"#;

const PATCH_NODE_STATE_PRESERVING_POLICY_SQL: &str = r#"
INSERT INTO node_desired_state (
    node_id,
    desired_generation,
    desired_state,
    requested_by,
    updated_by,
    state_reason,
    scheduling_paused,
    allow_workload_stop,
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
    false,
    NULL,
    strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    desired_generation = EXCLUDED.desired_generation,
    desired_state = EXCLUDED.desired_state,
    requested_by = EXCLUDED.requested_by,
    updated_by = EXCLUDED.updated_by,
    state_reason = EXCLUDED.state_reason,
    requested_at = EXCLUDED.requested_at,
    updated_at = EXCLUDED.updated_at
"#;

const PATCH_NODE_SCHEDULING_SQL: &str = r#"
UPDATE node_desired_state SET
    desired_generation = $2,
    requested_by = $3,
    updated_by = $4,
    scheduling_paused = $5,
    requested_at = strftime('%Y-%m-%dT%H:%M:%SZ', $6 / 1000.0, 'unixepoch'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $6 / 1000.0, 'unixepoch')
WHERE node_id = $1
"#;

const PATCH_NODE_DRAIN_INTENT_SQL: &str = r#"
INSERT INTO node_desired_state (
    node_id,
    desired_generation,
    desired_state,
    requested_by,
    updated_by,
    state_reason,
    scheduling_paused,
    allow_workload_stop,
    requested_at,
    updated_at
)
VALUES (
    $1,
    $2,
    'Draining',
    $3,
    $4,
    NULL,
    false,
    $5,
    strftime('%Y-%m-%dT%H:%M:%SZ', $6 / 1000.0, 'unixepoch'),
    strftime('%Y-%m-%dT%H:%M:%SZ', $6 / 1000.0, 'unixepoch')
)
ON CONFLICT (node_id) DO UPDATE SET
    desired_generation = EXCLUDED.desired_generation,
    desired_state = EXCLUDED.desired_state,
    requested_by = EXCLUDED.requested_by,
    updated_by = EXCLUDED.updated_by,
    allow_workload_stop = EXCLUDED.allow_workload_stop,
    requested_at = EXCLUDED.requested_at,
    updated_at = EXCLUDED.updated_at
"#;

#[derive(Clone)]
pub struct NodeRepository {
    pool: StorePool,
}

impl NodeRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn upsert_node(&self, input: &NodeUpsertInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_NODE_SQL)
            .bind(input.node_id.as_str())
            .bind(&input.hostname)
            .bind(&input.display_name)
            .bind(&input.certificate_serial)
            .bind(&input.agent_version)
            .bind(&input.control_plane_version)
            .bind(input.enrolled_unix_ms)
            .bind(input.last_seen_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn ensure_node_record(
        &self,
        node_id: &NodeId,
        hostname: Option<&str>,
        display_name: Option<&str>,
        seen_unix_ms: i64,
    ) -> Result<(), StoreError> {
        let fallback = node_id.as_str();
        sqlx::query(ENSURE_NODE_RECORD_SQL)
            .bind(fallback)
            .bind(hostname.unwrap_or(fallback))
            .bind(display_name.unwrap_or(fallback))
            .bind(seen_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_inventory(&self, input: &NodeInventoryInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_NODE_INVENTORY_SQL)
            .bind(input.node_id.as_str())
            .bind(&input.architecture)
            .bind(&input.kernel_version)
            .bind(&input.os_release)
            .bind(input.cpu_count)
            .bind(input.memory_bytes)
            .bind(input.disk_bytes)
            .bind(&input.cloud_hypervisor_version)
            .bind(&input.chv_agent_version)
            .bind(&input.chv_stord_version)
            .bind(&input.chv_nwd_version)
            .bind(&input.host_bundle_version)
            .bind(&input.inventory_status)
            .bind(&input.storage_classes)
            .bind(&input.network_capabilities)
            .bind(&input.labels)
            .bind(&input.hypervisor_capabilities)
            .bind(input.reported_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn append_version(&self, input: &NodeVersionInput) -> Result<(), StoreError> {
        sqlx::query(INSERT_NODE_VERSION_SQL)
            .bind(input.node_id.as_str())
            .bind(&input.component_name)
            .bind(&input.version)
            .bind(&input.source)
            .bind(input.reported_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_state(&self, input: &NodeStateInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_NODE_DESIRED_STATE_SQL)
            .bind(input.node_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(input.desired_state.as_str())
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(&input.state_reason)
            .bind(input.scheduling_paused)
            .bind(input.allow_workload_stop)
            .bind(input.requested_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_state_preserving_policy(
        &self,
        input: &NodeStatePatchInput,
    ) -> Result<(), StoreError> {
        sqlx::query(PATCH_NODE_STATE_PRESERVING_POLICY_SQL)
            .bind(input.node_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(input.desired_state.as_str())
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(&input.state_reason)
            .bind(input.requested_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_scheduling_paused(
        &self,
        input: &NodeSchedulingPatchInput,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(PATCH_NODE_SCHEDULING_SQL)
            .bind(input.node_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(input.scheduling_paused)
            .bind(input.requested_unix_ms)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "node_desired_state",
                id: input.node_id.to_string(),
            });
        }

        Ok(())
    }

    pub async fn set_drain_intent(&self, input: &NodeDrainIntentInput) -> Result<(), StoreError> {
        sqlx::query(PATCH_NODE_DRAIN_INTENT_SQL)
            .bind(input.node_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(input.allow_workload_stop)
            .bind(input.requested_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_certificate_serial(
        &self,
        node_id: &NodeId,
        serial: &str,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(UPDATE_NODE_CERTIFICATE_SQL)
            .bind(node_id.as_str())
            .bind(serial)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "node",
                id: node_id.to_string(),
            });
        }

        Ok(())
    }

    pub async fn upsert_bootstrap_result(
        &self,
        input: &NodeBootstrapResultInput,
    ) -> Result<(), StoreError> {
        sqlx::query(UPSERT_BOOTSTRAP_RESULT_SQL)
            .bind(input.node_id.as_str())
            .bind(&input.operation_id)
            .bind(input.success)
            .bind(&input.error_message)
            .bind(&input.details)
            .bind(input.started_unix_ms)
            .bind(input.completed_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct NodeUpsertInput {
    pub node_id: NodeId,
    pub hostname: String,
    pub display_name: String,
    pub certificate_serial: Option<String>,
    pub agent_version: Option<String>,
    pub control_plane_version: Option<String>,
    pub enrolled_unix_ms: i64,
    pub last_seen_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeInventoryInput {
    pub node_id: NodeId,
    pub architecture: String,
    pub kernel_version: Option<String>,
    pub os_release: Option<String>,
    pub cpu_count: i32,
    pub memory_bytes: i64,
    pub disk_bytes: Option<i64>,
    pub cloud_hypervisor_version: Option<String>,
    pub chv_agent_version: Option<String>,
    pub chv_stord_version: Option<String>,
    pub chv_nwd_version: Option<String>,
    pub host_bundle_version: Option<String>,
    pub inventory_status: Option<String>,
    pub storage_classes: Option<serde_json::Value>,
    pub network_capabilities: Option<serde_json::Value>,
    pub labels: Option<serde_json::Value>,
    pub hypervisor_capabilities: Option<serde_json::Value>,
    pub reported_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeVersionInput {
    pub node_id: NodeId,
    pub component_name: String,
    pub version: String,
    pub source: Option<String>,
    pub reported_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeStateInput {
    pub node_id: NodeId,
    pub desired_state: NodeState,
    pub desired_generation: Generation,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub state_reason: Option<String>,
    pub scheduling_paused: bool,
    pub allow_workload_stop: Option<bool>,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeStatePatchInput {
    pub node_id: NodeId,
    pub desired_state: NodeState,
    pub desired_generation: Generation,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub state_reason: Option<String>,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeSchedulingPatchInput {
    pub node_id: NodeId,
    pub desired_generation: Generation,
    pub scheduling_paused: bool,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeDrainIntentInput {
    pub node_id: NodeId,
    pub desired_generation: Generation,
    pub allow_workload_stop: Option<bool>,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct NodeBootstrapResultInput {
    pub node_id: NodeId,
    pub operation_id: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub details: Option<serde_json::Value>,
    pub started_unix_ms: Option<i64>,
    pub completed_unix_ms: i64,
}

fn generation_to_i64(generation: Generation) -> Result<i64, StoreError> {
    i64::try_from(generation.get()).map_err(|source| StoreError::InvalidConfiguration {
        reason: format!("generation out of range for bigint column: {source}"),
    })
}
