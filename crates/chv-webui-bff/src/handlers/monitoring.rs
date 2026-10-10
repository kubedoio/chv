//! Native monitoring read API (query/alerts contract v1, #602 PR-2).
//!
//! Boundaries (ADR-025): the browser talks only to this BFF; these
//! handlers are the sole monitoring surface. Every endpoint is
//! Viewer-role gated by the router middleware (role is checked before
//! any target is read or metric existence is presented); CHV v1 roles
//! are fleet-scoped, so resource visibility follows the operator's
//! fleet authorization.
//!
//! Honesty rules that hold across every response:
//! - Missing data is never zero: a non-valid point carries `quality`
//!   and no `value`; a missing series carries a `reason`.
//! - Counter integer values serialize as decimal strings (JSON numbers
//!   lose integer precision beyond 2^53).
//! - `monitoring_unavailable` means the monitoring subsystem is
//!   degraded — never that nodes or VMs are unhealthy.

use crate::auth::BearerToken;
use crate::router::AppState;
use crate::BffError;
use axum::{extract::State, response::Json};
use chv_monitoring_core::model::{SampleQuality, Source, TargetKind};
use chv_monitoring_core::registry;
use chv_monitoring_store::{
    CurrentSample, HistoryPoint, HistorySeries, MonitoringStore, SeriesReason,
    DEFAULT_MAX_POINTS_PER_SERIES,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

/// The overview cap from the query contract (at most 100 targets).
const MAX_OVERVIEW_TARGETS: usize = 100;

fn monitoring_store(state: &AppState) -> Result<&Arc<MonitoringStore>, BffError> {
    state.monitoring.as_ref().ok_or_else(|| {
        BffError::MonitoringUnavailable(
            "the monitoring subsystem is degraded or disabled; node and VM health are unaffected"
                .to_string(),
        )
    })
}

fn target_kind(raw: &str) -> Result<TargetKind, BffError> {
    raw.parse::<TargetKind>()
        .map_err(|_| BffError::MonitoringQuery {
            code: "invalid_target_kind".to_string(),
            message: format!(
                "target_kind must be one of node|vm|volume|network|check, got {raw:?}"
            ),
        })
}

/// Validate metric ids against the registry BEFORE touching the store
/// (the contract's `unknown_metric` error). The registry is static,
/// published data — listing it is not an authorization leak.
fn validate_metric_ids(metric_ids: &[String]) -> Result<(), BffError> {
    for id in metric_ids {
        if registry::lookup(id).is_none() {
            return Err(BffError::MonitoringQuery {
                code: "unknown_metric".to_string(),
                message: format!("unknown metric id {id:?}"),
            });
        }
    }
    Ok(())
}

/// Parse and registry-validate a source filter (the contract's
/// `unsupported_source` error).
fn validate_sources(metric_ids: &[String], raw: &[String]) -> Result<Vec<Source>, BffError> {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        let source = s.parse::<Source>().map_err(|_| BffError::MonitoringQuery {
            code: "unsupported_source".to_string(),
            message: format!("unknown source {s:?}"),
        })?;
        for id in metric_ids {
            let Some(def) = registry::lookup(id) else {
                continue;
            };
            if !def.allowed_sources.contains(&source) {
                return Err(BffError::MonitoringQuery {
                    code: "unsupported_source".to_string(),
                    message: format!("metric {id} does not allow source {s}"),
                });
            }
        }
        out.push(source);
    }
    Ok(out)
}

fn parse_resolution(raw: Option<&str>) -> Result<chv_monitoring_store::Resolution, BffError> {
    match raw {
        None | Some("auto") => Ok(chv_monitoring_store::Resolution::Auto),
        Some("raw") => Ok(chv_monitoring_store::Resolution::Raw),
        Some("5m") => Ok(chv_monitoring_store::Resolution::FiveMinute),
        Some("1h") => Ok(chv_monitoring_store::Resolution::OneHour),
        Some(other) => Err(BffError::MonitoringQuery {
            code: "invalid_range".to_string(),
            message: format!("resolution must be auto|raw|5m|1h, got {other:?}"),
        }),
    }
}

