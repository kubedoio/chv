use crate::db::MonitoringStore;
use crate::error::{IngestOutcome, MonitoringStoreError};
use chv_monitoring_core::model::{Sample, SampleValue};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

/// A node metric batch in its validated, post-conversion form (the
/// service layer converts the proto message to contract `Sample`s
/// through `SampleBuilder` before ingestion; the store enforces the
/// durable semantics).
#[derive(Clone, Debug)]
pub struct NodeBatch {
    /// The sender's per-run epoch identifier (dedup key component).
    pub boot_id: String,
    /// Monotonic within (sender, boot_id).
    pub sequence: u64,
    pub sent_at_ms: u64,
    pub samples: Vec<Sample>,
}

/// Canonical digest of a batch: any change to any field — including
/// dimension order — changes the digest, which is what turns a
/// same-key-different-body resend into a `ReplayConflict`.
/// `sent_at_ms` is deliberately EXCLUDED: it is send-attempt metadata,
/// not observation content — a retry of the same samples at a later
/// send time is a `Duplicate` (the previous durable outcome), never a
/// conflict.
pub fn batch_digest(batch: &NodeBatch) -> String {
    let mut hasher = Sha256::new();
    hasher.update(batch.boot_id.as_bytes());
    hasher.update(batch.sequence.to_le_bytes());
    hasher.update(batch.samples.len().to_le_bytes());
    for s in &batch.samples {
        let mut line = String::with_capacity(128);
        line.push_str(&format!(
            "{:?}|{}|{}|{:?}|{:?}|{:?}|{}|{:?}|",
            s.target_kind,
            s.target_id,
            s.metric_id,
            s.source,
            s.kind,
            s.unit,
            s.observed_at_ms,
            s.quality,
        ));
        match s.value {
            None => line.push('-'),
            Some(SampleValue::Float(v)) => line.push_str(&format!("f:{v:e}")),
            Some(SampleValue::Integer(v)) => line.push_str(&format!("i:{v}")),
        }
        line.push('|');
        for (k, v) in s.dimensions.iter() {
            line.push_str(k);
            line.push('\u{1f}');
            line.push_str(v);
            line.push('\u{1e}');
        }
        line.push('|');
        line.push_str(s.boot_id.as_deref().unwrap_or(""));
        line.push('|');
        line.push_str(s.identity_epoch.as_deref().unwrap_or(""));
        hasher.update(line.as_bytes());
    }
    hex(hasher.finalize())
}

fn hex(bytes: impl IntoIterator<Item = u8>) -> String {
    bytes.into_iter().map(|b| format!("{b:02x}")).collect()
}

/// Stable hash of a dimension set (sorted keys; the map is a `BTreeMap`
/// so the serialization is canonical).
pub fn dimensions_hash(dims: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_string(dims).unwrap_or_default().as_bytes());
    hex(hasher.finalize())
}

