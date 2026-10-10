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
    /// History series of the rule's target over the rate window
    /// (rate rules only; empty otherwise), with series identity.
    pub history: &'a [HistorySlice],
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

/// One series' history for rate evaluation: the raw points of a
/// single (metric, dimensions) series with their identity intact.
/// Rate conditions sum ONLY the series matching their own
/// metric_id and dimension subset — never points of other metrics
/// or unrelated dimensions (a group rule may carry rate conditions
/// on different metrics in one evaluation pass).
#[derive(Debug, Clone)]
pub struct HistorySlice {
    pub metric_id: String,
    pub dimensions: std::collections::BTreeMap<String, String>,
    pub points: Vec<HistoryPoint>,
}

/// A condition outcome after the rule's missing-data policy has been
/// applied. `NoData` means "ignore": no state change at all. `Gap`
/// means "unknown": no state change either — a data gap must never
/// clear a pending incident or resolve a firing one — but the gap is
/// recorded on an active incident.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleEvaluation {
    Met { observation: Observation },
    NotMet { observation: Observation },
    NoData,
    Gap { reason: String },
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
        // The rate's metric/dimension selection is applied per
        // series INSIDE evaluate_rate: history carries every
        // matching-metric series with identity, and the condition
        // sums only its own series.
        AlertRuleSpec::Rate {
            metric_id,
            dimension_match,
            operator,
            threshold_per_second,
            window_seconds,
        } => evaluate_rate(
            metric_id,
            dimension_match.as_ref(),
            operator,
            *threshold_per_second,
            *window_seconds,
            data,
        ),
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
    metric_id: &str,
    dimension_match: Option<&DimensionMatch>,
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
    // Only this condition's own series: same metric, dimensions a
    // superset of the (optional) match. Multiple matching series are
    // summed — the aggregate rate of everything selected (e.g. every
    // interface when no dimension_match narrows it).
    let matching: Vec<&HistorySlice> = data
        .history
        .iter()
        .filter(|s| s.metric_id == metric_id && dimensions_subset(&s.dimensions, dimension_match))
        .collect();
    let mut value_sum = 0.0f64;
    let mut window_sum_ms: u64 = 0;
    let mut last_ts: i64 = i64::MIN;
    for slice in matching {
        for point in &slice.points {
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
/// - `Unknown` (default): the gap is recorded on an active incident
///   but nothing changes state — a data gap must never clear a
///   pending incident, resolve a firing one, or reset a hold window.
///   (Recording the gap resets `clear_since`: recovery must be
///   re-established by real false observations, never by absence.)
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
            MissingDataPolicy::Unknown => RuleEvaluation::Gap { reason },
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
            RuleEvaluation::NotMet { .. } | RuleEvaluation::NoData | RuleEvaluation::Gap { .. } => {
                IncidentAction::None
            }
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
            // Unknown-policy absence holds it too, and the gap is
            // recorded on the incident (see apply_action).
            RuleEvaluation::Gap { .. } => IncidentAction::None,
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
            // Unknown-policy absence never resolves either; the gap
            // is recorded (and resets the recovery window — absence
            // is not evidence the condition recovered).
            RuleEvaluation::Gap { .. } => IncidentAction::None,
        },
        // Defensive: the evaluator only loads active incidents, so a
        // resolved snapshot never receives actions.
        IncidentStatus::Resolved => IncidentAction::None,
    }
}

// ---------------------------------------------------------------------------
// Incident identity and rendering
// ---------------------------------------------------------------------------

/// The persisted incident identity lives with the incident store
/// (`chv_controlplane_store::dedup_key`); re-exported here for the
/// evaluator's callers (worker tests, the g4b rig).
pub use chv_controlplane_store::dedup_key;

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
        Observation::Observed { value, .. } => {
            // Check-status observations carry the contract's numeric
            // state code as their value; render the state NAME —
            // `critical (http:app)`, not `2 (http:app)`.
            match spec {
                AlertRuleSpec::CheckStatus { .. } => {
                    let state = match *value as i64 {
                        0 => "ok".to_string(),
                        1 => "warning".to_string(),
                        2 => "critical".to_string(),
                        3 => "unknown".to_string(),
                        other => other.to_string(),
                    };
                    format!("{state} ({label})")
                }
                _ => format!("{value} ({label})"),
            }
        }
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
// Worker: the async half of the evaluator (ADR-027, prompt 05). Each
// tick lists enabled rules, queries the monitoring store for each
// rule's target, runs the pure core above, and applies the returned
// action through the operational store — with the notification
// enqueue on the SAME transaction as the transition it reports.
// ---------------------------------------------------------------------------

/// Channels the bootstrap has destinations configured for; events
/// are enqueued one per channel. Booleans only: no secret crosses
/// into the evaluator (the dispatcher signs at send time).
#[derive(Clone, Default)]
pub struct EvaluatorChannels {
    pub webhook: bool,
    pub slack: bool,
}

impl EvaluatorChannels {
    fn any(&self) -> bool {
        self.webhook || self.slack
    }
}

/// Background worker evaluating alert rules against the monitoring
/// store. Degraded monitoring degrades alerting, never the control
/// plane: every store/query error is logged and skipped (the rule
/// retries next tick); nothing here can panic or block VM lifecycle.
#[derive(Clone)]
pub struct AlertEvaluatorWorker {
    rules: chv_controlplane_store::AlertRuleRepository,
    alerts: chv_controlplane_store::AlertRepository,
    monitoring: std::sync::Arc<chv_monitoring_store::MonitoringStore>,
    channels: EvaluatorChannels,
}

/// History points per rate query (window_seconds ≤ 3600; raw points
/// arrive on the agent cadence, so this is generous headroom).
const RATE_MAX_POINTS: usize = 240;

/// Enabled rules listed (and therefore evaluated) per tick. The
/// create-time rule ceiling is bounded at 500 by
/// `[monitoring.alerting].max_rules` validation (see chv-config), so
/// this covers every possible rule population; it exists only as a
/// belt-and-braces bound on per-tick work.
const MAX_ENABLED_RULES_PER_TICK: i64 = 500;

