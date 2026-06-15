//! Architecture Designer apply / destroy metrics.
//!
//! Two metrics are emitted per `POST /v1/architectures/apply` and
//! `POST /v1/architectures/destroy` call:
//!
//! * `chv_architecture_apply_total{status}` — counter of apply attempts,
//!   labelled with the lifecycle status:
//!     - `started`  — incremented on entry, before any guard runs
//!     - `enqueued` — incremented when [`apply_plan`] returns `Ok`
//!     - `failed`   — incremented when [`apply_plan`] returns `Err` for
//!       any reason (4xx pre-condition or 5xx store)
//! * `chv_architecture_apply_duration_seconds` — histogram of wall-clock
//!   time from handler entry to the returned response.
//!
//! Both metric names are stable contracts and surface via the existing
//! Prometheus exporter wired in `cmd/chv-controlplane/src/bootstrap.rs`.
//!
//! [`apply_plan`]: chv_architecture_reconcile::apply::apply_plan

use std::time::Instant;

/// Counter name. Labelled with `status` ∈ {`started`, `enqueued`, `failed`}.
pub const CHV_ARCHITECTURE_APPLY_TOTAL: &str = "chv_architecture_apply_total";

/// Histogram name. Records seconds.
pub const CHV_ARCHITECTURE_APPLY_DURATION_SECONDS: &str = "chv_architecture_apply_duration_seconds";

/// Lifecycle marker for the apply counter.
///
/// The status label is a closed enum to keep label cardinality bounded —
/// metrics-exporter-prometheus does not enforce closed enums, so the type
/// system carries that contract here.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ApplyStatusLabel {
    Started,
    Enqueued,
    Failed,
}

impl ApplyStatusLabel {
    /// Stable label value emitted on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Enqueued => "enqueued",
            Self::Failed => "failed",
        }
    }
}

/// Increment the apply counter for `status`.
pub fn record_apply_status(status: ApplyStatusLabel) {
    metrics::counter!(
        CHV_ARCHITECTURE_APPLY_TOTAL,
        "status" => status.as_str()
    )
    .increment(1);
}

/// Lightweight RAII timer that records the apply duration when dropped.
///
/// Construct on handler entry; observe explicitly via [`Self::observe`] at
/// the response boundary. The `Drop` impl is a safety net — if a handler
/// returns through an unusual path (panic before observe) the histogram
/// still records something.
pub struct ApplyTimer {
    start: Instant,
    observed: bool,
}

impl ApplyTimer {
    /// Start a new timer.
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
            observed: false,
        }
    }

    /// Record the elapsed time in seconds and consume the timer.
    pub fn observe(mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        metrics::histogram!(CHV_ARCHITECTURE_APPLY_DURATION_SECONDS).record(elapsed);
        self.observed = true;
    }
}

impl Drop for ApplyTimer {
    fn drop(&mut self) {
        if !self.observed {
            let elapsed = self.start.elapsed().as_secs_f64();
            metrics::histogram!(CHV_ARCHITECTURE_APPLY_DURATION_SECONDS).record(elapsed);
        }
    }
}
