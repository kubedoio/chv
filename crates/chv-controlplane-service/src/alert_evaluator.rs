//! Native alert evaluation core (ADR-027, campaign #602, prompt 05 /
//! gate G4 part 2 — PR-6).
//!
//! This module is the PURE half of the alert evaluator: typed rule
//! conditions, the missing-data policy, the incident state machine
//! and the incident identity/rendering helpers. It touches no
//! database, spawns no task and never blocks — the orchestrator's
//! worker (wired separately) queries the monitoring store, feeds an
//! [`EvaluationData`] snapshot in, and applies the returned
//! [`IncidentAction`] through the operational store.
//!
//! Invariants, in evaluation order:
//!
//! 1. **Typed rules only** (ADR-027 decision 6): threshold,
//!    reset-safe rate, availability (staleness), guest check status
//!    and one-level bounded AND/OR groups. No PromQL, no SQL, no
//!    code execution — and no redefinition of the store's typed rule
//!    model ([`AlertRuleSpec`] and friends are imported, never
//!    redeclared).
//! 2. **Missing data is never zero**: an absent, stale or non-valid
//!    sample evaluates to [`ConditionResult::Missing`] with a
//!    reason, never to a fabricated `0` that could silently fire a
//!    `value < limit` rule.
//! 3. **Availability inverts**: for availability rules the series
//!    being stale, absent or valueless IS the condition — absence
//!    evaluates Met.
//! 4. **Rates are reset-safe**: counter history points carry
//!    same-epoch deltas over their own `window_ms` (the store's
//!    bucketing already refused resets and epoch crossings); the
//!    evaluator sums non-negative deltas over their real windows and
//!    only trusts a rate when the summed window covers at least half
//!    the rule's window.
//! 5. **Total functions**: no panic on any input. Defensive guards
//!    (non-finite values, empty groups, zero summed windows,
//!    out-of-range durations) degrade to Missing or a no-op action —
//!    never to a crash or a fabricated number.

use chv_controlplane_store::{
    AlertRule, AlertRuleSpec, CheckStatusMatch, DimensionMatch, GroupOp, MissingDataPolicy,
    ThresholdOperator, INCIDENT_STATUS_FIRING, INCIDENT_STATUS_PENDING, INCIDENT_STATUS_RESOLVED,
};
use chv_monitoring_core::model::SampleQuality;
use chv_monitoring_store::{CurrentSample, HistoryPoint, SeriesReason, StoredCheck};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Inputs and results
// ---------------------------------------------------------------------------

/// One evaluation's pre-queried inputs. The caller (the evaluator
/// worker) is responsible for scoping every slice to the rule's
/// target; `history` must already be the exact series (metric +
/// dimension match) the rule selects, over the rule's rate window.
#[derive(Debug)]
pub struct EvaluationData<'a> {
    pub now_ms: i64,
    /// Current samples of the rule's target (store's `/current`
    /// surface, pre-queried for the rule's metric(s)).
    pub current: &'a [CurrentSample],
    /// The target's check inventory (latest record per check).
    pub checks: &'a [StoredCheck],
    /// History points of the rule's exact series over the rate
    /// window (rate rules only; empty otherwise).
    pub history: &'a [HistoryPoint],
}

/// The measurement a condition evaluated against: a real value, or
/// an honest absence with its reason (the query contract's
/// `SeriesReason` wire vocabulary).
#[derive(Debug, Clone, PartialEq)]
pub enum Observation {
    Observed { value: f64, at_ms: i64 },
    Missing { reason: String },
}

/// One rule condition's outcome. `Met`/`NotMet` carry the
/// observation that decided them; `Missing` carries the absence
/// reason (never a zero — see invariant 2).
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionResult {
    Met { observation: Observation },
    NotMet { observation: Observation },
    Missing { reason: String },
}

/// A condition outcome after the rule's missing-data policy has been
/// applied. `NoData` means "ignore": no state change at all.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleEvaluation {
    Met { observation: Observation },
    NotMet { observation: Observation },
    NoData,
}

// ---------------------------------------------------------------------------
// Condition evaluation
// ---------------------------------------------------------------------------

/// Evaluate one rule's condition against a pre-queried snapshot.
/// Total on every input (see invariant 5).
pub fn evaluate_condition(spec: &AlertRuleSpec, data: &EvaluationData) -> ConditionResult {
    match spec {
        AlertRuleSpec::Threshold {
            metric_id,
            dimension_match,
            operator,
            threshold,
        } => evaluate_threshold(
            metric_id,
            dimension_match.as_ref(),
            operator,
            *threshold,
            data,
        ),
        // The rate's metric/dimension selection was applied by the
        // caller when it queried `data.history` — the points carry no
        // series identity, so there is nothing left to filter here.
        AlertRuleSpec::Rate {
            operator,
            threshold_per_second,
            window_seconds,
            ..
        } => evaluate_rate(operator, *threshold_per_second, *window_seconds, data),
        AlertRuleSpec::Availability {
            metric_id,
            dimension_match,
        } => evaluate_availability(metric_id, dimension_match.as_ref(), data),
        AlertRuleSpec::CheckStatus {
            check_id,
            status_match,
        } => evaluate_check_status(check_id, status_match, data),
        AlertRuleSpec::Group { op, conditions } => evaluate_group(op, conditions, data),
    }
}

fn evaluate_threshold(
    metric_id: &str,
    dimension_match: Option<&DimensionMatch>,
    operator: &ThresholdOperator,
    threshold: f64,
    data: &EvaluationData,
) -> ConditionResult {
    let Some(sample) = find_current_sample(data.current, metric_id, dimension_match) else {
        return ConditionResult::Missing {
            reason: SeriesReason::NotCollected.as_str().to_string(),
        };
    };
    match current_value(sample) {
        Ok(value) => {
            let observation = Observation::Observed {
                value,
                at_ms: ms_i64(sample.observed_at_ms),
            };
            if operator.matches(value, threshold) {
                ConditionResult::Met { observation }
            } else {
                ConditionResult::NotMet { observation }
            }
        }
        Err(reason) => ConditionResult::Missing { reason },
    }
}