fn map_store_error(e: chv_monitoring_store::MonitoringStoreError) -> BffError {
    match e {
        chv_monitoring_store::MonitoringStoreError::QueryRejected { code, reason } => {
            BffError::MonitoringQuery {
                code: code.as_str().to_string(),
                message: reason,
            }
        }
        other => BffError::MonitoringUnavailable(other.to_string()),
    }
}

fn quality_str(q: &SampleQuality) -> &'static str {
    q.as_str()
}

/// `value` for gauges; `integer_value` (decimal string) for counters.
/// A non-valid point carries neither — never a zero.
fn point_json(point: &HistoryPoint) -> Value {
    let mut v = json!({
        "timestamp_ms": point.timestamp_ms,
        "window_ms": point.window_ms,
        "quality": quality_str(&point.quality),
    });
    if point.quality == SampleQuality::Valid {
        if let Some(i) = point.integer_value {
            // Counter deltas: exact integers on the wire as decimal
            // strings (JSON numbers lose precision beyond 2^53).
            v["integer_value"] = Value::String(i.to_string());
        }
        if let Some(f) = point.value {
            v["value"] = json!(f);
        }
    }
    v
}

fn current_value_json(sample: &CurrentSample) -> Value {
    let mut v = json!({
        "metric_id": sample.metric_id,
        "source": sample.source.as_str(),
        "dimensions": sample.dimensions,
        "kind": sample.kind.as_str(),
        "unit": sample.unit.as_str(),
        "observed_at_ms": sample.observed_at_ms,
        "received_at_ms": sample.received_at_ms,
        "quality": quality_str(&sample.quality),
        "stale": sample.stale,
    });
    if sample.quality == SampleQuality::Valid {
        if let Some(i) = sample.integer_value {
            // Counter values: exact integers on the wire as decimal
            // strings, consistently with history points (JSON numbers
            // lose precision beyond 2^53).
            v["integer_value"] = Value::String(i.to_string());
        }
        if let Some(f) = sample.value {
            v["value"] = json!(f);
        }
    }
    v
}

