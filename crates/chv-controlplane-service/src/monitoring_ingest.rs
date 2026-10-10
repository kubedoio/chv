//! Node metric batch ingestion (campaign #602 PR-2, ingestion contract
//! `docs/specs/contracts/chv-monitoring-ingestion-v1.md`).
//!
//! Invariants enforced here, in order:
//!
//! 1. **Sender identity** — the server shim pins `node_id` to the
//!    peer's mTLS certificate (same interceptor as every other
//!    node-facing RPC); the service additionally requires the node to
//!    be enrolled.
//! 2. **Target ownership** — node-target samples must name the
//!    authenticated sender; VM-target samples must name a VM currently
//!    observed on that sender. A node can never write another node's
//!    or another node's VMs' telemetry.
//! 3. **Contract validation** — schema version, batch size, sample
//!    timestamps, registry membership (unknown metric or disallowed
//!    source ⇒ whole-batch `unsupported_metric`), value/quality
//!    consistency, dimension bounds.
//! 4. **Budgets** — per-sender rate cap, filesystem headroom floor.
//! 5. **Durability** — the store ACKs only committed batches
//!    (durable dedup by sender+boot_id+sequence with a high-water
//!    mark that fails closed).
//!
//! Ingestion is observational: every rejection is a typed outcome in
//! the response, never a mutation of VM or node state, and never an
//! error that could backpressure reconciliation.

use crate::error::ControlPlaneServiceError;
use crate::monitoring_validate::{reject, RawSample, RawValue, SampleRejection, MAX_BOOT_ID_BYTES};
use async_trait::async_trait;
use chv_controlplane_store::{NodeRepository, ObservedStateRepository};
use chv_controlplane_types::domain::{NodeId, ResourceId};
use chv_monitoring_core::model::{Sample, TargetKind};
use chv_monitoring_store::{headroom, IngestOutcome, MonitoringHealth, MonitoringStore, NodeBatch};
use control_plane_node_api::control_plane_node_api as proto;
use dashmap::DashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

// Caps and wire outcome vocabulary are shared with the guest-agent
// transport (monitoring_validate); re-exported here so the crate's
// public paths stay stable.
pub use crate::monitoring_validate::{
    MAX_SAMPLES_PER_BATCH, OUTCOME_ACCEPTED, OUTCOME_BATCH_TOO_LARGE, OUTCOME_DUPLICATE,
    OUTCOME_INGESTION_UNAVAILABLE, OUTCOME_INVALID_BATCH, OUTCOME_RATE_LIMITED,
    OUTCOME_REPLAY_CONFLICT, OUTCOME_SERIES_CAP_EXCEEDED, OUTCOME_STALE_SEQUENCE,
    OUTCOME_UNSUPPORTED_METRIC,
};

/// Rate-limit window.
const RATE_WINDOW_MS: u64 = 60 * 1000;

/// The Rust-side contract of the gRPC `MonitoringService`.
#[async_trait]
pub trait MonitoringIngestService: Send + Sync {
    async fn ingest_node_metric_batch(
        &self,
        request: proto::NodeMetricBatchRequest,
    ) -> Result<proto::NodeMetricBatchResponse, ControlPlaneServiceError>;
}

#[derive(Debug, Clone)]
struct IngestLimits {
    batches_per_minute: u32,
    min_headroom_bytes: u64,
}

/// Per-sender sliding rate window (bounded by the enrolled node count).
#[derive(Debug, Default)]
struct RateWindow {
    window_start_ms: u64,
    count: u32,
}

/// The monitoring ingestion service. `store: None` means monitoring is
/// disabled or degraded — every batch is answered
/// `ingestion_unavailable` and health carries the reason, and nothing
/// else in the control plane is affected.
#[derive(Clone)]
pub struct MonitoringIngestImplementation {
    store: Option<Arc<MonitoringStore>>,
    node_repo: NodeRepository,
    observed_state_repo: ObservedStateRepository,
    health: MonitoringHealth,
    rate_windows: Arc<DashMap<String, RateWindow>>,
    limits: IngestLimits,
}

impl MonitoringIngestImplementation {
    pub fn new(
        store: Option<Arc<MonitoringStore>>,
        node_repo: NodeRepository,
        observed_state_repo: ObservedStateRepository,
        health: MonitoringHealth,
        batches_per_minute: u32,
        min_headroom_bytes: u64,
    ) -> Self {
        Self {
            store,
            node_repo,
            observed_state_repo,
            health,
            rate_windows: Arc::new(DashMap::new()),
            limits: IngestLimits {
                batches_per_minute: batches_per_minute.max(1),
                min_headroom_bytes,
            },
        }
    }

    pub fn health(&self) -> &MonitoringHealth {
        &self.health
    }

    pub fn store(&self) -> Option<&Arc<MonitoringStore>> {
        self.store.as_ref()
    }

