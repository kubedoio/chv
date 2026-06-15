//! Drift detection data types.
//!
//! [`DriftFinding`] is a discriminated union (serde-tagged on `code`) that
//! describes a single deviation between an authored architecture baseline and
//! the live fleet snapshot. [`DriftReport`] aggregates findings with a
//! pre-computed summary so list views do not have to re-iterate findings.
//!
//! All types are pure data: no clocks, no I/O, no async. They serialize on
//! the wire as JSON and round-trip losslessly through serde.

use chv_controlplane_types::architecture::DriftStatus;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single drift finding. The `code` discriminant is stable wire and is the
/// key both UIs and metrics group on.
///
/// Each variant carries:
/// - `path` — a dotted/bracketed pointer into the baseline document, or one of
///   the synthetic prefixes `<<live>>/...` (resource present in the snapshot
///   but absent from the baseline) or `<<permissions>>` (caller permission
///   gap). Stable across calls; safe to render in UIs verbatim.
/// - `resource_ref` — `kind/name` form (e.g. `network/public`). Empty string
///   when no canonical resource exists (currently only `<<permissions>>`).
/// - `message` — operator-readable single-line description; UI may show as-is.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code")]
pub enum DriftFinding {
    /// Baseline declares a resource that is absent from the live snapshot.
    #[serde(rename = "DRIFT_MISSING_RESOURCE")]
    MissingResource {
        path: String,
        resource_ref: String,
        message: String,
    },

    /// Live snapshot has a resource the baseline does not declare. Emitted
    /// for transparency; the baseline is the source of truth, but extras may
    /// signal stale baseline or unmanaged sprawl.
    #[serde(rename = "DRIFT_UNEXPECTED_RESOURCE")]
    UnexpectedResource {
        path: String,
        resource_ref: String,
        message: String,
    },

    /// A non-numeric, non-network field on a resource that exists in both
    /// baseline and snapshot has a different value (e.g. `datastore.kind`,
    /// `image.format`).
    #[serde(rename = "DRIFT_FIELD_CHANGED")]
    FieldChanged {
        path: String,
        resource_ref: String,
        field: String,
        expected: String,
        actual: String,
        message: String,
    },

    /// A numeric capacity attribute differs (e.g. `node.cpu_cores`,
    /// `datastore.capacity_gb`).
    #[serde(rename = "DRIFT_CAPACITY_CHANGED")]
    CapacityChanged {
        path: String,
        resource_ref: String,
        field: String,
        expected: i64,
        actual: i64,
        message: String,
    },

    /// A network-shape field on a network present in both sides differs
    /// (`bridge`, `vlan_id`, or `cidr`). `expected`/`actual` are stringified
    /// because the wire formats are heterogenous (string vs u32).
    #[serde(rename = "DRIFT_NETWORK_CHANGED")]
    NetworkChanged {
        path: String,
        resource_ref: String,
        field: String,
        expected: Option<String>,
        actual: Option<String>,
        message: String,
    },

    /// The caller no longer holds the deploy permission the baseline assumes.
    /// Emitted at most once per report.
    #[serde(rename = "DRIFT_PERMISSION_CHANGED")]
    PermissionChanged {
        path: String,
        resource_ref: String,
        message: String,
    },

    /// An instance's network attachments cannot be satisfied by the live
    /// snapshot (the referenced network is missing).
    #[serde(rename = "DRIFT_ATTACHMENT_CHANGED")]
    AttachmentChanged {
        path: String,
        resource_ref: String,
        message: String,
    },
}

impl DriftFinding {
    /// Stable wire code (also the serde tag). Useful for grouping in the
    /// summary without re-serializing.
    pub fn code(&self) -> &'static str {
        match self {
            DriftFinding::MissingResource { .. } => "DRIFT_MISSING_RESOURCE",
            DriftFinding::UnexpectedResource { .. } => "DRIFT_UNEXPECTED_RESOURCE",
            DriftFinding::FieldChanged { .. } => "DRIFT_FIELD_CHANGED",
            DriftFinding::CapacityChanged { .. } => "DRIFT_CAPACITY_CHANGED",
            DriftFinding::NetworkChanged { .. } => "DRIFT_NETWORK_CHANGED",
            DriftFinding::PermissionChanged { .. } => "DRIFT_PERMISSION_CHANGED",
            DriftFinding::AttachmentChanged { .. } => "DRIFT_ATTACHMENT_CHANGED",
        }
    }
}

/// Aggregate counts across a report. Indexed by [`DriftFinding::code`] so
/// callers can render badges without iterating findings.
///
/// Counts are `i64` (not `usize`) so the wire shape is platform-independent —
/// every other DriftReport-adjacent count surfaced through the BFF wire
/// surface uses `i64`, and `usize` would otherwise serialize as a 32-bit
/// number on 32-bit targets and a 64-bit number on 64-bit targets.
#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct DriftSummary {
    /// Total number of findings (equals `findings.len()`).
    pub total: i64,
    /// Per-code histogram. Keys are the stable [`DriftFinding::code`] strings.
    pub by_type: BTreeMap<String, i64>,
}

/// Full drift report. `status` is `NoDrift` iff `findings` is empty.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DriftReport {
    pub status: DriftStatus,
    pub findings: Vec<DriftFinding>,
    pub summary: DriftSummary,
}
