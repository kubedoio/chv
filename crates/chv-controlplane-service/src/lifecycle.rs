use crate::error::ControlPlaneServiceError;
use async_trait::async_trait;
use chv_controlplane_store::{
    DesiredStateRepository, EventAppendInput, EventRepository, NetworkStatusPatchInput,
    NodeDrainIntentInput, NodeRepository, NodeSchedulingPatchInput, NodeStatePatchInput,
    OperationCreateInput, OperationRepository, OperationStatusUpdateInput, VmDesiredStateInput,
    VmPowerStatePatchInput, VolumeAttachmentPatchInput, VolumeResizePatchInput,
};
use chv_controlplane_types::domain::{
    EventSeverity, EventType, Generation, NodeId, NodeState, OperationId, OperationStatus,
    ResourceId, ResourceKind,
};
use chv_controlplane_types::fragment::VmSpec;
use control_plane_node_api::control_plane_node_api as proto;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[async_trait]
pub trait LifecycleService: Send + Sync {
    async fn create_vm(
        &self,
        request: proto::CreateVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn start_vm(
        &self,
        request: proto::StartVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn stop_vm(
        &self,
        request: proto::StopVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn reboot_vm(
        &self,
        request: proto::RebootVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn delete_vm(
        &self,
        request: proto::DeleteVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn resize_vm(
        &self,
        request: proto::ResizeVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn attach_volume(
        &self,
        request: proto::AttachVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn detach_volume(
        &self,
        request: proto::DetachVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn resize_volume(
        &self,
        request: proto::ResizeVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn snapshot_volume(
        &self,
        request: proto::SnapshotVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn restore_volume(
        &self,
        request: proto::RestoreVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn delete_volume_snapshot(
        &self,
        request: proto::DeleteVolumeSnapshotRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn clone_volume(
        &self,
        request: proto::CloneVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn pause_node_scheduling(
        &self,
        request: proto::PauseNodeSchedulingRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn resume_node_scheduling(
        &self,
        request: proto::ResumeNodeSchedulingRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn drain_node(
        &self,
        request: proto::DrainNodeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn enter_maintenance(
        &self,
        request: proto::EnterMaintenanceRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn exit_maintenance(
        &self,
        request: proto::ExitMaintenanceRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn pause_vm(
        &self,
        request: proto::PauseVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn resume_vm(
        &self,
        request: proto::ResumeVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn power_button_vm(
        &self,
        request: proto::PowerButtonVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn add_disk(
        &self,
        request: proto::AddDiskRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn remove_device(
        &self,
        request: proto::RemoveDeviceRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn add_net(
        &self,
        request: proto::AddNetRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn resize_disk(
        &self,
        request: proto::ResizeDiskRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn snapshot_vm(
        &self,
        request: proto::SnapshotVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn restore_snapshot(
        &self,
        request: proto::RestoreSnapshotRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn coredump_vm(
        &self,
        request: proto::CoredumpVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn start_network(
        &self,
        request: proto::StartNetworkRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn stop_network(
        &self,
        request: proto::StopNetworkRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;

    async fn restart_network(
        &self,
        request: proto::RestartNetworkRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;
}

#[derive(Clone)]
pub struct LifecycleServiceImplementation {
    node_repo: NodeRepository,
    operation_repo: OperationRepository,
    event_repo: EventRepository,
    desired_state_repo: DesiredStateRepository,
}

impl LifecycleServiceImplementation {
    pub fn new(
        node_repo: NodeRepository,
        operation_repo: OperationRepository,
        event_repo: EventRepository,
        desired_state_repo: DesiredStateRepository,
    ) -> Self {
        Self {
            node_repo,
            operation_repo,
            event_repo,
            desired_state_repo,
        }
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    fn meta_from_request(
        &self,
        meta: Option<proto::RequestMeta>,
    ) -> Result<proto::RequestMeta, ControlPlaneServiceError> {
        meta.ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing meta".into()))
    }

    fn normalize_requested_by(meta: &proto::RequestMeta) -> Option<String> {
        if meta.requested_by.is_empty() {
            None
        } else {
            Some(meta.requested_by.clone())
        }
    }

    fn desired_generation_from_meta(
        meta: &proto::RequestMeta,
    ) -> Result<Generation, ControlPlaneServiceError> {
        if meta.desired_state_version.is_empty() {
            return Err(ControlPlaneServiceError::InvalidArgument(
                "desired_state_version is required".into(),
            ));
        }
        Generation::from_str(&meta.desired_state_version).map_err(|_| {
            ControlPlaneServiceError::InvalidArgument("generation must be numeric".into())
        })
    }

    fn ok_ack(operation_id: &OperationId, summary: &str) -> proto::AckResponse {
        proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: operation_id.to_string(),
                status: "OK".into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: summary.into(),
            }),
        }
    }

    fn parse_node_id(s: String) -> Result<NodeId, ControlPlaneServiceError> {
        NodeId::new(s).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })
    }

    fn parse_vm_id(s: String) -> Result<ResourceId, ControlPlaneServiceError> {
        ResourceId::new(s)
            .map_err(|e| ControlPlaneServiceError::InvalidArgument(format!("invalid vm_id: {}", e)))
    }

    fn parse_volume_id(s: String) -> Result<ResourceId, ControlPlaneServiceError> {
        ResourceId::new(s).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid volume_id: {}", e))
        })
    }

    fn parse_network_id(s: String) -> Result<ResourceId, ControlPlaneServiceError> {
        ResourceId::new(s).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid network_id: {}", e))
        })
    }

    fn resource_id_from_node_id(node_id: &NodeId) -> Result<ResourceId, ControlPlaneServiceError> {
        ResourceId::new(node_id.as_str())
            .map_err(|e| ControlPlaneServiceError::Internal(format!("invalid resource_id: {}", e)))
    }

    async fn require_node_exists(&self, node_id: &NodeId) -> Result<(), ControlPlaneServiceError> {
        let row = sqlx::query("SELECT 1 FROM nodes WHERE node_id = $1")
            .bind(node_id.as_str())
            .fetch_optional(self.node_repo.pool())
            .await
            .map_err(|e| {
                ControlPlaneServiceError::Internal(format!("failed to check node: {}", e))
            })?;
        if row.is_none() {
            return Err(ControlPlaneServiceError::NotFound(format!(
                "node {} not found",
                node_id
            )));
        }
        Ok(())
    }

    async fn create_operation_and_emit(
        &self,
        operation_type: &str,
        node_id: NodeId,
        resource_kind: ResourceKind,
        resource_id: Option<ResourceId>,
        meta: &proto::RequestMeta,
        idempotency_discriminator: Option<String>,
    ) -> Result<OperationId, ControlPlaneServiceError> {
        self.require_node_exists(&node_id).await?;
        let now = Self::now_ms();
        let desired_generation = Self::desired_generation_from_meta(meta)?;
        let desired_generation_str = desired_generation.to_string();
        let resource_id_str = resource_id
            .as_ref()
            .map(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        let idempotency_key = if meta.operation_id.trim().is_empty() {
            match idempotency_discriminator {
                Some(discriminator) => format!(
                    "{}:{}:{}:{}:{}",
                    operation_type, node_id, resource_id_str, desired_generation_str, discriminator
                ),
                None => format!(
                    "{}:{}:{}:{}",
                    operation_type, node_id, resource_id_str, desired_generation_str
                ),
            }
        } else {
            format!("request:{}", meta.operation_id.trim())
        };

        let operation_id =
            OperationId::new(format!("{}-{}", operation_type, chv_common::gen_short_id()))
                .map_err(|e| {
                    ControlPlaneServiceError::Internal(format!("invalid operation_id: {}", e))
                })?;

        let receipt = self
            .operation_repo
            .create_or_get(&OperationCreateInput {
                operation_id,
                idempotency_key,
                resource_kind,
                resource_id: resource_id.clone(),
                operation_type: operation_type.into(),
                status: OperationStatus::Pending,
                requested_by: Self::normalize_requested_by(meta),
                updated_by: None,
                desired_generation: Some(desired_generation),
                observed_generation: None,
                correlation_id: None,
                requested_unix_ms: now,
            })
            .await?;

        self.event_repo
            .append(&EventAppendInput {
                occurred_unix_ms: now,
                event_type: EventType::OperationStarted,
                severity: EventSeverity::Info,
                resource_kind: Some(resource_kind),
                resource_id,
                node_id: Some(node_id),
                operation_id: Some(receipt.operation_id.clone()),
                actor_id: None,
                requested_by: Self::normalize_requested_by(meta),
                correlation_id: None,
                message: format!("{} started", operation_type),
                details: None,
            })
            .await?;

        Ok(receipt.operation_id)
    }

    async fn accept_operation(
        &self,
        operation_id: &OperationId,
    ) -> Result<(), ControlPlaneServiceError> {
        self.operation_repo
            .update_status(&OperationStatusUpdateInput {
                operation_id: operation_id.clone(),
                status: OperationStatus::Accepted,
                error_code: None,
                error_message: None,
                observed_generation: None,
                updated_by: None,
                updated_unix_ms: Self::now_ms(),
            })
            .await
            .map_err(|e| {
                ControlPlaneServiceError::Internal(format!(
                    "failed to update operation status: {}",
                    e
                ))
            })
    }

    async fn fail_operation(
        &self,
        operation_id: &OperationId,
        original_error: &ControlPlaneServiceError,
    ) -> Result<(), ControlPlaneServiceError> {
        self.operation_repo
            .update_status(&OperationStatusUpdateInput {
                operation_id: operation_id.clone(),
                status: OperationStatus::Failed,
                error_code: Some("INTENT_PERSISTENCE_FAILED".into()),
                error_message: Some(original_error.to_string()),
                observed_generation: None,
                updated_by: None,
                updated_unix_ms: Self::now_ms(),
            })
            .await
            .map_err(|e| {
                ControlPlaneServiceError::Internal(format!(
                    "failed to update operation status: {}",
                    e
                ))
            })?;

        self.event_repo
            .append(&EventAppendInput {
                occurred_unix_ms: Self::now_ms(),
                event_type: EventType::OperationFailed,
                severity: EventSeverity::Error,
                resource_kind: None,
                resource_id: None,
                node_id: None,
                operation_id: Some(operation_id.clone()),
                actor_id: None,
                requested_by: None,
                correlation_id: None,
                message: format!("operation failed: {}", original_error),
                details: None,
            })
            .await?;

        Ok(())
    }

    async fn persist_intent_and_accept<F, Fut, E>(
        &self,
        operation_id: &OperationId,
        persist: F,
    ) -> Result<(), ControlPlaneServiceError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), E>>,
        E: Into<ControlPlaneServiceError>,
    {
        match persist().await {
            Ok(()) => self.accept_operation(operation_id).await,
            Err(e) => {
                let err = e.into();
                self.fail_operation(operation_id, &err).await?;
                Err(err)
            }
        }
    }
}

#[async_trait]
impl LifecycleService for LifecycleServiceImplementation {
    async fn create_vm(
        &self,
        request: proto::CreateVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm = request
            .vm
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing vm".into()))?;
        let vm_id = Self::parse_vm_id(vm.vm_id)?;

        let spec: VmSpec = if vm.vm_spec_json.is_empty() {
            VmSpec {
                cpu_count: None,
                memory_bytes: None,
                image_ref: None,
                boot_mode: None,
                desired_power_state: None,
            }
        } else {
            serde_json::from_slice(&vm.vm_spec_json).map_err(|e| {
                ControlPlaneServiceError::InvalidArgument(format!("invalid vm_spec_json: {}", e))
            })?
        };

        let operation_id = self
            .create_operation_and_emit(
                "CreateVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(String::from_utf8_lossy(&vm.vm_spec_json).into_owned()),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .upsert_vm(&VmDesiredStateInput {
                    vm_id: vm_id.clone(),
                    node_id: Some(node_id.clone()),
                    display_name: vm_id.as_str().into(),
                    tenant_id: None,
                    placement_policy: None,
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    cpu_count: spec.cpu_count,
                    memory_bytes: spec.memory_bytes,
                    image_ref: spec.image_ref,
                    boot_mode: spec.boot_mode,
                    desired_power_state: Some("Created".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "create vm accepted"))
    }

    async fn start_vm(
        &self,
        request: proto::StartVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "StartVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Running".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "start vm accepted"))
    }

    async fn stop_vm(
        &self,
        request: proto::StopVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_type = if request.force {
            "ForceStopVm"
        } else {
            "StopVm"
        };
        let operation_id = self
            .create_operation_and_emit(
                operation_type,
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("force={}", request.force)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Stopped".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "stop vm accepted"))
    }

    async fn reboot_vm(
        &self,
        request: proto::RebootVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "RebootVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("force={}", request.force)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Rebooting".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "reboot vm accepted"))
    }

    async fn delete_vm(
        &self,
        request: proto::DeleteVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "DeleteVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("force={}", request.force)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Deleted".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "delete vm accepted"))
    }

    async fn resize_vm(
        &self,
        request: proto::ResizeVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "ResizeVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!(
                    "vcpus={:?},memory_bytes={:?}",
                    request.desired_vcpus, request.desired_memory_bytes
                )),
            )
            .await?;

        // Record intent accepted; actual resize is orchestrated by the reconciler
        // dispatching to the agent's resize_vm RPC.
        let _ = (&node_id, &vm_id);
        Ok(Self::ok_ack(&operation_id, "resize vm accepted"))
    }

    async fn attach_volume(
        &self,
        request: proto::AttachVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let volume = request
            .volume
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing volume".into()))?;
        let volume_id = Self::parse_volume_id(volume.volume_id)?;
        let vm_id = Self::parse_vm_id(volume.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "AttachVolume",
                node_id.clone(),
                ResourceKind::Volume,
                Some(volume_id.clone()),
                &meta,
                Some(format!("vm={}", vm_id)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_attachment(&VolumeAttachmentPatchInput {
                    volume_id: volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    attached_vm_id: Some(vm_id),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "attach volume accepted"))
    }

    async fn detach_volume(
        &self,
        request: proto::DetachVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let attached_vm_id = Self::parse_vm_id(request.vm_id)?;
        let volume_id = Self::parse_volume_id(request.volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "DetachVolume",
                node_id.clone(),
                ResourceKind::Volume,
                Some(volume_id.clone()),
                &meta,
                Some(format!("vm={}:force={}", attached_vm_id, request.force)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_attachment(&VolumeAttachmentPatchInput {
                    volume_id: volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    attached_vm_id: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "detach volume accepted"))
    }

    async fn resize_volume(
        &self,
        request: proto::ResizeVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let volume_id = Self::parse_volume_id(request.volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "ResizeVolume",
                node_id.clone(),
                ResourceKind::Volume,
                Some(volume_id.clone()),
                &meta,
                Some(format!("size={}", request.new_size_bytes)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_resize(&VolumeResizePatchInput {
                    volume_id: volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    resize_to_bytes: Some(request.new_size_bytes as i64),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "resize volume accepted"))
    }

    async fn snapshot_volume(
        &self,
        request: proto::SnapshotVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let volume_id = Self::parse_volume_id(request.volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "SnapshotVolume",
                node_id.clone(),
                ResourceKind::Volume,
                Some(volume_id.clone()),
                &meta,
                Some(format!("snapshot_name={}", request.snapshot_name)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_snapshot(&chv_controlplane_store::VolumeSnapshotPatchInput {
                    volume_id: volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    snapshot_op: Some("create".to_string()),
                    snapshot_name: Some(request.snapshot_name),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "snapshot volume accepted"))
    }

    async fn restore_volume(
        &self,
        request: proto::RestoreVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let volume_id = Self::parse_volume_id(request.volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "RestoreVolume",
                node_id.clone(),
                ResourceKind::Volume,
                Some(volume_id.clone()),
                &meta,
                Some(format!("snapshot_name={}", request.snapshot_name)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_snapshot(&chv_controlplane_store::VolumeSnapshotPatchInput {
                    volume_id: volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    snapshot_op: Some("restore".to_string()),
                    snapshot_name: Some(request.snapshot_name),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "restore volume accepted"))
    }

    async fn delete_volume_snapshot(
        &self,
        request: proto::DeleteVolumeSnapshotRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let volume_id = Self::parse_volume_id(request.volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "DeleteVolumeSnapshot",
                node_id.clone(),
                ResourceKind::Volume,
                Some(volume_id.clone()),
                &meta,
                Some(format!("snapshot_name={}", request.snapshot_name)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_snapshot(&chv_controlplane_store::VolumeSnapshotPatchInput {
                    volume_id: volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    snapshot_op: Some("delete".to_string()),
                    snapshot_name: Some(request.snapshot_name),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(
            &operation_id,
            "delete volume snapshot accepted",
        ))
    }

    async fn clone_volume(
        &self,
        request: proto::CloneVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let source_volume_id = Self::parse_volume_id(request.source_volume_id)?;
        let target_volume_id = Self::parse_volume_id(request.target_volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "CloneVolume",
                node_id.clone(),
                ResourceKind::Volume,
                Some(target_volume_id.clone()),
                &meta,
                Some(format!("source={}", source_volume_id)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_volume_clone(&chv_controlplane_store::VolumeClonePatchInput {
                    volume_id: target_volume_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    clone_source_volume_id: Some(source_volume_id.clone()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "clone volume accepted"))
    }

    async fn pause_node_scheduling(
        &self,
        request: proto::PauseNodeSchedulingRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = Self::resource_id_from_node_id(&node_id)?;
        let operation_id = self
            .create_operation_and_emit(
                "PauseNodeScheduling",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
                Some("paused=true".into()),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.node_repo
                .set_scheduling_paused(&NodeSchedulingPatchInput {
                    node_id: node_id.clone(),
                    desired_generation,
                    scheduling_paused: true,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(
            &operation_id,
            "pause node scheduling accepted",
        ))
    }

    async fn resume_node_scheduling(
        &self,
        request: proto::ResumeNodeSchedulingRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = Self::resource_id_from_node_id(&node_id)?;
        let operation_id = self
            .create_operation_and_emit(
                "ResumeNodeScheduling",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
                Some("paused=false".into()),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.node_repo
                .set_scheduling_paused(&NodeSchedulingPatchInput {
                    node_id: node_id.clone(),
                    desired_generation,
                    scheduling_paused: false,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(
            &operation_id,
            "resume node scheduling accepted",
        ))
    }

    async fn drain_node(
        &self,
        request: proto::DrainNodeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = Self::resource_id_from_node_id(&node_id)?;
        let operation_id = self
            .create_operation_and_emit(
                "DrainNode",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
                Some(format!(
                    "allow_workload_stop={}",
                    request.allow_workload_stop
                )),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.node_repo
                .set_drain_intent(&NodeDrainIntentInput {
                    node_id: node_id.clone(),
                    desired_generation,
                    allow_workload_stop: Some(request.allow_workload_stop),
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "drain node accepted"))
    }

    async fn enter_maintenance(
        &self,
        request: proto::EnterMaintenanceRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = Self::resource_id_from_node_id(&node_id)?;
        let operation_id = self
            .create_operation_and_emit(
                "EnterMaintenance",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
                Some(request.reason.clone()),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.node_repo
                .set_state_preserving_policy(&NodeStatePatchInput {
                    node_id: node_id.clone(),
                    desired_state: NodeState::Maintenance,
                    desired_generation,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    state_reason: Some(request.reason),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "enter maintenance accepted"))
    }

    async fn exit_maintenance(
        &self,
        request: proto::ExitMaintenanceRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = Self::resource_id_from_node_id(&node_id)?;
        let operation_id = self
            .create_operation_and_emit(
                "ExitMaintenance",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.node_repo
                .set_state_preserving_policy(&NodeStatePatchInput {
                    node_id: node_id.clone(),
                    desired_state: NodeState::TenantReady,
                    desired_generation,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    state_reason: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "exit maintenance accepted"))
    }

    async fn pause_vm(
        &self,
        request: proto::PauseVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "PauseVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Paused".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "pause vm accepted"))
    }

    async fn resume_vm(
        &self,
        request: proto::ResumeVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "ResumeVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Running".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "resume vm accepted"))
    }

    async fn power_button_vm(
        &self,
        request: proto::PowerButtonVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "PowerButtonVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_vm_power_state(&VmPowerStatePatchInput {
                    vm_id: vm_id.clone(),
                    desired_generation,
                    desired_status: None,
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    target_node_id: Some(node_id.clone()),
                    desired_power_state: Some("Stopped".into()),
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "power button vm accepted"))
    }

    async fn add_disk(
        &self,
        request: proto::AddDiskRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "AddDisk",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("disk_id={}", request.disk_id)),
            )
            .await?;

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "add disk accepted"))
    }

    async fn remove_device(
        &self,
        request: proto::RemoveDeviceRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "RemoveDevice",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("device_id={}", request.device_id)),
            )
            .await?;

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "remove device accepted"))
    }

    async fn add_net(
        &self,
        request: proto::AddNetRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "AddNet",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("net_id={}", request.net_id)),
            )
            .await?;

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "add net accepted"))
    }

    async fn resize_disk(
        &self,
        request: proto::ResizeDiskRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "ResizeDisk",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!(
                    "disk_id={}:size={}",
                    request.disk_id, request.new_size_bytes
                )),
            )
            .await?;

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "resize disk accepted"))
    }

    async fn snapshot_vm(
        &self,
        request: proto::SnapshotVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "SnapshotVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("destination={}", request.destination)),
            )
            .await?;

        if !request.destination.is_empty() {
            let _ = sqlx::query("UPDATE operations SET correlation_id = ? WHERE operation_id = ?")
                .bind(&request.destination)
                .bind(operation_id.as_str())
                .execute(self.operation_repo.pool())
                .await;
        }

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "snapshot vm accepted"))
    }

    async fn restore_snapshot(
        &self,
        request: proto::RestoreSnapshotRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "RestoreSnapshot",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("source={}", request.source)),
            )
            .await?;

        if !request.source.is_empty() {
            let _ = sqlx::query("UPDATE operations SET correlation_id = ? WHERE operation_id = ?")
                .bind(&request.source)
                .bind(operation_id.as_str())
                .execute(self.operation_repo.pool())
                .await;
        }

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "restore snapshot accepted"))
    }

    async fn coredump_vm(
        &self,
        request: proto::CoredumpVmRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let vm_id = Self::parse_vm_id(request.vm_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "CoredumpVm",
                node_id.clone(),
                ResourceKind::Vm,
                Some(vm_id.clone()),
                &meta,
                Some(format!("destination={}", request.destination)),
            )
            .await?;

        self.accept_operation(&operation_id).await?;

        Ok(Self::ok_ack(&operation_id, "coredump vm accepted"))
    }

    async fn start_network(
        &self,
        request: proto::StartNetworkRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let network_id = Self::parse_network_id(request.network_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "StartNetwork",
                node_id.clone(),
                ResourceKind::Network,
                Some(network_id.clone()),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_network_status(&NetworkStatusPatchInput {
                    network_id: network_id.clone(),
                    desired_generation,
                    desired_status: Some("Active".into()),
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "start network accepted"))
    }

    async fn stop_network(
        &self,
        request: proto::StopNetworkRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let network_id = Self::parse_network_id(request.network_id)?;

        let operation_type = if request.force {
            "ForceStopNetwork"
        } else {
            "StopNetwork"
        };
        let operation_id = self
            .create_operation_and_emit(
                operation_type,
                node_id.clone(),
                ResourceKind::Network,
                Some(network_id.clone()),
                &meta,
                Some(format!("force={}", request.force)),
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_network_status(&NetworkStatusPatchInput {
                    network_id: network_id.clone(),
                    desired_generation,
                    desired_status: Some("Inactive".into()),
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "stop network accepted"))
    }

    async fn restart_network(
        &self,
        request: proto::RestartNetworkRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let network_id = Self::parse_network_id(request.network_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "RestartNetwork",
                node_id.clone(),
                ResourceKind::Network,
                Some(network_id.clone()),
                &meta,
                None,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.persist_intent_and_accept(&operation_id, || async {
            self.desired_state_repo
                .set_network_status(&NetworkStatusPatchInput {
                    network_id: network_id.clone(),
                    desired_generation,
                    desired_status: Some("Restarting".into()),
                    requested_by: Self::normalize_requested_by(&meta),
                    updated_by: None,
                    requested_unix_ms: Self::now_ms(),
                })
                .await
        })
        .await?;

        Ok(Self::ok_ack(&operation_id, "restart network accepted"))
    }
}
