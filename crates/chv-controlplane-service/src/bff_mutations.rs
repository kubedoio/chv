use async_trait::async_trait;
use chv_controlplane_store::StorePool;
use chv_controlplane_types::domain::Generation;
use chv_webui_bff::{BffError, MutationService};
use chv_webui_bff_api::chv_webui_bff_v1::{
    MutateNetworkResponse, MutateNodeResponse, MutateVmResponse, MutateVolumeResponse,
};
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lifecycle::LifecycleService;
use crate::ControlPlaneServiceError;

#[derive(Clone)]
pub struct ControlPlaneMutationService {
    pool: StorePool,
    lifecycle_service: Arc<dyn LifecycleService>,
}

#[derive(sqlx::FromRow)]
struct VolumeLookupRow {
    node_id: String,
    vm_id: Option<String>,
    size_bytes: i64,
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

    fn build_meta(&self, node_id: String, requested_by: String) -> Option<proto::RequestMeta> {
        let generation = Self::fresh_generation();
        Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by,
            target_node_id: node_id,
            desired_state_version: generation.to_string(),
            request_unix_ms: Self::now_ms(),
        })
    }

    fn map_ack(
        &self,
        ack: Result<proto::AckResponse, ControlPlaneServiceError>,
    ) -> Result<proto::AckResponse, BffError> {
        let ack = ack.map_err(|e| BffError::Internal(e.to_string()))?;
        if ack.result.as_ref().map(|r| r.status.as_str()) != Some("OK") {
            let msg = ack
                .result
                .as_ref()
                .map(|r| r.human_summary.clone())
                .unwrap_or_else(|| "operation rejected".into());
            return Err(BffError::BadRequest(msg));
        }
        Ok(ack)
    }
}

#[async_trait]
impl MutationService for ControlPlaneMutationService {
    async fn mutate_vm(
        &self,
        vm_id: String,
        action: String,
        force: bool,
        requested_by: String,
    ) -> Result<MutateVmResponse, BffError> {
        // Look up the VM's node_id from the vms table.
        let node_id =
            sqlx::query_scalar::<_, Option<String>>("SELECT node_id FROM vms WHERE vm_id = $1")
                .bind(&vm_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| BffError::Internal(format!("failed to look up vm: {}", e)))?
                .ok_or_else(|| BffError::NotFound(format!("vm {} not found", vm_id)))?;

        let meta = self.build_meta(node_id.clone(), requested_by);

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

        let ack = self.map_ack(ack)?;
        let result = ack
            .result
            .ok_or_else(|| BffError::Internal("missing ack result".into()))?;

        Ok(MutateVmResponse {
            accepted: result.status == "OK",
            task_id: result.operation_id,
            vm_id,
            summary: result.human_summary,
        })
    }

    async fn mutate_node(
        &self,
        node_id: String,
        action: String,
        requested_by: String,
    ) -> Result<MutateNodeResponse, BffError> {
        let meta = self.build_meta(node_id.clone(), requested_by);

        let ack = match action.as_str() {
            "pause_scheduling" => {
                self.lifecycle_service
                    .pause_node_scheduling(proto::PauseNodeSchedulingRequest {
                        meta,
                        node_id: node_id.clone(),
                    })
                    .await
            }
            "resume_scheduling" => {
                self.lifecycle_service
                    .resume_node_scheduling(proto::ResumeNodeSchedulingRequest {
                        meta,
                        node_id: node_id.clone(),
                    })
                    .await
            }
            "drain" => {
                self.lifecycle_service
                    .drain_node(proto::DrainNodeRequest {
                        meta,
                        node_id: node_id.clone(),
                        allow_workload_stop: false,
                    })
                    .await
            }
            "enter_maintenance" => {
                self.lifecycle_service
                    .enter_maintenance(proto::EnterMaintenanceRequest {
                        meta,
                        node_id: node_id.clone(),
                        reason: "webui initiated".into(),
                    })
                    .await
            }
            "exit_maintenance" => {
                self.lifecycle_service
                    .exit_maintenance(proto::ExitMaintenanceRequest {
                        meta,
                        node_id: node_id.clone(),
                    })
                    .await
            }
            _ => return Err(BffError::BadRequest(format!("invalid action: {}", action))),
        };

        let ack = self.map_ack(ack)?;
        let result = ack
            .result
            .ok_or_else(|| BffError::Internal("missing ack result".into()))?;

        Ok(MutateNodeResponse {
            accepted: result.status == "OK",
            task_id: result.operation_id,
            node_id,
            summary: result.human_summary,
        })
    }

    async fn mutate_volume(
        &self,
        volume_id: String,
        action: String,
        force: bool,
        resize_bytes: Option<u64>,
        requested_by: String,
    ) -> Result<MutateVolumeResponse, BffError> {
        // Look up the volume's node_id from volumes and attachment/size from volume_desired_state/volumes.
        let row = sqlx::query_as::<_, VolumeLookupRow>(
            r#"
            SELECT
                v.node_id as node_id,
                vds.attached_vm_id as vm_id,
                v.capacity_bytes as size_bytes
            FROM volumes v
            JOIN volume_desired_state vds ON v.volume_id = vds.volume_id
            WHERE v.volume_id = $1
            "#,
        )
        .bind(&volume_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to look up volume: {}", e)))?
        .ok_or_else(|| BffError::NotFound(format!("volume {} not found", volume_id)))?;

        let meta = self.build_meta(row.node_id.clone(), requested_by);

        let ack = match action.as_str() {
            "attach" => {
                self.lifecycle_service
                    .attach_volume(proto::AttachVolumeRequest {
                        meta,
                        node_id: row.node_id.clone(),
                        volume: Some(proto::VolumeMutationSpec {
                            volume_id: volume_id.clone(),
                            vm_id: row.vm_id.unwrap_or_default(),
                            volume_spec_json: vec![],
                        }),
                    })
                    .await
            }
            "detach" => {
                self.lifecycle_service
                    .detach_volume(proto::DetachVolumeRequest {
                        meta,
                        node_id: row.node_id.clone(),
                        vm_id: row.vm_id.unwrap_or_default(),
                        volume_id: volume_id.clone(),
                        force,
                    })
                    .await
            }
            "resize" => {
                let new_size = resize_bytes.unwrap_or(row.size_bytes as u64);
                self.lifecycle_service
                    .resize_volume(proto::ResizeVolumeRequest {
                        meta,
                        node_id: row.node_id.clone(),
                        volume_id: volume_id.clone(),
                        new_size_bytes: new_size,
                    })
                    .await
            }
            _ => return Err(BffError::BadRequest(format!("invalid action: {}", action))),
        };

        let ack = self.map_ack(ack)?;
        let result = ack
            .result
            .ok_or_else(|| BffError::Internal("missing ack result".into()))?;

        Ok(MutateVolumeResponse {
            accepted: result.status == "OK",
            task_id: result.operation_id,
            volume_id,
            summary: result.human_summary,
        })
    }

    async fn mutate_network(
        &self,
        _network_id: String,
        _action: String,
        _force: bool,
        _requested_by: String,
    ) -> Result<MutateNetworkResponse, BffError> {
        Err(BffError::NotImplemented)
    }
}
