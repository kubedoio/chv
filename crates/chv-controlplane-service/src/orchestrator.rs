use crate::node_client::NodeClient;
use chv_controlplane_store::{
    HypervisorSettingsRepository, HypervisorSettingsRow, OperationRepository,
    OperationStatusUpdateInput, StorePool,
};
use chv_controlplane_types::domain::{OperationId, OperationStatus};
use chv_errors::ChvError;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

use chv_common::hypervisor::HypervisorOverrides;

/// Background task that polls for accepted operations and dispatches them to node agents.
pub struct Orchestrator {
    pool: StorePool,
    operation_repo: OperationRepository,
    agent_socket_pattern: String,
    kernel_path: String,
    firmware_path: String,
    tick_interval: Duration,
}

impl Orchestrator {
    pub fn new(
        pool: StorePool,
        operation_repo: OperationRepository,
        agent_socket_pattern: String,
        kernel_path: String,
        firmware_path: String,
    ) -> Self {
        Self {
            pool,
            operation_repo,
            agent_socket_pattern,
            kernel_path,
            firmware_path,
            tick_interval: Duration::from_secs(2),
        }
    }

    pub async fn run(self) {
        info!("orchestrator starting");
        let mut interval = tokio::time::interval(self.tick_interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.tick().await {
                warn!(error = %e, "orchestrator tick failed");
            }
        }
    }

    async fn tick(&self) -> Result<(), ChvError> {
        let rows = sqlx::query_as::<_, AcceptedOperationRow>(
            r#"
            SELECT
                o.operation_id,
                o.operation_type,
                o.resource_kind,
                o.resource_id,
                o.desired_generation,
                o.correlation_id,
                COALESCE(vds.target_node_id, vol.node_id, net.node_id) as node_id
            FROM operations o
            LEFT JOIN vm_desired_state vds ON o.resource_id = vds.vm_id
            LEFT JOIN volumes vol ON o.resource_id = vol.volume_id
            LEFT JOIN networks net ON o.resource_id = net.network_id
            WHERE o.status = 'Accepted'
            ORDER BY o.requested_at ASC
            LIMIT 10
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query accepted operations: {e}"),
        })?;

