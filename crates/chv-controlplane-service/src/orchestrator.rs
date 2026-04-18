use crate::node_client::NodeClient;
use chv_controlplane_store::{
    OperationRepository, OperationStatusUpdateInput, StorePool,
};
use chv_controlplane_types::domain::{OperationId, OperationStatus};
use chv_errors::ChvError;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

/// Background task that polls for accepted operations and dispatches them to node agents.
pub struct Orchestrator {
    pool: StorePool,
    operation_repo: OperationRepository,
    agent_socket_pattern: String,
    kernel_path: String,
    tick_interval: Duration,
}

impl Orchestrator {
    pub fn new(
        pool: StorePool,
        operation_repo: OperationRepository,
        agent_socket_pattern: String,
        kernel_path: String,
    ) -> Self {
        Self {
            pool,
            operation_repo,
            agent_socket_pattern,
            kernel_path,
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
        let node_id = row.node_id.as_deref().ok_or_else(|| ChvError::InvalidArgument {
            field: "node_id".to_string(),
            reason: format!(
                "operation {} has no target node",
                row.operation_id
            ),
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
                let vm_spec_json = self
                    .build_agent_vm_spec(&row.resource_id)
                    .await?;
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
                    .start_vm(node_id, &row.resource_id, &generation, &row.operation_id, None)
                    .await
            }
            "StopVm" => {
                client
                    .stop_vm(node_id, &row.resource_id, &generation, false, &row.operation_id, None)
                    .await
            }
            "RebootVm" => {
                client
                    .reboot_vm(node_id, &row.resource_id, &generation, false, &row.operation_id, None)
                    .await
            }
            "DeleteVm" => {
                client
                    .delete_vm(node_id, &row.resource_id, &generation, false, &row.operation_id, None)
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
                        reason: format!(
                            "agent rejected operation and status update failed: {e2}"
                        ),
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
    async fn build_agent_vm_spec(&self, vm_id: &str) -> Result<String, ChvError> {
        let vm_row = sqlx::query_as::<_, VmDesiredStateRow>(
            r#"
            SELECT
                v.display_name,
                vds.cpu_count,
                vds.memory_bytes,
                vds.image_ref,
                vds.desired_power_state
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

        let volume_rows = sqlx::query_as::<_, VolumeDesiredStateRow>(
            r#"
            SELECT
                volume_id,
                read_only
            FROM volume_desired_state
            WHERE attached_vm_id = ?
            ORDER BY volume_id
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

        let disks: Vec<AgentDiskSpec> = volume_rows
            .into_iter()
            .map(|v| AgentDiskSpec {
                volume_id: v.volume_id,
                read_only: v.read_only.unwrap_or(false),
            })
            .collect();

        let nics: Vec<AgentNicSpec> = nic_rows
            .into_iter()
            .map(|n| AgentNicSpec {
                network_id: n.network_id,
                mac_address: n.mac_address.unwrap_or_default(),
                ip_address: n.ip_address.unwrap_or_default(),
            })
            .collect();

        let desired_state = vm_row.desired_power_state.unwrap_or_else(|| "Running".to_string());

        let spec = AgentVmSpec {
            name: vm_row.display_name.unwrap_or_else(|| vm_id.to_string()),
            cpus: vm_row.cpu_count.unwrap_or(1) as u32,
            memory_bytes: vm_row.memory_bytes.unwrap_or(512 * 1024 * 1024) as u64,
            kernel_path,
            disks,
            nics,
            desired_state,
        };

        serde_json::to_string(&spec).map_err(|e| ChvError::Internal {
            reason: format!("failed to serialize agent vm spec: {e}"),
        })
    }

    fn resolve_kernel_path(&self, image_ref: &str) -> String {
        // For the first VM milestone, use a simple config-based mapping.
        // In production this would query an image registry.
        if image_ref == "default" || image_ref.is_empty() {
            self.kernel_path.clone()
        } else {
            format!("/var/lib/chv/kernels/{}", image_ref)
        }
    }
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
}

#[derive(sqlx::FromRow)]
struct VmDesiredStateRow {
    display_name: Option<String>,
    cpu_count: Option<i32>,
    memory_bytes: Option<i64>,
    image_ref: Option<String>,
    desired_power_state: Option<String>,
}

#[derive(sqlx::FromRow)]
struct VolumeDesiredStateRow {
    volume_id: String,
    read_only: Option<bool>,
}

#[derive(sqlx::FromRow)]
struct VmNicRow {
    network_id: String,
    mac_address: Option<String>,
    ip_address: Option<String>,
}

#[derive(serde::Serialize)]
struct AgentVmSpec {
    name: String,
    cpus: u32,
    memory_bytes: u64,
    kernel_path: String,
    disks: Vec<AgentDiskSpec>,
    nics: Vec<AgentNicSpec>,
    desired_state: String,
}

#[derive(serde::Serialize)]
struct AgentDiskSpec {
    volume_id: String,
    read_only: bool,
}

#[derive(serde::Serialize)]
struct AgentNicSpec {
    network_id: String,
    mac_address: String,
    ip_address: String,
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
