pub mod api;
mod backup_shipper;
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
mod migration_reaper;
mod node_client;
mod node_client_pool;
mod orchestrator;
pub mod overlay;
pub mod peer_identity;
mod reconcile;
mod server;
mod telemetry;

pub use backup_shipper::{shipper_from_destination, BackupShipper, NullShipper};
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
pub use migration_reaper::MigrationReaper;
pub use node_client::NodeClient;
pub use node_client_pool::NodeClientPool;
pub use orchestrator::Orchestrator;
pub use overlay::OverlayManager;
pub use peer_identity::{
    extract_peer_node_id_from_extensions, parse_node_id_from_der, verify_peer_matches,
    InsecurePeer, PeerIdentityError, PeerIdentityInterceptor, PeerNodeId,
};
pub use reconcile::{ReconcileService, ReconcileServiceImplementation};
pub use server::{
    EnrollmentServer, InventoryServer, LifecycleServer, ReconcileServer, TelemetryServer,
};
pub use telemetry::{TelemetryService, TelemetryServiceImplementation};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;
