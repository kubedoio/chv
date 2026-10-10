use crate::notification_outbox::{enqueue_on_tx, NotificationEventInput};
use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::{EventSeverity, NodeId, ResourceKind};

const CREATE_ALERT_SQL: &str = r#"
INSERT INTO alerts (
    alert_type,
    severity,
    resource_kind,
    resource_id,
    node_id,
    status,
    message,
    operation_id,
    opened_at
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    $7,
    $8,
    strftime('%Y-%m-%dT%H:%M:%SZ', $9 / 1000.0, 'unixepoch')
)
RETURNING alert_id
"#;

#[derive(Clone)]
pub struct AlertRepository {
    pool: StorePool,
}

impl AlertRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(&self, input: &AlertCreateInput) -> Result<String, StoreError> {
        let row: (String,) = sqlx::query_as(CREATE_ALERT_SQL)
            .bind(&input.alert_type)
            .bind(input.severity.as_str())
            .bind(input.resource_kind.as_ref().map(|k| k.as_str()))
            .bind(&input.resource_id)
            .bind(input.node_id.as_ref().map(NodeId::as_str))
            .bind(&input.status)
            .bind(&input.message)
            .bind(&input.operation_id)
            .bind(input.opened_unix_ms)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

pub struct AlertCreateInput {
    pub alert_type: String,
    pub severity: EventSeverity,
    pub resource_kind: Option<ResourceKind>,
    pub resource_id: Option<String>,
    pub node_id: Option<NodeId>,
    pub status: String,
    pub message: String,
    pub operation_id: Option<String>,
    pub opened_unix_ms: i64,
}

// ---------------------------------------------------------------------------
// Monitoring incidents (ADR-027, campaign #602, prompt 05 / G4 part 2)
//
// Monitoring incidents reuse this table with source='monitoring' and a
// pending | firing | resolved status vocabulary (enforced here, not by
// a column CHECK — operational rows keep 'open'). The lifecycle:
//
//   inactive --condition true--> pending --held for `for_seconds`-->
//   firing --condition false for `recovery_seconds`--> resolved
//
// `pending` cleared before the hold elapses is DELETEd (inactive is
// not a stored state). Acknowledgment and silence are overlays on
// `firing`/`pending` incidents — never resolutions. Firing and
// resolving transitions may carry an outbox enqueue on the SAME
// transaction, which is the crash-between-persistence-and-send
// guarantee.
// ---------------------------------------------------------------------------

/// Status vocabulary for monitoring incidents.
pub const INCIDENT_STATUS_PENDING: &str = "pending";
pub const INCIDENT_STATUS_FIRING: &str = "firing";
pub const INCIDENT_STATUS_RESOLVED: &str = "resolved";

/// `alerts.source` value for rule-driven incidents.
pub const ALERT_SOURCE_MONITORING: &str = "monitoring";
/// `alerts.alert_type` for rule-driven incidents (rule identity lives
/// in the rule_id/rule_revision columns; the name is in `message`).
pub const ALERT_TYPE_MONITORING_RULE: &str = "monitoring.rule";

/// An incident row, fully materialized. `Option<i64>` ms columns are
/// NULL until the lifecycle sets them.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct IncidentRow {
    pub alert_id: String,
    pub alert_type: String,
    pub severity: String,
    pub resource_kind: Option<String>,
    pub resource_id: Option<String>,
    pub node_id: Option<String>,
    pub status: String,
    pub message: String,
    pub opened_at: String,
    pub acknowledged_at: Option<String>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub source: String,
    pub rule_id: Option<String>,
    pub rule_revision: Option<i64>,
    pub dedup_key: Option<String>,
    pub pending_since_ms: Option<i64>,
    pub first_occurrence_ms: Option<i64>,
    pub last_occurrence_ms: Option<i64>,
    pub clear_since_ms: Option<i64>,
    pub last_observed: Option<String>,
    pub evidence_from_ms: Option<i64>,
    pub evidence_to_ms: Option<i64>,
    pub silenced_until_ms: Option<i64>,
    pub silenced_by: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct IncidentListFilter {
    pub status: Option<String>,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub rule_id: Option<String>,
    pub include_resolved: bool,
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct IncidentTransitionRow {
    pub transition_id: i64,
    pub alert_id: String,
    pub from_state: Option<String>,
    pub to_state: String,
    pub occurred_at_ms: i64,
    pub reason: String,
    pub measured: Option<String>,
}

pub struct IncidentOpenInput {
    pub rule_id: String,
    pub rule_revision: i64,
    /// rule + target + dimension match — see the evaluator's dedup
    /// key derivation.
    pub dedup_key: String,
    pub severity: String,
    pub target_kind: String,
    pub target_id: String,
    /// Node targets carry their node_id; VM targets leave it NULL
    /// (placement is not incident identity).
    pub node_id: Option<String>,
    pub message: String,
    pub now_ms: i64,
    /// Rendered, redacted last measurement (display text only).
    pub last_observed: Option<String>,
    pub evidence_from_ms: i64,
    pub evidence_to_ms: i64,
}

impl AlertRepository {
    /// The active (pending or firing) incident for a dedup key, if
    /// any. The partial unique index keeps this unambiguous.
    pub async fn find_active_incident(
        &self,
        dedup_key: &str,
    ) -> Result<Option<IncidentRow>, StoreError> {
        let row: Option<IncidentRow> = sqlx::query_as(
            r#"
            SELECT * FROM alerts
            WHERE dedup_key = $1 AND status IN ('pending','firing')
            "#,
        )
        .bind(dedup_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Open a pending incident. Racy double-opens lose to the partial
    /// unique index (a `Conflict`, which the evaluator treats as
    /// "already open" — idempotent by construction).
    pub async fn open_pending(&self, input: &IncidentOpenInput) -> Result<String, StoreError> {
        let mut tx = self.pool.begin().await?;
        let alert_id: (String,) = sqlx::query_as(
            r#"
            INSERT INTO alerts (
                alert_type, severity, resource_kind, resource_id, node_id,
                status, message, opened_at, created_at, updated_at,
                source, rule_id, rule_revision, dedup_key,
                pending_since_ms, first_occurrence_ms, last_occurrence_ms,
                last_observed, evidence_from_ms, evidence_to_ms
            )
            VALUES (
                $1, $2, $3, $4, $5,
                'pending', $6,
                strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch'),
                strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch'),
                strftime('%Y-%m-%dT%H:%M:%SZ', $7 / 1000.0, 'unixepoch'),
                'monitoring', $8, $9, $10,
                $7, $7, $7,
                $11, $12, $13
            )
            RETURNING alert_id
            "#,
        )
        .bind(ALERT_TYPE_MONITORING_RULE)
        .bind(&input.severity)
        .bind(&input.target_kind)
        .bind(&input.target_id)
        .bind(&input.node_id)
        .bind(&input.message)
        .bind(input.now_ms)
        .bind(&input.rule_id)
        .bind(input.rule_revision)
        .bind(&input.dedup_key)
        .bind(&input.last_observed)
        .bind(input.evidence_from_ms)
        .bind(input.evidence_to_ms)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO alert_transitions (
                alert_id, from_state, to_state, occurred_at_ms, reason, measured
            )
            VALUES ($1, NULL, 'pending', $2, $3, $4)
            "#,
        )
        .bind(&alert_id.0)
        .bind(input.now_ms)
        .bind("condition observed")
        .bind(&input.last_observed)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(alert_id.0)
    }

    /// Promote a pending incident to firing, optionally enqueueing the
    /// notification events (one per configured channel) on the same
    /// transaction (all-or-nothing). Returns `false` when the incident
    /// is no longer pending (the evaluator treats that as
    /// already-promoted, never an error).
    pub async fn promote_to_firing(
        &self,
        alert_id: &str,
        now_ms: i64,
        last_observed: Option<&str>,
        evidence_from_ms: i64,
        evidence_to_ms: i64,
        notify: &[NotificationEventInput],
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        let promoted: Option<(String,)> = sqlx::query_as(
            r#"
            UPDATE alerts SET
                status = 'firing', last_occurrence_ms = $2,
                last_observed = $3, evidence_from_ms = $4, evidence_to_ms = $5,
                clear_since_ms = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $2 / 1000.0, 'unixepoch')
            WHERE alert_id = $1 AND status = 'pending'
            RETURNING alert_id
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .bind(last_observed)
        .bind(evidence_from_ms)
        .bind(evidence_to_ms)
        .fetch_optional(&mut *tx)
        .await?;
        if promoted.is_none() {
            return Ok(false);
        }
        sqlx::query(
            r#"
            INSERT INTO alert_transitions (
                alert_id, from_state, to_state, occurred_at_ms, reason, measured
            )
            VALUES ($1, 'pending', 'firing', $2, 'condition sustained', $3)
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .bind(last_observed)
        .execute(&mut *tx)
        .await?;
        if !notify.is_empty() {
            for event in notify {
                enqueue_on_tx(&mut tx, event).await?;
            }
        }
        tx.commit().await?;
        Ok(true)
    }

    /// Refresh the observation on a still-pending or firing incident
    /// (last occurrence, measurement, evidence window). Also clears a
    /// stale recovery marker — the condition is true again.
    pub async fn note_observation(
        &self,
        alert_id: &str,
        now_ms: i64,
        last_observed: Option<&str>,
        evidence_from_ms: i64,
        evidence_to_ms: i64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            UPDATE alerts SET
                last_occurrence_ms = $2, last_observed = $3,
                evidence_from_ms = $4, evidence_to_ms = $5,
                clear_since_ms = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $2 / 1000.0, 'unixepoch')
            WHERE alert_id = $1 AND status IN ('pending','firing')
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .bind(last_observed)
        .bind(evidence_from_ms)
        .bind(evidence_to_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark the condition false on a firing incident (starts the
    /// recovery window). Idempotent: the first false observation
    /// wins until a true observation clears it.
    pub async fn mark_condition_false(
        &self,
        alert_id: &str,
        now_ms: i64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            UPDATE alerts SET clear_since_ms = $2
            WHERE alert_id = $1 AND status = 'firing' AND clear_since_ms IS NULL
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a pending incident cleared before its hold elapsed —
    /// `inactive` is not a stored state (spec state machine), and a
    /// never-fired pending leaves no historical noise.
    pub async fn clear_pending(&self, alert_id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM alerts WHERE alert_id = $1 AND status = 'pending'")
            .bind(alert_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Resolve a firing incident, optionally enqueueing the resolved
    /// notifications on the same transaction. Returns `false` when the
    /// incident is no longer firing (treated as already-resolved).
    pub async fn resolve_incident(
        &self,
        alert_id: &str,
        now_ms: i64,
        reason: &str,
        last_observed: Option<&str>,
        notify: &[NotificationEventInput],
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        let resolved: Option<(String,)> = sqlx::query_as(
            r#"
            UPDATE alerts SET
                status = 'resolved', clear_since_ms = NULL,
                last_occurrence_ms = $2, last_observed = $3,
                resolved_at = strftime('%Y-%m-%dT%H:%M:%SZ', $2 / 1000.0, 'unixepoch'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $2 / 1000.0, 'unixepoch')
            WHERE alert_id = $1 AND status = 'firing'
            RETURNING alert_id
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .bind(last_observed)
        .fetch_optional(&mut *tx)
        .await?;
        if resolved.is_none() {
            return Ok(false);
        }
        sqlx::query(
            r#"
            INSERT INTO alert_transitions (
                alert_id, from_state, to_state, occurred_at_ms, reason, measured
            )
            VALUES ($1, 'firing', 'resolved', $2, $3, $4)
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .bind(reason)
        .bind(last_observed)
        .execute(&mut *tx)
        .await?;
        if !notify.is_empty() {
            for event in notify {
                enqueue_on_tx(&mut tx, event).await?;
            }
        }
        tx.commit().await?;
        Ok(true)
    }

    /// Acknowledge an active incident. An overlay, never a
    /// resolution: acknowledged incidents keep firing and keep
    /// notifying (the contract's ack semantics).
    pub async fn acknowledge_incident(
        &self,
        alert_id: &str,
        actor: &str,
        now_ms: i64,
    ) -> Result<bool, StoreError> {
        if actor.is_empty() || actor.len() > 128 {
            return Err(StoreError::InvalidConfiguration {
                reason: "acknowledge actor must be 1..=128 bytes".into(),
            });
        }
        let result = sqlx::query(
            r#"
            UPDATE alerts SET
                acknowledged_at = strftime('%Y-%m-%dT%H:%M:%SZ', $2 / 1000.0, 'unixepoch'),
                acknowledged_by = $3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $2 / 1000.0, 'unixepoch')
            WHERE alert_id = $1 AND status IN ('pending','firing')
            "#,
        )
        .bind(alert_id)
        .bind(now_ms)
        .bind(actor)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Silence an active incident's notifications until a deadline.
    /// An overlay, never a resolution.
    pub async fn silence_incident(
        &self,
        alert_id: &str,
        actor: &str,
        until_ms: i64,
        now_ms: i64,
    ) -> Result<bool, StoreError> {
        if actor.is_empty() || actor.len() > 128 {
            return Err(StoreError::InvalidConfiguration {
                reason: "silence actor must be 1..=128 bytes".into(),
            });
        }
        if until_ms <= now_ms {
            return Err(StoreError::InvalidConfiguration {
                reason: "silence deadline must be in the future".into(),
            });
        }
        let result = sqlx::query(
            r#"
            UPDATE alerts SET
                silenced_until_ms = $2, silenced_by = $3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', $4 / 1000.0, 'unixepoch')
            WHERE alert_id = $1 AND status IN ('pending','firing')
            "#,
        )
        .bind(alert_id)
        .bind(until_ms)
        .bind(actor)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Paged incident listing for the alert center. With
    /// `include_resolved = false` only active incidents return;
    /// `pending` always returns (it is pre-notification, not
    /// pre-visibility).
    pub async fn list_incidents(
        &self,
        filter: &IncidentListFilter,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<IncidentRow>, i64), StoreError> {
        let rows: Vec<IncidentRow> = sqlx::query_as(
            r#"
            SELECT * FROM alerts
            WHERE source = 'monitoring'
              AND ($1 IS NULL OR status = $1)
              AND ($2 IS NULL OR resource_kind = $2)
              AND ($3 IS NULL OR resource_id = $3)
              AND ($4 IS NULL OR rule_id = $4)
              AND ($5 = 1 OR status IN ('pending','firing'))
            ORDER BY
                CASE status WHEN 'firing' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END,
                last_occurrence_ms DESC,
                alert_id
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(&filter.status)
        .bind(&filter.target_kind)
        .bind(&filter.target_id)
        .bind(&filter.rule_id)
        .bind(if filter.include_resolved { 1 } else { 0 })
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM alerts
            WHERE source = 'monitoring'
              AND ($1 IS NULL OR status = $1)
              AND ($2 IS NULL OR resource_kind = $2)
              AND ($3 IS NULL OR resource_id = $3)
              AND ($4 IS NULL OR rule_id = $4)
              AND ($5 = 1 OR status IN ('pending','firing'))
            "#,
        )
        .bind(&filter.status)
        .bind(&filter.target_kind)
        .bind(&filter.target_id)
        .bind(&filter.rule_id)
        .bind(if filter.include_resolved { 1 } else { 0 })
        .fetch_one(&self.pool)
        .await?;
        Ok((rows, total.0))
    }

    pub async fn get_incident(&self, alert_id: &str) -> Result<IncidentRow, StoreError> {
        let row: Option<IncidentRow> =
            sqlx::query_as("SELECT * FROM alerts WHERE alert_id = $1 AND source = 'monitoring'")
                .bind(alert_id)
                .fetch_optional(&self.pool)
                .await?;
        match row {
            Some(row) => Ok(row),
            None => Err(StoreError::NotFound {
                entity: "incident",
                id: alert_id.to_string(),
            }),
        }
    }

    /// Transition history for one incident (oldest first).
    pub async fn list_transitions(
        &self,
        alert_id: &str,
        limit: i64,
    ) -> Result<Vec<IncidentTransitionRow>, StoreError> {
        let rows: Vec<IncidentTransitionRow> = sqlx::query_as(
            r#"
            SELECT * FROM alert_transitions
            WHERE alert_id = $1
            ORDER BY occurred_at_ms, transition_id
            LIMIT $2
            "#,
        )
        .bind(alert_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification_outbox::NotificationEventInput;
    use crate::test_util::create_test_pool;

    fn open_input(dedup_key: &str, target_kind: &str, target_id: &str) -> IncidentOpenInput {
        IncidentOpenInput {
            rule_id: "rule-1".into(),
            rule_revision: 1,
            dedup_key: dedup_key.into(),
            severity: "warning".into(),
            target_kind: target_kind.into(),
            target_id: target_id.into(),
            node_id: None,
            message: "CPU pressure above 0.90 (rule 'VM CPU pressure')".into(),
            now_ms: 1_000_000,
            last_observed: Some("0.94 (vm.cpu.capacity_ratio)".into()),
            evidence_from_ms: 900_000,
            evidence_to_ms: 1_000_000,
        }
    }

    fn notify_event(alert_id: &str, event_type: &str) -> NotificationEventInput {
        NotificationEventInput {
            event_id: format!("evt-{event_type}-{alert_id}"),
            alert_id: alert_id.into(),
            incident_key: "rule-1:vm:vm-1:-".into(),
            event_type: event_type.into(),
            severity: "warning".into(),
            target_kind: "vm".into(),
            target_id: "vm-1".into(),
            summary: "VM CPU pressure firing".into(),
            occurred_at_ms: 1_100_000,
            payload: r#"{"schema_version":1}"#.into(),
            channel: "webhook".into(),
        }
    }

    #[tokio::test]
    async fn incident_lifecycle_pending_firing_resolved() {
        let repo = AlertRepository::new(create_test_pool().await);

        let alert_id = repo
            .open_pending(&open_input("rule-1:vm:vm-1:-", "vm", "vm-1"))
            .await
            .expect("open");
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.status, "pending");
        assert_eq!(incident.source, "monitoring");
        assert_eq!(incident.rule_id.as_deref(), Some("rule-1"));
        assert_eq!(incident.pending_since_ms, Some(1_000_000));
        assert!(incident.resolved_at.is_none());

        let transitions = repo
            .list_transitions(&alert_id, 10)
            .await
            .expect("transitions");
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].from_state, None);
        assert_eq!(transitions[0].to_state, "pending");

        // Promotion carries the notification on the same transaction.
        let firing_event = notify_event(&alert_id, "firing");
        let promoted = repo
            .promote_to_firing(
                &alert_id,
                1_200_000,
                Some("0.95 (x)"),
                1_100_000,
                1_200_000,
                std::slice::from_ref(&firing_event),
            )
            .await
            .expect("promote");
        assert!(promoted);
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.status, "firing");
        assert_eq!(incident.clear_since_ms, None);

        // The outbox row exists and carries the pre-rendered payload.
        let outbox: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM notification_outbox WHERE alert_id = $1")
                .bind(&alert_id)
                .fetch_one(repo.pool())
                .await
                .expect("count outbox");
        assert_eq!(outbox.0, 1);

        // A second promotion is a no-op, not an error.
        let again = repo
            .promote_to_firing(&alert_id, 1_300_000, None, 0, 0, &[])
            .await
            .expect("promote again");
        assert!(!again);

        // Observation refresh while firing keeps it firing.
        repo.note_observation(&alert_id, 1_400_000, Some("0.97 (x)"), 1_300_000, 1_400_000)
            .await
            .expect("observe");
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.status, "firing");
        assert_eq!(incident.last_observed.as_deref(), Some("0.97 (x)"));

        // Recovery window bookkeeping, then resolution with the
        // notification on the same transaction.
        repo.mark_condition_false(&alert_id, 1_500_000)
            .await
            .expect("mark false");
        repo.mark_condition_false(&alert_id, 1_550_000)
            .await
            .expect("mark false again (idempotent)");
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.clear_since_ms, Some(1_500_000));

        let resolved_event = notify_event(&alert_id, "resolved");
        let resolved = repo
            .resolve_incident(
                &alert_id,
                1_700_000,
                "condition false for recovery window",
                None,
                std::slice::from_ref(&resolved_event),
            )
            .await
            .expect("resolve");
        assert!(resolved);
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.status, "resolved");
        assert!(incident.resolved_at.is_some());
        assert_eq!(incident.clear_since_ms, None);

        let transitions = repo
            .list_transitions(&alert_id, 10)
            .await
            .expect("transitions");
        let states: Vec<&str> = transitions.iter().map(|t| t.to_state.as_str()).collect();
        assert_eq!(states, vec!["pending", "firing", "resolved"]);

        let outbox: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM notification_outbox WHERE alert_id = $1")
                .bind(&alert_id)
                .fetch_one(repo.pool())
                .await
                .expect("count outbox");
        assert_eq!(outbox.0, 2);

        // Resolving again is a no-op.
        let again = repo
            .resolve_incident(&alert_id, 1_800_000, "replay", None, &[])
            .await
            .expect("resolve again");
        assert!(!again);
    }

    #[tokio::test]
    async fn pending_cleared_before_hold_is_deleted() {
        let repo = AlertRepository::new(create_test_pool().await);
        let alert_id = repo
            .open_pending(&open_input("rule-1:vm:vm-2:-", "vm", "vm-2"))
            .await
            .expect("open");
        repo.clear_pending(&alert_id).await.expect("clear");
        assert!(matches!(
            repo.get_incident(&alert_id).await,
            Err(StoreError::NotFound { .. })
        ));
        assert!(repo
            .find_active_incident("rule-1:vm:vm-2:-")
            .await
            .expect("find")
            .is_none());
        // The transitions died with the row (CASCADE).
        let transitions: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM alert_transitions WHERE alert_id = $1")
                .bind(&alert_id)
                .fetch_one(repo.pool())
                .await
                .expect("count transitions");
        assert_eq!(transitions.0, 0);
    }

    #[tokio::test]
    async fn one_active_incident_per_dedup_key() {
        let repo = AlertRepository::new(create_test_pool().await);
        repo.open_pending(&open_input("rule-1:vm:vm-3:-", "vm", "vm-3"))
            .await
            .expect("first open");
        let second = repo
            .open_pending(&open_input("rule-1:vm:vm-3:-", "vm", "vm-3"))
            .await;
        assert!(
            matches!(second, Err(StoreError::Database(_))),
            "a duplicate active incident must conflict, got {second:?}"
        );

        // After resolution, the same dedup key may open again.
        let alert_id = repo
            .find_active_incident("rule-1:vm:vm-3:-")
            .await
            .expect("find")
            .expect("active")
            .alert_id;
        repo.promote_to_firing(&alert_id, 1_100_000, None, 0, 0, &[])
            .await
            .expect("promote");
        repo.resolve_incident(&alert_id, 1_200_000, "recovered", None, &[])
            .await
            .expect("resolve");
        repo.open_pending(&open_input("rule-1:vm:vm-3:-", "vm", "vm-3"))
            .await
            .expect("reopen after resolve");
    }

    #[tokio::test]
    async fn acknowledge_and_silence_are_overlays() {
        let repo = AlertRepository::new(create_test_pool().await);
        let alert_id = repo
            .open_pending(&open_input("rule-1:vm:vm-4:-", "vm", "vm-4"))
            .await
            .expect("open");
        repo.promote_to_firing(&alert_id, 1_100_000, None, 0, 0, &[])
            .await
            .expect("promote");

        assert!(repo
            .acknowledge_incident(&alert_id, "op-user", 1_200_000)
            .await
            .expect("ack"));
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.acknowledged_by.as_deref(), Some("op-user"));
        assert!(incident.acknowledged_at.is_some());
        assert_eq!(incident.status, "firing", "ack never resolves");

        assert!(repo
            .silence_incident(&alert_id, "op-user", 2_000_000, 1_250_000)
            .await
            .expect("silence"));
        let incident = repo.get_incident(&alert_id).await.expect("get");
        assert_eq!(incident.silenced_until_ms, Some(2_000_000));
        assert_eq!(incident.status, "firing", "silence never resolves");

        // A silence deadline in the past is rejected outright.
        assert!(repo
            .silence_incident(&alert_id, "op-user", 1_000_000, 1_250_000)
            .await
            .is_err());

        // Resolving still works under overlays.
        repo.resolve_incident(&alert_id, 1_300_000, "recovered", None, &[])
            .await
            .expect("resolve");
        // Ack on a resolved incident is a no-op.
        assert!(!repo
            .acknowledge_incident(&alert_id, "op-user", 1_400_000)
            .await
            .expect("ack resolved"));
    }

    #[tokio::test]
    async fn incident_listing_filters_and_orders() {
        let repo = AlertRepository::new(create_test_pool().await);
        let a = repo
            .open_pending(&open_input("r:vm:vm-a:-", "vm", "vm-a"))
            .await
            .expect("a");
        let b = repo
            .open_pending(&open_input("r:vm:vm-b:-", "vm", "vm-b"))
            .await
            .expect("b");
        repo.promote_to_firing(&b, 1_100_000, None, 0, 0, &[])
            .await
            .expect("promote b");
        let c = repo
            .open_pending(&open_input("r:node:node-c:-", "node", "node-c"))
            .await
            .expect("c");
        repo.promote_to_firing(&c, 1_100_000, None, 0, 0, &[])
            .await
            .expect("promote c");
        repo.resolve_incident(&c, 1_200_000, "recovered", None, &[])
            .await
            .expect("resolve c");

        // Active only: firing sorts before pending.
        let (active, total) = repo
            .list_incidents(&IncidentListFilter::default(), 10, 0)
            .await
            .expect("list");
        assert_eq!(total, 2);
        assert_eq!(active[0].alert_id, b);
        assert_eq!(active[1].alert_id, a);

        // Resolved included on request, sorted last.
        let (all, total) = repo
            .list_incidents(
                &IncidentListFilter {
                    include_resolved: true,
                    ..Default::default()
                },
                10,
                0,
            )
            .await
            .expect("list all");
        assert_eq!(total, 3);
        assert_eq!(all[2].status, "resolved");

        // Target and status filters.
        let (_vms, total) = repo
            .list_incidents(
                &IncidentListFilter {
                    target_kind: Some("vm".into()),
                    include_resolved: true,
                    ..Default::default()
                },
                10,
                0,
            )
            .await
            .expect("list vms");
        assert_eq!(total, 2);
        let (firing, total) = repo
            .list_incidents(
                &IncidentListFilter {
                    status: Some("firing".into()),
                    include_resolved: true,
                    ..Default::default()
                },
                10,
                0,
            )
            .await
            .expect("list firing");
        assert_eq!(total, 1);
        assert_eq!(firing[0].alert_id, b);
        let (by_target, total) = repo
            .list_incidents(
                &IncidentListFilter {
                    target_id: Some("vm-a".into()),
                    include_resolved: true,
                    ..Default::default()
                },
                10,
                0,
            )
            .await
            .expect("list by target");
        assert_eq!(total, 1);
        assert_eq!(by_target[0].alert_id, a);
    }
}
