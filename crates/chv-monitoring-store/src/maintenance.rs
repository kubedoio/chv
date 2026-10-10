use crate::db::MonitoringStore;
use crate::error::MonitoringStoreError;
use sqlx::Row;
use std::collections::BTreeSet;

/// What one maintenance pass did (operator visibility; also the
/// degraded-signal inputs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaintenanceReport {
    /// Rollup windows (re)computed.
    pub rollup_windows: u64,
    /// Raw rows deleted by retention.
    pub raw_deleted: u64,
    /// 5m rollup rows deleted by retention.
    pub rollup_5m_deleted: u64,
    /// 1h rollup rows deleted by retention.
    pub rollup_1h_deleted: u64,
    /// Dedup proofs deleted by retention.
    pub dedup_deleted: u64,
    /// Orphaned dimension sets deleted.
    pub dimension_sets_deleted: u64,
    /// Raw rows evicted by the hard size budget.
    pub evicted_raw: u64,
    /// Whether the WAL checkpoint ran.
    pub checkpointed: bool,
}

const META_LAST_PASS: &str = "maintenance_last_pass_ms";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const ROLLUP_5M_MS: i64 = 5 * 60 * 1000;
const ROLLUP_1H_MS: i64 = 60 * 60 * 1000;

/// A series identity for rollup computation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SeriesKey {
    target_kind: String,
    target_id: String,
    metric_id: String,
    source: String,
    dimensions_hash: String,
    kind: String,
}

impl MonitoringStore {
    /// One idempotent maintenance pass: recompute rollup windows that
    /// received raw data since the last pass, apply retention, enforce
    /// the size budget, garbage-collect orphaned dimension sets, and
    /// checkpoint the WAL. Safe to interrupt at any point — every step
    /// is idempotent and the next pass redoes incomplete work.
    pub async fn run_maintenance(
        &self,
        now_ms: u64,
    ) -> Result<MaintenanceReport, MonitoringStoreError> {
        let mut report = MaintenanceReport::default();
        let mut tx = self.pool.begin().await?;

        let last_pass: Option<String> =
            sqlx::query_scalar("SELECT value FROM monitoring_meta WHERE key = ?")
                .bind(META_LAST_PASS)
                .fetch_optional(&mut *tx)
                .await?;
        let since = last_pass.and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);

        // 1. Rollups: recompute every window touched by raw rows
        //    received since the last pass (late arrivals included —
        //    windows are rewritten wholesale, so interruption or
        //    re-processing cannot corrupt them). The first pass ever
        //    (last_pass = None) rolls up everything still in raw
        //    retention, which is the correct bootstrap.
        let touched: Vec<(String, String, String, String, String, String, i64)> = sqlx::query_as(
            "SELECT DISTINCT target_kind, target_id, metric_id, source, dimensions_hash, kind,
                    MAX(observed_at_ms) AS newest
             FROM monitoring_samples
             WHERE received_at_ms > ?
             GROUP BY target_kind, target_id, metric_id, source, dimensions_hash, kind",
        )
        .bind(since)
        .fetch_all(&mut *tx)
        .await?;

