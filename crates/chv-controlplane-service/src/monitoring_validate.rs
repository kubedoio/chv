//! Transport-neutral v1 sample validation shared by the node batch
//! (gRPC, PR-2) and guest agent (HTTPS, G3) ingestion paths.
//!
//! Both transports MUST enforce the same contract — a metric that is
//! invalid on one wire is invalid on the other. The only validation
//! that differs is sender/target ownership, which is transport
//! specific (mTLS node identity vs enrolled agent credential binding)
//! and stays with the callers. Everything else — registry membership,
//! source allowlists, kind/unit agreement, value/quality consistency,
//! integer counters, timestamp bounds, dimension rules, epoch
//! fencing — lives here exactly once.

use chv_monitoring_core::model::{
    MetricKind, Sample, SampleBuilder, SampleQuality, SampleValue, Source, TargetKind, Unit,
};
use chv_monitoring_core::registry;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Contract caps (ingestion v1 initial defaults).
pub const MAX_SAMPLES_PER_BATCH: usize = 512;
pub const MAX_SAMPLE_AGE_MS: i64 = 5 * 60 * 1000;
pub const MAX_FUTURE_SKEW_MS: i64 = 2 * 60 * 1000;
pub const MAX_BOOT_ID_BYTES: usize = 128;

/// Wire outcome vocabulary (ingestion contract v1).
pub const OUTCOME_ACCEPTED: &str = "accepted";
pub const OUTCOME_DUPLICATE: &str = "duplicate";
pub const OUTCOME_REPLAY_CONFLICT: &str = "replay_conflict";
pub const OUTCOME_INVALID_BATCH: &str = "invalid_batch";
pub const OUTCOME_BATCH_TOO_LARGE: &str = "batch_too_large";
pub const OUTCOME_UNSUPPORTED_METRIC: &str = "unsupported_metric";
pub const OUTCOME_RATE_LIMITED: &str = "rate_limited";
pub const OUTCOME_INGESTION_UNAVAILABLE: &str = "ingestion_unavailable";
pub const OUTCOME_SERIES_CAP_EXCEEDED: &str = "series_cap_exceeded";
pub const OUTCOME_STALE_SEQUENCE: &str = "stale_sequence";

/// A sample in its raw wire form, before validation. Both transports
/// map their wire types into this shape; the value keeps the wire's
/// numeric distinction so integer counters stay integer.
#[derive(Debug, Clone)]
pub struct RawSample {
    pub target_kind: String,
    pub target_id: String,
    pub metric_id: String,
    pub source: String,
    pub kind: String,
    pub unit: String,
    pub observed_at_ms: i64,
    pub quality: String,
    pub value: Option<RawValue>,
    pub dimensions: BTreeMap<String, String>,
    pub boot_id: String,
    pub identity_epoch: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RawValue {
    Float(f64),
    Integer(u64),
}

/// A whole-batch rejection with its contract outcome.
#[derive(Debug)]
pub struct SampleRejection {
    pub outcome: &'static str,
    pub detail: String,
}

pub fn reject(outcome: &'static str, detail: impl std::fmt::Display) -> SampleRejection {
    SampleRejection {
        outcome,
        detail: detail.to_string(),
    }
}

/// Structural target-kind check shared by both transports: the kind
/// must parse, and VM targets must be valid `ResourceId`s (the 16-byte
/// resource id cap). Callers add their ownership rules on top.
pub fn validate_target(s: &RawSample) -> Result<TargetKind, SampleRejection> {
    let target_kind = TargetKind::from_str(&s.target_kind)
        .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("target_kind: {e}")))?;
    if target_kind == TargetKind::Vm {
        chv_controlplane_types::domain::ResourceId::new(&s.target_id)
            .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("vm target_id: {e}")))?;
    }
    Ok(target_kind)
}

