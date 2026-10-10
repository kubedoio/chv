mod alerts;
mod architectures;
mod backups;
mod bootstrap_tokens;
mod credential_crypto;
mod db;
mod desired_state;
mod events;
mod hypervisor_settings;
mod images;
mod monitoring_agents;
mod network_exposures;
mod networks;
mod nodes;
mod observed_state;
mod operations;
mod vtep;

pub use alerts::{AlertCreateInput, AlertRepository};
pub use architectures::{
    is_active_run_conflict, ApplyRunCreateInput, ApplyRunRepository, ApplyRunUpdateInput,
    DriftReportCreateInput, DriftReportRepository, InventorySnapshotCreateInput,
    InventorySnapshotRepository, NetboxProjectionConfigRepository,
    NetboxProjectionConfigUpsertInput, NetboxProjectionRunCreateInput,
    NetboxProjectionRunRepository, PlanCreateInput, PlanRepository, PlanStatusUpdateInput,
    TopologyCreateInput, TopologyListFilter, TopologyRepository, TopologyUpdateInput,
    VersionCreateInput, VersionRepository, ACTIVE_RUN_CONFLICT_MARKER, MAX_ATTEMPTS,
};
pub use backups::{
    BackupJobCreateInput, BackupJobRow, BackupJobStatusUpdateInput, BackupJobUpdateInput,
    BackupRepository, BackupRestoreCreateInput, BackupRestoreRow, BackupScheduleCreateInput,
    BackupScheduleRow, BackupScheduleUpdateInput,
};
pub use bootstrap_tokens::{BootstrapTokenRepository, BootstrapTokenValidation};
pub use db::{
    connect_pool, migrations_path, migrator, run_migrations, ControlPlaneStoreConfig, StoreError,
    StorePool,
};
pub use desired_state::{
    CloneTargetMaterialization, CloneTargetSpec, DesiredStateRepository, NetworkDesiredStateInput,
    NetworkStatusPatchInput, VmDesiredStateInput, VmPowerStatePatchInput, VmResourcesPatchInput,
    VolumeAttachmentPatchInput, VolumeDesiredStateInput, VolumeResizePatchInput,
    VolumeSnapshotPatchInput, VolumeSummaryRow,
};
pub use events::{EventAppendInput, EventRepository};
pub use hypervisor_settings::{HypervisorSettingsRepository, HypervisorSettingsRow};
pub use images::{ImageRepository, ImageRow};
pub use monitoring_agents::{
    AgentAuthError, AgentCredential, AgentOsMetadata, ClaimConsumeError, ConsumedClaim,
    IssuedClaim, MonitoringAgentRepository, MonitoringAgentRow, RecordBatchOutcome,
};
pub use network_exposures::NetworkExposureInput;
pub use networks::{NetworkRepository, NetworkRow};
pub use nodes::{
    NodeBootstrapResultInput, NodeDrainIntentInput, NodeInventoryInput, NodeRepository,
    NodeSchedulingPatchInput, NodeStatePatchInput, NodeUpsertInput, NodeVersionInput,
    AUTHORITY_MODE_CORE_MANAGED, AUTHORITY_MODE_CORE_NATIVE, AUTHORITY_MODE_LEGACY,
};
pub use observed_state::{
    NetworkObservedStateInput, NodeObservedStateInput, ObservedStateRepository, VmMetricsInput,
    VmObservedStateInput, VolumeObservedStateInput,
};
pub use operations::{OperationCreateInput, OperationRepository, OperationStatusUpdateInput};
pub use vtep::{FabricPeerRecord, VtepEntry, VtepRepository};

#[cfg(any(test, feature = "test-util"))]
pub mod test_util;

#[cfg(test)]
mod tests;
