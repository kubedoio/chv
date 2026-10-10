use crate::db::MonitoringStore;
use crate::error::{MonitoringStoreError, QueryRejection};
use chv_monitoring_core::model::{MetricKind, SampleQuality, Source, TargetKind, Unit};
use sqlx::Row;
use std::collections::BTreeMap;

/// Requested resolution for a history query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Server picks by range and retention: raw for short ranges, 5m
    /// rollups up to the detailed-query limit, 1h beyond.
    Auto,
    Raw,
    FiveMinute,
    OneHour,
}

impl Resolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            Resolution::Auto => "auto",
            Resolution::Raw => "raw",
            Resolution::FiveMinute => "5m",
            Resolution::OneHour => "1h",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto" => Resolution::Auto,
            "raw" => Resolution::Raw,
            "5m" => Resolution::FiveMinute,
            "1h" => Resolution::OneHour,
            _ => return None,
        })
    }
}

/// One point of a history series. `value` is absent for non-valid
/// points — never a zero, NaN, or interpolated filler.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryPoint {
    pub timestamp_ms: u64,
    /// Window the point covers (raw points cover one bucket; rollup
    /// points cover the rollup window). Counter points carry the
    /// same-epoch delta over the window; the UI divides by `window_ms`
    /// for a rate.
    pub window_ms: u64,
    pub value: Option<f64>,
    /// For counter points: the exact integer delta when it is exact
    /// (the JSON BFF wire emits it as a decimal string beyond 2^53).
    pub integer_value: Option<i64>,
    pub quality: SampleQuality,
}

/// One series in a history response, labeled by source.
#[derive(Clone, Debug)]
pub struct HistorySeries {
    pub metric_id: String,
    pub source: Source,
    pub dimensions: BTreeMap<String, String>,
    pub kind: MetricKind,
    pub unit: Unit,
    pub points: Vec<HistoryPoint>,
    /// Valid points / total points in the requested range (0.0 when no
    /// points at all).
    pub coverage_ratio: f64,
    /// Whether thinning dropped points to honor the caller's
    /// `max_points` ceiling — an honesty signal for the wire contract
    /// (a series AT the ceiling is not necessarily truncated; only a
    /// series that EXCEEDED it is).
    pub truncated: bool,
    /// Why the series has no stored data, when it has none (the
    /// contract's single absence vocabulary).
    pub reason: Option<SeriesReason>,
}

/// The wire absence classification (query/alerts contract v1): one
/// mapping, not three vocabularies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesReason {
    /// The source layer does not implement this metric.
    Unsupported,
    /// Source down or never collected.
    NotCollected,
    /// Nothing stored in the requested range.
    NoHistory,
    /// Latest stored observation is too old to present as current.
    Stale,
}

impl SeriesReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            SeriesReason::Unsupported => "unsupported",
            SeriesReason::NotCollected => "not_collected",
            SeriesReason::NoHistory => "no_history",
            SeriesReason::Stale => "stale",
        }
    }
}

/// The latest sample of one series (the `/current` surface).
#[derive(Clone, Debug)]
pub struct CurrentSample {
    pub metric_id: String,
    pub source: Source,
    pub dimensions: BTreeMap<String, String>,
    pub kind: MetricKind,
    pub unit: Unit,
    pub observed_at_ms: u64,
    pub received_at_ms: u64,
    pub value: Option<f64>,
    pub integer_value: Option<i64>,
    pub quality: SampleQuality,
    /// Server-side staleness decision (metric-family thresholds).
    pub stale: bool,
}

/// Query limits (query/alerts contract v1 defaults).
pub const MAX_METRIC_IDS_PER_QUERY: usize = 8;
pub const MAX_POINTS_PER_SERIES: usize = 1000;
pub const DEFAULT_MAX_POINTS_PER_SERIES: usize = 240;
/// Detailed (raw/5m) query range ceiling; beyond this only 1h rollups.
pub const MAX_DETAILED_RANGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;
/// Aggregated view ceiling (1h rollup retention horizon).
pub const MAX_AGGREGATED_RANGE_MS: u64 = 180 * 24 * 60 * 60 * 1000;

