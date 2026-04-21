mod alerts;
mod bootstrap_tokens;
mod db;
mod desired_state;
mod events;
mod network_exposures;
mod nodes;
mod observed_state;
mod operations;

pub use alerts::{AlertCreateInput, AlertRepository};
pub use bootstrap_tokens::{BootstrapTokenRepository, BootstrapTokenValidation};
pub use db::{
    connect_pool, migrations_path, migrator, run_migrations, ControlPlaneStoreConfig, StoreError,
    StorePool,
};
pub use desired_state::{
    DesiredStateRepository, NetworkDesiredStateInput, VmDesiredStateInput, VmPowerStatePatchInput,
    VolumeAttachmentPatchInput, VolumeDesiredStateInput, VolumeResizePatchInput,
};
pub use events::{EventAppendInput, EventRepository};
pub use network_exposures::{NetworkExposureInput, NetworkExposureRepository};
pub use nodes::{
    NodeBootstrapResultInput, NodeDrainIntentInput, NodeInventoryInput, NodeRepository,
    NodeSchedulingPatchInput, NodeStateInput, NodeStatePatchInput, NodeUpsertInput,
    NodeVersionInput,
};
pub use observed_state::{
    NetworkObservedStateInput, NodeObservedStateInput, ObservedStateRepository,
    VmMetricsInput, VmObservedStateInput, VolumeObservedStateInput,
};
pub use operations::{OperationCreateInput, OperationRepository, OperationStatusUpdateInput};

#[cfg(any(test, feature = "test-util"))]
pub mod test_util;

#[cfg(test)]
mod tests;
