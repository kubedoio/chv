use async_trait::async_trait;
use chv_controlplane_store::StorePool;
use chv_controlplane_types::domain::Generation;
use chv_webui_bff::{BffError, MutationService};
use chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse;
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lifecycle::LifecycleService;

#[derive(Clone)]
pub struct ControlPlaneMutationService {
    pool: StorePool,
    lifecycle_service: Arc<dyn LifecycleService>,
}

impl ControlPlaneMutationService {
    pub fn new(pool: StorePool, lifecycle_service: Arc<dyn LifecycleService>) -> Self {
        Self {
            pool,
            lifecycle_service,
        }
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    fn fresh_generation() -> Generation {
        // Use current time in milliseconds as a monotonic generation.
        Generation::new(Self::now_ms() as u64)
    }
}

#[async_trait]
impl MutationService for ControlPlaneMutationService {
    async fn mutate_vm(
        &self,
        vm_id: String,
        action: String,
        force: bool,
    ) -> Result<MutateVmResponse, BffError> {
        // Look up the VM's node_id from the vms table.
        let node_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT node_id FROM vms WHERE vm_id = $1"
        )
        .bind(&vm_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to look up vm: {}", e)))?
        .ok_or_else(|| BffError::NotFound(format!("vm {} not found", vm_id)))?;

        let generation = Self::fresh_generation();
        let meta = Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "webui".into(),
            target_node_id: node_id.clone(),
            desired_state_version: generation.to_string(),
            request_unix_ms: Self::now_ms(),
        });

        let ack = match action.as_str() {
            "start" => {
                self.lifecycle_service
                    .start_vm(proto::StartVmRequest {
                        meta,
                        node_id: node_id.clone(),
                        vm_id: vm_id.clone(),
                    })
                    .await
            }
            "stop" => {
                self.lifecycle_service
                    .stop_vm(proto::StopVmRequest {
                        meta,
                        node_id: node_id.clone(),
                        vm_id: vm_id.clone(),
                        force,
                    })
                    .await
            }
            "restart" => {
                self.lifecycle_service
                    .reboot_vm(proto::RebootVmRequest {
                        meta,
                        node_id: node_id.clone(),
                        vm_id: vm_id.clone(),
                        force,
                    })
                    .await
            }
            _ => return Err(BffError::BadRequest(format!("invalid action: {}", action))),
        };

        let ack = ack.map_err(|e| BffError::Internal(e.to_string()))?;
        let result = ack.result.ok_or_else(|| BffError::Internal("missing ack result".into()))?;

        Ok(MutateVmResponse {
            accepted: result.status == "OK",
            task_id: result.operation_id,
            vm_id,
            summary: result.human_summary,
        })
    }
}