fn series_json(series: &HistorySeries) -> Value {
    json!({
        "metric_id": series.metric_id,
        "source": series.source.as_str(),
        "dimensions": series.dimensions,
        "kind": series.kind.as_str(),
        "unit": series.unit.as_str(),
        "points": series.points.iter().map(point_json).collect::<Vec<_>>(),
        "coverage_ratio": series.coverage_ratio,
        // Honest truncation: only a series whose points EXCEEDED the
        // ceiling and were thinned says true — a series merely AT the
        // ceiling is complete.
        "truncated": series.truncated,
        "reason": series.reason.as_ref().map(SeriesReason::as_str),
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `GET /v1/monitoring/catalog` — the metric registry: capabilities,
/// units, allowed sources (in preference order — the first allowed
/// source that has stored data is the most authoritative), registered
/// dimensions, and the quality vocabulary. Static contract data; no
/// store access, so it answers even while monitoring is degraded.
pub async fn catalog(
    crate::auth::BearerToken(_claims): BearerToken,
) -> Result<Json<Value>, BffError> {
    let metrics: Vec<Value> = registry::REGISTRY
        .iter()
        .map(|def| {
            json!({
                "metric_id": def.id,
                "kind": def.kind.as_str(),
                "unit": def.unit.as_str(),
                // Preference order: allowed_sources[0] is preferred.
                "sources": def.allowed_sources.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                "dimensions": def.dimensions,
            })
        })
        .collect();
    Ok(Json(json!({
        "schema_version": 1,
        "metrics": metrics,
        "qualities": [
            "valid", "insufficient_samples", "unsupported", "unavailable", "invalid", "stale"
        ],
        "series_reasons": ["unsupported", "not_collected", "no_history", "stale"],
    })))
}

#[derive(Deserialize)]
pub struct CurrentRequest {
    pub target_kind: String,
    pub target_id: String,
    #[serde(default)]
    pub metric_ids: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

/// `POST /v1/monitoring/current` — the latest authorized samples for
/// one target. `metric_ids` empty ⇒ every stored series of the target.
pub async fn current(
    crate::auth::BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<CurrentRequest>,
) -> Result<Json<Value>, BffError> {
    let store = monitoring_store(&state)?;
    let kind = target_kind(&payload.target_kind)?;
    validate_metric_ids(&payload.metric_ids)?;
    let sources = validate_sources(&payload.metric_ids, &payload.sources)?;
    let samples = store
        .query_current(
            &kind,
            &payload.target_id,
            &payload.metric_ids,
            if sources.is_empty() {
                None
            } else {
                Some(sources.as_slice())
            },
            now_ms(),
        )
        .await
        .map_err(map_store_error)?;
    let generated_at = now_ms();
    Ok(Json(json!({
        "schema_version": 1,
        "target_kind": payload.target_kind,
        "target_id": payload.target_id,
        "samples": samples.iter().map(current_value_json).collect::<Vec<_>>(),
        "generated_at_ms": generated_at,
    })))
}

#[derive(Deserialize)]
pub struct HistoryRequest {
    pub target_kind: String,
    pub target_id: String,
    pub metric_ids: Vec<String>,
    pub from_ms: u64,
    pub to_ms: u64,
    #[serde(default)]
    pub max_points_per_series: Option<usize>,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

/// `POST /v1/monitoring/history` — time-range samples with bounded
/// resolution for one target.
pub async fn history(
    crate::auth::BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<HistoryRequest>,
) -> Result<Json<Value>, BffError> {
    let store = monitoring_store(&state)?;
    let kind = target_kind(&payload.target_kind)?;
    if payload.metric_ids.is_empty() {
        return Err(BffError::MonitoringQuery {
            code: "query_too_large".to_string(),
            message: "history requires at least one metric_id".to_string(),
        });
    }
    validate_metric_ids(&payload.metric_ids)?;
    let sources = validate_sources(&payload.metric_ids, &payload.sources)?;
    let resolution = parse_resolution(payload.resolution.as_deref())?;
    let max_points = payload
        .max_points_per_series
        .unwrap_or(DEFAULT_MAX_POINTS_PER_SERIES);
    let series = store
        .query_history(
            &kind,
            &payload.target_id,
            &payload.metric_ids,
            if sources.is_empty() {
                None
            } else {
                Some(sources.as_slice())
            },
            payload.from_ms,
            payload.to_ms,
            max_points,
            resolution,
        )
        .await
        .map_err(map_store_error)?;
    // The contract: a missing series returns an empty points array and
    // a reason — a requested metric with no stored series for this
    // target is `not_collected` (the source never collected for this
    // target, or its history was evicted by retention), never silently
    // absent. Kind/unit come from the registry; no source is claimed
    // for data that does not exist.
    let mut series_json: Vec<Value> = series.iter().map(series_json).collect();
    for metric_id in &payload.metric_ids {
        if series.iter().any(|s| &s.metric_id == metric_id) {
            continue;
        }
        let def = registry::lookup(metric_id);
        series_json.push(json!({
            "metric_id": metric_id,
            "source": null,
            "kind": def.map(|d| d.kind.as_str()),
            "unit": def.map(|d| d.unit.as_str()),
            "points": [],
            "coverage_ratio": 0.0,
            "truncated": false,
            "reason": "not_collected",
        }));
    }
    let truncated = series.iter().any(|s| s.truncated);
    Ok(Json(json!({
        "schema_version": 1,
        "target_kind": payload.target_kind,
        "target_id": payload.target_id,
        "series": series_json,
        "generated_at_ms": now_ms(),
        "truncated": truncated,
    })))
}

#[derive(Deserialize)]
pub struct OverviewRequest {
    pub target_kind: String,
    #[serde(default)]
    pub target_ids: Vec<String>,
    #[serde(default)]
    pub metric_ids: Vec<String>,
}

/// The default summary metric set per family (registry gauges that
/// describe measured capacity and usage; no derived-only metrics —
/// those are computed at render time, not stored).
fn default_overview_metrics(kind: &TargetKind) -> &'static [&'static str] {
    match kind {
        TargetKind::Node => &[
            "node.cpu.capacity_ratio",
            "node.cpu.load1",
            "node.memory.total_bytes",
            "node.memory.available_bytes",
            "node.swap.used_bytes",
        ],
        _ => &["vm.cpu.cores_used", "vm.memory.host_accounted_bytes"],
    }
}

/// `POST /v1/monitoring/overview` — latest measured resource summaries
/// for up to 100 targets of one kind. When `target_ids` is empty the
/// server enumerates the targets that have stored series (bounded by
/// the same cap).
pub async fn overview(
    crate::auth::BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<OverviewRequest>,
) -> Result<Json<Value>, BffError> {
    let store = monitoring_store(&state)?;
    let kind = target_kind(&payload.target_kind)?;
    if payload.target_ids.len() > MAX_OVERVIEW_TARGETS {
        return Err(BffError::MonitoringQuery {
            code: "query_too_large".to_string(),
            message: format!("at most {MAX_OVERVIEW_TARGETS} targets per overview request"),
        });
    }
    let metric_ids: Vec<String> = if payload.metric_ids.is_empty() {
        default_overview_metrics(&kind)
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        payload.metric_ids.clone()
    };
    validate_metric_ids(&metric_ids)?;

    let target_ids = if payload.target_ids.is_empty() {
        store
            .list_targets(&kind, MAX_OVERVIEW_TARGETS)
            .await
            .map_err(map_store_error)?
    } else {
        payload.target_ids.clone()
    };

    let now = now_ms();
    let mut targets = Vec::with_capacity(target_ids.len());
    for target_id in &target_ids {
        let samples = store
            .query_current(&kind, target_id, &metric_ids, None, now)
            .await
            .map_err(map_store_error)?;
        let sample_values: Vec<Value> = samples.iter().map(current_value_json).collect();
        // The FRESHEST sample's age answers "is anything reporting for
        // this target?": one long-dead series among fresh ones must not
        // paint the whole target stale (worst-case age is visible
        // per-sample via each sample's own observed_at_ms).
        let age_seconds = samples
            .iter()
            .map(|s| now.saturating_sub(s.observed_at_ms) / 1000)
            .min();
        targets.push(json!({
            "target_id": target_id,
            "samples": sample_values,
            "age_seconds": age_seconds,
        }));
    }
    Ok(Json(json!({
        "schema_version": 1,
        "target_kind": payload.target_kind,
        "targets": targets,
        "generated_at_ms": now,
    })))
}

/// `GET /v1/monitoring/health` — monitoring subsystem availability,
/// age and loss counters. Distinguishes "monitoring degraded" from
/// node/VM health in both directions: this endpoint answers even when
/// the store is unavailable (it reports the degraded reason).
pub async fn health(
    crate::auth::BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    let snapshot = state.monitoring_health.snapshot();
    Ok(Json(json!({
        "schema_version": 1,
        "available": state.monitoring.is_some(),
        "degraded_reason": snapshot.degraded_reason,
        "last_ingest_at_ms": snapshot.last_ingest_at_ms,
        "last_maintenance_at_ms": snapshot.last_maintenance_at_ms,
        "accepted_batches": snapshot.accepted_batches,
        "duplicate_batches": snapshot.duplicate_batches,
        "rejected_batches": snapshot.rejected_batches,
        "unavailable_batches": snapshot.unavailable_batches,
        "headroom_bytes": snapshot.headroom_bytes,
        // True when the last headroom probe could not read the
        // filesystem: ingestion fails open in that window, but the
        // headroom floor is unverified until the next successful probe.
        "headroom_probe_failed": snapshot.headroom_probe_failed,
        "raw_samples": snapshot.raw_samples,
        "generated_at_ms": now_ms(),
    })))
}
