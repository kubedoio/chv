//! Native alerting API (query/alerts contract v1, #602 PR-6).
//!
//! Boundaries: the browser talks only to this BFF. Reads (incident
//! list/detail, rule list, delivery audit) are Viewer-role gated by
//! the router middleware; rule mutations, acknowledgment and silence
//! are Operator-gated; the delivery test is Admin-gated — the
//! contract's role table. CHV v1 roles are fleet-scoped, so incident
//! and rule visibility follows the operator's fleet authorization
//! (the same recorded v1 boundary as every other monitoring read).
//!
//! Rule wire shape: the contract's FLAT typed-rule example — common
//! fields plus the typed spec fields at the top level. The spec
//! fields are extracted and parsed by the store's strict spec parser
//! (unknown or missing fields are loud errors, never silent
//! reinterpretation), then every `metric_id` is registry-validated so
//! a rule on a nonexistent metric can never be created as a silent
//! never-firing trap. The `rule_type` is DERIVED from the spec shape
//! and returned; clients never send it.

use crate::auth::BearerToken;
use crate::router::AppState;
use crate::BffError;
use axum::{extract::State, response::Json};
use chv_controlplane_store::{
    AlertRule, AlertRuleSpec, IncidentListFilter, IncidentRow, IncidentTransitionRow,
    NotificationEventInput, OutboxEventRow,
};
use chv_monitoring_core::registry;
use serde::Deserialize;
use serde_json::{json, Value};

/// Page size ceiling (contract: configured pagination for fleet
/// lists).
const MAX_PAGE_SIZE: i64 = 100;
/// Delivery audit listing ceiling.
const MAX_DELIVERY_ROWS: i64 = 50;
/// Silence duration bounds (minutes).
const MIN_SILENCE_MINUTES: i64 = 1;
const MAX_SILENCE_MINUTES: i64 = 7 * 24 * 60;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn invalid(message: String) -> BffError {
    BffError::MonitoringQuery {
        code: "invalid_rule".to_string(),
        message,
    }
}

/// Best-effort durable audit trail for rule and incident mutations
/// (the contract requires an audit trail on rule changes).
async fn audit(
    state: &AppState,
    claims: &crate::auth::Claims,
    event: &str,
    message: String,
    details: Value,
) {
    use chv_controlplane_store::EventAppendInput;
    use chv_controlplane_types::domain::{ActorId, EventSeverity, EventType};
    let input = EventAppendInput {
        occurred_unix_ms: now_ms(),
        event_type: EventType::Audit,
        severity: EventSeverity::Info,
        resource_kind: None,
        resource_id: None,
        node_id: None,
        operation_id: None,
        actor_id: ActorId::new(&claims.username).ok(),
        requested_by: Some(claims.username.clone()),
        correlation_id: None,
        message,
        details: Some(details.to_string()),
    };
    if let Err(e) = state.event_repo.append(&input).await {
        tracing::warn!(error = %e, %event, "alerting audit event append failed");
    }
}

/// Body keys owned by the rule's common fields; everything else in
/// the request object belongs to the typed spec.
const COMMON_RULE_KEYS: &[&str] = &[
    "name",
    "target_kind",
    "target_id",
    "severity",
    "for_seconds",
    "recovery_seconds",
    "missing_data",
    "enabled",
    "rule_id",
    "expected_revision",
];

/// Extract and validate the typed spec from a flat rule body: strip
/// the common keys, parse the remainder with the store's strict
/// parser, then registry-validate every metric id.
fn extract_spec(body: &Value) -> Result<AlertRuleSpec, BffError> {
    let object = body
        .as_object()
        .ok_or_else(|| invalid("rule body must be a JSON object".into()))?;
    let mut spec_object = serde_json::Map::new();
    for (key, value) in object {
        if !COMMON_RULE_KEYS.contains(&key.as_str()) {
            spec_object.insert(key.clone(), value.clone());
        }
    }
    let spec: AlertRuleSpec = serde_json::from_value(Value::Object(spec_object))
        .map_err(|e| invalid(format!("invalid typed rule spec: {e}")))?;
    spec.validate().map_err(|e| invalid(format!("{e}")))?;
    validate_spec_metrics(&spec)?;
    Ok(spec)
}

