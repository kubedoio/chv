//! Typed alert rules (ADR-027, campaign #602, prompt 05 / gate G4
//! part 2).
//!
//! Rules are control-plane authority: durable workflow metadata in
//! the operational database, never time-series storage. v1 rules bind
//! exactly one target (no fleet wildcards) and one typed condition
//! set — threshold, reset-safe rate, availability (staleness),
//! guest-check status, or a one-level bounded AND/OR group. There is
//! deliberately no PromQL, no SQL and no code execution anywhere in
//! the rule model.
//!
//! Layered validation: this module enforces the STRUCTURE (typed
//! shapes, bounds, charsets, nesting limits) at every load — a row
//! whose `spec` no longer parses is a loud error, never a silent
//! skip. Metric-registry membership (does `metric_id` exist) is
//! enforced by the service layer, which owns the registry
//! dependency; the store never guesses about metrics.
//!
//! Concurrency: updates and deletes carry the caller's expected
//! `revision`; a mismatch is [`StoreError::StaleVersion`] and changes
//! nothing. Revisions advance monotonically and are never reused.

use crate::{StoreError, StorePool};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Dimension match: exact series selection. `BTreeMap` so the
/// canonical serialization (and therefore the incident dedup key) is
/// order-independent.
pub type DimensionMatch = BTreeMap<String, String>;

/// Maximum conditions in a group rule (spec: bounded AND/OR).
pub const MAX_GROUP_CONDITIONS: usize = 5;
/// Maximum keys in a dimension match.
pub const MAX_DIMENSION_MATCH_KEYS: usize = 2;
/// Bound for every identifier-ish string in a rule.
pub const MAX_ID_BYTES: usize = 128;
/// Bound for rule names.
pub const MAX_NAME_BYTES: usize = 128;
/// Hold-down / recovery bounds (seconds).
pub const MAX_FOR_SECONDS: i64 = 86_400;
/// Rate window bounds (seconds).
pub const MIN_RATE_WINDOW_SECONDS: i64 = 30;
pub const MAX_RATE_WINDOW_SECONDS: i64 = 3_600;

// ---------------------------------------------------------------------------
// Typed rule spec
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdOperator {
    GreaterThan,
    LessThan,
}

impl ThresholdOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThresholdOperator::GreaterThan => "greater_than",
            ThresholdOperator::LessThan => "less_than",
        }
    }

    /// Evaluate the comparison. Total on finite inputs.
    pub fn matches(&self, value: f64, threshold: f64) -> bool {
        match self {
            ThresholdOperator::GreaterThan => value > threshold,
            ThresholdOperator::LessThan => value < threshold,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingDataPolicy {
    /// Record the gap on the incident; do not fire (default).
    Unknown,
    /// Treat missing data as condition-true (fire on absence).
    Fire,
    /// Ignore missing data entirely (no state change).
    Ignore,
}

impl MissingDataPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            MissingDataPolicy::Unknown => "unknown",
            MissingDataPolicy::Fire => "fire",
            MissingDataPolicy::Ignore => "ignore",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatusMatch {
    Critical,
    Warning,
    Unknown,
}

impl CheckStatusMatch {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckStatusMatch::Critical => "critical",
            CheckStatusMatch::Warning => "warning",
            CheckStatusMatch::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupOp {
    And,
    Or,
}

impl GroupOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            GroupOp::And => "and",
            GroupOp::Or => "or",
        }
    }
}

/// The typed condition set. Serialized as the rule's `spec` column.
///
/// Deserialization is deliberately manual: the variants are
/// distinguished by their exact field sets (`deny_unknown_fields` on
/// each shape struct), so a mis-shaped spec — say a threshold with a
/// typo'd operator — fails loudly instead of silently parsing as a
/// bare `Availability` that ignores the unknown fields. The shapes
/// are disjoint by construction: each accepts only its own fields,
/// and a JSON object matching no shape is an error.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AlertRuleSpec {
    /// Absolute threshold on a metric's latest valid value.
    Threshold {
        metric_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dimension_match: Option<DimensionMatch>,
        operator: ThresholdOperator,
        threshold: f64,
    },
    /// Reset-safe rate: monotonic counter delta over a window,
    /// negative deltas (counter resets) skipped, never fabricated.
    Rate {
        metric_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dimension_match: Option<DimensionMatch>,
        operator: ThresholdOperator,
        threshold_per_second: f64,
        window_seconds: i64,
    },
    /// Availability: the condition is TRUE when the series is stale
    /// or not collected (the spec's missing-data rule class).
    Availability {
        metric_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dimension_match: Option<DimensionMatch>,
    },
    /// Guest check status (PR-5's check inventory).
    CheckStatus {
        check_id: String,
        status_match: CheckStatusMatch,
    },
    /// One-level bounded AND/OR over simple conditions. Nested groups
    /// are rejected by parsing, not just by validation.
    Group {
        op: GroupOp,
        conditions: Vec<AlertRuleSpec>,
    },
}

