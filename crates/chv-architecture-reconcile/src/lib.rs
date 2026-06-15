//! I/O implementations of `chv-architecture-validate`'s pure-data
//! traits. Pure-data lives in `chv_architecture_validate::fleet`; this
//! crate wires the [`InventoryProvider`] trait to live SQLite-backed
//! repositories so the BFF (and any other host) can capture a fleet
//! snapshot without depending on the validator's I/O boundary in a
//! second place.
//!
//! Re-exports the validate crate's fleet surface for convenience —
//! callers depend on `chv_architecture_reconcile` and get both the
//! trait and the implementation in one place.

pub use chv_architecture_validate::fleet::{
    capture, BackupTargetInfo, DatastoreInfo, FleetError, ImageInfo, InventoryProvider,
    InventorySnapshot, NetworkInfo, NodeInfo,
};

pub mod apply;
pub mod fleet_inventory;
pub mod plan;

pub use apply::{apply_plan, ApplyContext, ApplyError, ApplyOutcome, ConfirmationToken};
pub use fleet_inventory::FleetInventoryProvider;
pub use plan::{
    build_plan, compute as compute_diff, is_expired, order_changes, Diff, Plan, PlanSummary,
};