    fn respond(
        &self,
        request: &proto::NodeMetricBatchRequest,
        outcome: &str,
        accepted_samples: u32,
        retry_after_seconds: u32,
        summary: String,
    ) -> proto::NodeMetricBatchResponse {
        proto::NodeMetricBatchResponse {
            meta: Some(proto::ResultMeta {
                operation_id: request
                    .meta
                    .as_ref()
                    .map(|m| m.operation_id.clone())
                    .unwrap_or_default(),
                status: if outcome == OUTCOME_ACCEPTED || outcome == OUTCOME_DUPLICATE {
                    "ok".to_string()
                } else {
                    "rejected".to_string()
                },
                node_observed_generation: String::new(),
                error_code: if outcome == OUTCOME_ACCEPTED || outcome == OUTCOME_DUPLICATE {
                    String::new()
                } else {
                    outcome.to_string()
                },
                human_summary: summary,
            }),
            outcome: outcome.to_string(),
            accepted_samples,
            request_id: String::new(),
            retry_after_seconds,
        }
    }

    /// Per-sender rate window check. Returns the count of batches
    /// accepted in the current window including this one, or 0 when
    /// the window was reset for this batch.
    ///
    /// Fixed window: bursts straddling a window boundary can each
    /// consume a full budget, transiently allowing ~2× the nominal
    /// cap. Acceptable for advisory telemetry (the agent's cadence is
    /// one batch per 15 s against a 20/min cap) — documented here so
    /// the boundary allowance is a decision, not an accident.
    fn count_rate(&self, sender: &str, now_ms: u64) -> u32 {
        let mut entry = self.rate_windows.entry(sender.to_string()).or_default();
        let window = entry.value_mut();
        if now_ms.saturating_sub(window.window_start_ms) >= RATE_WINDOW_MS {
            window.window_start_ms = now_ms;
            window.count = 0;
        }
        window.count += 1;
        window.count
    }

    /// Filesystem headroom gate (ADR-027: a separate file on the same
    /// filesystem does not isolate disk-full risk). Degrades on the
    /// way down, recovers on the way back up.
    async fn check_headroom(&self) -> bool {
        let Some(store) = &self.store else {
            return false;
        };
        let Some(path) = store.db_path() else {
            return true;
        };
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or(path);
        match headroom::available_bytes(&dir) {
            Ok(available) => {
                self.health.update(|s| {
                    s.headroom_bytes = Some(available);
                    s.headroom_probe_failed = false;
                });
                if available < self.limits.min_headroom_bytes {
                    self.health.degrade_headroom(format!(
                        "filesystem headroom {} bytes below floor {} bytes",
                        available, self.limits.min_headroom_bytes
                    ));
                    false
                } else {
                    // Headroom recovered: clear exactly the headroom
                    // flavor — a store-flavor degradation survives.
                    self.health.clear_headroom();
                    true
                }
            }
            Err(e) => {
                // statvfs failure on the monitoring filesystem is
                // treated as unknown-but-suspicious: fail open for
                // ingestion (the durable size budget still bounds the
                // file) but surface it in health — the floor is
                // unverified until the next successful probe, and a
                // silently dropped protection is worse than a visible
                // one.
                tracing::warn!(error = %e, "monitoring headroom probe failed");
                self.health.update(|s| {
                    s.headroom_probe_failed = true;
                    // The last reading is stale; report unknown rather
                    // than a number we can no longer stand behind.
                    s.headroom_bytes = None;
                });
                true
            }
        }
    }

    /// Convert one proto sample to a contract `Sample`. The registry,
    /// source, kind/unit, value/quality, timestamp and dimension rules
    /// live in the shared validator (`monitoring_validate`) so the
    /// node and guest transports enforce identical contracts; this
    /// method adds the node transport's ownership rule (node targets
    /// must be self-referential) and maps the proto wire shape.
    fn convert_sample(
        &self,
        s: &proto::MetricSampleV1,
        sender_node_id: &str,
        now_ms: i64,
    ) -> Result<Sample, SampleRejection> {
        let target_kind = chv_monitoring_core::model::TargetKind::from_str(&s.target_kind)
            .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("target_kind: {e}")))?;
        match target_kind {
            TargetKind::Node => {
                if s.target_id != sender_node_id {
                    return Err(reject(
                        OUTCOME_INVALID_BATCH,
                        format!(
                            "node-target sample for {} from sender {sender_node_id}",
                            s.target_id
                        ),
                    ));
                }
            }
            TargetKind::Vm => {
                ResourceId::new(&s.target_id)
                    .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("vm target_id: {e}")))?;
            }
            _ => {
                return Err(reject(
                    OUTCOME_UNSUPPORTED_METRIC,
                    format!(
                        "target kind {} is not accepted on the node batch transport in v1",
                        s.target_kind
                    ),
                ));
            }
        }

        let raw = RawSample {
            target_kind: s.target_kind.clone(),
            target_id: s.target_id.clone(),
            metric_id: s.metric_id.clone(),
            source: s.source.clone(),
            kind: s.kind.clone(),
            unit: s.unit.clone(),
            observed_at_ms: s.observed_at_ms,
            quality: s.quality.clone(),
            value: s.value.as_ref().map(|v| match v {
                proto::metric_sample_v1::Value::FloatValue(f) => RawValue::Float(*f),
                proto::metric_sample_v1::Value::IntegerValue(i) => RawValue::Integer(*i),
            }),
            dimensions: s.dimensions.clone().into_iter().collect(),
            boot_id: s.boot_id.clone(),
            identity_epoch: s.identity_epoch.clone(),
        };
        crate::monitoring_validate::validate_sample(&raw, now_ms)
    }
}