/// Reset-safe rate over the rule's window. Counter history points
/// carry the same-epoch delta over their own `window_ms` (the store
/// bucketed reset-safely), so the rate is
/// `sum(deltas) / sum(window_ms) * 1000`. Points without a value
/// (reset or epoch-crossing buckets) and — defensively — negative
/// point values are skipped entirely, and the summed window must
/// cover at least half the rule's window before the rate is trusted;
/// anything less is an honest unknown.
fn evaluate_rate(
    operator: &ThresholdOperator,
    threshold_per_second: f64,
    window_seconds: i64,
    data: &EvaluationData,
) -> ConditionResult {
    let missing = |reason: &str| ConditionResult::Missing {
        reason: reason.to_string(),
    };
    let window_ms = window_seconds.saturating_mul(1000).max(0);
    let from_ms = data.now_ms.saturating_sub(window_ms);
    let mut value_sum = 0.0f64;
    let mut window_sum_ms: u64 = 0;
    let mut last_ts: i64 = i64::MIN;
    for point in data.history {
        let ts = ms_i64(point.timestamp_ms);
        if ts < from_ms || ts > data.now_ms {
            continue; // outside the rule's window
        }
        let Some(value) = point.value else {
            continue; // honest absence (reset / epoch crossing)
        };
        if value < 0.0 || !value.is_finite() {
            continue; // defensive reset guard: never a negative delta
        }
        value_sum += value;
        window_sum_ms = window_sum_ms.saturating_add(point.window_ms);
        last_ts = last_ts.max(ts);
    }
    if window_sum_ms == 0 {
        return missing(SeriesReason::NoHistory.as_str());
    }
    if window_sum_ms.saturating_mul(2) < window_ms as u64 {
        return missing(SampleQuality::InsufficientSamples.as_str());
    }
    let rate_per_second = value_sum / window_sum_ms as f64 * 1000.0;
    if !rate_per_second.is_finite() {
        return missing(SampleQuality::Invalid.as_str());
    }
    let observation = Observation::Observed {
        value: rate_per_second,
        at_ms: last_ts,
    };
    if operator.matches(rate_per_second, threshold_per_second) {
        ConditionResult::Met { observation }
    } else {
        ConditionResult::NotMet { observation }
    }
}

/// Availability: INVERTED semantics — the series being stale, absent
/// or valueless IS the condition, so absence evaluates Met (an
/// availability rule fires on absence); a fresh valid value means
/// the target is fine (NotMet).
fn evaluate_availability(
    metric_id: &str,
    dimension_match: Option<&DimensionMatch>,
    data: &EvaluationData,
) -> ConditionResult {
    let missing = |reason: String| ConditionResult::Met {
        observation: Observation::Missing { reason },
    };
    let Some(sample) = find_current_sample(data.current, metric_id, dimension_match) else {
        return missing(SeriesReason::NotCollected.as_str().to_string());
    };
    match current_value(sample) {
        Ok(value) => ConditionResult::NotMet {
            observation: Observation::Observed {
                value,
                at_ms: ms_i64(sample.observed_at_ms),
            },
        },
        Err(reason) => missing(reason),
    }
}

fn evaluate_check_status(
    check_id: &str,
    status_match: &CheckStatusMatch,
    data: &EvaluationData,
) -> ConditionResult {
    let missing = |reason: &str| ConditionResult::Missing {
        reason: reason.to_string(),
    };
    let Some(check) = data.checks.iter().find(|c| c.check_id == check_id) else {
        return missing(SeriesReason::NotCollected.as_str());
    };
    if check.stale {
        return missing(SeriesReason::Stale.as_str());
    }
    // The typed check state is the observation: the contract's state
    // code (0=ok, 1=warning, 2=critical, 3=unknown), never a float
    // the agent invented.
    let observation = Observation::Observed {
        value: check.status.code() as f64,
        at_ms: ms_i64(check.observed_at_ms),
    };
    if check.status.as_str() == status_match.as_str() {
        ConditionResult::Met { observation }
    } else {
        ConditionResult::NotMet { observation }
    }
}

/// One-level AND/OR over the group's conditions (validated rules hold
/// only the four simple shapes; a defensively nested group simply
/// recurses — the spec tree cannot cycle).
///
/// AND: any Missing makes the whole thing Missing (never fire on
/// partial evidence); otherwise any NotMet wins; all Met is Met.
///
/// OR: any Met wins; otherwise a known NotMet beats unknown (false
/// OR unknown is false only while no true is known); all Missing is
/// Missing.
///
/// Met/NotMet carry the first deciding observation.
fn evaluate_group(
    op: &GroupOp,
    conditions: &[AlertRuleSpec],
    data: &EvaluationData,
) -> ConditionResult {
    if conditions.is_empty() {
        // Unreachable through validated rules (2..=5 conditions);
        // total-function discipline: no evidence reads as unknown,
        // never as vacuously true or false.
        return ConditionResult::Missing {
            reason: "group has no conditions".to_string(),
        };
    }
    let mut first_met: Option<Observation> = None;
    let mut first_not_met: Option<Observation> = None;
    let mut first_missing: Option<String> = None;
    for condition in conditions {
        match evaluate_condition(condition, data) {
            ConditionResult::Met { observation } => {
                if first_met.is_none() {
                    first_met = Some(observation);
                }
            }
            ConditionResult::NotMet { observation } => {
                if first_not_met.is_none() {
                    first_not_met = Some(observation);
                }
            }
            ConditionResult::Missing { reason } => {
                if first_missing.is_none() {
                    first_missing = Some(reason);
                }
            }
        }
    }
    match op {
        GroupOp::And => {
            if let Some(reason) = first_missing {
                ConditionResult::Missing { reason }
            } else if let Some(observation) = first_not_met {
                ConditionResult::NotMet { observation }
            } else {
                // The conditions are non-empty and nothing was
                // Missing or NotMet, so a Met observation exists;
                // the fallback arm only keeps the function total.
                match first_met {
                    Some(observation) => ConditionResult::Met { observation },
                    None => ConditionResult::Missing {
                        reason: "group has no conditions".to_string(),
                    },
                }
            }
        }
        GroupOp::Or => {
            if let Some(observation) = first_met {
                ConditionResult::Met { observation }
            } else if let Some(observation) = first_not_met {
                ConditionResult::NotMet { observation }
            } else {
                match first_missing {
                    Some(reason) => ConditionResult::Missing { reason },
                    None => ConditionResult::Missing {
                        reason: "group has no conditions".to_string(),
                    },
                }
            }
        }
    }
}

/// First current sample of `metric_id` whose dimensions satisfy the
/// rule's exact-match selection. Subset semantics: every
/// dimension_match entry must be present-and-equal in the sample's
/// dimensions; extra sample dimensions are fine; no dimension_match
/// selects any series of the metric (first in slice order).
fn find_current_sample<'a>(
    samples: &'a [CurrentSample],
    metric_id: &str,
    dimension_match: Option<&DimensionMatch>,
) -> Option<&'a CurrentSample> {
    samples
        .iter()
        .find(|s| s.metric_id == metric_id && dimensions_subset(&s.dimensions, dimension_match))
}

fn dimensions_subset(
    sample_dimensions: &BTreeMap<String, String>,
    dimension_match: Option<&DimensionMatch>,
) -> bool {
    match dimension_match {
        None => true,
        Some(required) => required
            .iter()
            .all(|(key, value)| sample_dimensions.get(key) == Some(value)),
    }
}

/// A current sample's numeric value when it is an honest, current
/// measurement; otherwise the absence reason. A stale flag, a
/// non-valid quality, a missing or non-finite value each read as
/// absence — never as zero (invariant 2).
fn current_value(sample: &CurrentSample) -> Result<f64, String> {
    if sample.stale {
        return Err(SeriesReason::Stale.as_str().to_string());
    }
    if sample.quality != SampleQuality::Valid {
        return Err(sample.quality.as_str().to_string());
    }
    match sample_value(sample) {
        Some(value) if value.is_finite() => Ok(value),
        Some(_) => Err(SampleQuality::Invalid.as_str().to_string()),
        None => Err(SampleQuality::Unavailable.as_str().to_string()),
    }
}

