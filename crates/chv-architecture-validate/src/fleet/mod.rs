//! Layer-2 fleet consistency checks.
//!
//! Pure-data: this module never performs I/O. The [`InventoryProvider`]
//! trait is the I/O boundary — implementors live in
//! `chv-architecture-reconcile` and translate live cluster state into a
//! [`InventorySnapshot`]. Once a snapshot is captured, every check function
//! in this module is a synchronous, deterministic pure function over
//! `(&CHVArchitecture, &InventorySnapshot) -> Vec<Finding>`.
//!
//! Codes emitted here are declared in [`crate::codes`] and are part of the
//! BFF contract. See `docs/specs/architecture-designer/contracts/`.

mod checks;
pub mod inventory;

pub use checks::check_fleet;
pub use inventory::{
    capture, BackupTargetInfo, DatastoreInfo, ImageInfo, InventoryProvider, InventorySnapshot,
    NetworkInfo, NodeInfo, SecretInfo,
};

use thiserror::Error;

/// Errors raised by an [`InventoryProvider`] implementation. The validator
/// itself never produces these — they bubble up from the I/O layer.
#[derive(Debug, Error)]
pub enum FleetError {
    #[error("inventory provider error: {0}")]
    Provider(String),
    #[error("inventory unavailable")]
    Unavailable,
    #[error("invalid snapshot: {0}")]
    InvalidSnapshot(String),
}

#[cfg(test)]
mod tests;