impl MonitoringStore {
    /// Durably ingest one node batch. The entire effect — samples,
    /// dimension sets, series rows, dedup proof, high-water mark — is a
    /// single transaction: the returned `Accepted` means committed to
    /// SQLite, and any intermediate failure leaves no partial state.
    pub async fn ingest_node_batch(
        &self,
        sender_node_id: &str,
        batch: &NodeBatch,
        now_ms: u64,
    ) -> Result<IngestOutcome, MonitoringStoreError> {
        let digest = batch_digest(batch);
        let mut tx = self.pool.begin().await?;

        // 1. Dedup: the same (sender, boot_id, sequence) is either an
        //    identical retry (previous outcome) or a conflict.
        let existing: Option<(String, i64)> = sqlx::query_as(
            "SELECT batch_digest, accepted_samples FROM monitoring_ingest_dedup
             WHERE sender_node_id = ? AND boot_id = ? AND sequence = ?",
        )
        .bind(sender_node_id)
        .bind(&batch.boot_id)
        .bind(batch.sequence as i64)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((stored_digest, accepted)) = existing {
            return if stored_digest == digest {
                Ok(IngestOutcome::Duplicate {
                    samples: accepted.max(0) as u32,
                })
            } else {
                Ok(IngestOutcome::ReplayConflict)
            };
        }

        // 2. High-water mark: a sequence at or below the newest accepted
        //    for this epoch, without a retained proof, is an
        //    out-of-window replay. Fail closed — never re-insert.
        let watermark: Option<i64> = sqlx::query_scalar(
            "SELECT high_water_sequence FROM monitoring_ingest_watermarks
             WHERE sender_node_id = ? AND boot_id = ?",
        )
        .bind(sender_node_id)
        .bind(&batch.boot_id)
        .fetch_optional(&mut *tx)
        .await?;
        if watermark
            .map(|hw| batch.sequence as i64 <= hw)
            .unwrap_or(false)
        {
            return Ok(IngestOutcome::StaleSequence);
        }

        // 3. Series cap per target: existing registered series plus the
        //    new distinct series this batch introduces. Rejection is
        //    whole-batch (v1 never ACKs an ambiguous subset).
        let cap = self.config.max_series_per_target;
        let targets = collect_batch_targets(batch);
        for ((kind, target), new_series) in &targets {
            let existing_series: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM monitoring_series WHERE target_kind = ? AND target_id = ?",
            )
            .bind(kind)
            .bind(target)
            .fetch_one(&mut *tx)
            .await?;
            if existing_series + new_series.len() as i64 > cap {
                return Ok(IngestOutcome::SeriesCapExceeded {
                    target_id: target.clone(),
                    series: existing_series + new_series.len() as i64,
                    cap,
                });
            }
        }

        // 4. Dimension sets (upsert; last_seen refreshes GC).
        for s in &batch.samples {
            let hash = dimensions_hash_public(&s.dimensions);
            let json = serde_json::to_string(&dimension_map(&s.dimensions)).map_err(|e| {
                MonitoringStoreError::Degraded {
                    reason: format!("dimension serialization failed: {e}"),
                }
            })?;
            sqlx::query(
                "INSERT INTO monitoring_dimension_sets (dimensions_hash, dimensions_json, last_seen_at_ms)
                 VALUES (?, ?, ?)
                 ON CONFLICT(dimensions_hash) DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms",
            )
            .bind(&hash)
            .bind(&json)
            .bind(now_ms as i64)
            .execute(&mut *tx)
            .await?;
        }

        // 5. Samples. INSERT OR IGNORE keeps a retried batch idempotent
        //    at the row level: a same-epoch sample at the same
        //    observed_at_ms is the same observation.
        for s in &batch.samples {
            let hash = dimensions_hash_public(&s.dimensions);
            let (value_integer, value_real): (Option<i64>, Option<f64>) = match s.value {
                None => (None, None),
                Some(SampleValue::Integer(v)) => {
                    // SQLite INTEGER is i64; saturate (a byte counter
                    // reaches 2^63 only after centuries at line rate).
                    (Some(v.min(i64::MAX as u64) as i64), None)
                }
                Some(SampleValue::Float(v)) => (None, Some(v)),
            };
            let result = sqlx::query(
                "INSERT OR IGNORE INTO monitoring_samples (
                   target_kind, target_id, metric_id, source, dimensions_hash,
                   observed_at_ms, received_at_ms, value_integer, value_real,
                   quality, kind, unit, boot_id, identity_epoch
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(s.target_kind.as_str().to_string())
            .bind(&s.target_id)
            .bind(&s.metric_id)
            .bind(s.source.as_str().to_string())
            .bind(&hash)
            .bind(s.observed_at_ms as i64)
            .bind(now_ms as i64)
            .bind(value_integer)
            .bind(value_real)
            .bind(quality_str(s))
            .bind(s.kind.as_str().to_string())
            .bind(s.unit.as_str().to_string())
            .bind(s.boot_id.as_deref().unwrap_or(""))
            .bind(s.identity_epoch.as_deref().unwrap_or(""))
            .execute(&mut *tx)
            .await?;
            let _ = result;
        }

        // 6. Series registry upsert.
        for s in &batch.samples {
            let hash = dimensions_hash_public(&s.dimensions);
            sqlx::query(
                "INSERT INTO monitoring_series (
                   target_kind, target_id, metric_id, source, dimensions_hash,
                   kind, unit, last_observed_at_ms
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(target_kind, target_id, metric_id, source, dimensions_hash)
                 DO UPDATE SET last_observed_at_ms = excluded.last_observed_at_ms",
            )
            .bind(s.target_kind.as_str().to_string())
            .bind(&s.target_id)
            .bind(&s.metric_id)
            .bind(s.source.as_str().to_string())
            .bind(&hash)
            .bind(s.kind.as_str().to_string())
            .bind(s.unit.as_str().to_string())
            .bind(s.observed_at_ms as i64)
            .execute(&mut *tx)
            .await?;
        }

        // 7. Dedup proof + high-water mark, then commit: the ACK the
        //    caller returns is backed by this commit only.
        sqlx::query(
            "INSERT INTO monitoring_ingest_dedup
               (sender_node_id, boot_id, sequence, batch_digest, accepted_samples, received_at_ms)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(sender_node_id)
        .bind(&batch.boot_id)
        .bind(batch.sequence as i64)
        .bind(&digest)
        .bind(batch.samples.len() as i64)
        .bind(now_ms as i64)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO monitoring_ingest_watermarks (sender_node_id, boot_id, high_water_sequence, updated_at_ms)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(sender_node_id, boot_id)
             DO UPDATE SET high_water_sequence = MAX(high_water_sequence, excluded.high_water_sequence),
                           updated_at_ms = excluded.updated_at_ms",
        )
        .bind(sender_node_id)
        .bind(&batch.boot_id)
        .bind(batch.sequence as i64)
        .bind(now_ms as i64)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(IngestOutcome::Accepted {
            samples: batch.samples.len() as u32,
        })
    }

