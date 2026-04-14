use crate::enrollment::EnrollmentServiceImplementation;
use crate::error::ControlPlaneServiceError;
use crate::inventory::InventoryServiceImplementation;
use crate::reconcile::ReconcileServiceImplementation;
use crate::telemetry::TelemetryServiceImplementation;
use chv_controlplane_store::StorePool;
use control_plane_node_api::control_plane_node_api as proto;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct ControlPlaneRuntime {
    bind_addr: SocketAddr,
    runtime_dir: PathBuf,
}

impl ControlPlaneRuntime {
    pub fn new(bind_addr: SocketAddr, runtime_dir: PathBuf) -> Self {
        Self {
            bind_addr,
            runtime_dir,
        }
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }
}

#[derive(Clone)]
pub struct ControlPlaneComponents {
    store_pool: StorePool,
    enrollment_service: EnrollmentServiceImplementation,
    inventory_service: InventoryServiceImplementation,
    telemetry_service: TelemetryServiceImplementation,
    reconcile_service: ReconcileServiceImplementation,
}

impl ControlPlaneComponents {
    pub fn new(
        store_pool: StorePool,
        enrollment_service: EnrollmentServiceImplementation,
        inventory_service: InventoryServiceImplementation,
        telemetry_service: TelemetryServiceImplementation,
        reconcile_service: ReconcileServiceImplementation,
    ) -> Self {
        Self {
            store_pool,
            enrollment_service,
            inventory_service,
            telemetry_service,
            reconcile_service,
        }
    }

    pub fn store_pool(&self) -> &StorePool {
        &self.store_pool
    }

    pub fn enrollment_service(&self) -> &EnrollmentServiceImplementation {
        &self.enrollment_service
    }

    pub fn inventory_service(&self) -> &InventoryServiceImplementation {
        &self.inventory_service
    }

    pub fn telemetry_service(&self) -> &TelemetryServiceImplementation {
        &self.telemetry_service
    }

    pub fn reconcile_service(&self) -> &ReconcileServiceImplementation {
        &self.reconcile_service
    }
}

#[derive(Clone)]
pub struct ControlPlaneService {
    runtime: ControlPlaneRuntime,
    components: ControlPlaneComponents,
}

impl ControlPlaneService {
    pub fn new(runtime: ControlPlaneRuntime, components: ControlPlaneComponents) -> Self {
        Self {
            runtime,
            components,
        }
    }

    pub fn runtime(&self) -> &ControlPlaneRuntime {
        &self.runtime
    }

    pub fn components(&self) -> &ControlPlaneComponents {
        &self.components
    }

    pub async fn run(&self) -> Result<(), ControlPlaneServiceError> {
        let addr = self.runtime.bind_addr();

        let enrollment_server = proto::enrollment_service_server::EnrollmentServiceServer::new(
            crate::server::EnrollmentServer::new(Arc::new(
                self.components.enrollment_service.clone(),
            )),
        );

        let inventory_server = proto::inventory_service_server::InventoryServiceServer::new(
            crate::server::InventoryServer::new(Arc::new(
                self.components.inventory_service.clone(),
            )),
        );

        let telemetry_server = proto::telemetry_service_server::TelemetryServiceServer::new(
            crate::server::TelemetryServer::new(Arc::new(
                self.components.telemetry_service.clone(),
            )),
        );

        let reconcile_server = proto::reconcile_service_server::ReconcileServiceServer::new(
            crate::server::ReconcileServer::new(Arc::new(
                self.components.reconcile_service.clone(),
            )),
        );

        info!(?addr, "starting gRPC server");

        tonic::transport::Server::builder()
            .add_service(enrollment_server)
            .add_service(inventory_server)
            .add_service(telemetry_server)
            .add_service(reconcile_server)
            .serve(addr)
            .await
            .map_err(|e: tonic::transport::Error| {
                error!("gRPC server error: {}", e);
                ControlPlaneServiceError::Internal(e.to_string())
            })?;

        Ok(())
    }
}
