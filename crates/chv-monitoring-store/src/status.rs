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
    /// exhausted, corruption). `None` when healthy. Materialized from
    /// the flavor trackers below (store flavor takes precedence) —
    /// readers only need this field.
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
    /// True when the last headroom probe could not read the
    /// filesystem. Ingestion fails open in that window (the durable
    /// size budget still bounds the file), but the headroom floor is
    /// unverified until the next successful probe — surfaced instead
    /// of silently dropping the protection.
    pub headroom_probe_failed: bool,
    /// Raw sample count at the last maintenance pass.
    pub raw_samples: Option<u64>,
    /// Store-flavor degradation (store error, config-disabled,
    /// corruption): cleared by a durable ingest success or operator
    /// intervention, never by headroom recovery.
    pub store_degraded: Option<String>,
    /// Maintenance-flavor degradation (rollup/retention/checkpoint
    /// worker failure): cleared only by a successful maintenance pass —
    /// a durable ingest success must not flap it away while the worker
    /// is still failing (both run concurrently against the same store).
    pub maintenance_degraded: Option<String>,
    /// Headroom-flavor degradation (disk-full floor breach): cleared
    /// by headroom recovery only.
    pub headroom_degraded: Option<String>,
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

    /// Mark monitoring degraded with a store-flavor reason
    /// (idempotent; keeps the first reason).
    pub fn degrade(&self, reason: String) {
        self.update(|s| {
            if s.store_degraded.is_none() {
                s.store_degraded = Some(reason);
                s.recompute_degraded_reason();
            }
        });
    }

    /// Mark monitoring degraded because the headroom floor is breached
    /// (disk full). Replaceable by a newer headroom reading; cleared
    /// only by [`Self::clear_headroom`] — headroom recovery must never
    /// clear a store-flavor degradation, and vice versa.
    pub fn degrade_headroom(&self, reason: String) {
        self.update(|s| {
            s.headroom_degraded = Some(reason);
            s.recompute_degraded_reason();
        });
    }

    /// Mark monitoring degraded because a maintenance pass (rollups,
    /// retention, WAL checkpoint) failed. Cleared only by a successful
    /// pass — durable ingest successes run concurrently and must not
    /// flap this flavor away while the worker is still failing.
    pub fn degrade_maintenance(&self, reason: String) {
        self.update(|s| {
            s.maintenance_degraded = Some(reason);
            s.recompute_degraded_reason();
        });
    }

    /// Clear maintenance-flavor degradation (a pass succeeded).
    pub fn clear_maintenance(&self) {
        self.update(|s| {
            s.maintenance_degraded = None;
            s.recompute_degraded_reason();
        });
    }

    /// Clear headroom-flavor degradation (headroom recovered).
    pub fn clear_headroom(&self) {
        self.update(|s| {
            s.headroom_degraded = None;
            s.recompute_degraded_reason();
        });
    }

    /// Clear store-flavor degradation — called when a durable ingest
    /// succeeds (the store provably works again).
    pub fn clear_store_degraded(&self) {
        self.update(|s| {
            s.store_degraded = None;
            s.recompute_degraded_reason();
        });
    }

    /// Clear all degradation.
    pub fn clear_degraded(&self) {
        self.update(|s| {
            s.store_degraded = None;
            s.maintenance_degraded = None;
            s.headroom_degraded = None;
            s.degraded_reason = None;
        });
    }
}

impl MonitoringHealthSnapshot {
    /// `degraded_reason` is the presentation of the three flavor
    /// trackers: a store problem is the most severe and wins.
    fn recompute_degraded_reason(&mut self) {
        self.degraded_reason = self
            .store_degraded
            .clone()
            .or_else(|| self.maintenance_degraded.clone())
            .or_else(|| self.headroom_degraded.clone());
    }
}
