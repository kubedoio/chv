use chv_observability::{
    CHV_CP_CONSECUTIVE_DRIFT_TICKS, CHV_CP_CONVERGENCE_AVG_MS, CHV_CP_DRIFT_COUNT,
    CHV_CP_OPERATIONS_DISPATCHED_TOTAL, CHV_CP_PENDING_OPERATIONS, CHV_CP_RECONCILE_TICKS_TOTAL,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Convergence metrics for the control plane reconciler (ADR-002 M2).
///
/// Tracks the gap between desired and observed state so operators can see:
/// - How many resources are currently diverged
/// - How long convergence takes on average
/// - Whether drift is increasing or decreasing
#[derive(Debug, Clone)]
pub struct ConvergenceMetrics {
    /// Number of resources currently diverged from desired state
    pub drift_count: u32,
    /// Number of pending operations not yet dispatched
    pub pending_operations: u32,
    /// Total operations dispatched since process start
    pub operations_dispatched: u64,
    /// Average convergence time in milliseconds (exponential moving average)
    pub avg_convergence_ms: f64,
    /// Timestamp of last successful full convergence (drift_count == 0)
    pub last_converged_at: Option<Instant>,
    /// Number of consecutive ticks with drift > 0
    pub consecutive_drift_ticks: u32,
    /// Total reconcile loop iterations
    pub reconcile_ticks_total: u64,
}

impl Default for ConvergenceMetrics {
    fn default() -> Self {
        Self {
            drift_count: 0,
            pending_operations: 0,
            operations_dispatched: 0,
            avg_convergence_ms: 0.0,
            last_converged_at: None,
            consecutive_drift_ticks: 0,
            reconcile_ticks_total: 0,
        }
    }
}

impl ConvergenceMetrics {
    /// Record the start of a new reconcile tick.
    pub fn tick_start(&mut self) {
        self.reconcile_ticks_total += 1;
    }

    /// Update drift count after comparing desired vs observed state.
    pub fn record_drift(&mut self, drift_count: u32) {
        self.drift_count = drift_count;
        if drift_count == 0 {
            self.last_converged_at = Some(Instant::now());
            self.consecutive_drift_ticks = 0;
        } else {
            self.consecutive_drift_ticks += 1;
        }
    }

    /// Record the number of pending operations (Accepted + RetryPending).
    pub fn record_pending_operations(&mut self, count: u32) {
        self.pending_operations = count;
    }

    /// Record dispatched operations and update the convergence EMA.
    pub fn record_dispatch(&mut self, dispatched: u64, elapsed_ms: f64) {
        self.operations_dispatched += dispatched;
        // Exponential moving average with alpha=0.1
        self.avg_convergence_ms = 0.9 * self.avg_convergence_ms + 0.1 * elapsed_ms;
    }

    /// How many seconds ago the system last reached full convergence.
    pub fn last_converged_ago_seconds(&self) -> Option<f64> {
        self.last_converged_at.map(|t| t.elapsed().as_secs_f64())
    }

    /// Emit all metrics to the prometheus recorder via the `metrics` crate.
    pub fn emit_prometheus(&self) {
        metrics::gauge!(CHV_CP_DRIFT_COUNT).set(self.drift_count as f64);
        metrics::gauge!(CHV_CP_PENDING_OPERATIONS).set(self.pending_operations as f64);
        metrics::gauge!(CHV_CP_CONVERGENCE_AVG_MS).set(self.avg_convergence_ms);
        metrics::gauge!(CHV_CP_CONSECUTIVE_DRIFT_TICKS).set(self.consecutive_drift_ticks as f64);
        metrics::counter!(CHV_CP_RECONCILE_TICKS_TOTAL).absolute(self.reconcile_ticks_total);
        metrics::counter!(CHV_CP_OPERATIONS_DISPATCHED_TOTAL).absolute(self.operations_dispatched);
    }
}

/// Thread-safe shared handle to convergence metrics.
pub type SharedConvergenceMetrics = Arc<RwLock<ConvergenceMetrics>>;

/// Create a new shared convergence metrics instance.
pub fn new_shared() -> SharedConvergenceMetrics {
    Arc::new(RwLock::new(ConvergenceMetrics::default()))
}

/// JSON-serializable snapshot for the health endpoint.
#[derive(serde::Serialize)]
pub struct ConvergenceSnapshot {
    pub drift_count: u32,
    pub pending_operations: u32,
    pub avg_convergence_ms: f64,
    pub consecutive_drift_ticks: u32,
    pub last_converged_ago_seconds: Option<f64>,
    pub reconcile_ticks_total: u64,
    pub operations_dispatched_total: u64,
}

impl From<&ConvergenceMetrics> for ConvergenceSnapshot {
    fn from(m: &ConvergenceMetrics) -> Self {
        Self {
            drift_count: m.drift_count,
            pending_operations: m.pending_operations,
            avg_convergence_ms: (m.avg_convergence_ms * 10.0).round() / 10.0,
            consecutive_drift_ticks: m.consecutive_drift_ticks,
            last_converged_ago_seconds: m
                .last_converged_ago_seconds()
                .map(|s| (s * 10.0).round() / 10.0),
            reconcile_ticks_total: m.reconcile_ticks_total,
            operations_dispatched_total: m.operations_dispatched,
        }
    }
}