    /// Latest durably-accepted receipt time (for health/degraded
    /// signaling). `None` when nothing has ever been ingested.
    pub async fn last_ingest_at_ms(&self) -> Result<Option<u64>, MonitoringStoreError> {
        let ms: Option<i64> =
            sqlx::query_scalar("SELECT MAX(received_at_ms) FROM monitoring_ingest_dedup")
                .fetch_one(&self.pool)
                .await?;
        Ok(ms.map(|v| v.unsigned_abs()))
    }

    /// Count of raw samples currently stored (operator visibility).
    pub async fn raw_sample_count(&self) -> Result<u64, MonitoringStoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM monitoring_samples")
            .fetch_one(&self.pool)
            .await?;
        Ok(n.unsigned_abs())
    }

    /// Distinct series count per target kind (operator visibility).
    pub async fn series_counts(&self) -> Result<BTreeMap<String, u64>, MonitoringStoreError> {
        let rows =
            sqlx::query("SELECT target_kind, COUNT(*) FROM monitoring_series GROUP BY target_kind")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|r: sqlx::sqlite::SqliteRow| {
                let kind: String = r.get(0);
                let n: i64 = r.get(1);
                (kind, n.unsigned_abs())
            })
            .collect())
    }
}

/// (target_kind, target_id) → the distinct new series keys in a batch.
type BatchTargets = BTreeMap<(String, String), BTreeSet<(String, String, String)>>;

fn collect_batch_targets(batch: &NodeBatch) -> BatchTargets {
    let mut targets: BatchTargets = BTreeMap::new();
    for s in &batch.samples {
        let hash = dimensions_hash_public(&s.dimensions);
        targets
            .entry((s.target_kind.as_str().to_string(), s.target_id.clone()))
            .or_default()
            .insert((s.metric_id.clone(), s.source.as_str().to_string(), hash));
    }
    targets
}

fn quality_str(s: &Sample) -> &'static str {
    use chv_monitoring_core::model::SampleQuality::*;
    match s.quality {
        Valid => "valid",
        InsufficientSamples => "insufficient_samples",
        Unsupported => "unsupported",
        Unavailable => "unavailable",
        Invalid => "invalid",
        Stale => "stale",
    }
}

fn dimension_map(dims: &chv_monitoring_core::model::Dimensions) -> BTreeMap<String, String> {
    dims.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Public helper so internal modules share one canonical hash.
fn dimensions_hash_public(dims: &chv_monitoring_core::model::Dimensions) -> String {
    dimensions_hash(&dimension_map(dims))
}
