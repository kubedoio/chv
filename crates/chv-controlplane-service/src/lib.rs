pub mod api;
mod backup_worker;
mod bff_mutations;
pub mod circuit_breaker;
pub mod compat;
mod container;
pub mod convergence_metrics;
mod enrollment;
mod error;
mod inventory;
mod lifecycle;
pub mod migration;
mod node_client;
mod node_client_pool;
mod orchestrator;
pub mod overlay;
mod reconcile;
mod server;
mod telemetry;
pub mod upgrade;

pub use backup_worker::BackupWorker;
pub use bff_mutations::ControlPlaneMutationService;
pub use container::{ControlPlaneComponents, ControlPlaneRuntime, ControlPlaneService};
pub use convergence_metrics::{ConvergenceMetrics, SharedConvergenceMetrics};
pub use enrollment::{
    CaBackedCertificateIssuer, CertificateIssuer, EnrollmentService,
    EnrollmentServiceImplementation, IssuedCertificate,
};
pub use error::ControlPlaneServiceError;
pub use inventory::{InventoryService, InventoryServiceImplementation};
pub use lifecycle::{LifecycleService, LifecycleServiceImplementation};
pub use node_client::NodeClient;
pub use node_client_pool::NodeClientPool;
pub use orchestrator::Orchestrator;
pub use overlay::OverlayManager;
pub use reconcile::{ReconcileService, ReconcileServiceImplementation};
pub use server::{
    EnrollmentServer, InventoryServer, LifecycleServer, ReconcileServer, TelemetryServer,
};
pub use telemetry::{TelemetryService, TelemetryServiceImplementation};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;