#[async_trait]
impl MonitoringIngestService for MonitoringIngestImplementation {
    async fn ingest_node_metric_batch(
        &self,
        request: proto::NodeMetricBatchRequest,
    ) -> Result<proto::NodeMetricBatchResponse, ControlPlaneServiceError> {
        let now_ms = chrono::Utc::now().timestamp_millis();

        // 0. Store availability: disabled or degraded monitoring is a
        //    typed outcome, never an error.
        let Some(store) = &self.store else {
            self.health.update(|s| s.unavailable_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_INGESTION_UNAVAILABLE,
                0,
                0,
                "monitoring store is not available".to_string(),
            ));
        };

        // 1. Envelope validation.
        if request.schema_version != 1 {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_INVALID_BATCH,
                0,
                0,
                format!("schema_version {} is not 1", request.schema_version),
            ));
        }
        if request.boot_id.is_empty() || request.boot_id.len() > MAX_BOOT_ID_BYTES {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_INVALID_BATCH,
                0,
                0,
                "boot_id must be 1..=128 bytes".to_string(),
            ));
        }
        // The wire type is uint64 but the store's high-water mark is a
        // signed SQLite INTEGER: a sequence beyond i64::MAX could wrap
        // on the cast and masquerade as a low sequence. No honest
        // sender reaches it (one batch per 15 s is ~2M/year); reject it
        // at the boundary instead of storing a wrapped watermark.
        if request.sequence > i64::MAX as u64 {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_INVALID_BATCH,
                0,
                0,
                format!(
                    "sequence {} exceeds the i64 watermark range",
                    request.sequence
                ),
            ));
        }
        if request.samples.len() > MAX_SAMPLES_PER_BATCH {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_BATCH_TOO_LARGE,
                0,
                0,
                format!(
                    "{} samples exceeds the {} per-batch cap",
                    request.samples.len(),
                    MAX_SAMPLES_PER_BATCH
                ),
            ));
        }

        // 2. Sender: enrolled node (identity itself was pinned to the
        //    peer certificate by the server shim).
        let node_id = NodeId::new(&request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {e}"))
        })?;
        if !self.node_repo.node_exists(&node_id).await? {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_INVALID_BATCH,
                0,
                0,
                format!("sender node {} is not enrolled", request.node_id),
            ));
        }

        // 3. Rate cap per sender.
        if self.count_rate(&request.node_id, now_ms as u64) > self.limits.batches_per_minute {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_RATE_LIMITED,
                0,
                10,
                format!(
                    "more than {} batches per minute from this sender",
                    self.limits.batches_per_minute
                ),
            ));
        }

        // 4. Headroom gate (disk-full is a designed failure mode).
        if !self.check_headroom().await {
            self.health.update(|s| s.unavailable_batches += 1);
            return Ok(self.respond(
                &request,
                OUTCOME_INGESTION_UNAVAILABLE,
                0,
                30,
                "filesystem headroom below floor; monitoring degraded".to_string(),
            ));
        }

        // 5. Sample conversion + VM ownership. Whole-batch semantics:
        //    one bad sample rejects the batch (v1 never ACKs an
        //    ambiguous subset). Rejections are typed outcomes in the
        //    response, never gRPC errors.
        let mut samples = Vec::with_capacity(request.samples.len());
        for s in &request.samples {
            let sample = match self.convert_sample(s, &request.node_id, now_ms) {
                Ok(sample) => sample,
                Err(rejection) => {
                    self.health.update(|h| h.rejected_batches += 1);
                    return Ok(self.respond(&request, rejection.outcome, 0, 0, rejection.detail));
                }
            };
            if sample.target_kind == TargetKind::Vm {
                let vm_id = ResourceId::new(&sample.target_id).map_err(|e| {
                    ControlPlaneServiceError::InvalidArgument(format!("invalid vm_id: {e}"))
                })?;
                let owner = self.observed_state_repo.vm_reporting_node(&vm_id).await?;
                if owner.as_ref() != Some(&node_id) {
                    self.health.update(|h| h.rejected_batches += 1);
                    return Ok(self.respond(
                        &request,
                        OUTCOME_INVALID_BATCH,
                        0,
                        0,
                        format!("vm {vm_id} is not currently reported on sender {node_id}"),
                    ));
                }
            }
            samples.push(sample);
        }

        // 6. Durable ingestion. A store-level failure is a degraded
        //    monitoring outcome (the agent retries), never a gRPC
        //    error that could look like a control-plane failure.
        let batch = NodeBatch {
            boot_id: request.boot_id.clone(),
            sequence: request.sequence,
            sent_at_ms: request.sent_at_ms.unsigned_abs(),
            samples,
        };
        let outcome = match store
            .ingest_node_batch(node_id.as_str(), &batch, now_ms as u64)
            .await
        {
            Ok(outcome) => outcome,
            Err(e) => {
                tracing::warn!(error = %e, "monitoring ingest failed");
                self.health.degrade(format!("ingest failure: {e}"));
                self.health.update(|h| h.unavailable_batches += 1);
                return Ok(self.respond(
                    &request,
                    OUTCOME_INGESTION_UNAVAILABLE,
                    0,
                    5,
                    format!("monitoring store error: {e}"),
                ));
            }
        };
        let response = match outcome {
            IngestOutcome::Accepted { samples } => {
                self.health.update(|h| {
                    h.accepted_batches += 1;
                    h.last_ingest_at_ms = Some(now_ms as u64);
                });
                // A durable commit proves the store works: clear a
                // stale store-flavor degradation (headroom flavor is
                // not ours to clear here).
                self.health.clear_store_degraded();
                self.respond(
                    &request,
                    OUTCOME_ACCEPTED,
                    samples,
                    0,
                    format!("{samples} samples durably committed"),
                )
            }
            IngestOutcome::Duplicate { samples } => {
                self.health.update(|h| {
                    h.duplicate_batches += 1;
                    h.last_ingest_at_ms = Some(now_ms as u64);
                });
                self.health.clear_store_degraded();
                self.respond(
                    &request,
                    OUTCOME_DUPLICATE,
                    samples,
                    0,
                    "identical batch already committed".to_string(),
                )
            }
            IngestOutcome::ReplayConflict => {
                self.health.update(|h| h.rejected_batches += 1);
                self.respond(
                    &request,
                    OUTCOME_REPLAY_CONFLICT,
                    0,
                    0,
                    "same (sender, boot_id, sequence) with different content".to_string(),
                )
            }
            IngestOutcome::StaleSequence => {
                self.health.update(|h| h.rejected_batches += 1);
                self.respond(
                    &request,
                    OUTCOME_STALE_SEQUENCE,
                    0,
                    0,
                    "sequence at or below the high-water mark without a retained proof; \
                     start a new boot epoch"
                        .to_string(),
                )
            }
            IngestOutcome::SeriesCapExceeded {
                target_id,
                series,
                cap,
            } => {
                self.health.update(|h| h.rejected_batches += 1);
                self.respond(
                    &request,
                    OUTCOME_SERIES_CAP_EXCEEDED,
                    0,
                    0,
                    format!("target {target_id} would exceed the {cap}-series cap ({series})"),
                )
            }
        };
        Ok(response)
    }
}

