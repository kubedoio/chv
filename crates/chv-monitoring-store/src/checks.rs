//! The manager's service/check inventory (agent spec: "Discovery
//! creates service/check inventory in the manager"; ingestion
//! contract v1's `checks` array).
//!
//! One row per `(target_kind, target_id, check_id)` holds the LATEST
//! record for that check — an inventory, not a history. Rows only move
//! forward: the upsert's guard refuses to overwrite a newer
//! `observed_at_ms` with an older one, so a delayed batch cannot
//! regress the inventory. Like everything else in this store the table
//! is disposable telemetry: a failure here degrades monitoring, never
//! VM lifecycle.

use crate::db::MonitoringStore;
use crate::error::MonitoringStoreError;
use chv_monitoring_core::model::{CheckRecord, CheckStatus, TargetKind};
use sqlx::Row;

/// A stored check record: the validated [`CheckRecord`] plus the
/// manager-stamped receipt time, the recording agent, and the
/// server-side staleness decision.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredCheck {
    pub check_id: String,
    pub service_key: Option<String>,
    /// Typed state string form (`ok`/`warning`/`critical`/`unknown`).
    pub status: CheckStatus,
    pub summary: Option<String>,
    pub observed_at_ms: u64,
    pub received_at_ms: u64,
    pub agent_id: String,
    /// Server-side staleness decision (see [`CHECK_STALE_AFTER_MS`]).
    pub stale: bool,
}

/// Staleness window for check records. Checks ride the guest agent's
/// 60-second collection cadence, so the registry's metric-specific
/// override for the `check.*` family applies
/// (`chv_monitoring_core::registry::stale_after_ms` — 3× cadence, one
/// missed collection must not read as stale). The constant is only
/// the fallback if the registry override is ever removed; the lookup
/// below keeps this table and the registry from drifting apart.
pub const CHECK_STALE_AFTER_MS: u64 = 180_000;

fn check_stale_after_ms() -> u64 {
    chv_monitoring_core::registry::stale_after_ms("check.status").unwrap_or(CHECK_STALE_AFTER_MS)
}

impl MonitoringStore {
    /// Record the latest check results for one target in a single
    /// transaction: an upsert keyed by `(target_kind, target_id,
    /// check_id)`. The upsert's WHERE clause only moves a row forward —
    /// a batch carrying an older `observed_at_ms` than the stored one
    /// leaves the inventory untouched (an equal timestamp refreshes
    /// receipt metadata, keeping re-records idempotent).
    pub async fn record_checks(
        &self,
        sender_agent_id: &str,
        target_kind: &TargetKind,
        target_id: &str,
        checks: &[CheckRecord],
        now_ms: u64,
    ) -> Result<(), MonitoringStoreError> {
        let mut tx = self.pool.begin().await?;
        for c in checks {
            sqlx::query(
                "INSERT INTO monitoring_checks (
                   target_kind, target_id, check_id, service_key, status,
                   summary, observed_at_ms, received_at_ms, agent_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(target_kind, target_id, check_id) DO UPDATE SET
                   service_key = excluded.service_key,
                   status = excluded.status,
                   summary = excluded.summary,
                   observed_at_ms = excluded.observed_at_ms,
                   received_at_ms = excluded.received_at_ms,
                   agent_id = excluded.agent_id
                 WHERE excluded.observed_at_ms >= monitoring_checks.observed_at_ms",
            )
            .bind(target_kind.as_str())
            .bind(target_id)
            .bind(&c.check_id)
            .bind(&c.service_key)
            .bind(c.status.as_str())
            .bind(&c.summary)
            .bind(c.observed_at_ms as i64)
            .bind(now_ms as i64)
            .bind(sender_agent_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// The latest-record-per-check inventory for one target, with the
    /// server-side staleness decision (`stale` = the record's
    /// observation is older than the registry's `check.*` window —
    /// 180 s). A target with no recorded checks answers an empty vec
    /// (honest absence, never a fabricated row).
    pub async fn query_checks(
        &self,
        target_kind: &TargetKind,
        target_id: &str,
        now_ms: u64,
    ) -> Result<Vec<StoredCheck>, MonitoringStoreError> {
        let rows = sqlx::query(
            "SELECT check_id, service_key, status, summary, observed_at_ms, received_at_ms, agent_id
             FROM monitoring_checks
             WHERE target_kind = ? AND target_id = ?
             ORDER BY check_id ASC",
        )
        .bind(target_kind.as_str())
        .bind(target_id)
        .fetch_all(&self.pool)
        .await?;
        let threshold = check_stale_after_ms();
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let status_str: String = r.get("status");
            let status =
                CheckStatus::parse(&status_str).ok_or_else(|| MonitoringStoreError::Degraded {
                    reason: format!("stored check status {status_str:?} is unknown"),
                })?;
            let observed: i64 = r.get("observed_at_ms");
            let received: i64 = r.get("received_at_ms");
            out.push(StoredCheck {
                check_id: r.get("check_id"),
                service_key: r.get("service_key"),
                status,
                summary: r.get("summary"),
                observed_at_ms: observed.unsigned_abs(),
                received_at_ms: received.unsigned_abs(),
                agent_id: r.get("agent_id"),
                stale: now_ms.saturating_sub(observed.unsigned_abs()) > threshold,
            });
        }
        Ok(out)
    }
}