        for row in rows {
            if let Err(e) = self.dispatch_operation(&row).await {
                warn!(
                    operation_id = %row.operation_id,
                    operation_type = %row.operation_type,
                    error = %e,
                    "dispatch failed"
                );
                if let Err(update_err) = self
                    .operation_repo
                    .update_status(&OperationStatusUpdateInput {
                        operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                            ChvError::Internal {
                                reason: format!("invalid operation_id: {e}"),
                            }
                        })?,
                        status: OperationStatus::Failed,
                        error_code: Some("DISPATCH_FAILED".into()),
                        error_message: Some(e.to_string()),
                        observed_generation: None,
                        updated_by: Some("orchestrator".into()),
                        updated_unix_ms: now_unix_ms(),
                    })
                    .await
                {
                    error!(
                        operation_id = %row.operation_id,
                        error = %update_err,
                        "failed to update operation status after dispatch failure"
                    );
                }
            }
        }

        Ok(())
    }

    async fn dispatch_operation(&self, row: &AcceptedOperationRow) -> Result<(), ChvError> {
        let node_id = row
            .node_id
            .as_deref()
            .ok_or_else(|| ChvError::InvalidArgument {
                field: "node_id".to_string(),
                reason: format!("operation {} has no target node", row.operation_id),
            })?;

        let socket_path = self.resolve_agent_socket(node_id);
        let mut client = NodeClient::connect(&socket_path).await?;

        let generation = row
            .desired_generation
            .map(|g| g.to_string())
            .unwrap_or_else(|| "1".to_string());

        // Mark as Running before dispatch
        self.operation_repo
            .update_status(&OperationStatusUpdateInput {
                operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                    ChvError::Internal {
                        reason: format!("invalid operation_id: {e}"),
                    }
                })?,
                status: OperationStatus::Running,
                error_code: None,
                error_message: None,
                observed_generation: None,
                updated_by: Some("orchestrator".into()),
                updated_unix_ms: now_unix_ms(),
            })
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to mark operation Running: {e}"),
            })?;

        let ack = match row.operation_type.as_str() {
            "create" | "CreateVm" => {
                // Desired-state path: build full agent spec and dispatch ApplyVmDesiredState
                let vm_spec_json = self.build_agent_vm_spec(&row.resource_id).await?;
                client
                    .apply_vm_desired_state(
                        node_id,
                        &row.resource_id,
                        &generation,
                        vm_spec_json.into_bytes(),
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StartVm" => {
                client
                    .start_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StopVm" => {
                client
                    .stop_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ForceStopVm" => {
                client
                    .stop_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        true,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RebootVm" => {
                client
                    .reboot_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "DeleteVm" => {
                client
                    .delete_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "SnapshotVm" => {
                let destination = row.correlation_id.as_deref().unwrap_or("");
                client
                    .snapshot_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        destination,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RestoreSnapshot" => {
                let source = row.correlation_id.as_deref().unwrap_or("");
                client
                    .restore_snapshot(
                        node_id,
                        &row.resource_id,
                        &generation,
                        source,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "AttachVolume" => {
                let corr = row.correlation_id.as_deref().unwrap_or("");
                let vm_id = corr.strip_prefix("vm=").unwrap_or(corr);
                client
                    .attach_volume(
                        node_id,
                        &row.resource_id,
                        vm_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "DetachVolume" => {
                let corr = row.correlation_id.as_deref().unwrap_or("");
                let vm_id = corr
                    .strip_prefix("vm=")
                    .and_then(|s| s.split(':').next())
                    .unwrap_or(corr);
                let force = corr.contains("force=true");
                client
                    .detach_volume(
                        node_id,
                        &row.resource_id,
                        vm_id,
                        &generation,
                        force,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ResizeVolume" => {
                let new_size = row
                    .correlation_id
                    .as_deref()
                    .and_then(|s| s.strip_prefix("size="))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                client
                    .resize_volume(
                        node_id,
                        &row.resource_id,
                        &generation,
                        new_size,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "SnapshotVolume" => {
                let snapshot_name = row.correlation_id.as_deref().unwrap_or("");
                client
                    .snapshot_volume(
                        node_id,
                        &row.resource_id,
                        &generation,
                        snapshot_name,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RestoreVolume" => {
                let snapshot_name = row.correlation_id.as_deref().unwrap_or("");
                client
                    .restore_volume(
                        node_id,
                        &row.resource_id,
                        &generation,
                        snapshot_name,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "DeleteVolumeSnapshot" => {
                let snapshot_name = row.correlation_id.as_deref().unwrap_or("");
                client
                    .delete_volume_snapshot(
                        node_id,
                        &row.resource_id,
                        &generation,
                        snapshot_name,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "CloneVolume" => {
                let source = row.correlation_id.as_deref().unwrap_or("");
                let source_volume_id = source.strip_prefix("source=").unwrap_or(source);
                client
                    .clone_volume(
                        node_id,
                        source_volume_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StartNetwork" => {
                client
                    .start_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StopNetwork" => {
                client
                    .stop_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ForceStopNetwork" => {
                client
                    .stop_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        true,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RestartNetwork" => {
                client
                    .restart_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            other => {
                return Err(ChvError::Internal {
                    reason: format!("unsupported operation_type for dispatch: {other}"),
                });
            }
        };

        match ack {
            Ok(result) => {
                let status = result
                    .result
                    .as_ref()
                    .map(|r| r.status.as_str())
                    .unwrap_or("OK");
                let accepted = status.eq_ignore_ascii_case("ok");
                let final_status = if accepted {
                    OperationStatus::Succeeded
                } else {
                    OperationStatus::Failed
                };
                let error_message = if accepted {
                    None
                } else {
                    result.result.map(|r| r.human_summary)
                };
                self.operation_repo
                    .update_status(&OperationStatusUpdateInput {
                        operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                            ChvError::Internal {
                                reason: format!("invalid operation_id: {e}"),
                            }
                        })?,
                        status: final_status,
                        error_code: None,
                        error_message,
                        observed_generation: None,
                        updated_by: Some("orchestrator".into()),
                        updated_unix_ms: now_unix_ms(),
                    })
                    .await
                    .map_err(|e| ChvError::Internal {
                        reason: format!("failed to mark operation terminal: {e}"),
                    })?;

                // For successful resize, apply the new size to volumes.capacity_bytes
                if accepted && row.operation_type == "ResizeVolume" {
                    if let Some(new_size) = row
                        .correlation_id
                        .as_deref()
                        .and_then(|s| s.strip_prefix("size="))
                        .and_then(|s| s.parse::<i64>().ok())
                    {
                        let volume_id = &row.resource_id;
                        let _ = sqlx::query(
                            "UPDATE volumes SET capacity_bytes = ? WHERE volume_id = ?",
                        )
                        .bind(new_size)
                        .bind(volume_id)
                        .execute(&self.pool)
                        .await;
                        let _ = sqlx::query(
                            "UPDATE volume_desired_state SET resize_to_bytes = NULL WHERE volume_id = ?"
                        )
                        .bind(volume_id)
                        .execute(&self.pool)
                        .await;
                    }
                }

                info!(
                    operation_id = %row.operation_id,
                    operation_type = %row.operation_type,
                    node_id = %node_id,
                    "dispatch succeeded"
                );
                Ok(())
            }
            Err(e) => {
                self.operation_repo
                    .update_status(&OperationStatusUpdateInput {
                        operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                            ChvError::Internal {
                                reason: format!("invalid operation_id: {e}"),
                            }
                        })?,
                        status: OperationStatus::Failed,
                        error_code: Some("AGENT_REJECTED".into()),
                        error_message: Some(e.to_string()),
                        observed_generation: None,
                        updated_by: Some("orchestrator".into()),
                        updated_unix_ms: now_unix_ms(),
                    })
                    .await
                    .map_err(|e2| ChvError::Internal {
                        reason: format!("agent rejected operation and status update failed: {e2}"),
                    })?;
                Err(e)
            }
        }
    }

    fn resolve_agent_socket(&self, node_id: &str) -> PathBuf {
        if self.agent_socket_pattern.contains("{node_id}") {
            PathBuf::from(self.agent_socket_pattern.replace("{node_id}", node_id))
        } else {
            PathBuf::from(&self.agent_socket_pattern)
        }
    }

    /// Build the agent-compatible VmSpec JSON from control-plane DB records.
    pub(crate) async fn build_agent_vm_spec(&self, vm_id: &str) -> Result<String, ChvError> {
        let vm_row = sqlx::query_as::<_, VmDesiredStateRow>(
            r#"
            SELECT
                v.display_name,
                vds.cpu_count,
                vds.memory_bytes,
                vds.image_ref,
                vds.desired_power_state,
                vds.cloud_init_userdata,
                v.hv_cpu_nested,
                v.hv_cpu_amx,
                v.hv_cpu_kvm_hyperv,
                v.hv_memory_mergeable,
                v.hv_memory_hugepages,
                v.hv_memory_shared,
                v.hv_memory_prefault,
                v.hv_iommu,
                v.hv_rng_src,
                v.hv_watchdog,
                v.hv_landlock_enable,
                v.hv_serial_mode,
                v.hv_console_mode,
                v.hv_pvpanic,
                v.hv_tpm_type,
                v.hv_tpm_socket_path
            FROM vms v
            JOIN vm_desired_state vds ON v.vm_id = vds.vm_id
            WHERE v.vm_id = ?
            "#,
        )
        .bind(vm_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query vm desired state: {e}"),
        })?
        .ok_or_else(|| ChvError::NotFound {
            resource: "vm_desired_state".to_string(),
            id: vm_id.to_string(),
        })?;

        let global = HypervisorSettingsRepository::new(self.pool.clone())
            .get_settings()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(vm_id = %vm_id, error = %e, "failed to fetch hypervisor_settings, using defaults");
                HypervisorSettingsRow {
                    id: 1,
                    cpu_nested: chv_common::hypervisor::DEFAULT_CPU_NESTED,
                    cpu_amx: chv_common::hypervisor::DEFAULT_CPU_AMX,
                    cpu_kvm_hyperv: chv_common::hypervisor::DEFAULT_CPU_KVM_HYPERV,
                    memory_mergeable: chv_common::hypervisor::DEFAULT_MEMORY_MERGEABLE,
                    memory_hugepages: chv_common::hypervisor::DEFAULT_MEMORY_HUGEPAGES,
                    memory_shared: chv_common::hypervisor::DEFAULT_MEMORY_SHARED,
                    memory_prefault: chv_common::hypervisor::DEFAULT_MEMORY_PREFAULT,
                    iommu: chv_common::hypervisor::DEFAULT_IOMMU,
                    rng_src: chv_common::hypervisor::DEFAULT_RNG_SRC.to_string(),
                    watchdog: chv_common::hypervisor::DEFAULT_WATCHDOG,
                    landlock_enable: chv_common::hypervisor::DEFAULT_LANDLOCK_ENABLE,
                    serial_mode: chv_common::hypervisor::DEFAULT_SERIAL_MODE.to_string(),
                    console_mode: chv_common::hypervisor::DEFAULT_CONSOLE_MODE.to_string(),
                    pvpanic: chv_common::hypervisor::DEFAULT_PVPANIC,
                    tpm_type: chv_common::hypervisor::DEFAULT_TPM_TYPE.map(|s| s.to_string()),
                    tpm_socket_path: chv_common::hypervisor::DEFAULT_TPM_SOCKET_PATH.map(|s| s.to_string()),
                    profile_id: None,
                    updated_at: String::new(),
                }
            });

        let volume_rows = sqlx::query_as::<_, VolumeDesiredStateRow>(
            r#"
            SELECT
                vds.volume_id,
                vds.read_only,
                v.capacity_bytes
            FROM volume_desired_state vds
            JOIN volumes v ON v.volume_id = vds.volume_id
            WHERE vds.attached_vm_id = ?
            ORDER BY vds.volume_id
            "#,
        )
        .bind(vm_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query volume desired state: {e}"),
        })?;

        let nic_rows = sqlx::query_as::<_, VmNicRow>(
            r#"
            SELECT
                network_id,
                mac_address,
                ip_address
            FROM vm_nic_desired_state
            WHERE vm_id = ?
            ORDER BY nic_id
            "#,
        )
        .bind(vm_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query vm nic desired state: {e}"),
        })?;

        let kernel_path = if let Some(ref image_ref) = vm_row.image_ref {
            self.resolve_kernel_path(image_ref)
        } else {
            self.kernel_path.clone()
        };

        let disks: Vec<AgentDiskSpec> =
            volume_rows
                .into_iter()
                .map(|v| AgentDiskSpec {
                    volume_id: v.volume_id,
                    read_only: v.read_only.unwrap_or(false),
                    size_bytes: v.capacity_bytes.and_then(|b| {
                        if b > 0 {
                            Some(b as u64)
                        } else {
                            None
                        }
                    }),
                })
                .collect();

        let mut network_configs: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();
        for nic in &nic_rows {
            if network_configs.contains_key(&nic.network_id) {
                continue;
            }
            let row = sqlx::query_as::<_, NetworkDesiredStateRow>(
                "SELECT cidr, gateway FROM network_desired_state WHERE network_id = ?",
            )
            .bind(&nic.network_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to query network desired state: {e}"),
            })?;
            let cidr = row
                .as_ref()
                .and_then(|r| r.cidr.clone())
                .unwrap_or_default();
            let gateway = row
                .as_ref()
                .and_then(|r| r.gateway.clone())
                .unwrap_or_default();
            network_configs.insert(nic.network_id.clone(), (cidr, gateway));
        }

        let nics: Vec<AgentNicSpec> = nic_rows
            .into_iter()
            .map(|n| {
                let (cidr, gateway) = network_configs
                    .get(&n.network_id)
                    .cloned()
                    .unwrap_or_default();
                AgentNicSpec {
                    network_id: n.network_id,
                    mac_address: n.mac_address.unwrap_or_default(),
                    ip_address: n.ip_address.unwrap_or_default(),
                    cidr,
                    gateway,
                }
            })
            .collect();

        let desired_state = vm_row
            .desired_power_state
            .unwrap_or_else(|| "Running".to_string());

        let overrides = HypervisorOverrides {
            cpu_nested: Some(vm_row.hv_cpu_nested.unwrap_or(global.cpu_nested)),
            cpu_amx: Some(vm_row.hv_cpu_amx.unwrap_or(global.cpu_amx)),
            cpu_kvm_hyperv: Some(vm_row.hv_cpu_kvm_hyperv.unwrap_or(global.cpu_kvm_hyperv)),
            memory_mergeable: Some(
                vm_row
                    .hv_memory_mergeable
                    .unwrap_or(global.memory_mergeable),
            ),
            memory_hugepages: Some(
                vm_row
                    .hv_memory_hugepages
                    .unwrap_or(global.memory_hugepages),
            ),
            memory_shared: Some(vm_row.hv_memory_shared.unwrap_or(global.memory_shared)),
            memory_prefault: Some(vm_row.hv_memory_prefault.unwrap_or(global.memory_prefault)),
            iommu: Some(vm_row.hv_iommu.unwrap_or(global.iommu)),
            rng_src: Some(vm_row.hv_rng_src.unwrap_or_else(|| global.rng_src.clone())),
            watchdog: Some(vm_row.hv_watchdog.unwrap_or(global.watchdog)),
            landlock_enable: Some(vm_row.hv_landlock_enable.unwrap_or(global.landlock_enable)),
            serial_mode: Some(
                vm_row
                    .hv_serial_mode
                    .unwrap_or_else(|| global.serial_mode.clone()),
            ),
            console_mode: Some(
                vm_row
                    .hv_console_mode
                    .unwrap_or_else(|| global.console_mode.clone()),
            ),
            pvpanic: Some(vm_row.hv_pvpanic.unwrap_or(global.pvpanic)),
            tpm_type: vm_row
                .hv_tpm_type
                .clone()
                .or_else(|| global.tpm_type.clone())
                .or_else(|| chv_common::hypervisor::DEFAULT_TPM_TYPE.map(|s| s.to_string())),
            tpm_socket_path: vm_row
                .hv_tpm_socket_path
                .clone()
                .or_else(|| global.tpm_socket_path.clone())
                .or_else(|| chv_common::hypervisor::DEFAULT_TPM_SOCKET_PATH.map(|s| s.to_string())),
        };

        if let Err(e) = validate_merged_overrides(&overrides) {
            return Err(ChvError::InvalidArgument {
                field: "hypervisor_overrides".to_string(),
                reason: e,
            });
        }

        let spec = AgentVmSpec {
            name: vm_row.display_name.unwrap_or_else(|| vm_id.to_string()),
            cpus: vm_row.cpu_count.unwrap_or(1) as u32,
            memory_bytes: vm_row.memory_bytes.unwrap_or(512 * 1024 * 1024) as u64,
            kernel_path,
            firmware_path: Some(self.firmware_path.clone()),
            disk_seed_path: self.resolve_disk_seed_path(vm_row.image_ref.as_deref()),
            disks,
            nics,
            desired_state,
            cloud_init_userdata: vm_row.cloud_init_userdata,
            hypervisor_overrides: Some(overrides),
        };

        serde_json::to_string(&spec).map_err(|e| ChvError::Internal {
            reason: format!("failed to serialize agent vm spec: {e}"),
        })
    }

    fn resolve_kernel_path(&self, image_ref: &str) -> String {
        // For the first VM milestone, use a simple config-based mapping.
        // In production this would query an image registry.
        // If image_ref looks like a disk image path (absolute path or file:// URI),
        // use the default kernel path instead.
        if image_ref == "default"
            || image_ref.is_empty()
            || image_ref.starts_with('/')
            || image_ref.starts_with("file://")
        {
            self.kernel_path.clone()
        } else {
            format!("/var/lib/chv/kernels/{}", image_ref)
        }
    }

    fn resolve_disk_seed_path(&self, image_ref: Option<&str>) -> Option<String> {
        let image_ref = image_ref?.trim();
        if image_ref.is_empty() || image_ref == "default" {
            return None;
        }
        if let Some(path) = image_ref.strip_prefix("file://") {
            return Some(path.to_string());
        }
        if image_ref.starts_with('/') {
            return Some(image_ref.to_string());
        }
        Some(format!("/var/lib/chv/images/{}", image_ref))
    }
}

fn validate_merged_overrides(overrides: &HypervisorOverrides) -> Result<(), String> {
    if overrides.iommu == Some(true) && overrides.memory_shared != Some(true) {
        return Err("iommu=true requires memory_shared=true".to_string());
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct AcceptedOperationRow {
    operation_id: String,
    operation_type: String,
    #[allow(dead_code)]
    resource_kind: String,
    resource_id: String,
    desired_generation: Option<i64>,
    node_id: Option<String>,
    correlation_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct VmDesiredStateRow {
    display_name: Option<String>,
    cpu_count: Option<i32>,
    memory_bytes: Option<i64>,
    image_ref: Option<String>,
    desired_power_state: Option<String>,
    cloud_init_userdata: Option<String>,
    hv_cpu_nested: Option<bool>,
    hv_cpu_amx: Option<bool>,
    hv_cpu_kvm_hyperv: Option<bool>,
    hv_memory_mergeable: Option<bool>,
    hv_memory_hugepages: Option<bool>,
    hv_memory_shared: Option<bool>,
    hv_memory_prefault: Option<bool>,
    hv_iommu: Option<bool>,
    hv_rng_src: Option<String>,
    hv_watchdog: Option<bool>,
    hv_landlock_enable: Option<bool>,
    hv_serial_mode: Option<String>,
    hv_console_mode: Option<String>,
    hv_pvpanic: Option<bool>,
    hv_tpm_type: Option<String>,
    hv_tpm_socket_path: Option<String>,
}

#[derive(sqlx::FromRow)]
struct VolumeDesiredStateRow {
    volume_id: String,
    read_only: Option<bool>,
    capacity_bytes: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct VmNicRow {
    network_id: String,
    mac_address: Option<String>,
    ip_address: Option<String>,
}

#[derive(sqlx::FromRow)]
struct NetworkDesiredStateRow {
    cidr: Option<String>,
    gateway: Option<String>,
}

#[derive(serde::Serialize)]
struct AgentVmSpec {
    name: String,
    cpus: u32,
    memory_bytes: u64,
    kernel_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    firmware_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disk_seed_path: Option<String>,
    disks: Vec<AgentDiskSpec>,
    nics: Vec<AgentNicSpec>,
    desired_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cloud_init_userdata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hypervisor_overrides: Option<HypervisorOverrides>,
}

#[derive(serde::Serialize)]
struct AgentDiskSpec {
    volume_id: String,
    read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
}

#[derive(serde::Serialize)]
struct AgentNicSpec {
    network_id: String,
    mac_address: String,
    ip_address: String,
    cidr: String,
    gateway: String,
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