/// The maintenance worker: one bounded pass per interval (rollups,
/// retention, size budget, WAL checkpoint). A failed pass degrades
/// health and is retried on the next tick — monitoring maintenance
/// must never take down or block the control plane.
pub async fn run_monitoring_maintenance(
    store: Arc<MonitoringStore>,
    health: MonitoringHealth,
    interval: Duration,
    mut shutdown: tokio::sync::watch::Receiver<()>,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = ticker.tick() => {
                let now_ms = chrono::Utc::now().timestamp_millis().unsigned_abs();
                match store.run_maintenance(now_ms).await {
                    Ok(report) => {
                        if report.evicted_raw > 0 {
                            tracing::warn!(
                                evicted_raw = report.evicted_raw,
                                "monitoring db exceeded its size budget; oldest raw data evicted"
                            );
                        }
                        let raw_count = store.raw_sample_count().await.unwrap_or(0);
                        health.update(|h| {
                            h.last_maintenance_at_ms = Some(now_ms);
                            h.raw_samples = Some(raw_count);
                        });
                        health.clear_maintenance();
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "monitoring maintenance pass failed");
                        // Maintenance flavor: cleared only by a
                        // successful pass — a durable ingest success
                        // (which clears the STORE flavor) must not
                        // flap this away while the worker still fails.
                        health.degrade_maintenance(format!("maintenance failure: {e}"));
                    }
                }
            }
        }
    }
}