impl AlertEvaluatorWorker {
    pub fn new(
        rules: chv_controlplane_store::AlertRuleRepository,
        alerts: chv_controlplane_store::AlertRepository,
        monitoring: std::sync::Arc<chv_monitoring_store::MonitoringStore>,
        channels: EvaluatorChannels,
    ) -> Self {
        Self {
            rules,
            alerts,
            monitoring,
            channels,
        }
    }

    /// Run until shutdown, one bounded evaluation pass per tick.
    pub async fn run(
        &self,
        interval: std::time::Duration,
        mut shutdown: tokio::sync::watch::Receiver<()>,
    ) {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = ticker.tick() => {}
            }
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let evaluated = self.evaluation_pass(now_ms).await;
            tracing::debug!(rules_evaluated = evaluated, "alert evaluation pass");
        }
    }

    /// One pass over the enabled rules. Returns how many rules were
    /// evaluated. Individual rule failures are logged and skipped —
    /// one bad rule (or a flaky monitoring query) never blocks the
    /// rest of the tick.
    pub async fn evaluation_pass(&self, now_ms: i64) -> usize {
        let rules = match self.rules.list_enabled(MAX_ENABLED_RULES_PER_TICK).await {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!(error = %e, "alert evaluation: rule listing failed");
                return 0;
            }
        };
        for rule in &rules {
            if let Err(e) = self.evaluate_rule(rule, now_ms).await {
                tracing::warn!(
                    rule_id = %rule.rule_id,
                    error = %e,
                    "alert evaluation: rule skipped"
                );
            }
        }
        rules.len()
    }

    async fn evaluate_rule(
        &self,
        rule: &AlertRule,
        now_ms: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let kind = match rule.target_kind.as_str() {
            "node" => chv_monitoring_core::model::TargetKind::Node,
            "vm" => chv_monitoring_core::model::TargetKind::Vm,
            other => {
                return Err(format!("unknown target_kind {other:?}").into());
            }
        };

        // Gather exactly the data this rule's spec can read.
        let metric_ids = spec_metric_ids(&rule.spec);
        let needs_checks = spec_needs_checks(&rule.spec);
        let rate_window_secs = spec_rate_window_seconds(&rule.spec);

        let current = if metric_ids.is_empty() {
            // Check-status-only rules read no samples.
            Vec::new()
        } else {
            self.monitoring
                .query_current(&kind, &rule.target_id, &metric_ids, None, now_ms as u64)
                .await?
        };
        let checks = if needs_checks {
            self.monitoring
                .query_checks(&kind, &rule.target_id, now_ms as u64)
                .await?
        } else {
            Vec::new()
        };
        let history = if let Some(window_secs) = rate_window_secs {
            let from = (now_ms - window_secs.saturating_mul(1000)).max(0) as u64;
            let series = self
                .monitoring
                .query_history(
                    &kind,
                    &rule.target_id,
                    &metric_ids,
                    None,
                    from,
                    now_ms as u64,
                    RATE_MAX_POINTS,
                    chv_monitoring_store::Resolution::Raw,
                )
                .await?;
            // Series identity is preserved: each rate condition sums
            // only its own (metric, dimensions) series.
            series
                .into_iter()
                .map(|s| HistorySlice {
                    metric_id: s.metric_id,
                    dimensions: s.dimensions,
                    points: s.points,
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let data = EvaluationData {
            now_ms,
            current: &current,
            checks: &checks,
            history: &history,
        };
        let result = evaluate_condition(&rule.spec, &data);
        let evaluation = apply_missing_data_policy(result, rule.missing_data);

        let key = dedup_key(rule);
        let incident = self.alerts.find_active_incident(&key).await?;
        let snapshot = incident.as_ref().map(|row| IncidentSnapshot {
            status: match row.status.as_str() {
                INCIDENT_STATUS_PENDING => IncidentStatus::Pending,
                _ => IncidentStatus::Firing,
            },
            pending_since_ms: row.pending_since_ms.unwrap_or(now_ms),
            clear_since_ms: row.clear_since_ms,
        });

        let action = next_state(
            snapshot.as_ref(),
            &evaluation,
            now_ms,
            rule.for_seconds,
            rule.recovery_seconds,
        );
        self.apply_action(rule, incident.as_ref(), action, &evaluation, now_ms)
            .await?;
        Ok(())
    }

    /// Execute the state machine's decision through the operational
    /// store, with the notification enqueue riding the transition's
    /// transaction. Every branch is idempotent under a racing tick:
    /// guarded updates return "already done" instead of erroring.
    async fn apply_action(
        &self,
        rule: &AlertRule,
        incident: Option<&chv_controlplane_store::IncidentRow>,
        action: IncidentAction,
        evaluation: &RuleEvaluation,
        now_ms: i64,
    ) -> Result<(), chv_controlplane_store::StoreError> {
        let observation_text = match evaluation {
            RuleEvaluation::Met { observation } | RuleEvaluation::NotMet { observation } => {
                format_observation(&rule.spec, observation)
            }
            RuleEvaluation::NoData => "no data (ignored)".to_string(),
            RuleEvaluation::Gap { reason } => format!("no data ({reason})"),
        };
        // Evidence window: from the incident's first occurrence (or
        // the hold window at open) to now.
        let evidence_from = incident
            .and_then(|i| i.first_occurrence_ms)
            .unwrap_or_else(|| {
                now_ms.saturating_sub(rule.for_seconds.saturating_mul(1000).max(1_000))
            });
        let alert_id = incident.map(|i| i.alert_id.clone());

        match action {
            IncidentAction::OpenPending | IncidentAction::OpenFiring => {
                // Stale-pass guard: this pass may hold a rule that was
                // deleted or edited (target, dimension match, any
                // revision bump) after the pass listed it. Opening an
                // incident the CURRENT rule set will never evaluate
                // again would strand it — it could never recover or
                // resolve. Re-read the rule and open only when it is
                // still exactly this one.
                let still_current = match self.rules.get(&rule.rule_id).await {
                    Ok(current) => {
                        current.revision == rule.revision && dedup_key(&current) == dedup_key(rule)
                    }
                    Err(chv_controlplane_store::StoreError::NotFound { .. }) => false,
                    Err(e) => return Err(e),
                };
                if !still_current {
                    tracing::debug!(
                        rule_id = %rule.rule_id,
                        "alert evaluation: rule changed or was deleted mid-pass; not opening an incident"
                    );
                    return Ok(());
                }
                let message = format!("{} (rule '{}')", observation_text, rule.name);
                let alert_id = match self
                    .alerts
                    .open_pending(&chv_controlplane_store::IncidentOpenInput {
                        rule_id: rule.rule_id.clone(),
                        rule_revision: rule.revision,
                        dedup_key: dedup_key(rule),
                        severity: rule.severity.clone(),
                        target_kind: rule.target_kind.clone(),
                        target_id: rule.target_id.clone(),
                        // Monitoring incidents never set node_id: the
                        // FK would couple incident creation to node
                        // existence (a rule can outlive its node),
                        // and target identity already lives in
                        // resource_kind/resource_id + rule linkage.
                        node_id: None,
                        message,
                        now_ms,
                        last_observed: Some(observation_text.clone()),
                        evidence_from_ms: evidence_from,
                        evidence_to_ms: now_ms,
                    })
                    .await
                {
                    Ok(alert_id) => alert_id,
                    Err(e @ chv_controlplane_store::StoreError::Database(_)) => {
                        // A racing tick opened the same dedup key: the
                        // partial unique index already holds it.
                        tracing::debug!(
                            rule_id = %rule.rule_id,
                            error = %e,
                            "alert evaluation: incident already open"
                        );
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                };
                if action == IncidentAction::OpenFiring {
                    // Zero hold: fire in the same pass; the store
                    // records both transitions.
                    let notify = self.notify_events(
                        rule,
                        &alert_id,
                        chv_controlplane_store::EVENT_TYPE_FIRING,
                        &observation_text,
                        now_ms,
                        None,
                    );
                    self.alerts
                        .promote_to_firing(
                            &alert_id,
                            now_ms,
                            Some(&observation_text),
                            evidence_from,
                            now_ms,
                            &notify,
                        )
                        .await?;
                }
            }
            IncidentAction::PromoteToFiring => {
                let Some(alert_id) = &alert_id else {
                    return Ok(());
                };
                let notify = self.notify_events(
                    rule,
                    alert_id,
                    chv_controlplane_store::EVENT_TYPE_FIRING,
                    &observation_text,
                    now_ms,
                    incident,
                );
                self.alerts
                    .promote_to_firing(
                        alert_id,
                        now_ms,
                        Some(&observation_text),
                        evidence_from,
                        now_ms,
                        &notify,
                    )
                    .await?;
            }
            IncidentAction::Observe => {
                let Some(alert_id) = &alert_id else {
                    return Ok(());
                };
                self.alerts
                    .note_observation(
                        alert_id,
                        now_ms,
                        Some(&observation_text),
                        evidence_from,
                        now_ms,
                    )
                    .await?;
            }
            IncidentAction::None => {
                // A recorded gap (unknown policy on an active
                // incident) still refreshes the observation — which
                // also resets the recovery window: absence is not
                // evidence the condition recovered.
                if let RuleEvaluation::Gap { .. } = evaluation {
                    if let Some(alert_id) = &alert_id {
                        self.alerts
                            .note_observation(
                                alert_id,
                                now_ms,
                                Some(&observation_text),
                                evidence_from,
                                now_ms,
                            )
                            .await?;
                    }
                }
            }
            IncidentAction::ClearPending => {
                let Some(alert_id) = &alert_id else {
                    return Ok(());
                };
                self.alerts.clear_pending(alert_id).await?;
            }
            IncidentAction::StartRecovery => {
                let Some(alert_id) = &alert_id else {
                    return Ok(());
                };
                self.alerts.mark_condition_false(alert_id, now_ms).await?;
            }
            IncidentAction::Resolve => {
                let Some(alert_id) = &alert_id else {
                    return Ok(());
                };
                let notify = self.notify_events(
                    rule,
                    alert_id,
                    chv_controlplane_store::EVENT_TYPE_RESOLVED,
                    &observation_text,
                    now_ms,
                    incident,
                );
                self.alerts
                    .resolve_incident(
                        alert_id,
                        now_ms,
                        "condition false for the recovery window",
                        Some(&observation_text),
                        &notify,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// Build the notification events for a transition, one per
    /// configured channel — or an empty vec when notifications are
    /// off, no destination exists, or the incident is silenced (an
    /// overlay that suppresses delivery, never a resolution). All
    /// events carry the same pre-rendered envelope; the dispatcher
    /// renders channel-specific bodies at send time. Enqueueing on
    /// the transition's transaction is the caller's job (the store
    /// methods take the slice).
    fn notify_events(
        &self,
        rule: &AlertRule,
        alert_id: &str,
        event_type: &str,
        observation_text: &str,
        now_ms: i64,
        incident: Option<&chv_controlplane_store::IncidentRow>,
    ) -> Vec<chv_controlplane_store::NotificationEventInput> {
        if !self.channels.any() {
            return Vec::new();
        }
        // Silence is an overlay: the transition still happens, only
        // the notification is suppressed.
        if let Some(until) = incident.and_then(|i| i.silenced_until_ms) {
            if until > now_ms {
                return Vec::new();
            }
        }
        let summary = truncate_summary(&format!(
            "{} — {} ({})",
            rule.name, observation_text, rule.severity
        ));
        let resource_url = format!("/{}s/{}", rule.target_kind, rule.target_id);
        let incident_key = dedup_key(rule);
        let channels = [
            (
                self.channels.webhook,
                chv_controlplane_store::CHANNEL_WEBHOOK,
            ),
            (self.channels.slack, chv_controlplane_store::CHANNEL_SLACK),
        ];
        channels
            .iter()
            .filter(|(configured, _)| *configured)
            .map(|(_, channel)| {
                let event_id = uuid::Uuid::new_v4().to_string();
                let payload = chv_monitoring_core::notifications::render_envelope(
                    &event_id,
                    alert_id,
                    event_type,
                    &rule.severity,
                    &rule.target_kind,
                    &rule.target_id,
                    &summary,
                    now_ms,
                    &resource_url,
                );
                chv_controlplane_store::NotificationEventInput {
                    event_id,
                    alert_id: alert_id.to_string(),
                    incident_key: incident_key.clone(),
                    event_type: event_type.to_string(),
                    severity: rule.severity.clone(),
                    target_kind: rule.target_kind.clone(),
                    target_id: rule.target_id.clone(),
                    summary: summary.clone(),
                    occurred_at_ms: now_ms,
                    payload,
                    channel: channel.to_string(),
                }
            })
            .collect()
    }
}

/// Bound a notification summary to the outbox's 512-byte validation.
fn truncate_summary(text: &str) -> String {
    const MAX: usize = 480;
    if text.len() <= MAX {
        return text.to_string();
    }
    let mut cut = MAX;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_string()
}

/// Metric ids a spec (recursively) reads — deduplicated, since group
/// conditions may share a metric. Empty for check-status-only specs.
fn spec_metric_ids(spec: &AlertRuleSpec) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    fn collect(spec: &AlertRuleSpec, ids: &mut Vec<String>) {
        match spec {
            AlertRuleSpec::Threshold { metric_id, .. }
            | AlertRuleSpec::Rate { metric_id, .. }
            | AlertRuleSpec::Availability { metric_id, .. } => {
                if !ids.contains(metric_id) {
                    ids.push(metric_id.clone());
                }
            }
            AlertRuleSpec::CheckStatus { .. } => {}
            AlertRuleSpec::Group { conditions, .. } => {
                for condition in conditions {
                    collect(condition, ids);
                }
            }
        }
    }
    collect(spec, &mut ids);
    ids
}

/// Whether any (nested) condition reads the guest check inventory.
fn spec_needs_checks(spec: &AlertRuleSpec) -> bool {
    match spec {
        AlertRuleSpec::CheckStatus { .. } => true,
        AlertRuleSpec::Group { conditions, .. } => conditions.iter().any(spec_needs_checks),
        _ => false,
    }
}

/// The widest rate window a (nested) rate condition requires; `None`
/// when the spec reads no rates. The pure core filters points to each
/// condition's own window, so one query at the widest window feeds
/// every condition.
fn spec_rate_window_seconds(spec: &AlertRuleSpec) -> Option<i64> {
    match spec {
        AlertRuleSpec::Rate { window_seconds, .. } => Some(*window_seconds),
        AlertRuleSpec::Group { conditions, .. } => {
            conditions.iter().filter_map(spec_rate_window_seconds).max()
        }
        _ => None,
    }
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

    fn slice(metric_id: &str, points: &[HistoryPoint]) -> HistorySlice {
        HistorySlice {
            metric_id: metric_id.to_string(),
            dimensions: Default::default(),
            points: points.to_vec(),
        }
    }

    fn data<'a>(
        now_ms: i64,
        current: &'a [CurrentSample],
        checks: &'a [StoredCheck],
        history: &'a [HistorySlice],
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

    fn rate_history() -> Vec<HistorySlice> {
        // Window [9_700_000, 10_000_000] (300 s rule): 900 counter
        // units over 150 s of real observed windows — coverage is
        // exactly half the rule's window.
        vec![rate_points(&[
            point(9_750_000, 10_000, Some(100.0)),
            point(9_800_000, 20_000, Some(200.0)),
            point(9_900_000, 120_000, Some(600.0)),
        ])]
    }

    /// The rate spec's own series carrying the given points.
    fn rate_points(points: &[HistoryPoint]) -> HistorySlice {
        slice("vm.guest.net.rx_errors_total", points)
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
        let history = [rate_points(&[point(9_900_000, 10_000, Some(100.0))])];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "insufficient_samples");
    }

    #[test]
    fn rate_skips_points_without_values() {
        let spec = rate_spec(2.0, 300);
        let history = [rate_points(&[
            point(9_750_000, 100_000, None), // reset/epoch-crossing bucket
            point(9_850_000, 100_000, Some(200.0)),
            point(9_950_000, 100_000, Some(400.0)),
        ])];
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
        let history = [rate_points(&[
            point(9_950_000, 30_000, Some(-50.0)), // defensive reset guard
            point(9_980_000, 30_000, Some(600.0)),
        ])];
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
        let history = [rate_points(&[
            point(7_000_000, 290_000, Some(5_000.0)), // before the window
            point(9_900_000, 5_000, Some(100.0)),     // in window
            point(10_500_000, 100_000, Some(900.0)),  // future-dated
        ])];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        // Only 5 s of in-window coverage against a 300 s window.
        assert_eq!(missing_reason(&result), "insufficient_samples");
    }

    #[test]
    fn rate_zero_summed_window_is_missing_not_a_division_by_zero() {
        let spec = rate_spec(5.0, 60);
        let history = [rate_points(&[point(10_000_000, 0, Some(10.0))])];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert_eq!(missing_reason(&result), "no_history");
    }

    #[test]
    fn rate_sums_only_its_own_metric_series() {
        // Regression: history used to be flattened across series, so
        // a group rule with rate conditions on different metrics (or
        // a second series on the target) computed one combined rate.
        let spec = rate_spec(3.0, 300);
        let history = [
            rate_points(&[point(9_900_000, 150_000, Some(600.0))]), // 4/s
            HistorySlice {
                metric_id: "vm.guest.net.tx_errors_total".to_string(),
                dimensions: Default::default(),
                points: vec![point(9_900_000, 150_000, Some(600_000.0))], // would be 4000/s
            },
        ];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert!((value_of(&result) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rate_dimension_match_selects_only_matching_series() {
        let spec = AlertRuleSpec::Rate {
            metric_id: "vm.guest.net.rx_errors_total".to_string(),
            dimension_match: Some([("interface_id".to_string(), "eth0".to_string())].into()),
            operator: ThresholdOperator::GreaterThan,
            threshold_per_second: 3.0,
            window_seconds: 300,
        };
        let eth0 = HistorySlice {
            metric_id: "vm.guest.net.rx_errors_total".to_string(),
            dimensions: [("interface_id".to_string(), "eth0".to_string())].into(),
            points: vec![point(9_900_000, 150_000, Some(600.0))], // 4/s -> Met
        };
        let eth1 = HistorySlice {
            metric_id: "vm.guest.net.rx_errors_total".to_string(),
            dimensions: [("interface_id".to_string(), "eth1".to_string())].into(),
            points: vec![point(9_900_000, 150_000, Some(600_000.0))], // excluded
        };
        let history = [eth0, eth1];
        let input = data(10_000_000, &[], &[], &history);
        let result = evaluate_condition(&spec, &input);
        assert!(matches!(result, ConditionResult::Met { .. }));
        assert!((value_of(&result) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rate_no_matching_series_is_missing() {
        let spec = AlertRuleSpec::Rate {
            metric_id: "vm.guest.net.rx_errors_total".to_string(),
            dimension_match: Some([("interface_id".to_string(), "eth9".to_string())].into()),
            operator: ThresholdOperator::GreaterThan,
            threshold_per_second: 5.0,
            window_seconds: 300,
        };
        let other_dims = HistorySlice {
            metric_id: "vm.guest.net.rx_errors_total".to_string(),
            dimensions: [("interface_id".to_string(), "eth0".to_string())].into(),
            points: vec![point(9_900_000, 100_000, Some(600.0))],
        };
        let history = [other_dims];
        let input = data(10_000_000, &[], &[], &history);
        assert_eq!(
            missing_reason(&evaluate_condition(&spec, &input)),
            "no_history"
        );
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
        // Missing + Unknown: hold the state and record the gap — a
        // data gap is never evidence the condition stopped.
        assert_eq!(
            apply_missing_data_policy(
                ConditionResult::Missing {
                    reason: "stale".to_string()
                },
                MissingDataPolicy::Unknown
            ),
            RuleEvaluation::Gap {
                reason: "stale".to_string()
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
            "critical (service:nginx.service)"
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

/// End-to-end worker tests: a real file-backed monitoring store, the
/// real operational store, and the real pure core — no fakes between
/// them. Times are explicit (`evaluation_pass(now_ms)`), so the state
/// machine is driven deterministically tick by tick.
#[cfg(test)]
mod worker_tests {
    use super::*;
    use chv_controlplane_store::test_util::create_test_pool;
    use chv_controlplane_store::{
        AlertRepository, AlertRuleRepository, NotificationOutboxRepository, RuleCreateInput,
        RuleUpdateInput,
    };
    use chv_monitoring_core::model::{SampleBuilder, SampleValue, Source, TargetKind};
    use chv_monitoring_store::{IngestOutcome, MonitoringStore, MonitoringStoreConfig, NodeBatch};

    struct Fixture {
        worker: AlertEvaluatorWorker,
        rules: AlertRuleRepository,
        alerts: AlertRepository,
        outbox: NotificationOutboxRepository,
        monitoring: std::sync::Arc<MonitoringStore>,
        /// Monotonic batch sequence (the store dedups replays by
        /// boot_id + sequence).
        next_sequence: std::cell::Cell<u64>,
        _dir: tempfile::TempDir,
    }

    /// Fixed evaluation clock base: passes run at BASE, BASE+30s, …
    const BASE_MS: i64 = 10_000_000_000;

    async fn fixture(channels: EvaluatorChannels) -> Fixture {
        let pool = create_test_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let monitoring = std::sync::Arc::new(
            MonitoringStore::connect(MonitoringStoreConfig {
                database_url: format!("sqlite://{}/monitoring.db", dir.path().display()),
                migrations_dir: std::path::PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../cmd/chv-controlplane/monitoring-migrations"
                )),
                ..MonitoringStoreConfig::default()
            })
            .await
            .expect("connect monitoring store"),
        );
        let rules = AlertRuleRepository::new(pool.clone());
        let alerts = AlertRepository::new(pool.clone());
        let outbox = NotificationOutboxRepository::new(pool.clone());
        let worker =
            AlertEvaluatorWorker::new(rules.clone(), alerts.clone(), monitoring.clone(), channels);
        Fixture {
            worker,
            rules,
            alerts,
            outbox,
            monitoring,
            next_sequence: std::cell::Cell::new(0),
            _dir: dir,
        }
    }

    impl Fixture {
        /// Ingest one gauge sample for node-1's CPU ratio.
        async fn seed_cpu(&self, observed_at_ms: u64, value: f64) {
            let sample = SampleBuilder::new(
                TargetKind::Node,
                "node-1",
                "node.cpu.capacity_ratio",
                Source::NodeOs,
                observed_at_ms,
            )
            .unwrap()
            .value(SampleValue::Float(value))
            .build()
            .unwrap();
            let sequence = self.next_sequence.get();
            self.next_sequence.set(sequence + 1);
            let batch = NodeBatch {
                boot_id: "boot-1".to_string(),
                sequence,
                sent_at_ms: observed_at_ms,
                samples: vec![sample],
            };
            let outcome = self
                .monitoring
                .ingest_node_batch("node-1", &batch, observed_at_ms)
                .await
                .expect("ingest");
            assert!(matches!(outcome, IngestOutcome::Accepted { samples: 1 }));
        }
    }

    fn cpu_rule(for_seconds: i64, recovery_seconds: i64, threshold: f64) -> RuleCreateInput {
        RuleCreateInput {
            name: "Node CPU pressure".into(),
            enabled: true,
            target_kind: "node".into(),
            target_id: "node-1".into(),
            spec: AlertRuleSpec::Threshold {
                metric_id: "node.cpu.capacity_ratio".into(),
                dimension_match: None,
                operator: ThresholdOperator::GreaterThan,
                threshold,
            },
            severity: "warning".into(),
            for_seconds,
            recovery_seconds,
            missing_data: MissingDataPolicy::Unknown,
            created_by: "test".into(),
            now_ms: BASE_MS,
        }
    }

    async fn active_incident(
        f: &Fixture,
        dedup: &str,
    ) -> Option<chv_controlplane_store::IncidentRow> {
        f.alerts.find_active_incident(dedup).await.expect("find")
    }

    async fn outbox_count(f: &Fixture, event_type: &str) -> usize {
        f.outbox
            .list_recent(100)
            .await
            .expect("list")
            .iter()
            .filter(|e| e.event_type == event_type)
            .count()
    }

    #[tokio::test]
    async fn stale_rule_snapshot_never_opens_an_incident() {
        // A pass can hold a rule row that was deleted or edited after
        // the pass listed it. Opening an incident from the stale
        // snapshot would strand it: the current rule set would never
        // evaluate that dedup key again, so it could never recover or
        // resolve. The open path re-reads the rule and refuses.
        let f = fixture(EvaluatorChannels {
            webhook: false,
            slack: false,
        })
        .await;
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");
        f.seed_cpu(BASE_MS as u64 - 30_000, 0.95).await;

        // Deleted mid-pass: the stale snapshot must not open.
        f.rules
            .delete(&rule.rule_id, rule.revision)
            .await
            .expect("delete");
        f.worker
            .evaluate_rule(&rule, BASE_MS)
            .await
            .expect("evaluate stale snapshot");
        assert!(
            active_incident(&f, &dedup_key(&rule)).await.is_none(),
            "a deleted rule's stale snapshot must not open an incident"
        );

        // Edited mid-pass to a different dimension match (the dedup
        // key changes): the stale snapshot must not open at the OLD
        // key.
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule 2");
        let updated = f
            .rules
            .update(&RuleUpdateInput {
                rule_id: rule.rule_id.clone(),
                expected_revision: rule.revision,
                name: rule.name.clone(),
                enabled: None,
                spec: AlertRuleSpec::Threshold {
                    metric_id: "vm.guest.fs.available_bytes".into(),
                    dimension_match: Some([("mount_id".to_string(), "/".to_string())].into()),
                    operator: ThresholdOperator::LessThan,
                    threshold: 2.0,
                },
                severity: "warning".into(),
                for_seconds: 0,
                recovery_seconds: 60,
                missing_data: MissingDataPolicy::Unknown,
                updated_by: "test".into(),
                now_ms: BASE_MS,
            })
            .await
            .expect("update");
        assert_ne!(dedup_key(&updated), dedup_key(&rule));
        f.worker
            .evaluate_rule(&rule, BASE_MS)
            .await
            .expect("evaluate stale snapshot");
        assert!(
            active_incident(&f, &dedup_key(&rule)).await.is_none(),
            "the old dedup key's stale snapshot must not open"
        );
    }

    #[tokio::test]
    async fn zero_hold_rule_fires_immediately_with_notification() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: false,
        })
        .await;
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        let evaluated = f.worker.evaluation_pass(BASE_MS).await;
        assert_eq!(evaluated, 1);

        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("incident opened");
        assert_eq!(incident.status, "firing");
        assert_eq!(incident.rule_revision, Some(1));
        // The firing notification was enqueued on the transition.
        assert_eq!(outbox_count(&f, "firing").await, 1);
    }

    #[tokio::test]
    async fn hold_window_promotes_then_recovers() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: false,
        })
        .await;
        // 120s hold, 60s recovery.
        let rule = f.rules.create(&cpu_rule(120, 60, 0.9)).await.expect("rule");

        // Tick 1: condition true, hold not elapsed -> pending, no
        // notification yet.
        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("pending opened");
        assert_eq!(incident.status, "pending");
        assert_eq!(outbox_count(&f, "firing").await, 0);

        // Tick 2 at +180s: hold elapsed -> firing + notification.
        f.seed_cpu((BASE_MS as u64) + 150_000, 0.96).await;
        f.worker.evaluation_pass(BASE_MS + 180_000).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("still open");
        assert_eq!(incident.status, "firing");
        assert_eq!(outbox_count(&f, "firing").await, 1);

        // Tick 3 at +210s: condition false -> recovery starts, no
        // resolution yet.
        f.seed_cpu((BASE_MS as u64) + 200_000, 0.5).await;
        f.worker.evaluation_pass(BASE_MS + 210_000).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("still firing");
        assert_eq!(incident.status, "firing");
        assert_eq!(incident.clear_since_ms, Some(BASE_MS + 210_000));

        // Tick 4 at +280s: a FRESH false observation (resolution is
        // never based on stale/absent data) with the recovery window
        // elapsed -> resolved + notification.
        f.seed_cpu((BASE_MS as u64) + 250_000, 0.5).await;
        f.worker.evaluation_pass(BASE_MS + 280_000).await;
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_none());
        assert_eq!(outbox_count(&f, "resolved").await, 1);
        // The firing and resolved events are the only notifications.
        let all = f.outbox.list_recent(100).await.expect("list");
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn pending_cleared_before_hold_is_deleted() {
        let f = fixture(EvaluatorChannels::default()).await;
        let rule = f.rules.create(&cpu_rule(600, 60, 0.9)).await.expect("rule");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_some());

        // Condition false well before the 600s hold: the pending
        // incident is deleted, not stored as noise.
        f.seed_cpu((BASE_MS as u64) + 30_000, 0.4).await;
        f.worker.evaluation_pass(BASE_MS + 60_000).await;
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_none());
    }

    #[tokio::test]
    async fn missing_data_never_fires_unknown_policy() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: false,
        })
        .await;
        // No samples ingested at all: the series is absent.
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");
        f.worker.evaluation_pass(BASE_MS).await;
        assert!(
            active_incident(&f, &dedup_key(&rule)).await.is_none(),
            "absent data must not fire a threshold rule under the unknown policy"
        );
        assert_eq!(f.outbox.list_recent(100).await.expect("list").len(), 0);
    }

    #[tokio::test]
    async fn stale_sample_is_not_zero() {
        let f = fixture(EvaluatorChannels::default()).await;
        // A `value < 0.5` rule would fire on a fabricated zero; a
        // stale sample must evaluate Missing instead.
        let mut input = cpu_rule(0, 60, 0.5);
        input.spec = AlertRuleSpec::Threshold {
            metric_id: "node.cpu.capacity_ratio".into(),
            dimension_match: None,
            operator: ThresholdOperator::LessThan,
            threshold: 0.5,
        };
        let rule = f.rules.create(&input).await.expect("rule");

        // Sample ingested, but evaluated 10 minutes later: stale.
        f.seed_cpu((BASE_MS as u64) - 600_000, 0.1).await;
        f.worker.evaluation_pass(BASE_MS).await;
        assert!(
            active_incident(&f, &dedup_key(&rule)).await.is_none(),
            "stale data must not be read as zero"
        );
    }

    #[tokio::test]
    async fn availability_rule_fires_on_absence() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: false,
        })
        .await;
        let mut input = cpu_rule(0, 60, 0.9);
        input.spec = AlertRuleSpec::Availability {
            metric_id: "node.cpu.capacity_ratio".into(),
            dimension_match: None,
        };
        let rule = f.rules.create(&input).await.expect("rule");

        // Absence IS the condition.
        f.worker.evaluation_pass(BASE_MS).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("availability fires on absence");
        assert_eq!(incident.status, "firing");
        assert_eq!(outbox_count(&f, "firing").await, 1);

        // A fresh healthy value starts recovery…
        f.seed_cpu((BASE_MS as u64) + 30_000, 0.42).await;
        f.worker.evaluation_pass(BASE_MS + 60_000).await;
        // …and a still-fresh value after the recovery window resolves
        // it (a stale sample would read as absence and re-fire).
        f.seed_cpu((BASE_MS as u64) + 100_000, 0.42).await;
        f.worker.evaluation_pass(BASE_MS + 130_000).await;
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_none());
        assert_eq!(outbox_count(&f, "resolved").await, 1);
    }

    #[tokio::test]
    async fn silenced_incident_transitions_without_notifying() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: false,
        })
        .await;
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("firing");
        assert_eq!(outbox_count(&f, "firing").await, 1);

        // Silence past the next evaluation, then let the condition
        // clear: the transition happens, the notification does not.
        f.alerts
            .silence_incident(&incident.alert_id, "op-user", BASE_MS + 300_000, BASE_MS)
            .await
            .expect("silence");
        f.seed_cpu((BASE_MS as u64) + 60_000, 0.4).await;
        f.worker.evaluation_pass(BASE_MS + 120_000).await;
        // A fresh false observation for the resolving pass (stale
        // data would hold the incident, not resolve it).
        f.seed_cpu((BASE_MS as u64) + 190_000, 0.4).await;
        f.worker.evaluation_pass(BASE_MS + 200_000).await;
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_none());
        assert_eq!(
            outbox_count(&f, "resolved").await,
            0,
            "a silenced incident resolves without notifying"
        );
    }

    #[tokio::test]
    async fn no_destination_means_incidents_without_outbox_rows() {
        let f = fixture(EvaluatorChannels::default()).await;
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("incident");
        assert_eq!(incident.status, "firing");
        assert_eq!(
            f.outbox.list_recent(100).await.expect("list").len(),
            0,
            "unconfigured notifications enqueue nothing"
        );
    }

    #[tokio::test]
    async fn both_channels_enqueue_one_event_each() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: true,
        })
        .await;
        f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        let events = f.outbox.list_recent(100).await.expect("list");
        assert_eq!(events.len(), 2);
        let channels: std::collections::BTreeSet<&str> =
            events.iter().map(|e| e.channel.as_str()).collect();
        assert_eq!(
            channels,
            std::collections::BTreeSet::from(["webhook", "slack"])
        );
        // Both carry the same pre-rendered envelope.
        for event in &events {
            let payload: serde_json::Value =
                serde_json::from_str(&event.payload).expect("envelope json");
            assert_eq!(payload["event_type"], "firing");
            assert_eq!(payload["severity"], "warning");
            assert_eq!(payload["target_id"], "node-1");
        }
    }

    #[tokio::test]
    async fn rule_revision_updates_incident_openings() {
        let f = fixture(EvaluatorChannels::default()).await;
        let rule = f.rules.create(&cpu_rule(600, 60, 0.9)).await.expect("rule");

        // Open pending under revision 1.
        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("pending");
        assert_eq!(incident.rule_revision, Some(1));

        // Edit the rule (revision 2): the pending incident keeps
        // evaluating under the new revision at promotion time.
        f.rules
            .update(&chv_controlplane_store::RuleUpdateInput {
                rule_id: rule.rule_id.clone(),
                expected_revision: 1,
                name: "Node CPU pressure".into(),
                enabled: Some(true),
                spec: AlertRuleSpec::Threshold {
                    metric_id: "node.cpu.capacity_ratio".into(),
                    dimension_match: None,
                    operator: ThresholdOperator::GreaterThan,
                    threshold: 0.9,
                },
                severity: "critical".into(),
                for_seconds: 120,
                recovery_seconds: 60,
                missing_data: MissingDataPolicy::Unknown,
                updated_by: "test".into(),
                now_ms: BASE_MS + 10_000,
            })
            .await
            .expect("update");

        f.seed_cpu((BASE_MS as u64) + 150_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS + 180_000).await;
        let incident = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("promoted");
        assert_eq!(incident.status, "firing");
        // Severity is captured at open time: the mid-pending rule edit
        // applies to future incidents, not the one already open.
        assert_eq!(incident.severity, "warning");
    }

    #[tokio::test]
    async fn disabled_rules_are_not_evaluated() {
        let f = fixture(EvaluatorChannels::default()).await;
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");
        f.rules
            .update(&chv_controlplane_store::RuleUpdateInput {
                rule_id: rule.rule_id.clone(),
                expected_revision: 1,
                name: "Node CPU pressure".into(),
                enabled: Some(false),
                spec: AlertRuleSpec::Threshold {
                    metric_id: "node.cpu.capacity_ratio".into(),
                    dimension_match: None,
                    operator: ThresholdOperator::GreaterThan,
                    threshold: 0.9,
                },
                severity: "warning".into(),
                for_seconds: 0,
                recovery_seconds: 60,
                missing_data: MissingDataPolicy::Unknown,
                updated_by: "test".into(),
                now_ms: BASE_MS + 10_000,
            })
            .await
            .expect("disable");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        let evaluated = f.worker.evaluation_pass(BASE_MS).await;
        assert_eq!(evaluated, 0, "disabled rules are skipped");
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_none());
    }

    #[tokio::test]
    async fn data_gap_holds_state_and_resets_recovery_under_unknown_policy() {
        let f = fixture(EvaluatorChannels {
            webhook: true,
            slack: false,
        })
        .await;
        let rule = f.rules.create(&cpu_rule(0, 60, 0.9)).await.expect("rule");

        // Fire on a fresh high value.
        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        f.worker.evaluation_pass(BASE_MS).await;
        active_incident(&f, &dedup_key(&rule))
            .await
            .expect("firing");
        assert_eq!(outbox_count(&f, "firing").await, 1);

        // The feed stops. Passes far past the recovery window with
        // stale data must NOT resolve the incident and must NOT
        // notify: absence is never evidence of recovery.
        f.worker.evaluation_pass(BASE_MS + 600_000).await;
        f.worker.evaluation_pass(BASE_MS + 700_000).await;
        let held = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("still firing across the gap");
        assert_eq!(held.status, "firing");
        assert_eq!(outbox_count(&f, "resolved").await, 0);
        assert!(
            held.last_observed
                .as_deref()
                .unwrap_or("")
                .contains("no data"),
            "the gap is recorded on the incident: {:?}",
            held.last_observed
        );

        // A real false observation starts recovery…
        f.seed_cpu((BASE_MS as u64) + 800_000, 0.4).await;
        f.worker.evaluation_pass(BASE_MS + 830_000).await;
        let recovering = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("still firing");
        assert_eq!(
            recovering.clear_since_ms,
            Some(BASE_MS + 830_000),
            "recovery starts on the real false observation"
        );

        // …another gap resets it (absence cannot carry the recovery
        // window to completion).
        f.worker.evaluation_pass(BASE_MS + 900_000).await;
        let gapped = active_incident(&f, &dedup_key(&rule))
            .await
            .expect("still firing");
        assert_eq!(
            gapped.clear_since_ms, None,
            "the gap resets the recovery window"
        );

        // …and a contiguous fresh false window finally resolves.
        f.seed_cpu((BASE_MS as u64) + 950_000, 0.4).await;
        f.worker.evaluation_pass(BASE_MS + 980_000).await;
        f.seed_cpu((BASE_MS as u64) + 1_050_000, 0.4).await;
        f.worker.evaluation_pass(BASE_MS + 1_080_000).await;
        assert!(
            active_incident(&f, &dedup_key(&rule)).await.is_none(),
            "a real contiguous false window resolves"
        );
        assert_eq!(outbox_count(&f, "resolved").await, 1);
    }

    #[tokio::test]
    async fn every_enabled_rule_evaluates_beyond_old_list_limits() {
        // Regression: the pass once listed enabled rules with a
        // history-points constant (240) as the limit, silently
        // freezing rules 241+ out of evaluation. The create ceiling
        // allows up to 500, so a pass must evaluate them all.
        let f = fixture(EvaluatorChannels::default()).await;
        for i in 0..260 {
            let mut input = cpu_rule(600, 60, 0.9);
            input.name = format!("rule {i:03}");
            f.rules.create(&input).await.expect("rule create");
        }
        let evaluated = f.worker.evaluation_pass(BASE_MS).await;
        assert_eq!(
            evaluated, 260,
            "all enabled rules evaluate regardless of position in the list"
        );
    }

    #[tokio::test]
    async fn rules_created_disabled_do_not_evaluate() {
        let f = fixture(EvaluatorChannels::default()).await;
        let mut input = cpu_rule(0, 60, 0.9);
        input.enabled = false;
        let rule = f.rules.create(&input).await.expect("rule");
        assert!(!rule.enabled, "create honors the enabled flag");

        f.seed_cpu((BASE_MS as u64) - 30_000, 0.95).await;
        let evaluated = f.worker.evaluation_pass(BASE_MS).await;
        assert_eq!(evaluated, 0);
        assert!(active_incident(&f, &dedup_key(&rule)).await.is_none());

        // An update that omits `enabled` keeps the disabled state
        // (a partial update must never silently re-enable).
        f.rules
            .update(&chv_controlplane_store::RuleUpdateInput {
                rule_id: rule.rule_id.clone(),
                expected_revision: 1,
                name: "renamed".into(),
                enabled: None,
                spec: AlertRuleSpec::Threshold {
                    metric_id: "node.cpu.capacity_ratio".into(),
                    dimension_match: None,
                    operator: ThresholdOperator::GreaterThan,
                    threshold: 0.9,
                },
                severity: "warning".into(),
                for_seconds: 0,
                recovery_seconds: 60,
                missing_data: MissingDataPolicy::Unknown,
                updated_by: "test".into(),
                now_ms: BASE_MS + 10_000,
            })
            .await
            .expect("rename");
        let evaluated = f.worker.evaluation_pass(BASE_MS).await;
        assert_eq!(evaluated, 0, "omitting enabled keeps the rule disabled");
    }
}
