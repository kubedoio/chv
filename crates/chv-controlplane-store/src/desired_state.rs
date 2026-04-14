use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{Generation, NodeId, ResourceId};

const UPSERT_VM_SQL: &str = r#"
INSERT INTO vms (
    vm_id,
    node_id,
    display_name,
    tenant_id,
    placement_policy,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    to_timestamp($6 / 1000.0)
)
ON CONFLICT (vm_id) DO UPDATE SET
    node_id = EXCLUDED.node_id,
    display_name = EXCLUDED.display_name,
    tenant_id = EXCLUDED.tenant_id,
    placement_policy = EXCLUDED.placement_policy,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_VM_DESIRED_STATE_SQL: &str = r#"
INSERT INTO vm_desired_state (
    vm_id,
    desired_generation,
    desired_status,
    requested_by,
    updated_by,
    target_node_id,
    cpu_count,
    memory_bytes,
    image_ref,
    boot_mode,
    desired_power_state,
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
    to_timestamp($12 / 1000.0),
    to_timestamp($12 / 1000.0)
)
ON CONFLICT (vm_id) DO UPDATE SET
    desired_generation = EXCLUDED.desired_generation,
    desired_status = EXCLUDED.desired_status,
    requested_by = EXCLUDED.requested_by,
    updated_by = EXCLUDED.updated_by,
    target_node_id = EXCLUDED.target_node_id,
    cpu_count = EXCLUDED.cpu_count,
    memory_bytes = EXCLUDED.memory_bytes,
    image_ref = EXCLUDED.image_ref,
    boot_mode = EXCLUDED.boot_mode,
    desired_power_state = EXCLUDED.desired_power_state,
    requested_at = EXCLUDED.requested_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_VOLUME_SQL: &str = r#"
INSERT INTO volumes (
    volume_id,
    node_id,
    display_name,
    capacity_bytes,
    volume_kind,
    storage_class,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    to_timestamp($7 / 1000.0)
)
ON CONFLICT (volume_id) DO UPDATE SET
    node_id = EXCLUDED.node_id,
    display_name = EXCLUDED.display_name,
    capacity_bytes = EXCLUDED.capacity_bytes,
    volume_kind = EXCLUDED.volume_kind,
    storage_class = EXCLUDED.storage_class,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_VOLUME_DESIRED_STATE_SQL: &str = r#"
INSERT INTO volume_desired_state (
    volume_id,
    desired_generation,
    desired_status,
    requested_by,
    updated_by,
    attached_vm_id,
    attachment_mode,
    device_name,
    read_only,
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
    to_timestamp($10 / 1000.0),
    to_timestamp($10 / 1000.0)
)
ON CONFLICT (volume_id) DO UPDATE SET
    desired_generation = EXCLUDED.desired_generation,
    desired_status = EXCLUDED.desired_status,
    requested_by = EXCLUDED.requested_by,
    updated_by = EXCLUDED.updated_by,
    attached_vm_id = EXCLUDED.attached_vm_id,
    attachment_mode = EXCLUDED.attachment_mode,
    device_name = EXCLUDED.device_name,
    read_only = EXCLUDED.read_only,
    requested_at = EXCLUDED.requested_at,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_NETWORK_SQL: &str = r#"
INSERT INTO networks (
    network_id,
    node_id,
    display_name,
    network_class,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    to_timestamp($5 / 1000.0)
)
ON CONFLICT (network_id) DO UPDATE SET
    node_id = EXCLUDED.node_id,
    display_name = EXCLUDED.display_name,
    network_class = EXCLUDED.network_class,
    updated_at = EXCLUDED.updated_at
"#;

const UPSERT_NETWORK_DESIRED_STATE_SQL: &str = r#"
INSERT INTO network_desired_state (
    network_id,
    desired_generation,
    desired_status,
    requested_by,
    updated_by,
    requested_at,
    updated_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    to_timestamp($6 / 1000.0),
    to_timestamp($6 / 1000.0)
)
ON CONFLICT (network_id) DO UPDATE SET
    desired_generation = EXCLUDED.desired_generation,
    desired_status = EXCLUDED.desired_status,
    requested_by = EXCLUDED.requested_by,
    updated_by = EXCLUDED.updated_by,
    requested_at = EXCLUDED.requested_at,
    updated_at = EXCLUDED.updated_at