const ROLLUP_5M_MS: i64 = 5 * 60 * 1000;
const ROLLUP_1H_MS: i64 = 60 * 60 * 1000;

/// Staleness thresholds by target family (3× the family's default
/// collection cadence, floored at one minute).
fn stale_after_ms(kind: &TargetKind) -> u64 {
    match kind {
        TargetKind::Node => 60_000,
        _ => 90_000,
    }
}

/// A raw stored point (internal representation before bucketing).
#[derive(Clone, Debug)]
struct RawPoint {
    observed_at_ms: i64,
    value_integer: Option<i64>,
    value_real: Option<f64>,
    quality: String,
    boot_id: String,
    identity_epoch: String,
}

impl MonitoringStore {
    /// History for one target: one series per (metric, source,
    /// dimension set) that has stored data in the range, bucketed to at
    /// most `max_points` points per series. Non-valid points are
    /// excluded from gauge numerators and denominators alike; counter
    /// points are same-epoch deltas only — resets and epoch crossings
    /// emit no value for the crossing bucket.
    #[allow(clippy::too_many_arguments)]
    pub async fn query_history(
        &self,
        target_kind: &TargetKind,
        target_id: &str,
        metric_ids: &[String],
        sources: Option<&[Source]>,
        from_ms: u64,
        to_ms: u64,
        max_points: usize,
        resolution: Resolution,
    ) -> Result<Vec<HistorySeries>, MonitoringStoreError> {
        if metric_ids.is_empty() || metric_ids.len() > MAX_METRIC_IDS_PER_QUERY {
            return Err(MonitoringStoreError::QueryRejected {
                code: QueryRejection::QueryTooLarge,
                reason: format!(
                    "between 1 and {} metric ids required",
                    MAX_METRIC_IDS_PER_QUERY
                ),
            });
        }
        if from_ms >= to_ms {
            return Err(MonitoringStoreError::QueryRejected {
                code: QueryRejection::InvalidRange,
                reason: "from_ms must be before to_ms".to_string(),
            });
        }
        let range = to_ms - from_ms;
        if range > MAX_AGGREGATED_MS_PUBLIC {
            return Err(MonitoringStoreError::QueryRejected {
                code: QueryRejection::QueryTooLarge,
                reason: format!("range exceeds the {MAX_AGGREGATED_MS_PUBLIC} ms ceiling"),
            });
        }
        let max_points = max_points.clamp(1, MAX_POINTS_PER_SERIES);

        // Resolution selection: raw only within raw retention; 5m up to
        // the detailed ceiling; 1h beyond (auto). An explicit raw/5m
        // request beyond its ceiling is rejected, not silently widened.
        let tier = match resolution {
            Resolution::Auto => {
                if range <= self.config.raw_retention_ms {
                    QueryTier::Raw
                } else if range <= MAX_DETAILED_RANGE_MS {
                    QueryTier::Rollup5m
                } else {
                    QueryTier::Rollup1h
                }
            }
            Resolution::Raw => {
                if range > self.config.raw_retention_ms {
                    return Err(MonitoringStoreError::QueryRejected {
                        code: QueryRejection::InvalidRange,
                        reason: format!(
                            "raw resolution is limited to the {} ms raw retention",
                            self.config.raw_retention_ms
                        ),
                    });
                }
                QueryTier::Raw
            }
            Resolution::FiveMinute => {
                if range > MAX_DETAILED_RANGE_MS {
                    return Err(MonitoringStoreError::QueryRejected {
                        code: QueryRejection::InvalidRange,
                        reason: "5m resolution is limited to the 30-day detailed ceiling"
                            .to_string(),
                    });
                }
                QueryTier::Rollup5m
            }
            Resolution::OneHour => QueryTier::Rollup1h,
        };

        // Series metadata rows for the requested metrics/sources.
        let mut series_rows = sqlx::query(
            "SELECT metric_id, source, dimensions_hash, kind, unit
             FROM monitoring_series
             WHERE target_kind = ? AND target_id = ?",
        )
        .bind(target_kind.as_str())
        .bind(target_id)
        .fetch_all(&self.pool)
        .await?;
        series_rows.retain(|r| {
            let mid: &str = r.get("metric_id");
            metric_ids.iter().any(|m| m == mid)
                && sources
                    .map(|ss| {
                        let src: &str = r.get("source");
                        ss.iter().any(|s| s.as_str() == src)
                    })
                    .unwrap_or(true)
        });

        let dimension_sets = self.load_dimension_sets().await?;
        let mut out = Vec::with_capacity(series_rows.len());
        for row in series_rows {
            let metric_id: String = row.get("metric_id");
            let source_str: String = row.get("source");
            let dimensions_hash: String = row.get("dimensions_hash");
            let kind: MetricKind = row
                .get::<String, _>("kind")
                .parse()
                .map_err(|e| MonitoringStoreError::Degraded { reason: e })?;
            let unit: Unit = row
                .get::<String, _>("unit")
                .parse()
                .map_err(|e| MonitoringStoreError::Degraded { reason: e })?;
            let Ok(source) = source_str.parse::<Source>() else {
                continue;
            };
            let dimensions = dimension_sets
                .get(&dimensions_hash)
                .cloned()
                .unwrap_or_default();

            let (points, coverage, truncated) = match tier {
                QueryTier::Raw => {
                    let raw = self
                        .fetch_raw(
                            target_kind,
                            target_id,
                            &metric_id,
                            source.as_str(),
                            &dimensions_hash,
                            from_ms,
                            to_ms,
                        )
                        .await?;
                    // Coverage counts raw valid points over all raw
                    // points in range — the bucketing below may fold a
                    // mixed bucket into a single valid point, and that
                    // folding must not inflate the honesty signal.
                    let valid_raw = raw.iter().filter(|p| p.quality == "valid").count();
                    let coverage = if raw.is_empty() {
                        0.0
                    } else {
                        valid_raw as f64 / raw.len() as f64
                    };
                    let (bucketed, truncated) = bucket_raw(raw, from_ms, to_ms, max_points, &kind);
                    (bucketed, coverage, truncated)
                }
                QueryTier::Rollup5m | QueryTier::Rollup1h => {
                    let tier_str = match tier {
                        QueryTier::Rollup5m => "5m",
                        _ => "1h",
                    };
                    let window_ms = match tier {
                        QueryTier::Rollup5m => ROLLUP_5M_MS,
                        _ => ROLLUP_1H_MS,
                    };
                    let rollups = self
                        .fetch_rollups(
                            target_kind,
                            target_id,
                            &metric_id,
                            source.as_str(),
                            &dimensions_hash,
                            tier_str,
                            from_ms,
                            to_ms,
                        )
                        .await?;
                    let valid: i64 = rollups.iter().map(|r| r.valid_points).sum();
                    let total: i64 = rollups.iter().map(|r| r.total_points).sum();
                    let coverage = if total > 0 {
                        valid as f64 / total as f64
                    } else {
                        0.0
                    };
                    let mut points: Vec<HistoryPoint> = rollups
                        .into_iter()
                        .map(|r| rollup_point(r, window_ms.unsigned_abs(), &kind))
                        .collect();
                    let truncated = points.len() > max_points;
                    if truncated {
                        points = thin_to_max(points, max_points);
                    }
                    (points, coverage, truncated)
                }
            };

            let reason = if points.is_empty() {
                Some(
                    self.classify_absence(target_kind, target_id, &metric_id)
                        .await?,
                )
            } else {
                None
            };

            out.push(HistorySeries {
                metric_id,
                source,
                dimensions,
                kind,
                unit,
                points,
                coverage_ratio: coverage,
                truncated,
                reason,
            });
        }
        Ok(out)
    }

