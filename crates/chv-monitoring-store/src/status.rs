use std::sync::Arc;
use std::sync::RwLock;

/// A point-in-time view of the monitoring subsystem's health, surfaced
/// through `/v1/monitoring/health` and the deep health check.
///
/// Degradation is the designed failure mode (ADR-027): a missing,
/// corrupt, full, or unwritable monitoring database never fails the
/// control plane or VM lifecycle — it flips this state and ingestion
/// refuses batches until the operator intervenes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonitoringHealthSnapshot {
    /// Why monitoring is degraded (store unavailable, disk headroom
    /// exhausted, corruption). `None` when healthy.
    pub degraded_reason: Option<String>,
    /// Newest durably-accepted ingest (unix ms).
    pub last_ingest_at_ms: Option<u64>,
    /// Newest completed maintenance pass (unix ms).
    pub last_maintenance_at_ms: Option<u64>,
    pub accepted_batches: u64,
    pub duplicate_batches: u64,
    pub rejected_batches: u64,
    /// Batches refused because the store was unavailable/degraded.
    pub unavailable_batches: u64,
    /// Last observed filesystem headroom on the monitoring filesystem
    /// (bytes), when known.
    pub headroom_bytes: Option<u64>,
    /// Raw sample count at the last maintenance pass.
    pub raw_samples: Option<u64>,
}

/// Shared, lock-protected health state written by the ingestion path
/// and the maintenance worker, read by the BFF health endpoints.
#[derive(Debug, Default)]
pub struct MonitoringHealth {
    inner: Arc<RwLock<MonitoringHealthSnapshot>>,
}

impl Clone for MonitoringHealth {
    fn clone(&self) -> Self {
        MonitoringHealth {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl MonitoringHealth {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> MonitoringHealthSnapshot {
        self.inner
            .read()
            .expect("monitoring health lock poisoned")
            .clone()
    }

    /// Apply a mutation to the snapshot under the lock.
    pub fn update<F: FnOnce(&mut MonitoringHealthSnapshot)>(&self, f: F) {
        let mut guard = self.inner.write().expect("monitoring health lock poisoned");
        f(&mut guard);
    }

    /// Mark monitoring degraded (idempotent; keeps the first reason
    /// unless `replace` is set).
    pub fn degrade(&self, reason: String) {
        self.update(|s| {
            if s.degraded_reason.is_none() {
                s.degraded_reason = Some(reason);
            }
        });
    }

    /// Clear degradation (e.g. headroom recovered).
    pub fn clear_degraded(&self) {
        self.update(|s| {
            s.degraded_reason = None;
        });
    }
}