/// Every metric a rule references must exist in the published
/// registry — a rule on a nonexistent metric is a silent never-firing
/// trap, refused loudly at creation time.
fn validate_spec_metrics(spec: &AlertRuleSpec) -> Result<(), BffError> {
    match spec {
        AlertRuleSpec::Threshold { metric_id, .. }
        | AlertRuleSpec::Rate { metric_id, .. }
        | AlertRuleSpec::Availability { metric_id, .. } => {
            if registry::lookup(metric_id).is_none() {
                return Err(BffError::MonitoringQuery {
                    code: "unknown_metric".to_string(),
                    message: format!("unknown metric id {metric_id:?}"),
                });
            }
            Ok(())
        }
        AlertRuleSpec::CheckStatus { .. } => Ok(()),
        AlertRuleSpec::Group { conditions, .. } => {
            for condition in conditions {
                validate_spec_metrics(condition)?;
            }
            Ok(())
        }
    }
}

/// Render a rule as the contract's flat wire shape.
fn rule_wire(rule: &AlertRule) -> Value {
    let mut wire = serde_json::to_value(&rule.spec).expect("spec serialization is infallible");
    let object = wire.as_object_mut().expect("spec serializes to an object");
    object.insert("rule_id".into(), json!(rule.rule_id));
    object.insert("name".into(), json!(rule.name));
    object.insert("enabled".into(), json!(rule.enabled));
    object.insert("target_kind".into(), json!(rule.target_kind));
    object.insert("target_id".into(), json!(rule.target_id));
    object.insert("rule_type".into(), json!(rule.spec.rule_type()));
    object.insert("severity".into(), json!(rule.severity));
    object.insert("for_seconds".into(), json!(rule.for_seconds));
    object.insert("recovery_seconds".into(), json!(rule.recovery_seconds));
    object.insert("missing_data".into(), json!(rule.missing_data.as_str()));
    object.insert("revision".into(), json!(rule.revision));
    object.insert("created_by".into(), json!(rule.created_by));
    object.insert("created_at_ms".into(), json!(rule.created_at_ms));
    object.insert("updated_at_ms".into(), json!(rule.updated_at_ms));
    wire
}

/// Render an incident row (snake_case, per the contract's field
/// vocabulary).
fn incident_wire(row: &IncidentRow) -> Value {
    json!({
        "alert_id": row.alert_id,
        "status": row.status,
        "severity": row.severity,
        "rule_id": row.rule_id,
        "rule_revision": row.rule_revision,
        "dedup_key": row.dedup_key,
        "target_kind": row.resource_kind,
        "target_id": row.resource_id,
        "node_id": row.node_id,
        "message": row.message,
        "last_observed": row.last_observed,
        "opened_at": row.opened_at,
        "acknowledged_at": row.acknowledged_at,
        "acknowledged_by": row.acknowledged_by,
        "silenced_until_ms": row.silenced_until_ms,
        "silenced_by": row.silenced_by,
        "pending_since_ms": row.pending_since_ms,
        "first_occurrence_ms": row.first_occurrence_ms,
        "last_occurrence_ms": row.last_occurrence_ms,
        "evidence_from_ms": row.evidence_from_ms,
        "evidence_to_ms": row.evidence_to_ms,
        "resolved_at": row.resolved_at,
    })
}

fn transition_wire(row: &IncidentTransitionRow) -> Value {
    json!({
        "from_state": row.from_state,
        "to_state": row.to_state,
        "occurred_at_ms": row.occurred_at_ms,
        "reason": row.reason,
        "measured": row.measured,
    })
}

fn delivery_wire(row: &OutboxEventRow) -> Value {
    json!({
        "event_id": row.event_id,
        "alert_id": row.alert_id,
        "event_type": row.event_type,
        "severity": row.severity,
        "target_kind": row.target_kind,
        "target_id": row.target_id,
        "summary": row.summary,
        "channel": row.channel,
        "status": row.status,
        "attempts": row.attempts,
        "next_attempt_at_ms": row.next_attempt_at_ms,
        "last_attempt_ms": row.last_attempt_ms,
        "last_response": row.last_response,
        "occurred_at_ms": row.occurred_at_ms,
        "updated_at_ms": row.updated_at_ms,
    })
}

fn page(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    (
        limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE),
        offset.unwrap_or(0).max(0),
    )
}

