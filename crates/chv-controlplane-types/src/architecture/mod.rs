//! Architecture Designer domain types.
//!
//! These types are the in-memory representation of the architecture
//! designer entities defined in
//! `docs/specs/component/architecture-designer-data-model.md`.
//!
//! Conventions:
//! - Newtype IDs (e.g. [`ArchitectureId`]) reject empty strings via
//!   [`IdentifierError`] so service code never has to defend against blank
//!   identifiers.
//! - Status enums use `serde(rename_all = "snake_case")` to match the
//!   on-the-wire enum strings used by the YAML/API contracts.
//! - Timestamps are [`chrono::DateTime<chrono::Utc>`]; the SQLite layer
//!   parses the `text` columns back into chrono.

mod drift;
mod finding;
mod model;
mod plan;

pub use drift::{
    ArchitectureApplyRun, ArchitectureApplyRunId, ArchitectureDriftReport,
    ArchitectureDriftReportId, DriftStatus, RunStatus,
};
pub use finding::{Finding, Severity};
pub use model::{
    ArchitectureId, ArchitectureStatus, ArchitectureTopology, ArchitectureVersion,
    ArchitectureVersionId, FleetCheckStatus, ValidationStatus,
};
pub use plan::{
    ArchitecturePlan, ArchitecturePlanId, InventorySnapshot, InventorySnapshotId, PlanAction,
    PlanChange, PlanMode, PlanStatus, ResourceType, Risk,
};
