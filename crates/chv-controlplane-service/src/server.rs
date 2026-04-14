use crate::enrollment::EnrollmentService;
use crate::inventory::InventoryService;
use crate::reconcile::ReconcileService;
use crate::telemetry::TelemetryService;
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;
use tonic::{Request, Response, Status};

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
        let resp = self
            .service
            .enroll_node(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn rotate_node_certificate(
        &self,
        request: Request<proto::RotateNodeCertificateRequest>,
    ) -> Result<Response<proto::RotateNodeCertificateResponse>, Status> {
        let resp = self
            .service
            .rotate_node_certificate(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn report_bootstrap_result(
        &self,
        request: Request<proto::ReportBootstrapResultRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .report_bootstrap_result(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
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
        let resp = self
            .service
            .report_node_inventory(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn report_service_versions(
        &self,
        request: Request<proto::ReportServiceVersionsRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .report_service_versions(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
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
        let resp = self
            .service
            .report_node_state(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn report_vm_state(
        &self,
        request: Request<proto::VmStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .report_vm_state(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn report_volume_state(
        &self,
        request: Request<proto::VolumeStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .report_volume_state(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn report_network_state(
        &self,
        request: Request<proto::NetworkStateReport>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .report_network_state(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn publish_event(
        &self,
        request: Request<proto::PublishEventRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .publish_event(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn publish_alert(
        &self,
        request: Request<proto::PublishAlertRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .publish_alert(request.into_inner())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
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
        let resp = self
            .service
            .apply_node_desired_state(request.into_inner())
            .await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn apply_vm_desired_state(
        &self,
        request: Request<proto::ApplyVmDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .apply_vm_desired_state(request.into_inner())
            .await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn apply_volume_desired_state(
        &self,
        request: Request<proto::ApplyVolumeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .apply_volume_desired_state(request.into_inner())
            .await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn apply_network_desired_state(
        &self,
        request: Request<proto::ApplyNetworkDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .apply_network_desired_state(request.into_inner())
            .await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }

    async fn acknowledge_desired_state_version(
        &self,
        request: Request<proto::AcknowledgeDesiredStateVersionRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let resp = self
            .service
            .acknowledge_desired_state_version(request.into_inner())
            .await
            .map_err(tonic::Status::from)?;
        Ok(Response::new(resp))
    }
}
