//! Plan generation: pure-data diff between a desired architecture model
//! and a captured fleet inventory snapshot, plus deterministic ordering.
//!
//! Inputs come from earlier phases:
//!
//! * `desired: &CHVArchitecture` — the validated model (Phase 1) the user
//!   wants to converge the fleet onto.
//! * `snapshot: &InventorySnapshot` — the captured fleet state (Phase 3).
//! * `mode: PlanMode` — `Apply` produces creates/updates/replaces; `Destroy`
//!   produces deletes for every desired resource (the "tear it all down"
//!   path). Other modes (`DryRun`, `Confirm`) currently behave like
//!   `Apply` for diff purposes — Phase 5 may differentiate.
//!
//! The output is intentionally pure data so the BFF can serialize it as the
//! `plan_json` column in the `architecture_plans` table and return it
//! verbatim from `POST /v1/architectures/plan`.

use chv_architecture_validate::{fleet::InventorySnapshot, model::CHVArchitecture};
use chv_common::clock::Clock;
use chv_controlplane_types::architecture::{ArchitecturePlan, PlanAction, PlanChange, PlanMode};
use serde::{Deserialize, Serialize};

mod diff;
mod order;

#[cfg(test)]
mod integration_smoke;
#[cfg(test)]
mod tests;

pub use diff::{compute, Diff};
pub use order::order_changes;

/// Top-level plan result that the BFF persists as `plan_json` and returns
/// in the `/v1/architectures/plan` response. Mirrors the
/// [`Plan result`](docs/specs/architecture-designer/contracts/validation-plan-contract.md)
/// contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Apply or destroy semantics — used by the UI to colour the preview
    /// and by Phase-5 apply to decide whether to gate behind typed
    /// confirmation.
    pub mode: PlanMode,
    /// Ordered, deterministic list of changes.
    pub changes: Vec<PlanChange>,
    /// Aggregate counts derived from `changes`. Stored alongside the changes
    /// for cheap UI rendering without re-walking the list.
    pub summary: PlanSummary,
    /// Inlined warnings from the validation/fleet-check stages so the
    /// response is self-contained.
    pub warnings: Vec<String>,
}

/// Counts of plan changes by [`PlanAction`], plus the carried warning count.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct PlanSummary {
    /// Number of `Create` changes.
    pub create: u32,
    /// Number of `Update` changes.
    pub update: u32,
    /// Number of `Delete` changes.
    pub delete: u32,
    /// Number of `Replace` changes.
    pub replace: u32,
    /// Number of `NoOp` changes.
    pub no_op: u32,
    /// Number of warnings carried in [`Plan::warnings`].
    pub warnings: u32,
}

impl PlanSummary {
    /// Build a [`PlanSummary`] by counting actions across `changes`.
    pub fn from_changes(changes: &[PlanChange], warnings: u32) -> Self {
        let mut s = Self {
            warnings,
            ..Self::default()
        };
        for c in changes {
            match c.action {
                PlanAction::Create => s.create += 1,
                PlanAction::Update => s.update += 1,
                PlanAction::Delete => s.delete += 1,
                PlanAction::Replace => s.replace += 1,
                PlanAction::NoOp => s.no_op += 1,
            }
        }
        s
    }
}

/// Build a fully-populated [`Plan`] from a desired model, an inventory
/// snapshot, plan mode and a list of pre-computed warning strings.
pub fn build_plan(
    desired: &CHVArchitecture,
    snapshot: &InventorySnapshot,
    mode: PlanMode,
    warnings: Vec<String>,
) -> Plan {
    let diff = diff::compute(desired, snapshot, mode);
    let warning_count = warnings.len() as u32;
    let summary = PlanSummary::from_changes(&diff.changes, warning_count);
    Plan {
        mode,
        changes: diff.changes,
        summary,
        warnings,
    }
}

/// Returns `true` when `plan.expires_at` is strictly before `clock.now()`.
///
/// Phase-4 plans carry a 15-minute TTL persisted as `expires_at`. The BFF
/// must reject `apply` and `confirm` requests against plans that have
/// passed their expiry — this helper centralizes that check so the apply
/// path and the periodic sweeper stay in lockstep.
pub fn is_expired<C: Clock + ?Sized>(plan: &ArchitecturePlan, clock: &C) -> bool {
    clock.now() > plan.expires_at
}