// Strict wire shapes: each accepts exactly its own fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdShape {
    metric_id: String,
    #[serde(default)]
    dimension_match: Option<DimensionMatch>,
    operator: ThresholdOperator,
    threshold: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RateShape {
    metric_id: String,
    #[serde(default)]
    dimension_match: Option<DimensionMatch>,
    operator: ThresholdOperator,
    threshold_per_second: f64,
    window_seconds: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AvailabilityShape {
    metric_id: String,
    #[serde(default)]
    dimension_match: Option<DimensionMatch>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckStatusShape {
    check_id: String,
    status_match: CheckStatusMatch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupShape {
    op: GroupOp,
    conditions: Vec<serde_json::Value>,
}

/// Parse one spec JSON value into a typed spec. `depth` is 0 at the
/// top level; nested groups are rejected outright (one level only).
fn parse_spec_value(value: &serde_json::Value, depth: usize) -> Result<AlertRuleSpec, String> {
    let object_error = |value: &serde_json::Value| {
        format!("spec must be a JSON object matching one of the typed rule shapes (got {value})")
    };
    // Group first: `op` + `conditions` is its exclusive signature.
    if let Ok(shape) = serde_json::from_value::<GroupShape>(value.clone()) {
        if depth > 0 {
            return Err("group rules may not nest (one level of AND/OR only)".to_string());
        }
        let mut conditions = Vec::with_capacity(shape.conditions.len());
        for condition in &shape.conditions {
            conditions.push(parse_spec_value(condition, depth + 1)?);
        }
        return Ok(AlertRuleSpec::Group {
            op: shape.op,
            conditions,
        });
    }
    if let Ok(shape) = serde_json::from_value::<ThresholdShape>(value.clone()) {
        return Ok(AlertRuleSpec::Threshold {
            metric_id: shape.metric_id,
            dimension_match: shape.dimension_match,
            operator: shape.operator,
            threshold: shape.threshold,
        });
    }
    if let Ok(shape) = serde_json::from_value::<RateShape>(value.clone()) {
        return Ok(AlertRuleSpec::Rate {
            metric_id: shape.metric_id,
            dimension_match: shape.dimension_match,
            operator: shape.operator,
            threshold_per_second: shape.threshold_per_second,
            window_seconds: shape.window_seconds,
        });
    }
    if let Ok(shape) = serde_json::from_value::<AvailabilityShape>(value.clone()) {
        return Ok(AlertRuleSpec::Availability {
            metric_id: shape.metric_id,
            dimension_match: shape.dimension_match,
        });
    }
    if let Ok(shape) = serde_json::from_value::<CheckStatusShape>(value.clone()) {
        return Ok(AlertRuleSpec::CheckStatus {
            check_id: shape.check_id,
            status_match: shape.status_match,
        });
    }
    Err(object_error(value))
}

impl<'de> Deserialize<'de> for AlertRuleSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        parse_spec_value(&value, 0).map_err(serde::de::Error::custom)
    }
}

impl AlertRuleSpec {
    /// The `rule_type` column value for this spec.
    pub fn rule_type(&self) -> &'static str {
        match self {
            AlertRuleSpec::Threshold { .. } => "threshold",
            AlertRuleSpec::Rate { .. } => "rate",
            AlertRuleSpec::Availability { .. } => "availability",
            AlertRuleSpec::CheckStatus { .. } => "check_status",
            AlertRuleSpec::Group { .. } => "group",
        }
    }

    /// Structural validation: bounds, charsets, nesting. Returns a
    /// loud, specific error — never a silent skip.
    pub fn validate(&self) -> Result<(), StoreError> {
        self.validate_inner(0)
    }

    fn validate_inner(&self, depth: usize) -> Result<(), StoreError> {
        let invalid = |reason: String| StoreError::InvalidConfiguration { reason };
        match self {
            AlertRuleSpec::Threshold {
                metric_id,
                dimension_match,
                threshold,
                ..
            } => {
                validate_metric_id(metric_id)?;
                validate_dimension_match(dimension_match.as_ref())?;
                if !threshold.is_finite() || *threshold < 0.0 || *threshold > 1e15 {
                    return Err(invalid(format!(
                        "threshold must be a finite number in 0..=1e15 (got {threshold})"
                    )));
                }
                Ok(())
            }
            AlertRuleSpec::Rate {
                metric_id,
                dimension_match,
                threshold_per_second,
                window_seconds,
                ..
            } => {
                validate_metric_id(metric_id)?;
                validate_dimension_match(dimension_match.as_ref())?;
                if !threshold_per_second.is_finite()
                    || *threshold_per_second < 0.0
                    || *threshold_per_second > 1e12
                {
                    return Err(invalid(format!(
                        "threshold_per_second must be a finite number in 0..=1e12 (got {threshold_per_second})"
                    )));
                }
                if !(MIN_RATE_WINDOW_SECONDS..=MAX_RATE_WINDOW_SECONDS).contains(window_seconds) {
                    return Err(invalid(format!(
                        "rate window_seconds must be {MIN_RATE_WINDOW_SECONDS}..={MAX_RATE_WINDOW_SECONDS} (got {window_seconds})"
                    )));
                }
                Ok(())
            }
            AlertRuleSpec::Availability {
                metric_id,
                dimension_match,
            } => {
                validate_metric_id(metric_id)?;
                validate_dimension_match(dimension_match.as_ref())
            }
            AlertRuleSpec::CheckStatus {
                check_id,
                status_match,
            } => {
                if check_id.is_empty()
                    || check_id.len() > MAX_ID_BYTES
                    || !check_id.bytes().all(|b| {
                        b.is_ascii_alphanumeric()
                            || matches!(b, b'.' | b'_' | b':' | b'/' | b'-' | b'@')
                    })
                {
                    return Err(invalid(format!(
                        "check_id must be 1..={MAX_ID_BYTES} bytes of [A-Za-z0-9._:/@-]"
                    )));
                }
                let _ = status_match;
                Ok(())
            }
            AlertRuleSpec::Group { op, conditions } => {
                let _ = op;
                if depth > 0 {
                    return Err(invalid(
                        "group rules may not nest (one level of AND/OR only)".into(),
                    ));
                }
                if !(2..=MAX_GROUP_CONDITIONS).contains(&conditions.len()) {
                    return Err(invalid(format!(
                        "group conditions must list 2..={MAX_GROUP_CONDITIONS} entries (got {})",
                        conditions.len()
                    )));
                }
                for condition in conditions {
                    condition.validate_inner(depth + 1)?;
                }
                Ok(())
            }
        }
    }
}

