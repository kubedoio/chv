//! Drift detection: compare an authored CHV architecture baseline against a
//! live fleet snapshot and emit a structured [`DriftReport`].
//!
//! The module is split into:
//! - [`types`] — the [`DriftFinding`] enum, [`DriftReport`], [`DriftSummary`].
//! - [`compute`] — the pure [`compute_drift`] function.
//!
//! All public surface is re-exported here so callers can `use
//! chv_architecture_reconcile::drift::*;` without reaching into submodules.
//!
//! See `compute` for the ordering invariant findings respect — it matters
//! for idempotency tests and for stable rendering in the UI.

pub mod compute;
pub mod types;

pub use compute::compute_drift;
pub use types::{DriftFinding, DriftReport, DriftSummary};

// Re-export DriftStatus so callers can build/destructure DriftReport without
// reaching into chv-controlplane-types directly.
pub use chv_controlplane_types::architecture::DriftStatus;

#[cfg(test)]
mod tests;