/// Validate one raw sample against the v1 registry and model rules.
/// Whole-batch rejection semantics: any error here rejects the batch.
pub fn validate_sample(s: &RawSample, now_ms: i64) -> Result<Sample, SampleRejection> {
    let target_kind = validate_target(s)?;

    let metric = registry::lookup(&s.metric_id).ok_or_else(|| {
        reject(
            OUTCOME_UNSUPPORTED_METRIC,
            format!("unknown metric {}", s.metric_id),
        )
    })?;
    let source = Source::from_str(&s.source)
        .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("source: {e}")))?;
    if !metric.allowed_sources.contains(&source) {
        return Err(reject(
            OUTCOME_UNSUPPORTED_METRIC,
            format!("metric {} does not allow source {}", s.metric_id, s.source),
        ));
    }
    let kind = MetricKind::from_str(&s.kind)
        .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("kind: {e}")))?;
    if kind != metric.kind {
        return Err(reject(
            OUTCOME_INVALID_BATCH,
            format!(
                "metric {} is a {:?}, not {}",
                s.metric_id, metric.kind, s.kind
            ),
        ));
    }
    let unit =
        Unit::from_str(&s.unit).map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("unit: {e}")))?;
    if unit != metric.unit {
        return Err(reject(
            OUTCOME_INVALID_BATCH,
            format!(
                "metric {} is measured in {:?}, not {}",
                s.metric_id, metric.unit, s.unit
            ),
        ));
    }

    let quality = SampleQuality::parse(&s.quality).ok_or_else(|| {
        reject(
            OUTCOME_INVALID_BATCH,
            format!("unknown quality {}", s.quality),
        )
    })?;
    let value = match (s.value, quality) {
        (Some(RawValue::Float(v)), SampleQuality::Valid) => Some(SampleValue::Float(v)),
        (Some(RawValue::Integer(v)), SampleQuality::Valid) => Some(SampleValue::Integer(v)),
        (None, SampleQuality::Valid) => {
            return Err(reject(
                OUTCOME_INVALID_BATCH,
                format!("metric {} claims quality valid with no value", s.metric_id),
            ));
        }
        (Some(_), _) => {
            return Err(reject(
                OUTCOME_INVALID_BATCH,
                format!(
                    "metric {} carries a value with non-valid quality {}",
                    s.metric_id, s.quality
                ),
            ));
        }
        (None, _) => None,
    };
    // Counters are integer-valued on the wire and in the store
    // (exact decimal-string deltas): a float counter would be
    // accepted and durably stored, then never surfaced by the
    // integer-only query/rollup paths — reject it at the boundary
    // instead of committing data that can never be read back.
    if metric.kind == MetricKind::Counter {
        if let Some(SampleValue::Float(_)) = value {
            return Err(reject(
                OUTCOME_INVALID_BATCH,
                format!(
                    "counter metric {} must carry an integer value, not a float",
                    s.metric_id
                ),
            ));
        }
    }

    // Timestamp bounds: live raw ingestion only.
    let age = now_ms - s.observed_at_ms;
    if age > MAX_SAMPLE_AGE_MS {
        return Err(reject(
            OUTCOME_INVALID_BATCH,
            format!(
                "metric {} observation is {} ms old (max {})",
                s.metric_id, age, MAX_SAMPLE_AGE_MS
            ),
        ));
    }
    if age < -MAX_FUTURE_SKEW_MS {
        return Err(reject(
            OUTCOME_INVALID_BATCH,
            format!(
                "metric {} observation is {} ms in the future (max skew {})",
                s.metric_id, -age, MAX_FUTURE_SKEW_MS
            ),
        ));
    }

    let builder = SampleBuilder::new(
        target_kind,
        &s.target_id,
        &s.metric_id,
        source,
        s.observed_at_ms as u64,
    )
    .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("sample rejected: {e}")))?;
    let builder = match quality {
        SampleQuality::Valid => builder,
        other => builder.quality(other),
    };
    let builder = match value {
        Some(v) => builder.value(v),
        None => builder,
    };
    // Counter-epoch fence: build() rejects a valid counter sample
    // without one (a restarted counter source must never be
    // subtracted as a delta). Non-counter series may omit it.
    let builder = match (s.boot_id.as_str(), s.identity_epoch.as_str()) {
        (boot, epoch) if !boot.is_empty() && !epoch.is_empty() => builder.epoch(boot, epoch),
        _ => builder,
    };
    let mut builder = builder;
    for (k, v) in &s.dimensions {
        builder = builder
            .dimension(k, v)
            .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("dimension: {e}")))?;
    }
    builder
        .build()
        .map_err(|e| reject(OUTCOME_INVALID_BATCH, format!("sample rejected: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(metric_id: &str, source: &str) -> RawSample {
        RawSample {
            target_kind: "vm".into(),
            target_id: "vm-abcdefghijkl".into(),
            metric_id: metric_id.into(),
            source: source.into(),
            kind: "gauge".into(),
            unit: "count".into(),
            observed_at_ms: 1_000,
            quality: "valid".into(),
            value: Some(RawValue::Integer(42)),
            dimensions: BTreeMap::new(),
            boot_id: "boot-1".into(),
            identity_epoch: "agent-credential-generation-1".into(),
        }
    }

    #[test]
    fn unknown_metric_is_unsupported() {
        let s = raw("made.up.metric", "guest_agent");
        let err = validate_sample(&s, 2_000).unwrap_err();
        assert_eq!(err.outcome, OUTCOME_UNSUPPORTED_METRIC);
    }

    #[test]
    fn source_outside_allowlist_is_unsupported() {
        // guest metric claimed with a node source
        let s = raw("vm.guest.load1", "node_os");
        let err = validate_sample(&s, 2_000).unwrap_err();
        assert_eq!(err.outcome, OUTCOME_UNSUPPORTED_METRIC);
    }

    #[test]
    fn valid_guest_sample_passes() {
        let s = raw("vm.guest.load1", "guest_agent");
        let sample = validate_sample(&s, 2_000).expect("valid");
        assert_eq!(sample.metric_id, "vm.guest.load1");
    }

    #[test]
    fn valid_quality_requires_value() {
        let mut s = raw("vm.guest.load1", "guest_agent");
        s.value = None;
        let err = validate_sample(&s, 2_000).unwrap_err();
        assert_eq!(err.outcome, OUTCOME_INVALID_BATCH);
        assert!(err.detail.contains("no value"));
    }

    #[test]
    fn stale_observation_rejected() {
        let mut s = raw("vm.guest.load1", "guest_agent");
        s.observed_at_ms = 0;
        let err = validate_sample(&s, 10 * 60 * 1000).unwrap_err();
        assert_eq!(err.outcome, OUTCOME_INVALID_BATCH);
        assert!(err.detail.contains("old"));
    }
}