// ---------------------------------------------------------------------------
// Viewer reads
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ListIncidentsBody {
    pub status: Option<String>,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub rule_id: Option<String>,
    #[serde(default)]
    pub include_resolved: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `POST /v1/monitoring/alerts` — incidents for the alert center.
/// `pending` is always visible (pre-notification, not pre-visibility).
pub async fn list_incidents(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<ListIncidentsBody>,
) -> Result<Json<Value>, BffError> {
    let _ = bearer;
    if let Some(status) = &body.status {
        if !matches!(status.as_str(), "pending" | "firing" | "resolved") {
            return Err(invalid(format!(
                "status filter must be pending|firing|resolved, got {status:?}"
            )));
        }
    }
    if let Some(kind) = &body.target_kind {
        if !matches!(kind.as_str(), "node" | "vm") {
            return Err(invalid(format!(
                "target_kind filter must be node|vm, got {kind:?}"
            )));
        }
    }
    let (limit, offset) = page(body.limit, body.offset);
    let filter = IncidentListFilter {
        status: body.status,
        target_kind: body.target_kind,
        target_id: body.target_id,
        rule_id: body.rule_id,
        include_resolved: body.include_resolved,
    };
    let (incidents, total) = state
        .alert_repo
        .list_incidents(&filter, limit, offset)
        .await?;
    Ok(Json(json!({
        "incidents": incidents.iter().map(incident_wire).collect::<Vec<_>>(),
        "total": total,
    })))
}

#[derive(Deserialize)]
pub struct IncidentDetailBody {
    pub alert_id: String,
}

/// `POST /v1/monitoring/alerts/detail` — one incident with its
/// transition history.
pub async fn incident_detail(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<IncidentDetailBody>,
) -> Result<Json<Value>, BffError> {
    let _ = bearer;
    let incident = state.alert_repo.get_incident(&body.alert_id).await?;
    let transitions = state
        .alert_repo
        .list_transitions(&body.alert_id, 100)
        .await?;
    Ok(Json(json!({
        "incident": incident_wire(&incident),
        "transitions": transitions.iter().map(transition_wire).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct ListRulesBody {
    #[serde(default)]
    pub enabled_only: bool,
    pub target_kind: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `POST /v1/monitoring/alert-rules` — list rules (Viewer).
pub async fn list_rules(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<ListRulesBody>,
) -> Result<Json<Value>, BffError> {
    let _ = bearer;
    if let Some(kind) = &body.target_kind {
        if !matches!(kind.as_str(), "node" | "vm") {
            return Err(invalid(format!(
                "target_kind filter must be node|vm, got {kind:?}"
            )));
        }
    }
    let (limit, offset) = page(body.limit, body.offset);
    let (rules, total) = state
        .alert_rules
        .list(
            body.enabled_only,
            body.target_kind.as_deref(),
            limit,
            offset,
        )
        .await?;
    Ok(Json(json!({
        "rules": rules.iter().map(rule_wire).collect::<Vec<_>>(),
        "total": total,
    })))
}

#[derive(Deserialize)]
pub struct ListDeliveriesBody {
    pub limit: Option<i64>,
}

/// `POST /v1/monitoring/notifications/deliveries` — the delivery
/// audit view (any status, newest first).
pub async fn list_deliveries(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<ListDeliveriesBody>,
) -> Result<Json<Value>, BffError> {
    let _ = bearer;
    let limit = body.limit.unwrap_or(20).clamp(1, MAX_DELIVERY_ROWS);
    let deliveries = state.notification_outbox.list_recent(limit).await?;
    Ok(Json(json!({
        "deliveries": deliveries.iter().map(delivery_wire).collect::<Vec<_>>(),
    })))
}

// ---------------------------------------------------------------------------
// Operator mutations
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateRuleBody {
    pub name: String,
    pub target_kind: String,
    pub target_id: String,
    pub severity: String,
    pub for_seconds: Option<i64>,
    pub recovery_seconds: Option<i64>,
    pub missing_data: Option<String>,
    pub enabled: Option<bool>,
}

/// `POST /v1/monitoring/alert-rules/create` — create a typed rule.
/// The spec fields ride flat in the same body (contract example
/// shape); `extract_spec` parses and validates them strictly.
pub async fn create_rule(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    let claims = &bearer.0;
    let common: CreateRuleBody = serde_json::from_value(body.clone())
        .map_err(|e| BffError::BadRequest(format!("invalid rule body: {e}")))?;
    let spec = extract_spec(&body)?;
    let missing_data = parse_missing_data(common.missing_data.as_deref())?;

    // The configured ceiling is a loud limit, not a silent clamp.
    let (_, total) = state.alert_rules.list(false, None, 1, 0).await?;
    if total >= state.alerting_max_rules {
        return Err(BffError::Conflict(format!(
            "the configured alert rule ceiling ({} rules) is reached; raise \
             monitoring.alerting.max_rules or delete unused rules",
            state.alerting_max_rules
        )));
    }

    let rule = state
        .alert_rules
        .create(&chv_controlplane_store::RuleCreateInput {
            name: common.name,
            // Honored: the UI's create-from-template flow relies on
            // creating DISABLED rules (templates never auto-enable).
            enabled: common.enabled.unwrap_or(true),
            target_kind: common.target_kind,
            target_id: common.target_id,
            spec,
            severity: common.severity,
            for_seconds: common.for_seconds.unwrap_or(300),
            recovery_seconds: common.recovery_seconds.unwrap_or(120),
            missing_data,
            created_by: claims.username.clone(),
            now_ms: now_ms(),
        })
        .await?;
    audit(
        &state,
        claims,
        "monitoring.alert_rule.create",
        format!("alert rule {:?} created", rule.name),
        json!({
            "event": "monitoring.alert_rule.create",
            "rule_id": rule.rule_id,
            "rule_type": rule.spec.rule_type(),
            "target_kind": rule.target_kind,
            "target_id": rule.target_id,
        }),
    )
    .await;
    Ok(Json(json!({ "rule": rule_wire(&rule) })))
}

#[derive(Deserialize)]
pub struct UpdateRuleBody {
    pub rule_id: String,
    pub expected_revision: i64,
    pub name: String,
    pub severity: String,
    pub for_seconds: Option<i64>,
    pub recovery_seconds: Option<i64>,
    pub missing_data: Option<String>,
    pub enabled: Option<bool>,
}

/// `POST /v1/monitoring/alert-rules/update` — update under a revision
/// precondition (a mismatch is a 409 conflict; the client reloads).
pub async fn update_rule(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    let claims = &bearer.0;
    let common: UpdateRuleBody = serde_json::from_value(body.clone())
        .map_err(|e| BffError::BadRequest(format!("invalid rule body: {e}")))?;
    let spec = extract_spec(&body)?;
    let missing_data = parse_missing_data(common.missing_data.as_deref())?;

    let rule = state
        .alert_rules
        .update(&chv_controlplane_store::RuleUpdateInput {
            rule_id: common.rule_id,
            expected_revision: common.expected_revision,
            name: common.name,
            enabled: common.enabled,
            spec,
            severity: common.severity,
            for_seconds: common.for_seconds.unwrap_or(300),
            recovery_seconds: common.recovery_seconds.unwrap_or(120),
            missing_data,
            updated_by: claims.username.clone(),
            now_ms: now_ms(),
        })
        .await?;
    audit(
        &state,
        claims,
        "monitoring.alert_rule.update",
        format!("alert rule {:?} updated", rule.name),
        json!({
            "event": "monitoring.alert_rule.update",
            "rule_id": rule.rule_id,
            "revision": rule.revision,
        }),
    )
    .await;
    Ok(Json(json!({ "rule": rule_wire(&rule) })))
}

#[derive(Deserialize)]
pub struct DeleteRuleBody {
    pub rule_id: String,
    pub expected_revision: i64,
}

/// `POST /v1/monitoring/alert-rules/delete` — delete under a revision
/// precondition. Incidents the rule produced are historical record
/// and are NOT deleted.
pub async fn delete_rule(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<DeleteRuleBody>,
) -> Result<Json<Value>, BffError> {
    let claims = &bearer.0;
    state
        .alert_rules
        .delete(&body.rule_id, body.expected_revision)
        .await?;
    audit(
        &state,
        claims,
        "monitoring.alert_rule.delete",
        format!("alert rule {} deleted", body.rule_id),
        json!({
            "event": "monitoring.alert_rule.delete",
            "rule_id": body.rule_id,
        }),
    )
    .await;
    Ok(Json(json!({ "deleted": true })))
}

#[derive(Deserialize)]
pub struct AcknowledgeBody {
    pub alert_id: String,
}

/// `POST /v1/monitoring/alerts/acknowledge` — an overlay marking
/// human attention; it never resolves the incident and never stops
/// firing/resolved notifications.
pub async fn acknowledge(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<AcknowledgeBody>,
) -> Result<Json<Value>, BffError> {
    let claims = &bearer.0;
    let acknowledged = state
        .alert_repo
        .acknowledge_incident(&body.alert_id, &claims.username, now_ms())
        .await?;
    if !acknowledged {
        return Err(BffError::NotFound(format!(
            "no active incident {}",
            body.alert_id
        )));
    }
    audit(
        &state,
        claims,
        "monitoring.incident.acknowledge",
        format!("incident {} acknowledged", body.alert_id),
        json!({
            "event": "monitoring.incident.acknowledge",
            "alert_id": body.alert_id,
        }),
    )
    .await;
    Ok(Json(json!({ "acknowledged": true })))
}

#[derive(Deserialize)]
pub struct SilenceBody {
    pub alert_id: String,
    /// Relative duration (exclusive with `until_ms`).
    pub duration_minutes: Option<i64>,
    /// Absolute deadline, epoch ms (exclusive with `duration_minutes`).
    pub until_ms: Option<i64>,
}

/// `POST /v1/monitoring/alerts/silence` — a notification overlay with
/// a deadline; it never resolves the incident.
pub async fn silence(
    State(state): State<AppState>,
    bearer: BearerToken,
    Json(body): Json<SilenceBody>,
) -> Result<Json<Value>, BffError> {
    let claims = &bearer.0;
    let now = now_ms();
    let until_ms = match (body.duration_minutes, body.until_ms) {
        (Some(minutes), None) => {
            if !(MIN_SILENCE_MINUTES..=MAX_SILENCE_MINUTES).contains(&minutes) {
                return Err(invalid(format!(
                    "duration_minutes must be {MIN_SILENCE_MINUTES}..={MAX_SILENCE_MINUTES}"
                )));
            }
            now + minutes * 60_000
        }
        (None, Some(until)) => {
            // Pre-validate: the store enforces this too, but its error
            // class is a store violation, not a request-shape error.
            // The absolute form is bounded to the same horizon as
            // duration_minutes: no effectively-forever silences.
            if until <= now {
                return Err(invalid("until_ms must be in the future".into()));
            }
            if until > now + MAX_SILENCE_MINUTES * 60_000 {
                return Err(invalid(format!(
                    "until_ms must be at most {MAX_SILENCE_MINUTES} minutes out"
                )));
            }
            until
        }
        (Some(_), Some(_)) => {
            return Err(invalid(
                "send either duration_minutes or until_ms, not both".into(),
            ));
        }
        (None, None) => {
            return Err(invalid("send duration_minutes or until_ms".into()));
        }
    };
    let silenced = state
        .alert_repo
        .silence_incident(&body.alert_id, &claims.username, until_ms, now)
        .await?;
    if !silenced {
        return Err(BffError::NotFound(format!(
            "no active incident {}",
            body.alert_id
        )));
    }
    audit(
        &state,
        claims,
        "monitoring.incident.silence",
        format!("incident {} silenced until {until_ms}", body.alert_id),
        json!({
            "event": "monitoring.incident.silence",
            "alert_id": body.alert_id,
            "until_ms": until_ms,
        }),
    )
    .await;
    Ok(Json(json!({ "silenced": true, "until_ms": until_ms })))
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

/// `POST /v1/monitoring/notifications/test` — enqueue a test event
/// (Admin). The dispatcher signs and delivers it like any firing
/// notification; a missing destination is an honest error, not a
/// silent no-op.
pub async fn test_notification(
    State(state): State<AppState>,
    bearer: BearerToken,
) -> Result<Json<Value>, BffError> {
    let claims = &bearer.0;
    let Some(channel) = state.notification_channels.test_channel() else {
        return Err(BffError::Conflict(
            "no notification destination is configured; set \
             monitoring.notifications.webhook_url or slack_webhook_url \
             (and the signing secret) first"
                .into(),
        ));
    };
    let event_id = uuid_v4();
    let summary = format!("CHV notification test by {}", claims.username);
    let payload = chv_monitoring_core::notifications::render_envelope(
        &event_id,
        "test",
        chv_controlplane_store::EVENT_TYPE_TEST,
        "info",
        "node",
        "test",
        &summary,
        now_ms(),
        "/nodes/test",
    );
    state
        .notification_outbox
        .enqueue(&NotificationEventInput {
            event_id: event_id.clone(),
            alert_id: "test".into(),
            incident_key: "test".into(),
            event_type: chv_controlplane_store::EVENT_TYPE_TEST.into(),
            severity: "info".into(),
            target_kind: "node".into(),
            target_id: "test".into(),
            summary,
            occurred_at_ms: now_ms(),
            payload,
            channel: channel.to_string(),
        })
        .await?;
    audit(
        &state,
        claims,
        "monitoring.notification.test",
        "notification delivery test enqueued".into(),
        json!({
            "event": "monitoring.notification.test",
            "outbox_event_id": event_id,
        }),
    )
    .await;
    Ok(Json(json!({ "enqueued": true, "event_id": event_id })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_missing_data(
    raw: Option<&str>,
) -> Result<chv_controlplane_store::MissingDataPolicy, BffError> {
    use chv_controlplane_store::MissingDataPolicy;
    match raw {
        None | Some("unknown") => Ok(MissingDataPolicy::Unknown),
        Some("fire") => Ok(MissingDataPolicy::Fire),
        Some("ignore") => Ok(MissingDataPolicy::Ignore),
        Some(other) => Err(invalid(format!(
            "missing_data must be unknown|fire|ignore, got {other:?}"
        ))),
    }
}

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}
