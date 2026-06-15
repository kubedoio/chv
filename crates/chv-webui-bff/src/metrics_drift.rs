//! Architecture Designer drift-detection metrics.
//!
//! Single counter emitted per `POST /v1/architectures/drift` call:
//!
//! * `chv_architecture_drift_total{status}` — counter of drift checks,
//!   labelled with the lifecycle status:
//!     - `no_drift`     — fresh compute returned an empty `findings` list
//!     - `drifted`      — fresh compute returned at least one finding
//!     - `unknown`      — fresh compute returned `DriftStatus::Unknown`
//!       (defensive label for the wire enum's `Unknown` variant; not
//!       emitted by `compute_drift` today, but reserved so a future code
//!       path that yields `Unknown` does not silently fold into `no_drift`)
//!     - `check_failed` — snapshot capture or YAML parse failed; the BFF
//!       persisted a `check_failed` report and returned 200
//!     - `cache_hit`    — the most recent persisted report was within the
//!       5-minute TTL and `force_refresh` was false
//!
//! The `cache_hit` label is the only non-DriftStatus value here. It's a
//! deliberate cardinality bound — a fresh compute always produces one of
//! the three real statuses, but the cache path skips compute entirely so
//! the underlying status of the cached row is irrelevant to the wire
//! contract. UIs that need the cached status should read the response
//! body, not the metrics.
//!
//! The metric name is a stable contract and surfaces via the existing
//! Prometheus exporter wired in `cmd/chv-controlplane/src/bootstrap.rs`.

/// Counter name. Labelled with `status` ∈
/// {`no_drift`, `drifted`, `unknown`, `check_failed`, `cache_hit`}.
pub const CHV_ARCHITECTURE_DRIFT_TOTAL: &str = "chv_architecture_drift_total";

/// Closed-set status label for the drift counter.
///
/// Cardinality is bounded to five values; the type system carries that
/// contract because the metrics crate does not enforce closed enums.
///
/// `Unknown` is reserved for the wire enum's `DriftStatus::Unknown` variant
/// so that a fresh compute landing on it (via a future code path) does not
/// silently fold into `NoDrift` — see review note M4 on metric-label drift.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DriftStatusLabel {
    NoDrift,
    Drifted,
    Unknown,
    CheckFailed,
    CacheHit,
}

impl DriftStatusLabel {
    /// Stable label value emitted on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoDrift => "no_drift",
            Self::Drifted => "drifted",
            Self::Unknown => "unknown",
            Self::CheckFailed => "check_failed",
            Self::CacheHit => "cache_hit",
        }
    }
}

/// Increment the drift counter for `status`.
pub fn record_drift_status(status: DriftStatusLabel) {
    metrics::counter!(
        CHV_ARCHITECTURE_DRIFT_TOTAL,
        "status" => status.as_str()
    )
    .increment(1);
}