        for (target_kind, target_id, metric_id, source, dimensions_hash, kind, newest) in &touched {
            let series = SeriesKey {
                target_kind: target_kind.clone(),
                target_id: target_id.clone(),
                metric_id: metric_id.clone(),
                source: source.clone(),
                dimensions_hash: dimensions_hash.clone(),
                kind: kind.clone(),
            };
            for (tier, window_ms) in [("5m", ROLLUP_5M_MS), ("1h", ROLLUP_1H_MS)] {
                // Windows from the oldest raw data still relevant to
                // this series through its newest touched observation.
                let windows: BTreeSet<i64> = sqlx::query(
                    "SELECT DISTINCT (observed_at_ms / ?) * ? FROM monitoring_samples
                     WHERE target_kind = ? AND target_id = ? AND metric_id = ?
                       AND source = ? AND dimensions_hash = ?",
                )
                .bind(window_ms)
                .bind(window_ms)
                .bind(&series.target_kind)
                .bind(&series.target_id)
                .bind(&series.metric_id)
                .bind(&series.source)
                .bind(&series.dimensions_hash)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r: sqlx::sqlite::SqliteRow| r.get::<i64, _>(0))
                .collect();
                for window_start in windows {
                    self.rollup_window(
                        &mut tx,
                        &series,
                        tier,
                        window_ms,
                        window_start,
                        &mut report,
                    )
                    .await?;
                }
                let _ = newest;
            }
        }

        // 2. Retention.
        let raw_cutoff = now_ms.saturating_sub(self.config.raw_retention_ms) as i64;
        let raw_deleted = sqlx::query("DELETE FROM monitoring_samples WHERE observed_at_ms < ?")
            .bind(raw_cutoff)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        report.raw_deleted = raw_deleted;

        let rollup_5m_cutoff = now_ms.saturating_sub(self.config.rollup_5m_retention_ms) as i64;
        report.rollup_5m_deleted =
            sqlx::query("DELETE FROM monitoring_rollups WHERE tier = '5m' AND window_start_ms < ?")
                .bind(rollup_5m_cutoff)
                .execute(&mut *tx)
                .await?
                .rows_affected();

        let rollup_1h_cutoff = now_ms.saturating_sub(self.config.rollup_1h_retention_ms) as i64;
        report.rollup_1h_deleted =
            sqlx::query("DELETE FROM monitoring_rollups WHERE tier = '1h' AND window_start_ms < ?")
                .bind(rollup_1h_cutoff)
                .execute(&mut *tx)
                .await?
                .rows_affected();

        let dedup_cutoff = now_ms.saturating_sub(self.config.dedup_retention_ms) as i64;
        report.dedup_deleted =
            sqlx::query("DELETE FROM monitoring_ingest_dedup WHERE received_at_ms < ?")
                .bind(dedup_cutoff)
                .execute(&mut *tx)
                .await?
                .rows_affected();

        // 3. Orphaned dimension sets: unreferenced by samples and
        //    series rows (series rows for evicted targets keep their
        //    dictionary entry; dropping the series row drops it here).
        report.dimension_sets_deleted = sqlx::query(
            "DELETE FROM monitoring_dimension_sets WHERE dimensions_hash NOT IN (
               SELECT DISTINCT dimensions_hash FROM monitoring_samples
               UNION
               SELECT DISTINCT dimensions_hash FROM monitoring_series
             )",
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // Series rows whose target stopped reporting long past raw
        // retention are forgotten (the query path then reports
        // no_history rather than an eternal stale row).
        sqlx::query("DELETE FROM monitoring_series WHERE last_observed_at_ms < ?")
            .bind(raw_cutoff)
            .execute(&mut *tx)
            .await?;

        // 4. Hard size budget: evict oldest raw data first, one day at
        //    a time, then oldest rollups — telemetry is disposable by
        //    design, and eviction is visible through this report.
        tx.commit().await?;
        let mut size = self.db_size_bytes().await?;
        if size > self.config.max_db_bytes {
            let mut tx = self.pool.begin().await?;
            loop {
                let oldest: Option<i64> =
                    sqlx::query_scalar("SELECT MIN(observed_at_ms) FROM monitoring_samples")
                        .fetch_one(&mut *tx)
                        .await?;
                let Some(oldest) = oldest else { break };
                let day_floor = (oldest / DAY_MS) * DAY_MS;
                let deleted =
                    sqlx::query("DELETE FROM monitoring_samples WHERE observed_at_ms < ?")
                        .bind(day_floor + DAY_MS)
                        .execute(&mut *tx)
                        .await?
                        .rows_affected();
                report.evicted_raw += deleted;
                if deleted == 0 {
                    break;
                }
                tx.commit().await?;
                tx = self.pool.begin().await?;
                size = self.db_size_bytes().await?;
                if size <= self.config.max_db_bytes {
                    break;
                }
            }
            tx.commit().await?;
        }

        // 5. WAL checkpoint (bounded WAL growth; TRUNCATE returns the
        //    file to minimal size).
        report.checkpointed = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0;

        // 6. Advance the pass cursor.
        sqlx::query(
            "INSERT INTO monitoring_meta (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(META_LAST_PASS)
        .bind(now_ms.to_string())
        .execute(&self.pool)
        .await?;

        Ok(report)
    }

    #[allow(clippy::too_many_arguments)]
    async fn rollup_window(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        series: &SeriesKey,
        tier: &str,
        window_ms: i64,
        window_start: i64,
        report: &mut MaintenanceReport,
    ) -> Result<(), MonitoringStoreError> {
        let rows = sqlx::query(
            "SELECT observed_at_ms, value_integer, value_real, quality, boot_id, identity_epoch
             FROM monitoring_samples
             WHERE target_kind = ? AND target_id = ? AND metric_id = ?
               AND source = ? AND dimensions_hash = ?
               AND observed_at_ms >= ? AND observed_at_ms < ?
             ORDER BY observed_at_ms ASC",
        )
        .bind(&series.target_kind)
        .bind(&series.target_id)
        .bind(&series.metric_id)
        .bind(&series.source)
        .bind(&series.dimensions_hash)
        .bind(window_start)
        .bind(window_start + window_ms)
        .fetch_all(&mut **tx)
        .await?;

        let mut valid_count = 0i64;
        let total_count = rows.len() as i64;
        let mut value_min = f64::INFINITY;
        let mut value_max = f64::NEG_INFINITY;
        let mut value_sum = 0.0f64;
        let mut counter_first: Option<i64> = None;
        let mut counter_last: Option<i64> = None;
        // Number of valid integer observations in the window: a delta
        // needs a real interval (two observations). A lone sample
        // carries no rate — emitting 0 would fabricate an idle window.
        let mut counter_obs = 0i64;
        let mut counter_delta: Option<i64> = None;
        let mut counter_epoch: Option<(String, String)> = None;
        // A reset is an epoch change between consecutive valid counter
        // points OR a non-monotonic step: either one makes the window's
        // delta honestly absent (never a negative or fabricated spike).
        let mut counter_reset = false;

        for row in &rows {
            let quality: String = row.get("quality");
            let value_integer: Option<i64> = row.get("value_integer");
            let value_real: Option<f64> = row.get("value_real");
            if quality != "valid" {
                continue;
            }
            valid_count += 1;
            // Gauges may be stored as exact integers (byte counts) or
            // floats — both are observations for min/max/sum.
            if let Some(v) = value_real.or_else(|| value_integer.map(|i| i as f64)) {
                value_min = value_min.min(v);
                value_max = value_max.max(v);
                value_sum += v;
            }
            if let Some(v) = value_integer {
                let boot_id: String = row.get("boot_id");
                let epoch: String = row.get("identity_epoch");
                let epoch_key = (boot_id, epoch);
                if let Some(prev_epoch) = counter_epoch.take() {
                    if prev_epoch != epoch_key {
                        counter_reset = true;
                    } else if let Some(prev_v) = counter_last {
                        if v < prev_v {
                            counter_reset = true;
                        }
                    }
                }
                counter_epoch = Some(epoch_key);
                if counter_first.is_none() {
                    counter_first = Some(v);
                }
                counter_last = Some(v);
                counter_obs += 1;
            }
        }
        if let (Some(f), Some(l)) = (counter_first, counter_last) {
            if !counter_reset && counter_obs >= 2 {
                counter_delta = Some(l - f);
            }
        }

        let (value_min, value_max, value_sum, value_count) = if valid_count > 0 {
            (
                Some(value_min),
                Some(value_max),
                Some(value_sum),
                Some(valid_count),
            )
        } else {
            (None, None, None, None)
        };

        if total_count == 0 {
            // Window emptied by retention/eviction between the touch
            // scan and now: drop any stale rollup row.
            sqlx::query(
                "DELETE FROM monitoring_rollups
                 WHERE target_kind = ? AND target_id = ? AND metric_id = ?
                   AND source = ? AND dimensions_hash = ? AND tier = ? AND window_start_ms = ?",
            )
            .bind(&series.target_kind)
            .bind(&series.target_id)
            .bind(&series.metric_id)
            .bind(&series.source)
            .bind(&series.dimensions_hash)
            .bind(tier)
            .bind(window_start)
            .execute(&mut **tx)
            .await?;
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO monitoring_rollups (
               target_kind, target_id, metric_id, source, dimensions_hash, tier,
               window_start_ms, window_ms, value_min, value_max, value_sum, value_count,
               counter_first, counter_last, counter_delta, valid_points, total_points
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(target_kind, target_id, metric_id, source, dimensions_hash, tier, window_start_ms)
             DO UPDATE SET
               window_ms = excluded.window_ms,
               value_min = excluded.value_min, value_max = excluded.value_max,
               value_sum = excluded.value_sum, value_count = excluded.value_count,
               counter_first = excluded.counter_first, counter_last = excluded.counter_last,
               counter_delta = excluded.counter_delta,
               valid_points = excluded.valid_points, total_points = excluded.total_points",
        )
        .bind(&series.target_kind)
        .bind(&series.target_id)
        .bind(&series.metric_id)
        .bind(&series.source)
        .bind(&series.dimensions_hash)
        .bind(tier)
        .bind(window_start)
        .bind(window_ms)
        .bind(value_min)
        .bind(value_max)
        .bind(value_sum)
        .bind(value_count)
        .bind(counter_first)
        .bind(counter_last)
        .bind(counter_delta)
        .bind(valid_count)
        .bind(total_count)
        .execute(&mut **tx)
        .await?;
        report.rollup_windows += 1;
        Ok(())
    }
}