    /// Latest sample per series for one target, with server-side
    /// staleness decisions. A series whose raw points have been evicted
    /// by retention still reports — with `stale` quality and no value —
    /// so "current" never fabricates a reading and never silently
    /// forgets a target.
    /// The distinct target ids of one kind that have any stored series
    /// (bounded by `limit`). The `/overview` surface uses this to
    /// enumerate targets when the request does not narrow them; the
    /// bound is the contract's 100-target overview cap.
    pub async fn list_targets(
        &self,
        target_kind: &TargetKind,
        limit: usize,
    ) -> Result<Vec<String>, MonitoringStoreError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT target_id FROM monitoring_series \
             WHERE target_kind = ? ORDER BY target_id LIMIT ?",
        )
        .bind(target_kind.as_str())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn query_current(
        &self,
        target_kind: &TargetKind,
        target_id: &str,
        metric_ids: &[String],
        sources: Option<&[Source]>,
        now_ms: u64,
    ) -> Result<Vec<CurrentSample>, MonitoringStoreError> {
        if metric_ids.len() > MAX_METRIC_IDS_PER_QUERY {
            return Err(MonitoringStoreError::QueryRejected {
                code: QueryRejection::QueryTooLarge,
                reason: format!("at most {} metric ids per query", MAX_METRIC_IDS_PER_QUERY),
            });
        }
        let rows = sqlx::query(
            "SELECT s.metric_id, s.source, s.dimensions_hash, s.kind, s.unit, s.last_observed_at_ms
             FROM monitoring_series s
             WHERE s.target_kind = ? AND s.target_id = ?",
        )
        .bind(target_kind.as_str())
        .bind(target_id)
        .fetch_all(&self.pool)
        .await?;
        let dimension_sets = self.load_dimension_sets().await?;
        let threshold = stale_after_ms(target_kind);
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let metric_id: String = row.get("metric_id");
            if !metric_ids.is_empty() && !metric_ids.contains(&metric_id) {
                continue;
            }
            let source_str: String = row.get("source");
            if let Some(ss) = sources {
                if !ss.iter().any(|s| s.as_str() == source_str) {
                    continue;
                }
            }
            let Ok(source) = source_str.parse::<Source>() else {
                continue;
            };
            let dimensions_hash: String = row.get("dimensions_hash");
            let kind: MetricKind = row
                .get::<String, _>("kind")
                .parse()
                .map_err(|e| MonitoringStoreError::Degraded { reason: e })?;
            let unit: Unit = row
                .get::<String, _>("unit")
                .parse()
                .map_err(|e| MonitoringStoreError::Degraded { reason: e })?;
            let last_observed: i64 = row.get("last_observed_at_ms");

            let latest = sqlx::query(
                "SELECT observed_at_ms, received_at_ms, value_integer, value_real, quality
                 FROM monitoring_samples
                 WHERE target_kind = ? AND target_id = ? AND metric_id = ?
                   AND source = ? AND dimensions_hash = ?
                 ORDER BY observed_at_ms DESC LIMIT 1",
            )
            .bind(target_kind.as_str())
            .bind(target_id)
            .bind(&metric_id)
            .bind(&source_str)
            .bind(&dimensions_hash)
            .fetch_optional(&self.pool)
            .await?;

            let dimensions = dimension_sets
                .get(&dimensions_hash)
                .cloned()
                .unwrap_or_default();
            let stale = now_ms.saturating_sub(last_observed.unsigned_abs()) > threshold;

            let sample = match latest {
                Some(latest) => {
                    let observed: i64 = latest.get("observed_at_ms");
                    let received: i64 = latest.get("received_at_ms");
                    let quality_str: String = latest.get("quality");
                    let quality = SampleQuality::parse(&quality_str).ok_or_else(|| {
                        MonitoringStoreError::Degraded {
                            reason: format!("stored quality {quality_str:?} is unknown"),
                        }
                    })?;
                    let value_integer: Option<i64> = latest.get("value_integer");
                    let value_real: Option<f64> = latest.get("value_real");
                    CurrentSample {
                        metric_id: metric_id.clone(),
                        source,
                        dimensions,
                        kind,
                        unit,
                        observed_at_ms: observed.unsigned_abs(),
                        received_at_ms: received.unsigned_abs(),
                        value: value_real.or_else(|| value_integer.map(|v| v as f64)),
                        integer_value: value_integer,
                        quality,
                        stale,
                    }
                }
                None => CurrentSample {
                    metric_id: metric_id.clone(),
                    source,
                    dimensions,
                    kind,
                    unit,
                    observed_at_ms: last_observed.unsigned_abs(),
                    received_at_ms: 0,
                    value: None,
                    integer_value: None,
                    quality: SampleQuality::Stale,
                    stale: true,
                },
            };
            out.push(sample);
        }
        Ok(out)
    }

    /// Why a series has no points in the requested range: observations
    /// exist elsewhere in time (`stale`) or nowhere (`no_history`).
    async fn classify_absence(
        &self,
        target_kind: &TargetKind,
        target_id: &str,
        metric_id: &str,
    ) -> Result<SeriesReason, MonitoringStoreError> {
        let any: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(observed_at_ms) FROM monitoring_samples
             WHERE target_kind = ? AND target_id = ? AND metric_id = ?",
        )
        .bind(target_kind.as_str())
        .bind(target_id)
        .bind(metric_id)
        .fetch_optional(&self.pool)
        .await?;
        match any {
            Some(_) => Ok(SeriesReason::Stale),
            None => Ok(SeriesReason::NoHistory),
        }
    }

    async fn load_dimension_sets(
        &self,
    ) -> Result<BTreeMap<String, BTreeMap<String, String>>, MonitoringStoreError> {
        let rows =
            sqlx::query("SELECT dimensions_hash, dimensions_json FROM monitoring_dimension_sets")
                .fetch_all(&self.pool)
                .await?;
        let mut map = BTreeMap::new();
        for row in rows {
            let hash: String = row.get("dimensions_hash");
            let json: String = row.get("dimensions_json");
            if let Ok(dims) = serde_json::from_str::<BTreeMap<String, String>>(&json) {
                map.insert(hash, dims);
            }
        }
        Ok(map)
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_raw(
        &self,
        target_kind: &TargetKind,
        target_id: &str,
        metric_id: &str,
        source: &str,
        dimensions_hash: &str,
        from_ms: u64,
        to_ms: u64,
    ) -> Result<Vec<RawPoint>, MonitoringStoreError> {
        let rows = sqlx::query(
            "SELECT observed_at_ms, value_integer, value_real, quality, boot_id, identity_epoch
             FROM monitoring_samples
             WHERE target_kind = ? AND target_id = ? AND metric_id = ?
               AND source = ? AND dimensions_hash = ?
               AND observed_at_ms >= ? AND observed_at_ms <= ?
             ORDER BY observed_at_ms ASC",
        )
        .bind(target_kind.as_str())
        .bind(target_id)
        .bind(metric_id)
        .bind(source)
        .bind(dimensions_hash)
        .bind(from_ms as i64)
        .bind(to_ms as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| RawPoint {
                observed_at_ms: r.get("observed_at_ms"),
                value_integer: r.get("value_integer"),
                value_real: r.get("value_real"),
                quality: r.get("quality"),
                boot_id: r.get("boot_id"),
                identity_epoch: r.get("identity_epoch"),
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_rollups(
        &self,
        target_kind: &TargetKind,
        target_id: &str,
        metric_id: &str,
        source: &str,
        dimensions_hash: &str,
        tier: &str,
        from_ms: u64,
        to_ms: u64,
    ) -> Result<Vec<StoredRollup>, MonitoringStoreError> {
        let rows = sqlx::query(
            "SELECT window_start_ms, value_sum, value_count,
                    counter_delta, valid_points, total_points
             FROM monitoring_rollups
             WHERE target_kind = ? AND target_id = ? AND metric_id = ?
               AND source = ? AND dimensions_hash = ? AND tier = ?
               AND window_start_ms >= ? AND window_start_ms <= ?
             ORDER BY window_start_ms ASC",
        )
        .bind(target_kind.as_str())
        .bind(target_id)
        .bind(metric_id)
        .bind(source)
        .bind(dimensions_hash)
        .bind(tier)
        .bind(from_ms as i64)
        .bind(to_ms as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| StoredRollup {
                window_start_ms: r.get("window_start_ms"),
                value_sum: r.get("value_sum"),
                value_count: r.get("value_count"),
                counter_delta: r.get("counter_delta"),
                valid_points: r.get("valid_points"),
                total_points: r.get("total_points"),
            })
            .collect())
    }
}

/// Public alias for the aggregated-range ceiling used in error text.
pub const MAX_AGGREGATED_MS_PUBLIC: u64 = MAX_AGGREGATED_RANGE_MS;

#[derive(Clone, Debug)]
struct StoredRollup {
    window_start_ms: i64,
    value_sum: Option<f64>,
    value_count: Option<i64>,
    counter_delta: Option<i64>,
    valid_points: i64,
    total_points: i64,
}

enum QueryTier {
    Raw,
    Rollup5m,
    Rollup1h,
}

fn rollup_point(r: StoredRollup, window_ms: u64, kind: &MetricKind) -> HistoryPoint {
    match kind {
        MetricKind::Counter => {
            // A counter window is Valid only when it carries a real
            // observed interval (a delta): a reset, an epoch crossing,
            // or a lone sample in the window leaves the rate honestly
            // absent — a "Valid" point without a value would teach
            // consumers that Valid means has-a-number.
            let quality = if r.counter_delta.is_some() {
                SampleQuality::Valid
            } else {
                SampleQuality::Unavailable
            };
            HistoryPoint {
                timestamp_ms: r.window_start_ms.unsigned_abs(),
                window_ms,
                value: r.counter_delta.map(|d| d as f64),
                integer_value: r.counter_delta,
                quality,
            }
        }
        _ => {
            let mean = r
                .value_sum
                .zip(r.value_count)
                .filter(|(_, c)| *c > 0)
                .map(|(s, c)| s / c as f64);
            // Same rule as counters: Valid means the point carries a
            // number. A window whose valid samples left no aggregate
            // (e.g. every point non-valid) is an honest absence, not a
            // "valid" hole. Windows with zero samples never get a
            // rollup row (the maintenance pass deletes those), so
            // reaching here means the window was observed.
            let quality = if mean.is_some() {
                SampleQuality::Valid
            } else {
                SampleQuality::Unavailable
            };
            HistoryPoint {
                timestamp_ms: r.window_start_ms.unsigned_abs(),
                window_ms,
                value: mean,
                integer_value: None,
                quality,
            }
        }
    }
}

/// Bucket raw points to at most `max_points` buckets. Gauges: mean of
/// valid points per bucket (integer- and float-stored values both
/// aggregate; non-valid points are excluded from numerator and
/// denominator). Counters: the same-epoch delta between consecutive
/// buckets' last valid values — a reset or epoch crossing emits no
/// value for the crossing bucket, never a negative — and each point's
/// `window_ms` is the real time between the two samples it subtracts,
/// so a delta that crosses an ingestion gap is rated against the gap,
/// not against a single bucket.
fn bucket_raw(
    raw: Vec<RawPoint>,
    from_ms: u64,
    to_ms: u64,
    max_points: usize,
    kind: &MetricKind,
) -> (Vec<HistoryPoint>, bool) {
    if raw.is_empty() {
        return (Vec::new(), false);
    }
    let range = (to_ms - from_ms).max(1);
    let bucket_ms = (range / max_points.max(1) as u64).max(1);
    let n_buckets = ((range / bucket_ms) + 1) as usize;

    let bucket_of = |t: i64| {
        ((((t - from_ms as i64).max(0) as u64) / bucket_ms).min(n_buckets.saturating_sub(1) as u64))
            as usize
    };

    let mut out = Vec::new();
    match kind {
        MetricKind::Counter => {
            // Per bucket: last valid value, its epoch, and its timestamp.
            let mut last_val = vec![None::<i64>; n_buckets];
            let mut last_epoch = vec![None::<(String, String)>; n_buckets];
            let mut last_ts = vec![None::<i64>; n_buckets];
            let mut has_nonvalid = vec![false; n_buckets];
            for p in &raw {
                let idx = bucket_of(p.observed_at_ms);
                if let Some(v) = p.value_integer {
                    last_val[idx] = Some(v);
                    last_epoch[idx] = Some((p.boot_id.clone(), p.identity_epoch.clone()));
                    last_ts[idx] = Some(p.observed_at_ms);
                } else {
                    has_nonvalid[idx] = true;
                }
            }
            // `prev` carries the previous bucket's last value AND its
            // timestamp: the delta between two bucket-last values spans
            // the real time between those samples (which crosses any
            // ingestion gap), so the point's `window_ms` must be that
            // span — attributing a gap-crossing delta to a single
            // bucket would inflate the rate by the gap's length.
            let mut prev: Option<(i64, (String, String), i64)> = None;
            for idx in 0..n_buckets {
                if let (Some(v), Some(ep), Some(ts)) =
                    (last_val[idx], last_epoch[idx].clone(), last_ts[idx])
                {
                    let point = match &prev {
                        Some((pv, pep, pts)) if pep == &ep => {
                            let delta = v - pv;
                            if delta >= 0 {
                                // The rate's time base is the observed
                                // span between the two samples (which
                                // crosses any ingestion gap), never the
                                // nominal bucket width — attributing a
                                // gap-crossing delta to one bucket
                                // would inflate the rate.
                                let span = (ts - pts).max(1) as u64;
                                Some((Some(delta), SampleQuality::Valid, span))
                            } else {
                                // Reset inside the bucket: no value.
                                Some((None, SampleQuality::Unavailable, bucket_ms))
                            }
                        }
                        _ if has_nonvalid[idx] => {
                            Some((None, SampleQuality::Unavailable, bucket_ms))
                        }
                        _ => None,
                    };
                    if let Some((delta, quality, span)) = point {
                        out.push(HistoryPoint {
                            timestamp_ms: from_ms + (idx as u64) * bucket_ms,
                            window_ms: span,
                            value: delta.map(|d| d as f64),
                            integer_value: delta,
                            quality,
                        });
                    }
                    prev = Some((v, ep, ts));
                } else if has_nonvalid[idx] {
                    out.push(HistoryPoint {
                        timestamp_ms: from_ms + (idx as u64) * bucket_ms,
                        window_ms: bucket_ms,
                        value: None,
                        integer_value: None,
                        quality: SampleQuality::Unavailable,
                    });
                }
            }
        }
        _ => {
            let mut sums = vec![(0.0f64, 0usize); n_buckets];
            let mut nonvalid: Vec<usize> = vec![0; n_buckets];
            for p in &raw {
                let idx = bucket_of(p.observed_at_ms);
                // Gauges may be stored as exact integers (byte counts)
                // or floats — either is a valid observation for the
                // mean; only a non-valid quality is excluded.
                match p.value_real.or_else(|| p.value_integer.map(|v| v as f64)) {
                    Some(v) => {
                        sums[idx].0 += v;
                        sums[idx].1 += 1;
                    }
                    None => nonvalid[idx] += 1,
                }
            }
            for idx in 0..n_buckets {
                let (sum, count) = sums[idx];
                if count > 0 {
                    out.push(HistoryPoint {
                        timestamp_ms: from_ms + (idx as u64) * bucket_ms,
                        window_ms: bucket_ms,
                        value: Some(sum / count as f64),
                        integer_value: None,
                        quality: SampleQuality::Valid,
                    });
                } else if nonvalid[idx] > 0 {
                    out.push(HistoryPoint {
                        timestamp_ms: from_ms + (idx as u64) * bucket_ms,
                        window_ms: bucket_ms,
                        value: None,
                        integer_value: None,
                        quality: SampleQuality::Unavailable,
                    });
                }
            }
        }
    }
    let truncated = out.len() > max_points;
    if truncated {
        out = thin_to_max(out, max_points);
    }
    (out, truncated)
}

/// Deterministic thinning that preserves order and endpoints.
fn thin_to_max(points: Vec<HistoryPoint>, max_points: usize) -> Vec<HistoryPoint> {
    if points.len() <= max_points {
        return points;
    }
    if max_points <= 1 {
        // A one-point ceiling keeps the newest observation — the
        // naive push-first-then-last shape below would return TWO
        // points for a one-point cap.
        return vec![points[points.len() - 1].clone()];
    }
    let mut thinned = Vec::with_capacity(max_points);
    thinned.push(points[0].clone());
    let step = (points.len() - 1) as f64 / (max_points - 1).max(1) as f64;
    let mut next = step;
    while thinned.len() < max_points - 1 {
        let idx = (next.round() as usize).min(points.len() - 1);
        thinned.push(points[idx].clone());
        next += step;
    }
    thinned.push(points[points.len() - 1].clone());
    thinned
}
