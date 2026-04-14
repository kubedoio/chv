use crate::error::ControlPlaneServiceError;
use async_trait::async_trait;
use chv_controlplane_store::{
    DesiredStateRepository, EventAppendInput, EventRepository, NodeRepository, NodeStateInput,
    OperationCreateInput, OperationRepository, VmDesiredStateInput,
};
use chv_controlplane_types::constants::STATUS_OK;
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
            Ok(Generation::new(1))
        } else {
            Generation::from_str(&meta.desired_state_version).map_err(|_| {
                ControlPlaneServiceError::InvalidArgument("generation must be numeric".into())
            })
        }
    }

    fn ok_ack(operation_id: &OperationId, summary: &str) -> proto::AckResponse {
        proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: operation_id.to_string(),
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: summary.into(),
            }),
        }
    }

    fn parse_node_id(s: String) -> Result<NodeId, ControlPlaneServiceError> {
        NodeId::new(s)
            .map_err(|e| ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e)))
    }

    fn parse_vm_id(s: String) -> Result<ResourceId, ControlPlaneServiceError> {
        ResourceId::new(s)
            .map_err(|e| ControlPlaneServiceError::InvalidArgument(format!("invalid vm_id: {}", e)))
    }

    fn parse_volume_id(s: String) -> Result<ResourceId, ControlPlaneServiceError> {
        ResourceId::new(s)
            .map_err(|e| ControlPlaneServiceError::InvalidArgument(format!("invalid volume_id: {}", e)))
    }

    async fn require_node_exists(&self, node_id: &NodeId) -> Result<(), ControlPlaneServiceError> {
        let row = sqlx::query("SELECT 1 FROM nodes WHERE node_id = $1")
            .bind(node_id.as_str())
            .fetch_optional(self.node_repo.pool())
            .await
            .map_err(|e| ControlPlaneServiceError::Internal(format!("failed to check node: {}", e)))?;
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
    ) -> Result<OperationId, ControlPlaneServiceError> {
        self.require_node_exists(&node_id).await?;
        let now = Self::now_ms();
        let desired_generation_str = if meta.desired_state_version.is_empty() {
            "0".to_string()
        } else {
            meta.desired_state_version.clone()
        };
        let resource_id_str = resource_id.as_ref().map(|r| r.as_str()).unwrap_or("").to_string();
        let idempotency_key = format!(
            "{}:{}:{}:{}",
            operation_type, node_id, resource_id_str, desired_generation_str
        );

        let operation_id =
            OperationId::new(format!("{}-{}", operation_type, uuid::Uuid::new_v4())).map_err(
                |e| ControlPlaneServiceError::Internal(format!("invalid operation_id: {}", e)),
            )?;

        let desired_generation = Self::desired_generation_from_meta(meta)?;

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
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.desired_state_repo
            .upsert_vm(&VmDesiredStateInput {
                vm_id: vm_id.clone(),
                node_id: Some(node_id.clone()),
                display_name: vm_id.as_str().into(),
                tenant_id: None,
                placement_policy: None,
                desired_generation,
                desired_status: STATUS_OK.into(),
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
            .create_operation_and_emit("StartVm", node_id, ResourceKind::Vm, Some(vm_id), &meta)
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

        let operation_id = self
            .create_operation_and_emit("StopVm", node_id, ResourceKind::Vm, Some(vm_id), &meta)
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
            .create_operation_and_emit("RebootVm", node_id, ResourceKind::Vm, Some(vm_id), &meta)
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
            .create_operation_and_emit("DeleteVm", node_id, ResourceKind::Vm, Some(vm_id), &meta)
            .await?;

        Ok(Self::ok_ack(&operation_id, "delete vm accepted"))
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

        let operation_id = self
            .create_operation_and_emit(
                "AttachVolume",
                node_id,
                ResourceKind::Volume,
                Some(volume_id),
                &meta,
            )
            .await?;

        Ok(Self::ok_ack(&operation_id, "attach volume accepted"))
    }

    async fn detach_volume(
        &self,
        request: proto::DetachVolumeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;
        let volume_id = Self::parse_volume_id(request.volume_id)?;

        let operation_id = self
            .create_operation_and_emit(
                "DetachVolume",
                node_id,
                ResourceKind::Volume,
                Some(volume_id),
                &meta,
            )
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
                node_id,
                ResourceKind::Volume,
                Some(volume_id),
                &meta,
            )
            .await?;

        Ok(Self::ok_ack(&operation_id, "resize volume accepted"))
    }

    async fn pause_node_scheduling(
        &self,
        request: proto::PauseNodeSchedulingRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = ResourceId::new(node_id.as_str()).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("invalid resource_id: {}", e))
        })?;
        let operation_id = self
            .create_operation_and_emit(
                "PauseNodeScheduling",
                node_id,
                ResourceKind::Node,
                Some(resource_id),
                &meta,
            )
            .await?;

        Ok(Self::ok_ack(&operation_id, "pause node scheduling accepted"))
    }

    async fn resume_node_scheduling(
        &self,
        request: proto::ResumeNodeSchedulingRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = ResourceId::new(node_id.as_str()).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("invalid resource_id: {}", e))
        })?;
        let operation_id = self
            .create_operation_and_emit(
                "ResumeNodeScheduling",
                node_id,
                ResourceKind::Node,
                Some(resource_id),
                &meta,
            )
            .await?;

        Ok(Self::ok_ack(&operation_id, "resume node scheduling accepted"))
    }

    async fn drain_node(
        &self,
        request: proto::DrainNodeRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let meta = self.meta_from_request(request.meta)?;
        let node_id = Self::parse_node_id(request.node_id)?;

        let resource_id = ResourceId::new(node_id.as_str()).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("invalid resource_id: {}", e))
        })?;
        let operation_id = self
            .create_operation_and_emit(
                "DrainNode",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.node_repo
            .upsert_state(&NodeStateInput {
                node_id: node_id.clone(),
                desired_state: NodeState::Draining,
                desired_generation,
                requested_by: Self::normalize_requested_by(&meta),
                updated_by: None,
                state_reason: None,
                requested_unix_ms: Self::now_ms(),
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

        let resource_id = ResourceId::new(node_id.as_str()).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("invalid resource_id: {}", e))
        })?;
        let operation_id = self
            .create_operation_and_emit(
                "EnterMaintenance",
                node_id.clone(),
                ResourceKind::Node,
                Some(resource_id),
                &meta,
            )
            .await?;

        let desired_generation = Self::desired_generation_from_meta(&meta)?;

        self.node_repo
            .upsert_state(&NodeStateInput {
                node_id: node_id.clone(),
                desired_state: NodeState::Maintenance,
                desired_generation,
                requested_by: Self::normalize_requested_by(&meta),
                updated_by: None,
                state_reason: Some(request.reason),
                requested_unix_ms: Self::now_ms(),
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

        let resource_id = ResourceId::new(node_id.as_str()).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("invalid resource_id: {}", e))
        })?;
        let operation_id = self
            .create_operation_and_emit(
                "ExitMaintenance",
                node_id,
                ResourceKind::Node,
                Some(resource_id),
                &meta,
            )
            .await?;

        Ok(Self::ok_ack(&operation_id, "exit maintenance accepted"))
    }
}