fn validate_metric_id(metric_id: &str) -> Result<(), StoreError> {
    if metric_id.is_empty()
        || metric_id.len() > MAX_ID_BYTES
        || metric_id.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return Err(StoreError::InvalidConfiguration {
            reason: format!("metric_id must be 1..={MAX_ID_BYTES} printable bytes"),
        });
    }
    Ok(())
}

fn validate_dimension_match(dimension_match: Option<&DimensionMatch>) -> Result<(), StoreError> {
    if let Some(map) = dimension_match {
        if map.len() > MAX_DIMENSION_MATCH_KEYS {
            return Err(StoreError::InvalidConfiguration {
                reason: format!(
                    "dimension_match may carry at most {MAX_DIMENSION_MATCH_KEYS} keys"
                ),
            });
        }
        for (key, value) in map {
            if key.is_empty()
                || key.len() > MAX_ID_BYTES
                || key.bytes().any(|b| b < 0x20 || b == 0x7f)
                || value.is_empty()
                || value.len() > MAX_ID_BYTES
                || value.bytes().any(|b| b < 0x20 || b == 0x7f)
            {
                return Err(StoreError::InvalidConfiguration {
                    reason: format!(
                        "dimension_match entries must be 1..={MAX_ID_BYTES} printable bytes"
                    ),
                });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Row + repository
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AlertRuleRow {
    pub rule_id: String,
    pub name: String,
    pub enabled: bool,
    pub target_kind: String,
    pub target_id: String,
    pub rule_type: String,
    pub spec: String,
    pub severity: String,
    pub for_seconds: i64,
    pub recovery_seconds: i64,
    pub missing_data: String,
    pub revision: i64,
    pub created_by: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// A rule with its spec parsed and validated — what callers actually
/// use. Parsing happens at every load, so a corrupted `spec` column
/// is a loud error the moment the rule is touched, never a silent
/// skip at evaluation time.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertRule {
    pub rule_id: String,
    pub name: String,
    pub enabled: bool,
    pub target_kind: String,
    pub target_id: String,
    pub severity: String,
    pub for_seconds: i64,
    pub recovery_seconds: i64,
    pub missing_data: MissingDataPolicy,
    pub revision: i64,
    pub created_by: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub spec: AlertRuleSpec,
}

impl AlertRule {
    /// The `rule_type` column value for this rule's spec.
    pub fn rule_type(&self) -> &'static str {
        self.spec.rule_type()
    }
}

impl TryFrom<AlertRuleRow> for AlertRule {
    type Error = StoreError;

    fn try_from(row: AlertRuleRow) -> Result<Self, Self::Error> {
        let spec: AlertRuleSpec =
            serde_json::from_str(&row.spec).map_err(|e| StoreError::InvalidConfiguration {
                reason: format!("alert rule {} has an unparseable spec: {e}", row.rule_id),
            })?;
        spec.validate()
            .map_err(|e| StoreError::InvalidConfiguration {
                reason: format!("alert rule {} failed spec validation: {e}", row.rule_id),
            })?;
        let missing_data = match row.missing_data.as_str() {
            "unknown" => MissingDataPolicy::Unknown,
            "fire" => MissingDataPolicy::Fire,
            "ignore" => MissingDataPolicy::Ignore,
            other => {
                return Err(StoreError::InvalidConfiguration {
                    reason: format!(
                        "alert rule {} has unknown missing_data policy {other:?}",
                        row.rule_id
                    ),
                })
            }
        };
        // The discriminator column and the parsed spec must agree —
        // a drift means someone edited the database by hand.
        if row.rule_type != spec.rule_type() {
            return Err(StoreError::InvalidConfiguration {
                reason: format!(
                    "alert rule {} rule_type column {:?} does not match its spec kind {:?}",
                    row.rule_id,
                    row.rule_type,
                    spec.rule_type()
                ),
            });
        }
        Ok(Self {
            rule_id: row.rule_id,
            name: row.name,
            enabled: row.enabled,
            target_kind: row.target_kind,
            target_id: row.target_id,
            severity: row.severity,
            for_seconds: row.for_seconds,
            recovery_seconds: row.recovery_seconds,
            missing_data,
            revision: row.revision,
            created_by: row.created_by,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
            spec,
        })
    }
}

/// Common (non-spec) field bounds. Spec validation is separate so
/// partial-update paths can reuse it.
fn validate_common_fields(
    name: &str,
    severity: &str,
    for_seconds: i64,
    recovery_seconds: i64,
) -> Result<(), StoreError> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES || name.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return Err(StoreError::InvalidConfiguration {
            reason: format!("name must be 1..={MAX_NAME_BYTES} printable bytes"),
        });
    }
    if !matches!(severity, "critical" | "warning" | "info") {
        return Err(StoreError::InvalidConfiguration {
            reason: format!("severity must be critical|warning|info (got {severity:?})"),
        });
    }
    if !(0..=MAX_FOR_SECONDS).contains(&for_seconds) {
        return Err(StoreError::InvalidConfiguration {
            reason: format!("for_seconds must be 0..={MAX_FOR_SECONDS} (got {for_seconds})"),
        });
    }
    if !(0..=MAX_FOR_SECONDS).contains(&recovery_seconds) {
        return Err(StoreError::InvalidConfiguration {
            reason: format!(
                "recovery_seconds must be 0..={MAX_FOR_SECONDS} (got {recovery_seconds})"
            ),
        });
    }
    Ok(())
}

fn validate_target(target_kind: &str, target_id: &str) -> Result<(), StoreError> {
    if !matches!(target_kind, "node" | "vm") {
        return Err(StoreError::InvalidConfiguration {
            reason: format!("target_kind must be 'node' or 'vm' (got {target_kind:?})"),
        });
    }
    if target_id.is_empty() || target_id.len() > MAX_ID_BYTES {
        return Err(StoreError::InvalidConfiguration {
            reason: format!("target_id must be 1..={MAX_ID_BYTES} bytes"),
        });
    }
    Ok(())
}

pub struct RuleCreateInput {
    pub name: String,
    /// Rules are created in the caller's chosen state. The UI's
    /// create-from-template flow creates DISABLED rules (templates
    /// never auto-enable); direct API creates default to enabled.
    pub enabled: bool,
    pub target_kind: String,
    pub target_id: String,
    pub spec: AlertRuleSpec,
    pub severity: String,
    pub for_seconds: i64,
    pub recovery_seconds: i64,
    pub missing_data: MissingDataPolicy,
    pub created_by: String,
    pub now_ms: i64,
}

pub struct RuleUpdateInput {
    pub rule_id: String,
    /// The revision the caller believes is current. A mismatch
    /// changes nothing and returns [`StoreError::StaleVersion`].
    pub expected_revision: i64,
    pub name: String,
    /// `None` keeps the current enabled state (a partial update must
    /// never silently re-enable a disabled rule).
    pub enabled: Option<bool>,
    pub spec: AlertRuleSpec,
    pub severity: String,
    pub for_seconds: i64,
    pub recovery_seconds: i64,
    pub missing_data: MissingDataPolicy,
    pub updated_by: String,
    pub now_ms: i64,
}

#[derive(Clone)]
pub struct AlertRuleRepository {
    pool: StorePool,
}

impl AlertRuleRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    /// Create a rule. The row's `rule_type` column is derived from
    /// the spec, so the two can never disagree at birth.
    pub async fn create(&self, input: &RuleCreateInput) -> Result<AlertRule, StoreError> {
        validate_common_fields(
            &input.name,
            &input.severity,
            input.for_seconds,
            input.recovery_seconds,
        )?;
        validate_target(&input.target_kind, &input.target_id)?;
        input.spec.validate()?;
        let spec_json =
            serde_json::to_string(&input.spec).map_err(|e| StoreError::InvalidConfiguration {
                reason: format!("spec serialization failed: {e}"),
            })?;
        let row: AlertRuleRow = sqlx::query_as(
            r#"
            INSERT INTO alert_rules (
                name, enabled, target_kind, target_id, rule_type, spec,
                severity, for_seconds, recovery_seconds, missing_data,
                revision, created_by, created_at_ms, updated_at_ms
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, $11, $12, $12)
            RETURNING *
            "#,
        )
        .bind(&input.name)
        .bind(input.enabled)
        .bind(&input.target_kind)
        .bind(&input.target_id)
        .bind(input.spec.rule_type())
        .bind(&spec_json)
        .bind(&input.severity)
        .bind(input.for_seconds)
        .bind(input.recovery_seconds)
        .bind(input.missing_data.as_str())
        .bind(&input.created_by)
        .bind(input.now_ms)
        .fetch_one(&self.pool)
        .await?;
        row.try_into()
    }

    /// Update a rule under a revision precondition. Zero matching
    /// rows is a stale revision (the rule may also have been deleted
    /// — both surface as a conflict, never as a silent no-op).
    /// Target identity is immutable: an update never moves a rule
    /// between targets (delete + recreate instead).
    pub async fn update(&self, input: &RuleUpdateInput) -> Result<AlertRule, StoreError> {
        validate_common_fields(
            &input.name,
            &input.severity,
            input.for_seconds,
            input.recovery_seconds,
        )?;
        input.spec.validate()?;
        let spec_json =
            serde_json::to_string(&input.spec).map_err(|e| StoreError::InvalidConfiguration {
                reason: format!("spec serialization failed: {e}"),
            })?;
        let row: Option<AlertRuleRow> = sqlx::query_as(
            r#"
            UPDATE alert_rules SET
                name = $1, enabled = COALESCE($2, enabled), rule_type = $3, spec = $4,
                severity = $5, for_seconds = $6, recovery_seconds = $7,
                missing_data = $8, revision = revision + 1, updated_at_ms = $9
            WHERE rule_id = $10 AND revision = $11
            RETURNING *
            "#,
        )
        .bind(&input.name)
        .bind(input.enabled)
        .bind(input.spec.rule_type())
        .bind(&spec_json)
        .bind(&input.severity)
        .bind(input.for_seconds)
        .bind(input.recovery_seconds)
        .bind(input.missing_data.as_str())
        .bind(input.now_ms)
        .bind(&input.rule_id)
        .bind(input.expected_revision)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => row.try_into(),
            None => {
                let current: Option<(i64,)> =
                    sqlx::query_as("SELECT revision FROM alert_rules WHERE rule_id = $1")
                        .bind(&input.rule_id)
                        .fetch_optional(&self.pool)
                        .await?;
                match current {
                    Some((current,)) => Err(StoreError::StaleVersion {
                        entity: "alert rule",
                        id: input.rule_id.clone(),
                        expected: input.expected_revision,
                        current,
                    }),
                    None => Err(StoreError::NotFound {
                        entity: "alert rule",
                        id: input.rule_id.clone(),
                    }),
                }
            }
        }
    }

    /// Delete a rule under a revision precondition. Incidents the
    /// rule produced are NOT deleted — they are historical record;
    /// new evaluations simply stop.
    pub async fn delete(&self, rule_id: &str, expected_revision: i64) -> Result<(), StoreError> {
        let result = sqlx::query("DELETE FROM alert_rules WHERE rule_id = $1 AND revision = $2")
            .bind(rule_id)
            .bind(expected_revision)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            let current: Option<(i64,)> =
                sqlx::query_as("SELECT revision FROM alert_rules WHERE rule_id = $1")
                    .bind(rule_id)
                    .fetch_optional(&self.pool)
                    .await?;
            return match current {
                Some((current,)) => Err(StoreError::StaleVersion {
                    entity: "alert rule",
                    id: rule_id.to_string(),
                    expected: expected_revision,
                    current,
                }),
                None => Err(StoreError::NotFound {
                    entity: "alert rule",
                    id: rule_id.to_string(),
                }),
            };
        }
        Ok(())
    }

    pub async fn get(&self, rule_id: &str) -> Result<AlertRule, StoreError> {
        let row: Option<AlertRuleRow> =
            sqlx::query_as("SELECT * FROM alert_rules WHERE rule_id = $1")
                .bind(rule_id)
                .fetch_optional(&self.pool)
                .await?;
        match row {
            Some(row) => row.try_into(),
            None => Err(StoreError::NotFound {
                entity: "alert rule",
                id: rule_id.to_string(),
            }),
        }
    }

    /// Paged listing for the UI. `enabled_only` and `target_kind`
    /// are optional filters; the total is the unpaginated count.
    pub async fn list(
        &self,
        enabled_only: bool,
        target_kind: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<AlertRule>, i64), StoreError> {
        let rows: Vec<AlertRuleRow> = sqlx::query_as(
            r#"
            SELECT * FROM alert_rules
            WHERE ($1 = 0 OR enabled = 1)
              AND ($2 IS NULL OR target_kind = $2)
            ORDER BY created_at_ms DESC, rule_id
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(if enabled_only { 1 } else { 0 })
        .bind(target_kind)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM alert_rules
            WHERE ($1 = 0 OR enabled = 1)
              AND ($2 IS NULL OR target_kind = $2)
            "#,
        )
        .bind(if enabled_only { 1 } else { 0 })
        .bind(target_kind)
        .fetch_one(&self.pool)
        .await?;
        let rules = rows
            .into_iter()
            .map(AlertRule::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rules, total.0))
    }

    /// Every enabled rule, for the evaluator (bounded by the
    /// configured ceiling — the caller passes it as `limit`).
    pub async fn list_enabled(&self, limit: i64) -> Result<Vec<AlertRule>, StoreError> {
        let rows: Vec<AlertRuleRow> = sqlx::query_as(
            "SELECT * FROM alert_rules WHERE enabled = 1 ORDER BY created_at_ms, rule_id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(AlertRule::try_from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::create_test_pool;

    fn threshold_spec() -> AlertRuleSpec {
        AlertRuleSpec::Threshold {
            metric_id: "vm.cpu.capacity_ratio".into(),
            dimension_match: None,
            operator: ThresholdOperator::GreaterThan,
            threshold: 0.9,
        }
    }

    fn create_input(spec: AlertRuleSpec, target_kind: &str, target_id: &str) -> RuleCreateInput {
        RuleCreateInput {
            name: "test rule".into(),
            enabled: true,
            target_kind: target_kind.into(),
            target_id: target_id.into(),
            spec,
            severity: "warning".into(),
            for_seconds: 300,
            recovery_seconds: 120,
            missing_data: MissingDataPolicy::Unknown,
            created_by: "op-user".into(),
            now_ms: 1_000_000,
        }
    }

    #[test]
    fn spec_parsing_is_strict_and_disjoint() {
        // Each well-formed shape parses to its own variant.
        let threshold: AlertRuleSpec =
            serde_json::from_str(r#"{"metric_id":"m","operator":"greater_than","threshold":1}"#)
                .expect("threshold");
        assert!(matches!(threshold, AlertRuleSpec::Threshold { .. }));

        let rate: AlertRuleSpec = serde_json::from_str(
            r#"{"metric_id":"m","operator":"less_than","threshold_per_second":2,"window_seconds":60}"#,
        )
        .expect("rate");
        assert!(matches!(rate, AlertRuleSpec::Rate { .. }));

        let availability: AlertRuleSpec =
            serde_json::from_str(r#"{"metric_id":"m"}"#).expect("availability");
        assert!(matches!(availability, AlertRuleSpec::Availability { .. }));

        let check: AlertRuleSpec =
            serde_json::from_str(r#"{"check_id":"service:httpd","status_match":"critical"}"#)
                .expect("check status");
        assert!(matches!(check, AlertRuleSpec::CheckStatus { .. }));

        let group: AlertRuleSpec = serde_json::from_str(
            r#"{"op":"and","conditions":[{"metric_id":"m"},{"check_id":"c","status_match":"warning"}]}"#,
        )
        .expect("group");
        assert!(matches!(group, AlertRuleSpec::Group { .. }));

        // A mis-shaped threshold (typo'd operator) must NOT silently
        // parse as Availability by ignoring the unknown fields.
        let mis_shaped = serde_json::from_str::<AlertRuleSpec>(
            r#"{"metric_id":"m","operator":"moar","threshold":1}"#,
        );
        assert!(mis_shaped.is_err(), "mis-shaped spec must be rejected");

        // Unknown fields are rejected in every shape.
        assert!(
            serde_json::from_str::<AlertRuleSpec>(r#"{"metric_id":"m","surprise":true}"#).is_err()
        );
        assert!(serde_json::from_str::<AlertRuleSpec>(
            r#"{"metric_id":"m","operator":"greater_than","threshold":1,"extra":0}"#
        )
        .is_err());

        // Nested groups are rejected at parse time.
        assert!(serde_json::from_str::<AlertRuleSpec>(
            r#"{"op":"and","conditions":[{"op":"or","conditions":[{"metric_id":"m"},{"metric_id":"n"}]}]}"#
        )
        .is_err());

        // Round-trips survive serialize -> deserialize.
        for spec in [&threshold, &rate, &availability, &check, &group] {
            let json = serde_json::to_string(spec).expect("serialize");
            let back: AlertRuleSpec = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&back, spec, "roundtrip of {json}");
        }
    }

    #[tokio::test]
    async fn create_get_roundtrip_derives_rule_type() {
        let repo = AlertRuleRepository::new(create_test_pool().await);
        let mut spec = threshold_spec();
        if let AlertRuleSpec::Threshold {
            ref mut dimension_match,
            ..
        } = spec
        {
            let mut map = DimensionMatch::new();
            map.insert("mount_id".into(), "ext4:/".into());
            *dimension_match = Some(map);
        }
        let rule = repo
            .create(&create_input(spec.clone(), "vm", "vm-1"))
            .await
            .expect("create");
        assert_eq!(rule.rule_type(), "threshold");
        assert_eq!(rule.revision, 1);
        assert_eq!(rule.spec, spec);
        assert!(rule.enabled);

        let fetched = repo.get(&rule.rule_id).await.expect("get");
        assert_eq!(fetched, rule);
    }

    #[tokio::test]
    async fn update_requires_current_revision() {
        let repo = AlertRuleRepository::new(create_test_pool().await);
        let rule = repo
            .create(&create_input(threshold_spec(), "node", "node-1"))
            .await
            .expect("create");

        let update = RuleUpdateInput {
            rule_id: rule.rule_id.clone(),
            expected_revision: 1,
            name: "renamed".into(),
            enabled: Some(false),
            spec: AlertRuleSpec::Availability {
                metric_id: "node.cpu.capacity_ratio".into(),
                dimension_match: None,
            },
            severity: "critical".into(),
            for_seconds: 60,
            recovery_seconds: 30,
            missing_data: MissingDataPolicy::Fire,
            updated_by: "op-user".into(),
            now_ms: 2_000_000,
        };
        let updated = repo.update(&update).await.expect("update");
        assert_eq!(updated.revision, 2);
        assert!(!updated.enabled);
        assert_eq!(updated.rule_type(), "availability");
        assert_eq!(updated.name, "renamed");

        // A stale replay changes nothing.
        let stale = repo.update(&update).await;
        assert!(matches!(
            stale,
            Err(StoreError::StaleVersion {
                expected: 1,
                current: 2,
                ..
            })
        ));
        let untouched = repo.get(&rule.rule_id).await.expect("get");
        assert_eq!(untouched.revision, 2);
        assert_eq!(untouched.name, "renamed");
    }

    #[tokio::test]
    async fn delete_requires_current_revision_and_persists_history() {
        let repo = AlertRuleRepository::new(create_test_pool().await);
        let rule = repo
            .create(&create_input(threshold_spec(), "vm", "vm-2"))
            .await
            .expect("create");

        assert!(matches!(
            repo.delete(&rule.rule_id, 7).await,
            Err(StoreError::StaleVersion { .. })
        ));
        repo.delete(&rule.rule_id, 1).await.expect("delete");
        assert!(matches!(
            repo.get(&rule.rule_id).await,
            Err(StoreError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn validation_rejects_out_of_bounds_rules() {
        let repo = AlertRuleRepository::new(create_test_pool().await);

        let mut input = create_input(threshold_spec(), "vm", "vm-3");
        input.name = "".into();
        assert!(repo.create(&input).await.is_err());

        let mut input = create_input(threshold_spec(), "vm", "vm-3");
        input.severity = "severe".into();
        assert!(repo.create(&input).await.is_err());

        let input = create_input(threshold_spec(), "cluster", "c-1");
        assert!(repo.create(&input).await.is_err());

        let mut input = create_input(threshold_spec(), "vm", "vm-3");
        input.for_seconds = MAX_FOR_SECONDS + 1;
        assert!(repo.create(&input).await.is_err());

        let input = create_input(
            AlertRuleSpec::Threshold {
                metric_id: "m".into(),
                dimension_match: None,
                operator: ThresholdOperator::LessThan,
                threshold: -1.0,
            },
            "vm",
            "vm-3",
        );
        assert!(repo.create(&input).await.is_err());

        let input = create_input(
            AlertRuleSpec::Rate {
                metric_id: "m".into(),
                dimension_match: None,
                operator: ThresholdOperator::GreaterThan,
                threshold_per_second: 1.0,
                window_seconds: 10,
            },
            "vm",
            "vm-3",
        );
        assert!(repo.create(&input).await.is_err());

        let input = create_input(
            AlertRuleSpec::CheckStatus {
                check_id: "bad id!".into(),
                status_match: CheckStatusMatch::Critical,
            },
            "vm",
            "vm-3",
        );
        assert!(repo.create(&input).await.is_err());

        let mut dimensions = DimensionMatch::new();
        for i in 0..3 {
            dimensions.insert(format!("d{i}"), "v".into());
        }
        let input = create_input(
            AlertRuleSpec::Threshold {
                metric_id: "m".into(),
                dimension_match: Some(dimensions),
                operator: ThresholdOperator::GreaterThan,
                threshold: 1.0,
            },
            "vm",
            "vm-3",
        );
        assert!(repo.create(&input).await.is_err());

        // Group bounds: single condition and seven conditions.
        let one_condition = AlertRuleSpec::Group {
            op: GroupOp::And,
            conditions: vec![AlertRuleSpec::Availability {
                metric_id: "m".into(),
                dimension_match: None,
            }],
        };
        let input = create_input(one_condition, "vm", "vm-3");
        assert!(repo.create(&input).await.is_err());

        let seven_conditions = AlertRuleSpec::Group {
            op: GroupOp::Or,
            conditions: (0..7)
                .map(|_| AlertRuleSpec::Availability {
                    metric_id: "m".into(),
                    dimension_match: None,
                })
                .collect(),
        };
        let input = create_input(seven_conditions, "vm", "vm-3");
        assert!(repo.create(&input).await.is_err());
    }

    #[tokio::test]
    async fn list_filters_enabled_and_target_kind() {
        let repo = AlertRuleRepository::new(create_test_pool().await);
        let a = repo
            .create(&create_input(threshold_spec(), "vm", "vm-a"))
            .await
            .expect("create a");
        repo.create(&create_input(threshold_spec(), "node", "node-a"))
            .await
            .expect("create b");
        let mut disabled = create_input(threshold_spec(), "vm", "vm-b");
        disabled.name = "off".into();
        let c = repo.create(&disabled).await.expect("create c");
        let update = RuleUpdateInput {
            rule_id: c.rule_id.clone(),
            expected_revision: 1,
            name: "off".into(),
            enabled: Some(false),
            spec: threshold_spec(),
            severity: "warning".into(),
            for_seconds: 300,
            recovery_seconds: 120,
            missing_data: MissingDataPolicy::Unknown,
            updated_by: "op".into(),
            now_ms: 2_000_000,
        };
        repo.update(&update).await.expect("disable");

        let (all, total) = repo.list(false, None, 10, 0).await.expect("list");
        assert_eq!(total, 3);
        assert_eq!(all.len(), 3);

        let (enabled, total) = repo.list(true, None, 10, 0).await.expect("list enabled");
        assert_eq!(total, 2);
        assert!(!enabled.iter().any(|r| r.rule_id == c.rule_id));

        let (vms, total) = repo.list(false, Some("vm"), 10, 0).await.expect("list vms");
        assert_eq!(total, 2);
        assert!(vms.iter().all(|r| r.target_kind == "vm"));

        let (paged, total) = repo.list(false, None, 2, 1).await.expect("page");
        assert_eq!(total, 3);
        assert_eq!(paged.len(), 2);

        let enabled_all = repo.list_enabled(10).await.expect("list_enabled");
        assert_eq!(enabled_all.len(), 2);
        assert!(enabled_all.iter().any(|r| r.rule_id == a.rule_id));
    }

    #[tokio::test]
    async fn corrupted_spec_is_a_loud_error_on_load() {
        let pool = create_test_pool().await;
        let repo = AlertRuleRepository::new(pool.clone());
        sqlx::query(
            r#"
            INSERT INTO alert_rules (
                name, target_kind, target_id, rule_type, spec, severity,
                created_by, created_at_ms, updated_at_ms
            )
            VALUES ('bad', 'vm', 'vm-x', 'threshold', '{"nonsense":true}', 'warning', 'x', 1, 1)
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed corrupted rule");

        let err = repo.list(false, None, 10, 0).await;
        assert!(err.is_err(), "a corrupted spec must fail the load");

        // A rule_type column drifting from its spec is also caught.
        sqlx::query(
            r#"
            INSERT INTO alert_rules (
                name, target_kind, target_id, rule_type, spec, severity,
                created_by, created_at_ms, updated_at_ms
            )
            VALUES (
                'drift', 'vm', 'vm-y', 'availability',
                '{"metric_id":"m","operator":"greater_than","threshold":1}',
                'warning', 'x', 1, 1
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed drifted rule");
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT rule_id FROM alert_rules WHERE name = 'drift'")
                .fetch_all(&pool)
                .await
                .expect("find drifted");
        let drift_err = repo.get(&rows[0].0).await;
        assert!(drift_err.is_err(), "rule_type drift must fail the load");
    }
}