"#;

#[derive(Clone)]
pub struct DesiredStateRepository {
    pool: StorePool,
}

impl DesiredStateRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn upsert_vm(&self, input: &VmDesiredStateInput) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(UPSERT_VM_SQL)
            .bind(input.vm_id.as_str())
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(&input.display_name)
            .bind(&input.tenant_id)
            .bind(&input.placement_policy)
            .bind(input.requested_unix_ms)
            .execute(&mut *tx)
            .await?;

        sqlx::query(UPSERT_VM_DESIRED_STATE_SQL)
            .bind(input.vm_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(&input.desired_status)
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(input.target_node_id.as_ref().map(NodeId::as_str))
            .bind(input.cpu_count)
            .bind(input.memory_bytes)
            .bind(&input.image_ref)
            .bind(&input.boot_mode)
            .bind(&input.desired_power_state)
            .bind(input.requested_unix_ms)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn upsert_volume(&self, input: &VolumeDesiredStateInput) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(UPSERT_VOLUME_SQL)
            .bind(input.volume_id.as_str())
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(&input.display_name)
            .bind(input.capacity_bytes)
            .bind(&input.volume_kind)
            .bind(&input.storage_class)
            .bind(input.requested_unix_ms)
            .execute(&mut *tx)
            .await?;

        sqlx::query(UPSERT_VOLUME_DESIRED_STATE_SQL)
            .bind(input.volume_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(&input.desired_status)
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(input.attached_vm_id.as_ref().map(ResourceId::as_str))
            .bind(&input.attachment_mode)
            .bind(&input.device_name)
            .bind(input.read_only)
            .bind(input.requested_unix_ms)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn upsert_network(&self, input: &NetworkDesiredStateInput) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(UPSERT_NETWORK_SQL)
            .bind(input.network_id.as_str())
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(&input.display_name)
            .bind(&input.network_class)
            .bind(input.requested_unix_ms)
            .execute(&mut *tx)
            .await?;

        sqlx::query(UPSERT_NETWORK_DESIRED_STATE_SQL)
            .bind(input.network_id.as_str())
            .bind(generation_to_i64(input.desired_generation)?)
            .bind(&input.desired_status)
            .bind(&input.requested_by)
            .bind(&input.updated_by)
            .bind(input.requested_unix_ms)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct VmDesiredStateInput {
    pub vm_id: ResourceId,
    pub node_id: Option<NodeId>,
    pub display_name: String,
    pub tenant_id: Option<String>,
    pub placement_policy: Option<String>,
    pub desired_generation: Generation,
    pub desired_status: String,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub target_node_id: Option<NodeId>,
    pub cpu_count: Option<i32>,
    pub memory_bytes: Option<i64>,
    pub image_ref: Option<String>,
    pub boot_mode: Option<String>,
    pub desired_power_state: Option<String>,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct VolumeDesiredStateInput {
    pub volume_id: ResourceId,
    pub node_id: Option<NodeId>,
    pub display_name: String,
    pub capacity_bytes: i64,
    pub volume_kind: Option<String>,
    pub storage_class: Option<String>,
    pub desired_generation: Generation,
    pub desired_status: String,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub attached_vm_id: Option<ResourceId>,
    pub attachment_mode: Option<String>,
    pub device_name: Option<String>,
    pub read_only: bool,
    pub requested_unix_ms: i64,
}

#[derive(Clone)]
pub struct NetworkDesiredStateInput {
    pub network_id: ResourceId,
    pub node_id: Option<NodeId>,
    pub display_name: String,
    pub network_class: Option<String>,
    pub desired_generation: Generation,
    pub desired_status: String,
    pub requested_by: Option<String>,
    pub updated_by: Option<String>,
    pub requested_unix_ms: i64,
}

fn generation_to_i64(generation: Generation) -> Result<i64, StoreError> {
    i64::try_from(generation.get()).map_err(|source| StoreError::InvalidConfiguration {
        reason: format!("generation out of range for bigint column: {source}"),
    })
}
