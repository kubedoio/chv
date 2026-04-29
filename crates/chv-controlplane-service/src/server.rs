use crate::enrollment::EnrollmentService;
use crate::inventory::InventoryService;
use crate::lifecycle::LifecycleService;
use crate::reconcile::ReconcileService;
use crate::telemetry::TelemetryService;
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::Instrument;



fn extract_op_id<T>(request: &Request<T>) -> Option<String> {
    request
        .metadata()
        .get(chv_common::OPERATION_ID_METADATA_KEY)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub struct EnrollmentServer {
    service: Arc<dyn EnrollmentService>,
}

impl EnrollmentServer {
    pub fn new(service: Arc<dyn EnrollmentService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl proto::enrollment_service_server::EnrollmentService for EnrollmentServer {
    async fn enroll_node(
        &self,
        request: Request<proto::EnrollmentRequest>,
    ) -> Result<Response<proto::EnrollmentResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("enroll_node", %op_id);
        let resp = self
            .service
            .enroll_node(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn rotate_node_certificate(
        &self,
        request: Request<proto::RotateNodeCertificateRequest>,
    ) -> Result<Response<proto::RotateNodeCertificateResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("rotate_node_certificate", %op_id);
        let resp = self
            .service
            .rotate_node_certificate(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn report_bootstrap_result(
        &self,
        request: Request<proto::ReportBootstrapResultRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_bootstrap_result", %op_id);
        let resp = self
            .service
            .report_bootstrap_result(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }
}

pub struct InventoryServer {
    service: Arc<dyn InventoryService>,
}

impl InventoryServer {
    pub fn new(service: Arc<dyn InventoryService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl proto::inventory_service_server::InventoryService for InventoryServer {
    async fn report_node_inventory(
        &self,
        request: Request<proto::ReportNodeInventoryRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_node_inventory", %op_id);
        let resp = self
            .service
            .report_node_inventory(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "report_node_inventory failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }

    async fn report_service_versions(
        &self,
        request: Request<proto::ReportServiceVersionsRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_service_versions", %op_id);
        let resp = self
            .service
            .report_service_versions(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "report_service_versions failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }
}

pub struct TelemetryServer {
    service: Arc<dyn TelemetryService>,
}

impl TelemetryServer {
    pub fn new(service: Arc<dyn TelemetryService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl proto::telemetry_service_server::TelemetryService for TelemetryServer {
    async fn report_node_state(
        &self,
        request: Request<proto::NodeStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_node_state", %op_id);
        let resp = self
            .service
            .report_node_state(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "report_node_state failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }

    async fn report_vm_state(
        &self,
        request: Request<proto::VmStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_vm_state", %op_id);
        let resp = self
            .service
            .report_vm_state(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "report_vm_state failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }

    async fn report_volume_state(
        &self,
        request: Request<proto::VolumeStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_volume_state", %op_id);
        let resp = self
            .service
            .report_volume_state(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "report_volume_state failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }

    async fn report_network_state(
        &self,
        request: Request<proto::NetworkStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("report_network_state", %op_id);
        let resp = self
            .service
            .report_network_state(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "report_network_state failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }

    async fn publish_event(
        &self,
        request: Request<proto::PublishEventRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("publish_event", %op_id);
        let resp = self
            .service
            .publish_event(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "publish_event failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }

    async fn publish_alert(
        &self,
        request: Request<proto::PublishAlertRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("publish_alert", %op_id);
        let resp = self
            .service
            .publish_alert(request.into_inner())
            .instrument(_span).await
            .map_err(|e| {
                tracing::warn!(error = %e, "publish_alert failed");
                tonic::Status::from(e)
            })?;
        Ok(Response::new(resp))
    }
}

pub struct LifecycleServer {
    service: Arc<dyn LifecycleService>,
}

impl LifecycleServer {
    pub fn new(service: Arc<dyn LifecycleService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl proto::lifecycle_service_server::LifecycleService for LifecycleServer {
    async fn create_vm(
        &self,
        request: Request<proto::CreateVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("create_vm", %op_id);
        let resp = self
            .service
            .create_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn start_vm(
        &self,
        request: Request<proto::StartVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("start_vm", %op_id);
        let resp = self
            .service
            .start_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn stop_vm(
        &self,
        request: Request<proto::StopVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("stop_vm", %op_id);
        let resp = self
            .service
            .stop_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn reboot_vm(
        &self,
        request: Request<proto::RebootVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("reboot_vm", %op_id);
        let resp = self
            .service
            .reboot_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn delete_vm(
        &self,
        request: Request<proto::DeleteVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("delete_vm", %op_id);
        let resp = self
            .service
            .delete_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn resize_vm(
        &self,
        request: Request<proto::ResizeVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("resize_vm", %op_id);
        let resp = self
            .service
            .resize_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn attach_volume(
        &self,
        request: Request<proto::AttachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("attach_volume", %op_id);
        let resp = self
            .service
            .attach_volume(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn detach_volume(
        &self,
        request: Request<proto::DetachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("detach_volume", %op_id);
        let resp = self
            .service
            .detach_volume(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn resize_volume(
        &self,
        request: Request<proto::ResizeVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("resize_volume", %op_id);
        let resp = self
            .service
            .resize_volume(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn snapshot_volume(
        &self,
        request: Request<proto::SnapshotVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("snapshot_volume", %op_id);
        let resp = self
            .service
            .snapshot_volume(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn restore_volume(
        &self,
        request: Request<proto::RestoreVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("restore_volume", %op_id);
        let resp = self
            .service
            .restore_volume(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn delete_volume_snapshot(
        &self,
        request: Request<proto::DeleteVolumeSnapshotRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("delete_volume_snapshot", %op_id);
        let resp = self
            .service
            .delete_volume_snapshot(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn clone_volume(
        &self,
        request: Request<proto::CloneVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("clone_volume", %op_id);
        let resp = self
            .service
            .clone_volume(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn pause_node_scheduling(
        &self,
        request: Request<proto::PauseNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("pause_node_scheduling", %op_id);
        let resp = self
            .service
            .pause_node_scheduling(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn resume_node_scheduling(
        &self,
        request: Request<proto::ResumeNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("resume_node_scheduling", %op_id);
        let resp = self
            .service
            .resume_node_scheduling(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn drain_node(
        &self,
        request: Request<proto::DrainNodeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("drain_node", %op_id);
        let resp = self
            .service
            .drain_node(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn enter_maintenance(
        &self,
        request: Request<proto::EnterMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("enter_maintenance", %op_id);
        let resp = self
            .service
            .enter_maintenance(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn exit_maintenance(
        &self,
        request: Request<proto::ExitMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("exit_maintenance", %op_id);
        let resp = self
            .service
            .exit_maintenance(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn pause_vm(
        &self,
        request: Request<proto::PauseVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("pause_vm", %op_id);
        let resp = self
            .service
            .pause_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn resume_vm(
        &self,
        request: Request<proto::ResumeVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("resume_vm", %op_id);
        let resp = self
            .service
            .resume_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn power_button_vm(
        &self,
        request: Request<proto::PowerButtonVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("power_button_vm", %op_id);
        let resp = self
            .service
            .power_button_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn add_disk(
        &self,
        request: Request<proto::AddDiskRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("add_disk", %op_id);
        let resp = self
            .service
            .add_disk(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn remove_device(
        &self,
        request: Request<proto::RemoveDeviceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("remove_device", %op_id);
        let resp = self
            .service
            .remove_device(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn add_net(
        &self,
        request: Request<proto::AddNetRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("add_net", %op_id);
        let resp = self
            .service
            .add_net(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn resize_disk(
        &self,
        request: Request<proto::ResizeDiskRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("resize_disk", %op_id);
        let resp = self
            .service
            .resize_disk(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn snapshot_vm(
        &self,
        request: Request<proto::SnapshotVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("snapshot_vm", %op_id);
        let resp = self
            .service
            .snapshot_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn restore_snapshot(
        &self,
        request: Request<proto::RestoreSnapshotRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("restore_snapshot", %op_id);
        let resp = self
            .service
            .restore_snapshot(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn coredump_vm(
        &self,
        request: Request<proto::CoredumpVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("coredump_vm", %op_id);
        let resp = self
            .service
            .coredump_vm(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn start_network(
        &self,
        request: Request<proto::StartNetworkRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("start_network", %op_id);
        let resp = self
            .service
            .start_network(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn stop_network(
        &self,
        request: Request<proto::StopNetworkRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("stop_network", %op_id);
        let resp = self
            .service
            .stop_network(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn restart_network(
        &self,
        request: Request<proto::RestartNetworkRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("restart_network", %op_id);
        let resp = self
            .service
            .restart_network(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn ping_vmm(
        &self,
        _request: Request<proto::PingVmmRequest>,
    ) -> Result<Response<proto::PingVmmResponse>, Status> {
        Err(tonic::Status::unimplemented(
            "ping_vmm not supported on control plane",
        ))
    }
}

pub struct ReconcileServer {
    service: Arc<dyn ReconcileService>,
}

impl ReconcileServer {
    pub fn new(service: Arc<dyn ReconcileService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl proto::reconcile_service_server::ReconcileService for ReconcileServer {
    async fn apply_node_desired_state(
        &self,
        request: Request<proto::ApplyNodeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("apply_node_desired_state", %op_id);
        let resp = self
            .service
            .apply_node_desired_state(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn apply_vm_desired_state(
        &self,
        request: Request<proto::ApplyVmDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("apply_vm_desired_state", %op_id);
        let resp = self
            .service
            .apply_vm_desired_state(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn apply_volume_desired_state(
        &self,
        request: Request<proto::ApplyVolumeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("apply_volume_desired_state", %op_id);
        let resp = self
            .service
            .apply_volume_desired_state(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn apply_network_desired_state(
        &self,
        request: Request<proto::ApplyNetworkDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("apply_network_desired_state", %op_id);
        let resp = self
            .service
            .apply_network_desired_state(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn acknowledge_desired_state_version(
        &self,
        request: Request<proto::AcknowledgeDesiredStateVersionRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let op_id = extract_op_id(&request).unwrap_or_default();
        let _span = tracing::info_span!("acknowledge_desired_state_version", %op_id);
        let resp = self
            .service
            .acknowledge_desired_state_version(request.into_inner())
            .instrument(_span).await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }
}