/// A sample's value: the float column first, the exact integer
/// column as f64 fallback (byte counters are integer-stored).
fn sample_value(sample: &CurrentSample) -> Option<f64> {
    sample
        .value
        .or_else(|| sample.integer_value.map(|v| v as f64))
}

/// u64 milliseconds → i64 without overflow: clocks far beyond any
/// real timestamp clamp instead of wrapping.
fn ms_i64(ms: u64) -> i64 {
    ms.min(i64::MAX as u64) as i64
}

// ---------------------------------------------------------------------------
// Missing-data policy
// ---------------------------------------------------------------------------

/// Apply the rule's missing-data policy to a condition result:
///
/// - `Unknown` (default): the incident records the gap but does not
///   fire — Missing becomes NotMet with a Missing observation.
/// - `Fire`: absence is condition-true — Missing becomes Met.
/// - `Ignore`: absence is no state change at all — NoData.
///
/// Met/NotMet pass through untouched.
pub fn apply_missing_data_policy(
    result: ConditionResult,
    policy: MissingDataPolicy,
) -> RuleEvaluation {
    match result {
        ConditionResult::Met { observation } => RuleEvaluation::Met { observation },
        ConditionResult::NotMet { observation } => RuleEvaluation::NotMet { observation },
        ConditionResult::Missing { reason } => match policy {
            MissingDataPolicy::Unknown => RuleEvaluation::NotMet {
                observation: Observation::Missing { reason },
            },
            MissingDataPolicy::Fire => RuleEvaluation::Met {
                observation: Observation::Missing { reason },
            },
            MissingDataPolicy::Ignore => RuleEvaluation::NoData,
        },
    }
}

// ---------------------------------------------------------------------------
// Incident state machine
// ---------------------------------------------------------------------------

/// The monitoring incident status vocabulary (the operational rows
/// keep their own `open` vocabulary; this maps to the store's
/// `INCIDENT_STATUS_*` strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentStatus {
    Pending,
    Firing,
    Resolved,
}

impl IncidentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IncidentStatus::Pending => INCIDENT_STATUS_PENDING,
            IncidentStatus::Firing => INCIDENT_STATUS_FIRING,
            IncidentStatus::Resolved => INCIDENT_STATUS_RESOLVED,
        }
    }
}

/// The active incident's state as the worker loads it. Only pending
/// and firing incidents are ever loaded — the evaluator never acts
/// on resolved history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncidentSnapshot {
    pub status: IncidentStatus,
    pub pending_since_ms: i64,
    pub clear_since_ms: Option<i64>,
}

/// One step of the spec's state machine
/// (inactive -> pending -> firing -> resolved; a pending incident
/// cleared before the hold is DELETEd, not stored). Acknowledgment
/// and silence are overlays handled elsewhere — they never appear
/// here. Absence (NoData) never clears a pending incident and never
/// resolves a firing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentAction {
    OpenPending,
    OpenFiring,
    PromoteToFiring,
    ClearPending,
    StartRecovery,
    Resolve,
    Observe,
    None,
}

pub fn next_state(
    incident: Option<&IncidentSnapshot>,
    evaluation: &RuleEvaluation,
    now_ms: i64,
    for_seconds: i64,
    recovery_seconds: i64,
) -> IncidentAction {
    let Some(incident) = incident else {
        return match evaluation {
            RuleEvaluation::Met { .. } => {
                if for_seconds > 0 {
                    IncidentAction::OpenPending
                } else {
                    // Zero hold: fire immediately; the store still
                    // records both transitions.
                    IncidentAction::OpenFiring
                }
            }
            RuleEvaluation::NotMet { .. } | RuleEvaluation::NoData => IncidentAction::None,
        };
    };
    match incident.status {
        IncidentStatus::Pending => match evaluation {
            RuleEvaluation::Met { .. } => {
                let held_ms = now_ms.saturating_sub(incident.pending_since_ms);
                if for_seconds <= 0 || held_ms >= for_seconds.saturating_mul(1000) {
                    // A zero (or defensively negative) hold never
                    // parks in pending — it opens firing; this arm
                    // only covers a rule edited after opening.
                    IncidentAction::PromoteToFiring
                } else {
                    IncidentAction::Observe
                }
            }
            // Cleared before the hold elapsed: deleted, not stored.
            RuleEvaluation::NotMet { .. } => IncidentAction::ClearPending,
            // Ignore-policy absence holds the pending: absence is
            // never evidence the condition stopped.
            RuleEvaluation::NoData => IncidentAction::None,
        },
        IncidentStatus::Firing => match evaluation {
            // Condition true again: the worker refreshes the incident
            // and clears the recovery marker.
            RuleEvaluation::Met { .. } => IncidentAction::Observe,
            RuleEvaluation::NotMet { .. } => match incident.clear_since_ms {
                // First false since the last true: the worker stamps
                // the recovery start (no notification).
                None => IncidentAction::StartRecovery,
                Some(clear_since) => {
                    let recovering_ms = now_ms.saturating_sub(clear_since);
                    if recovery_seconds <= 0
                        || recovering_ms >= recovery_seconds.saturating_mul(1000)
                    {
                        IncidentAction::Resolve
                    } else {
                        IncidentAction::None
                    }
                }
            },
            // Never resolve on absence.
            RuleEvaluation::NoData => IncidentAction::None,
        },
        // Defensive: the evaluator only loads active incidents, so a
        // resolved snapshot never receives actions.
        IncidentStatus::Resolved => IncidentAction::None,
    }
}

// ---------------------------------------------------------------------------
// Incident identity and rendering
// ---------------------------------------------------------------------------

/// The incident identity: `{rule_id}:{target_kind}:{target_id}:{dim}`.
/// The dimension part is the canonical (BTreeMap-ordered) compact
/// JSON of the rule's dimension match — order-independent by
/// construction — or `-` when the rule has no dimension match or is
/// not dimension-shaped (check_status / group rules).
pub fn dedup_key(rule: &AlertRule) -> String {
    let dimension_part = match &rule.spec {
        AlertRuleSpec::Threshold {
            dimension_match, ..
        }
        | AlertRuleSpec::Rate {
            dimension_match, ..
        }
        | AlertRuleSpec::Availability {
            dimension_match, ..
        } => match dimension_match {
            Some(match_map) => serde_json::to_string(match_map).unwrap_or_else(|_| "-".to_string()),
            None => "-".to_string(),
        },
        AlertRuleSpec::CheckStatus { .. } | AlertRuleSpec::Group { .. } => "-".to_string(),
    };
    format!(
        "{}:{}:{}:{}",
        rule.rule_id, rule.target_kind, rule.target_id, dimension_part
    )
}

/// Hard byte bound for rendered observation text (`last_observed`
/// column), cut at a UTF-8 char boundary.
const MAX_OBSERVATION_BYTES: usize = 160;

/// Rendered `last_observed` text: a compact, redacted, bounded
/// measurement like `0.94 (vm.cpu.capacity_ratio)` or
/// `missing (stale) (node.cpu.capacity_ratio)`. The label is the
/// rule's own metric/check identifier — never raw payloads, agent
/// claims or summaries — and the whole string is byte-bounded.
pub fn format_observation(spec: &AlertRuleSpec, observation: &Observation) -> String {
    let label = match spec {
        AlertRuleSpec::Threshold { metric_id, .. }
        | AlertRuleSpec::Rate { metric_id, .. }
        | AlertRuleSpec::Availability { metric_id, .. } => metric_id.as_str(),
        AlertRuleSpec::CheckStatus { check_id, .. } => check_id.as_str(),
        AlertRuleSpec::Group { .. } => "group",
    };
    let text = match observation {
        Observation::Observed { value, .. } => format!("{value} ({label})"),
        Observation::Missing { reason } => format!("missing ({reason}) ({label})"),
    };
    bound_text(text)
}

fn bound_text(text: String) -> String {
    if text.len() <= MAX_OBSERVATION_BYTES {
        return text;
    }
    let mut end = MAX_OBSERVATION_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = text;
    text.truncate(end);
    text
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chv_controlplane_store::{CheckStatusMatch, ThresholdOperator};
    use chv_monitoring_core::model::{CheckStatus, MetricKind, Source, Unit};
    use std::collections::BTreeMap;

    // -- fixtures -----------------------------------------------------------

    fn dims(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn sample(
        metric_id: &str,
        dimensions: &[(&str, &str)],
        value: Option<f64>,
        quality: SampleQuality,
        stale: bool,
    ) -> CurrentSample {
        CurrentSample {
            metric_id: metric_id.to_string(),
            source: Source::NodeOs,
            dimensions: dims(dimensions),
            kind: MetricKind::Gauge,
            unit: Unit::Ratio,
            observed_at_ms: 1_000_000,
            received_at_ms: 1_000_050,
            value,
            integer_value: None,
            quality,
            stale,
        }
    }

    fn point(timestamp_ms: u64, window_ms: u64, value: Option<f64>) -> HistoryPoint {
        HistoryPoint {
            timestamp_ms,
            window_ms,
            value,
            integer_value: None,
            quality: if value.is_some() {
                SampleQuality::Valid
            } else {
                SampleQuality::Unavailable
            },
        }
    }

    fn check(check_id: &str, status: CheckStatus, stale: bool) -> StoredCheck {
        StoredCheck {
            check_id: check_id.to_string(),
            service_key: None,
            status,
            summary: None,
            observed_at_ms: 900_000,
            received_at_ms: 900_050,
            agent_id: "agent-1".to_string(),
            stale,
        }
    }

    fn data<'a>(
        now_ms: i64,
        current: &'a [CurrentSample],
        checks: &'a [StoredCheck],
        history: &'a [HistoryPoint],
    ) -> EvaluationData<'a> {
        EvaluationData {
            now_ms,
            current,
            checks,
            history,
        }
    }

    fn threshold_spec(operator: ThresholdOperator, threshold: f64) -> AlertRuleSpec {
        AlertRuleSpec::Threshold {
            metric_id: "vm.cpu.capacity_ratio".to_string(),
            dimension_match: None,
            operator,
            threshold,
        }
    }

    fn dim_threshold_spec(
        dimension_match: Option<DimensionMatch>,
        operator: ThresholdOperator,
        threshold: f64,
    ) -> AlertRuleSpec {
        AlertRuleSpec::Threshold {
            metric_id: "vm.guest.fs.available_bytes".to_string(),
            dimension_match,
            operator,
            threshold,
        }
    }

    fn rate_spec(threshold_per_second: f64, window_seconds: i64) -> AlertRuleSpec {
        AlertRuleSpec::Rate {
            metric_id: "vm.guest.net.rx_errors_total".to_string(),
            dimension_match: None,
            operator: ThresholdOperator::GreaterThan,
            threshold_per_second,
            window_seconds,
        }
    }

    fn cond(metric_id: &str, threshold: f64) -> AlertRuleSpec {
        AlertRuleSpec::Threshold {
            metric_id: metric_id.to_string(),
            dimension_match: None,
            operator: ThresholdOperator::GreaterThan,
            threshold,
        }
    }

    fn rule(spec: AlertRuleSpec) -> AlertRule {
        AlertRule {
            rule_id: "rule-1".to_string(),
            name: "test rule".to_string(),
            enabled: true,
            target_kind: "vm".to_string(),
            target_id: "vm-1".to_string(),
            severity: "warning".to_string(),
            for_seconds: 300,
            recovery_seconds: 120,
            missing_data: MissingDataPolicy::Unknown,
            revision: 1,
            created_by: "op-user".to_string(),
            created_at_ms: 0,
            updated_at_ms: 0,
            spec,
        }
    }

    fn met(value: f64) -> Observation {
        Observation::Observed {
            value,
            at_ms: 1_000_000,
        }
    }

    /// The observed value of a Met/NotMet condition result.
    fn value_of(result: &ConditionResult) -> f64 {
        match result {
            ConditionResult::Met {
                observation: Observation::Observed { value, .. },
            }
            | ConditionResult::NotMet {
                observation: Observation::Observed { value, .. },
            } => *value,
            other => panic!("expected an observed value, got {other:?}"),
        }
    }

    fn missing_reason(result: &ConditionResult) -> String {
        match result {
            ConditionResult::Missing { reason } => reason.clone(),
            ConditionResult::Met {
                observation: Observation::Missing { reason },
            }
            | ConditionResult::NotMet {
                observation: Observation::Missing { reason },
            } => reason.clone(),
            other => panic!("expected a missing reason, got {other:?}"),
        }
    }

    // -- threshold ----------------------------------------------------------

    #[test]
    fn threshold_greater_than_met_when_above() {
        let spec = threshold_spec(ThresholdOperator::GreaterThan, 0.9);
        let samples = [sample(
            "vm.cpu.capacity_ratio",
            &[],
            Some(0.95),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: met(0.95)
            }
        );
    }

    #[test]
    fn threshold_greater_than_not_met_when_below() {
        let spec = threshold_spec(ThresholdOperator::GreaterThan, 0.9);
        let samples = [sample(
            "vm.cpu.capacity_ratio",
            &[],
            Some(0.42),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::NotMet {
                observation: met(0.42)
            }
        );
    }

    #[test]
    fn threshold_less_than_met_when_below() {
        let spec = threshold_spec(ThresholdOperator::LessThan, 0.1);
        let samples = [sample(
            "vm.cpu.capacity_ratio",
            &[],
            Some(0.05),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: met(0.05)
            }
        );
    }

    #[test]
    fn threshold_boundary_equality_matches_neither_operator() {
        let samples = [sample(
            "vm.cpu.capacity_ratio",
            &[],
            Some(0.9),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        for operator in [ThresholdOperator::GreaterThan, ThresholdOperator::LessThan] {
            let spec = threshold_spec(operator, 0.9);
            assert_eq!(
                evaluate_condition(&spec, &input),
                ConditionResult::NotMet {
                    observation: met(0.9)
                },
                "boundary equality must not match {operator:?}"
            );
        }
    }

    #[test]
    fn threshold_dimension_match_is_subset_semantics() {
        let mut match_map = DimensionMatch::new();
        match_map.insert("mount_id".to_string(), "ext4:/".to_string());
        let spec = dim_threshold_spec(
            Some(match_map),
            ThresholdOperator::LessThan,
            2_147_483_648.0,
        );
        // Extra sample dimensions are fine: every dimension_match
        // entry is present-and-equal.
        let samples = [sample(
            "vm.guest.fs.available_bytes",
            &[
                ("mount_id", "ext4:/"),
                ("device", "sda1"),
                ("fstype", "ext4"),
            ],
            Some(1_073_741_824.0),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: met(1_073_741_824.0)
            }
        );
    }

    #[test]
    fn threshold_dimension_match_rejects_mismatched_value() {
        let mut match_map = DimensionMatch::new();
        match_map.insert("mount_id".to_string(), "ext4:/".to_string());
        let spec = dim_threshold_spec(
            Some(match_map),
            ThresholdOperator::LessThan,
            2_147_483_648.0,
        );
        let samples = [
            sample(
                "vm.guest.fs.available_bytes",
                &[("mount_id", "xfs:/data")],
                Some(1_073_741_824.0),
                SampleQuality::Valid,
                false,
            ),
            // A different metric must not be picked up either.
            sample(
                "other.metric",
                &[("mount_id", "ext4:/")],
                Some(0.0),
                SampleQuality::Valid,
                false,
            ),
        ];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Missing {
                reason: "not_collected".to_string()
            }
        );
    }

    #[test]
    fn threshold_dimension_match_requires_present_keys() {
        let mut match_map = DimensionMatch::new();
        match_map.insert("mount_id".to_string(), "ext4:/".to_string());
        let spec = dim_threshold_spec(
            Some(match_map),
            ThresholdOperator::LessThan,
            2_147_483_648.0,
        );
        let samples = [sample(
            "vm.guest.fs.available_bytes",
            &[("device", "sda1")],
            Some(1_073_741_824.0),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Missing {
                reason: "not_collected".to_string()
            }
        );
    }

    #[test]
    fn threshold_without_dimension_match_takes_first_series_of_metric() {
        let spec = threshold_spec(ThresholdOperator::GreaterThan, 0.5);
        let samples = [
            sample(
                "vm.cpu.capacity_ratio",
                &[("cpu", "0")],
                Some(0.1),
                SampleQuality::Valid,
                false,
            ),
            sample(
                "vm.cpu.capacity_ratio",
                &[("cpu", "1")],
                Some(0.8),
                SampleQuality::Valid,
                false,
            ),
        ];
        let input = data(1_100_000, &samples, &[], &[]);
        // No dimension_match: any series of the metric, first match.
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::NotMet {
                observation: met(0.1)
            }
        );
    }

    #[test]
    fn threshold_stale_sample_is_missing() {
        let spec = threshold_spec(ThresholdOperator::GreaterThan, 0.9);
        let samples = [sample(
            "vm.cpu.capacity_ratio",
            &[],
            Some(0.95),
            SampleQuality::Valid,
            true,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "stale");
    }

    #[test]
    fn threshold_absent_series_is_missing() {
        let spec = threshold_spec(ThresholdOperator::GreaterThan, 0.9);
        let input = data(1_100_000, &[], &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Missing {
                reason: "not_collected".to_string()
            }
        );
    }

    #[test]
    fn threshold_none_value_is_missing_never_zero() {
        let spec = threshold_spec(ThresholdOperator::LessThan, 0.5);
        let samples = [sample(
            "vm.cpu.capacity_ratio",
            &[],
            None,
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        // A missing value must not read as 0 (which would fire the
        // less-than rule).
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "unavailable");
    }

    #[test]
    fn threshold_non_valid_quality_is_missing() {
        let spec = threshold_spec(ThresholdOperator::LessThan, 0.5);
        for quality in [
            SampleQuality::InsufficientSamples,
            SampleQuality::Unsupported,
            SampleQuality::Unavailable,
            SampleQuality::Invalid,
            SampleQuality::Stale,
        ] {
            let samples = [sample(
                "vm.cpu.capacity_ratio",
                &[],
                Some(0.1),
                quality,
                false,
            )];
            let input = data(1_100_000, &samples, &[], &[]);
            let result = evaluate_condition(&spec, &input);
            assert_eq!(
                missing_reason(&result),
                quality.as_str(),
                "quality {quality:?} must read as missing"
            );
        }
    }

    #[test]
    fn threshold_integer_value_extraction() {
        let spec = dim_threshold_spec(None, ThresholdOperator::LessThan, 3_000_000_000.0);
        let mut integer_sample = sample(
            "vm.guest.fs.available_bytes",
            &[],
            None,
            SampleQuality::Valid,
            false,
        );
        integer_sample.integer_value = Some(2_147_483_648);
        let samples = [integer_sample];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: met(2_147_483_648.0)
            }
        );
    }

    // -- availability (inverted) --------------------------------------------

    #[test]
    fn availability_fresh_value_is_not_met() {
        let spec = AlertRuleSpec::Availability {
            metric_id: "node.cpu.capacity_ratio".to_string(),
            dimension_match: None,
        };
        let samples = [sample(
            "node.cpu.capacity_ratio",
            &[],
            Some(0.42),
            SampleQuality::Valid,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::NotMet {
                observation: met(0.42)
            }
        );
    }

    #[test]
    fn availability_stale_series_is_met() {
        let spec = AlertRuleSpec::Availability {
            metric_id: "node.cpu.capacity_ratio".to_string(),
            dimension_match: None,
        };
        let samples = [sample(
            "node.cpu.capacity_ratio",
            &[],
            Some(0.42),
            SampleQuality::Valid,
            true,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert_eq!(missing_reason(&result), "stale");
    }

    #[test]
    fn availability_absent_series_is_met() {
        let spec = AlertRuleSpec::Availability {
            metric_id: "node.cpu.capacity_ratio".to_string(),
            dimension_match: None,
        };
        let input = data(1_100_000, &[], &[], &[]);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert_eq!(missing_reason(&result), "not_collected");
    }

    #[test]
    fn availability_valueless_series_is_met() {
        let spec = AlertRuleSpec::Availability {
            metric_id: "node.cpu.capacity_ratio".to_string(),
            dimension_match: None,
        };
        // Fresh but non-valid: no value means no availability.
        let samples = [sample(
            "node.cpu.capacity_ratio",
            &[],
            None,
            SampleQuality::Unavailable,
            false,
        )];
        let input = data(1_100_000, &samples, &[], &[]);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert_eq!(missing_reason(&result), "unavailable");
    }

    // -- check status ---------------------------------------------------------

    #[test]
    fn check_status_matching_is_met() {
        let spec = AlertRuleSpec::CheckStatus {
            check_id: "service:g4-http.service".to_string(),
            status_match: CheckStatusMatch::Critical,
        };
        let checks = [check(
            "service:g4-http.service",
            CheckStatus::Critical,
            false,
        )];
        let input = data(1_100_000, &[], &checks, &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: Observation::Observed {
                    value: 2.0, // critical's state code
                    at_ms: 900_000
                }
            }
        );
    }

    #[test]
    fn check_status_mismatching_is_not_met() {
        let spec = AlertRuleSpec::CheckStatus {
            check_id: "service:g4-http.service".to_string(),
            status_match: CheckStatusMatch::Critical,
        };
        let checks = [check("service:g4-http.service", CheckStatus::Ok, false)];
        let input = data(1_100_000, &[], &checks, &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::NotMet {
                observation: Observation::Observed {
                    value: 0.0,
                    at_ms: 900_000
                }
            }
        );
    }

    #[test]
    fn check_status_stale_is_missing() {
        let spec = AlertRuleSpec::CheckStatus {
            check_id: "service:g4-http.service".to_string(),
            status_match: CheckStatusMatch::Critical,
        };
        let checks = [check(
            "service:g4-http.service",
            CheckStatus::Critical,
            true,
        )];
        let input = data(1_100_000, &[], &checks, &[]);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "stale");
    }

    #[test]
    fn check_status_absent_is_missing() {
        let spec = AlertRuleSpec::CheckStatus {
            check_id: "service:absent.service".to_string(),
            status_match: CheckStatusMatch::Warning,
        };
        let checks = [check("service:other.service", CheckStatus::Warning, false)];
        let input = data(1_100_000, &[], &checks, &[]);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "not_collected");
    }

    // -- rate -----------------------------------------------------------------

    fn rate_history() -> Vec<HistoryPoint> {
        // Window [9_700_000, 10_000_000] (300 s rule): 900 counter
        // units over 150 s of real observed windows — coverage is
        // exactly half the rule's window.
        vec![
            point(9_750_000, 10_000, Some(100.0)),
            point(9_800_000, 20_000, Some(200.0)),
            point(9_900_000, 120_000, Some(600.0)),
        ]
    }

    #[test]
    fn rate_sums_window_weighted_deltas() {
        let spec = rate_spec(5.0, 300);
        let history = rate_history();
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        // (100 + 200 + 600) / (10_000 + 20_000 + 120_000) * 1000 = 6/s.
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert!((value_of(&result) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn rate_below_threshold_is_not_met() {
        let spec = rate_spec(10.0, 300);
        let history = rate_history();
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::NotMet { .. }));
        assert!((value_of(&result) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn rate_exactly_half_window_coverage_is_enough() {
        // The fixture above sums to exactly 150 s of window against a
        // 300 s rule: exactly half must NOT read as insufficient.
        let spec = rate_spec(0.0, 300);
        let history = rate_history();
        let input = data(10_000_000, &[], &[], &history);
        assert!(matches!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met { .. }
        ));
    }

    #[test]
    fn rate_insufficient_window_coverage_is_missing() {
        let spec = rate_spec(5.0, 300);
        let history = [point(9_900_000, 10_000, Some(100.0))];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "insufficient_samples");
    }

    #[test]
    fn rate_skips_points_without_values() {
        let spec = rate_spec(2.0, 300);
        let history = [
            point(9_750_000, 100_000, None), // reset/epoch-crossing bucket
            point(9_850_000, 100_000, Some(200.0)),
            point(9_950_000, 100_000, Some(400.0)),
        ];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        // 600 / 200 s = 3/s; the valueless point contributes neither
        // value nor window.
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert!((value_of(&result) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn rate_skips_negative_point_values() {
        let spec = rate_spec(0.01, 60);
        let history = [
            point(9_950_000, 30_000, Some(-50.0)), // defensive reset guard
            point(9_980_000, 30_000, Some(600.0)),
        ];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        // Only the positive point counts: 600 / 30 s = 20/s.
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert!((value_of(&result) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn rate_empty_history_is_missing() {
        let spec = rate_spec(5.0, 300);
        let input = data(10_000_000, &[], &[], &[]);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "no_history");
    }

    #[test]
    fn rate_ignores_points_outside_the_window() {
        let spec = rate_spec(5.0, 300);
        let history = [
            point(7_000_000, 290_000, Some(5_000.0)), // before the window
            point(9_900_000, 5_000, Some(100.0)),     // in window
            point(10_500_000, 100_000, Some(900.0)),  // future-dated
        ];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        // Only 5 s of in-window coverage against a 300 s window.
        assert_eq!(missing_reason(&result), "insufficient_samples");
    }

    #[test]
    fn rate_zero_summed_window_is_missing_not_a_division_by_zero() {
        let spec = rate_spec(5.0, 60);
        let history = [point(10_000_000, 0, Some(10.0))];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "no_history");
    }

    // -- group ----------------------------------------------------------------

    #[test]
    fn group_and_all_met_is_met() {
        let spec = AlertRuleSpec::Group {
            op: GroupOp::And,
            conditions: vec![cond("m.a", 0.9), cond("m.b", 0.9)],
        };
        let samples = [
            sample("m.a", &[], Some(0.95), SampleQuality::Valid, false),
            sample("m.b", &[], Some(0.99), SampleQuality::Valid, false),
        ];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: met(0.95) // first Met observation
            }
        );
    }

    #[test]
    fn group_and_not_met_beats_met() {
        let spec = AlertRuleSpec::Group {
            op: GroupOp::And,
            conditions: vec![cond("m.a", 0.9), cond("m.b", 0.9)],
        };
        let samples = [
            sample("m.a", &[], Some(0.95), SampleQuality::Valid, false),
            sample("m.b", &[], Some(0.5), SampleQuality::Valid, false),
        ];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::NotMet {
                observation: met(0.5)
            }
        );
    }

    #[test]
    fn group_and_missing_dominates() {
        // AND of a true condition and an unknown one is unknown:
        // never fire on partial evidence.
        let spec = AlertRuleSpec::Group {
            op: GroupOp::And,
            conditions: vec![cond("m.a", 0.9), cond("m.missing", 0.9)],
        };
        let samples = [sample("m.a", &[], Some(0.95), SampleQuality::Valid, false)];
        let input = data(1_100_000, &samples, &[], &[]);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::Missing { .. }));
    }

    #[test]
    fn group_and_not_met_with_missing_is_missing() {
        // Unknown dominates known-false in AND too.
        let spec = AlertRuleSpec::Group {
            op: GroupOp::And,
            conditions: vec![cond("m.b", 0.9), cond("m.missing", 0.9)],
        };
        let samples = [sample("m.b", &[], Some(0.5), SampleQuality::Valid, false)];
        let input = data(1_100_000, &samples, &[], &[]);
        assert!(matches!(
            evaluate_condition(&spec, &input),
            ConditionResult::Missing { .. }
        ));
    }

    #[test]
    fn group_or_any_met_wins() {
        let spec = AlertRuleSpec::Group {
            op: GroupOp::Or,
            conditions: vec![cond("m.b", 0.9), cond("m.a", 0.9)],
        };
        let samples = [
            sample("m.b", &[], Some(0.5), SampleQuality::Valid, false),
            sample("m.a", &[], Some(0.95), SampleQuality::Valid, false),
        ];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::Met {
                observation: met(0.95)
            }
        );
    }

    #[test]
    fn group_or_known_false_with_unknown_is_not_met() {
        // OR of known-false and unknown is false only while no true
        // is known.
        let spec = AlertRuleSpec::Group {
            op: GroupOp::Or,
            conditions: vec![cond("m.b", 0.9), cond("m.missing", 0.9)],
        };
        let samples = [sample("m.b", &[], Some(0.5), SampleQuality::Valid, false)];
        let input = data(1_100_000, &samples, &[], &[]);
        assert_eq!(
            evaluate_condition(&spec, &input),
            ConditionResult::NotMet {
                observation: met(0.5)
            }
        );
    }

    #[test]
    fn group_or_all_missing_is_missing() {
        let spec = AlertRuleSpec::Group {
            op: GroupOp::Or,
            conditions: vec![cond("m.missing1", 0.9), cond("m.missing2", 0.9)],
        };
        let input = data(1_100_000, &[], &[], &[]);
        assert!(matches!(
            evaluate_condition(&spec, &input),
            ConditionResult::Missing { .. }
        ));
    }

    #[test]
    fn group_or_mixes_threshold_and_check_status() {
        let spec = AlertRuleSpec::Group {
            op: GroupOp::Or,
            conditions: vec![
                cond("m.b", 0.9),
                AlertRuleSpec::CheckStatus {
                    check_id: "service:nginx.service".to_string(),
                    status_match: CheckStatusMatch::Critical,
                },
            ],
        };
        let samples = [sample("m.b", &[], Some(0.5), SampleQuality::Valid, false)];
        let checks = [check("service:nginx.service", CheckStatus::Critical, false)];
        let input = data(1_100_000, &samples, &checks, &[]);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert_eq!(value_of(&result), 2.0);
    }

    #[test]
    fn group_without_conditions_is_missing_not_vacuous() {
        // Unreachable through validated rules; total-function
        // discipline says no evidence reads as unknown.
        let spec = AlertRuleSpec::Group {
            op: GroupOp::And,
            conditions: vec![],
        };
        let input = data(1_100_000, &[], &[], &[]);
        assert!(matches!(
            evaluate_condition(&spec, &input),
            ConditionResult::Missing { .. }
        ));
    }

    // -- missing-data policy ---------------------------------------------------

    #[test]
    fn missing_data_policy_full_matrix() {
        let observed = Observation::Observed {
            value: 1.0,
            at_ms: 5,
        };
        let absent = Observation::Missing {
            reason: "stale".to_string(),
        };
        for policy in [
            MissingDataPolicy::Unknown,
            MissingDataPolicy::Fire,
            MissingDataPolicy::Ignore,
        ] {
            assert_eq!(
                apply_missing_data_policy(
                    ConditionResult::Met {
                        observation: observed.clone()
                    },
                    policy
                ),
                RuleEvaluation::Met {
                    observation: observed.clone()
                }
            );
            assert_eq!(
                apply_missing_data_policy(
                    ConditionResult::NotMet {
                        observation: observed.clone()
                    },
                    policy
                ),
                RuleEvaluation::NotMet {
                    observation: observed.clone()
                }
            );
        }
        // Missing + Unknown: record the gap, do not fire.
        assert_eq!(
            apply_missing_data_policy(
                ConditionResult::Missing {
                    reason: "stale".to_string()
                },
                MissingDataPolicy::Unknown
            ),
            RuleEvaluation::NotMet {
                observation: absent.clone()
            }
        );
        // Missing + Fire: absence is condition-true.
        assert_eq!(
            apply_missing_data_policy(
                ConditionResult::Missing {
                    reason: "stale".to_string()
                },
                MissingDataPolicy::Fire
            ),
            RuleEvaluation::Met {
                observation: absent.clone()
            }
        );
        // Missing + Ignore: no state change at all.
        assert_eq!(
            apply_missing_data_policy(
                ConditionResult::Missing {
                    reason: "stale".to_string()
                },
                MissingDataPolicy::Ignore
            ),
            RuleEvaluation::NoData
        );
    }

    // -- state machine ---------------------------------------------------------

    fn evaluation_met() -> RuleEvaluation {
        RuleEvaluation::Met {
            observation: Observation::Observed {
                value: 1.0,
                at_ms: 0,
            },
        }
    }

    fn evaluation_not_met() -> RuleEvaluation {
        RuleEvaluation::NotMet {
            observation: Observation::Observed {
                value: 0.0,
                at_ms: 0,
            },
        }
    }

    #[test]
    fn no_incident_met_opens_pending_when_hold_positive() {
        assert_eq!(
            next_state(None, &evaluation_met(), 10_000_000, 300, 120),
            IncidentAction::OpenPending
        );
    }

    #[test]
    fn no_incident_met_opens_firing_when_hold_zero() {
        assert_eq!(
            next_state(None, &evaluation_met(), 10_000_000, 0, 120),
            IncidentAction::OpenFiring
        );
    }

    #[test]
    fn no_incident_not_met_and_no_data_do_nothing() {
        assert_eq!(
            next_state(None, &evaluation_not_met(), 10_000_000, 300, 120),
            IncidentAction::None
        );
        assert_eq!(
            next_state(None, &RuleEvaluation::NoData, 10_000_000, 300, 120),
            IncidentAction::None
        );
    }

    #[test]
    fn pending_promotes_when_hold_elapsed() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Pending,
            pending_since_ms: 9_000_000,
            clear_since_ms: None,
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_met(), 10_000_000, 300, 120),
            IncidentAction::PromoteToFiring
        );
    }

    #[test]
    fn pending_observes_while_hold_running() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Pending,
            pending_since_ms: 9_900_000, // 100 s held, 300 s required
            clear_since_ms: None,
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_met(), 10_000_000, 300, 120),
            IncidentAction::Observe
        );
    }

    #[test]
    fn pending_promotes_immediately_when_hold_zero() {
        // Defensive: a zero-hold rule edited after opening must not
        // park in pending forever.
        let incident = IncidentSnapshot {
            status: IncidentStatus::Pending,
            pending_since_ms: 9_999_999,
            clear_since_ms: None,
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_met(), 10_000_000, 0, 120),
            IncidentAction::PromoteToFiring
        );
    }

    #[test]
    fn pending_clears_on_not_met() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Pending,
            pending_since_ms: 9_900_000,
            clear_since_ms: None,
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_not_met(), 10_000_000, 300, 120),
            IncidentAction::ClearPending
        );
    }

    #[test]
    fn pending_holds_on_no_data() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Pending,
            pending_since_ms: 9_900_000,
            clear_since_ms: None,
        };
        assert_eq!(
            next_state(
                Some(&incident),
                &RuleEvaluation::NoData,
                10_000_000,
                300,
                120
            ),
            IncidentAction::None
        );
    }

    #[test]
    fn firing_starts_recovery_on_first_not_met() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Firing,
            pending_since_ms: 9_000_000,
            clear_since_ms: None,
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_not_met(), 10_000_000, 300, 120),
            IncidentAction::StartRecovery
        );
    }

    #[test]
    fn firing_stays_quiet_while_recovering() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Firing,
            pending_since_ms: 9_000_000,
            clear_since_ms: Some(9_950_000), // 50 s recovering, 120 s required
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_not_met(), 10_000_000, 300, 120),
            IncidentAction::None
        );
    }

    #[test]
    fn firing_resolves_when_recovery_elapsed() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Firing,
            pending_since_ms: 9_000_000,
            clear_since_ms: Some(9_000_000), // 1000 s recovering >= 120 s
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_not_met(), 10_000_000, 300, 120),
            IncidentAction::Resolve
        );
    }

    #[test]
    fn firing_resolves_immediately_when_recovery_zero() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Firing,
            pending_since_ms: 9_000_000,
            clear_since_ms: Some(9_999_999),
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_not_met(), 10_000_000, 300, 0),
            IncidentAction::Resolve
        );
    }

    #[test]
    fn firing_observes_on_met() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Firing,
            pending_since_ms: 9_000_000,
            clear_since_ms: Some(9_500_000), // mid-recovery
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_met(), 10_000_000, 300, 120),
            IncidentAction::Observe
        );
    }

    #[test]
    fn firing_holds_on_no_data() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Firing,
            pending_since_ms: 9_000_000,
            clear_since_ms: Some(9_000_000), // recovery long elapsed
        };
        assert_eq!(
            next_state(
                Some(&incident),
                &RuleEvaluation::NoData,
                10_000_000,
                300,
                120
            ),
            IncidentAction::None
        );
    }

    #[test]
    fn resolved_snapshot_never_acts() {
        let incident = IncidentSnapshot {
            status: IncidentStatus::Resolved,
            pending_since_ms: 9_000_000,
            clear_since_ms: Some(9_000_000),
        };
        assert_eq!(
            next_state(Some(&incident), &evaluation_met(), 10_000_000, 300, 120),
            IncidentAction::None
        );
        assert_eq!(
            next_state(Some(&incident), &evaluation_not_met(), 10_000_000, 300, 120),
            IncidentAction::None
        );
        assert_eq!(
            next_state(
                Some(&incident),
                &RuleEvaluation::NoData,
                10_000_000,
                300,
                120
            ),
            IncidentAction::None
        );
    }

    #[test]
    fn incident_status_maps_to_store_vocabulary() {
        assert_eq!(IncidentStatus::Pending.as_str(), "pending");
        assert_eq!(IncidentStatus::Firing.as_str(), "firing");
        assert_eq!(IncidentStatus::Resolved.as_str(), "resolved");
    }

    // -- dedup key --------------------------------------------------------------

    #[test]
    fn dedup_key_includes_canonical_dimensions() {
        let mut match_map = DimensionMatch::new();
        match_map.insert("mount_id".to_string(), "ext4:/".to_string());
        let key = dedup_key(&rule(dim_threshold_spec(
            Some(match_map),
            ThresholdOperator::LessThan,
            1.0,
        )));
        assert_eq!(key, "rule-1:vm:vm-1:{\"mount_id\":\"ext4:/\"}");
    }

    #[test]
    fn dedup_key_is_dimension_order_independent() {
        let mut a = DimensionMatch::new();
        a.insert("mount_id".to_string(), "ext4:/".to_string());
        a.insert("device".to_string(), "sda1".to_string());
        let mut b = DimensionMatch::new();
        b.insert("device".to_string(), "sda1".to_string());
        b.insert("mount_id".to_string(), "ext4:/".to_string());
        let key_a = dedup_key(&rule(dim_threshold_spec(
            Some(a),
            ThresholdOperator::LessThan,
            1.0,
        )));
        let key_b = dedup_key(&rule(dim_threshold_spec(
            Some(b),
            ThresholdOperator::LessThan,
            1.0,
        )));
        assert_eq!(key_a, key_b);
        // BTreeMap canonical order: "device" sorts before "mount_id".
        assert_eq!(
            key_a,
            "rule-1:vm:vm-1:{\"device\":\"sda1\",\"mount_id\":\"ext4:/\"}"
        );
    }

    #[test]
    fn dedup_key_dash_for_undimensioned_rules() {
        assert_eq!(
            dedup_key(&rule(threshold_spec(ThresholdOperator::GreaterThan, 1.0))),
            "rule-1:vm:vm-1:-"
        );
        assert_eq!(dedup_key(&rule(rate_spec(1.0, 60))), "rule-1:vm:vm-1:-");
        assert_eq!(
            dedup_key(&rule(AlertRuleSpec::CheckStatus {
                check_id: "service:nginx.service".to_string(),
                status_match: CheckStatusMatch::Critical,
            })),
            "rule-1:vm:vm-1:-"
        );
        assert_eq!(
            dedup_key(&rule(AlertRuleSpec::Group {
                op: GroupOp::And,
                conditions: vec![cond("m.a", 1.0), cond("m.b", 1.0)],
            })),
            "rule-1:vm:vm-1:-"
        );
    }

    #[test]
    fn dedup_key_distinguishes_rules_and_targets() {
        let mut rule_a = rule(threshold_spec(ThresholdOperator::GreaterThan, 1.0));
        rule_a.rule_id = "rule-2".to_string();
        let mut rule_b = rule(threshold_spec(ThresholdOperator::GreaterThan, 1.0));
        rule_b.target_kind = "node".to_string();
        rule_b.target_id = "node-9".to_string();
        assert_eq!(dedup_key(&rule_a), "rule-2:vm:vm-1:-");
        assert_eq!(dedup_key(&rule_b), "rule-1:node:node-9:-");
    }

    // -- observation rendering ----------------------------------------------------

    #[test]
    fn format_observed_renders_value_and_metric() {
        let spec = threshold_spec(ThresholdOperator::GreaterThan, 0.9);
        let observation = Observation::Observed {
            value: 0.94,
            at_ms: 1_000_000,
        };
        assert_eq!(
            format_observation(&spec, &observation),
            "0.94 (vm.cpu.capacity_ratio)"
        );
    }

    #[test]
    fn format_missing_renders_reason_and_metric() {
        let spec = AlertRuleSpec::Availability {
            metric_id: "node.cpu.capacity_ratio".to_string(),
            dimension_match: None,
        };
        let observation = Observation::Missing {
            reason: "stale".to_string(),
        };
        assert_eq!(
            format_observation(&spec, &observation),
            "missing (stale) (node.cpu.capacity_ratio)"
        );
    }

    #[test]
    fn format_check_status_renders_state_code_and_check_id() {
        let spec = AlertRuleSpec::CheckStatus {
            check_id: "service:nginx.service".to_string(),
            status_match: CheckStatusMatch::Critical,
        };
        let observation = Observation::Observed {
            value: 2.0,
            at_ms: 900_000,
        };
        assert_eq!(
            format_observation(&spec, &observation),
            "2 (service:nginx.service)"
        );
    }

    #[test]
    fn format_observation_is_bounded_and_printable() {
        let spec = AlertRuleSpec::Threshold {
            metric_id: "a".repeat(400),
            dimension_match: None,
            operator: ThresholdOperator::GreaterThan,
            threshold: 1.0,
        };
        let observation = Observation::Observed {
            value: 1e300,
            at_ms: 0,
        };
        let text = format_observation(&spec, &observation);
        assert!(
            text.len() <= MAX_OBSERVATION_BYTES,
            "got {} bytes",
            text.len()
        );
        assert!(text.chars().all(|c| !c.is_control()));
        // The bound cuts, it never grows.
        assert!(text.starts_with('1'));
    }
}
