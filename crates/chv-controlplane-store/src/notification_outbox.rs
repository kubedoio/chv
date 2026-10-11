//! Durable notification outbox (ADR-027, campaign #602, prompt 05 /
//! gate G4 part 2).
//!
//! At-least-once delivery with idempotent enqueue: `event_id` is the
//! primary key and inserts use `OR IGNORE`, so a crash between alert
//! persistence and webhook send can never duplicate a notification —
//! the evaluator re-enqueues, the insert is a no-op, the dispatcher
//! delivers exactly as many events as were accepted.
//!
//! `payload` is the pre-rendered, redacted contract envelope. The
//! dispatcher never re-derives notification content from incident
//! state, so nothing beyond the contract's fields can ever be sent,
//! and a rule edit can never retroactively change a queued message.
//!
//! Claiming (`claim_due`) is a single atomic `UPDATE … RETURNING`:
//! the due-time doubles as the lease, so a crashed dispatcher's
//! claims expire and the events retry without a reaper.

use crate::{StoreError, StorePool};
use sqlx::Transaction;

/// Event types (contract vocabulary).
pub const EVENT_TYPE_FIRING: &str = "firing";
pub const EVENT_TYPE_RESOLVED: &str = "resolved";
pub const EVENT_TYPE_ACKNOWLEDGED: &str = "acknowledged";
pub const EVENT_TYPE_DELIVERY_FAILED: &str = "delivery_failed";
pub const EVENT_TYPE_TEST: &str = "test";

/// Delivery channels.
pub const CHANNEL_WEBHOOK: &str = "webhook";
pub const CHANNEL_SLACK: &str = "slack";

/// Everything needed to enqueue one notification event. The payload
/// is fully rendered by the caller; this structure carries it, never
/// builds it.
#[derive(Debug, Clone)]
pub struct NotificationEventInput {
    /// Idempotency key (uuid) — a duplicate insert is ignored.
    pub event_id: String,
    pub alert_id: String,
    pub incident_key: String,
    pub event_type: String,
    pub severity: String,
    pub target_kind: String,
    pub target_id: String,
    pub summary: String,
    pub occurred_at_ms: i64,
    /// Pre-rendered, redacted contract envelope (JSON).
    pub payload: String,
    pub channel: String,
}

impl NotificationEventInput {
    fn validate(&self) -> Result<(), StoreError> {
        for (label, value) in [
            ("event_id", &self.event_id),
            ("alert_id", &self.alert_id),
            ("incident_key", &self.incident_key),
            ("target_kind", &self.target_kind),
            ("target_id", &self.target_id),
        ] {
            if value.is_empty() || value.len() > 256 {
                return Err(StoreError::InvalidConfiguration {
                    reason: format!("notification event {label} must be 1..=256 bytes"),
                });
            }
        }
        if !matches!(
            self.event_type.as_str(),
            EVENT_TYPE_FIRING
                | EVENT_TYPE_RESOLVED
                | EVENT_TYPE_ACKNOWLEDGED
                | EVENT_TYPE_DELIVERY_FAILED
                | EVENT_TYPE_TEST
        ) {
            return Err(StoreError::InvalidConfiguration {
                reason: format!("unknown notification event_type {:?}", self.event_type),
            });
        }
        if !matches!(self.channel.as_str(), CHANNEL_WEBHOOK | CHANNEL_SLACK) {
            return Err(StoreError::InvalidConfiguration {
                reason: format!("unknown notification channel {:?}", self.channel),
            });
        }
        if self.summary.is_empty() || self.summary.len() > 512 {
            return Err(StoreError::InvalidConfiguration {
                reason: "notification summary must be 1..=512 bytes".into(),
            });
        }
        if self.payload.is_empty() || self.payload.len() > 16 * 1024 {
            return Err(StoreError::InvalidConfiguration {
                reason: "notification payload must be 1..=16384 bytes".into(),
            });
        }
        serde_json::from_str::<serde_json::Value>(&self.payload).map_err(|e| {
            StoreError::InvalidConfiguration {
                reason: format!("notification payload is not valid JSON: {e}"),
            }
        })?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct OutboxEventRow {
    pub event_id: String,
    pub alert_id: String,
    pub incident_key: String,
    pub event_type: String,
    pub severity: String,
    pub target_kind: String,
    pub target_id: String,
    pub summary: String,
    pub occurred_at_ms: i64,
    pub payload: String,
    pub channel: String,
    pub status: String,
    pub attempts: i64,
    pub next_attempt_at_ms: i64,
    pub last_attempt_ms: Option<i64>,
    pub last_response: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

const ENQUEUE_SQL: &str = r#"
INSERT OR IGNORE INTO notification_outbox (
    event_id, alert_id, incident_key, event_type, severity,
    target_kind, target_id, summary, occurred_at_ms, payload,
    channel, status, attempts, next_attempt_at_ms, created_at_ms, updated_at_ms
)
VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, $8, $9, $10,
    $11, 'pending', 0, $9, $9, $9
)
"#;

/// Enqueue on an open transaction — the ONLY way an enqueue may join
/// an incident transition, giving the crash-between-persistence-and-
/// send guarantee (both rows or neither).
pub async fn enqueue_on_tx(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    input: &NotificationEventInput,
) -> Result<bool, StoreError> {
    input.validate()?;
    let result = sqlx::query(ENQUEUE_SQL)
        .bind(&input.event_id)
        .bind(&input.alert_id)
        .bind(&input.incident_key)
        .bind(&input.event_type)
        .bind(&input.severity)
        .bind(&input.target_kind)
        .bind(&input.target_id)
        .bind(&input.summary)
        .bind(input.occurred_at_ms)
        .bind(&input.payload)
        .bind(&input.channel)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[derive(Clone)]
pub struct NotificationOutboxRepository {
    pool: StorePool,
}

impl NotificationOutboxRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    /// Idempotent enqueue. Returns `true` when the event was newly
    /// inserted, `false` when an event with the same id already
    /// existed (the desired outcome of a crash-replay).
    pub async fn enqueue(&self, input: &NotificationEventInput) -> Result<bool, StoreError> {
        input.validate()?;
        let result = sqlx::query(ENQUEUE_SQL)
            .bind(&input.event_id)
            .bind(&input.alert_id)
            .bind(&input.incident_key)
            .bind(&input.event_type)
            .bind(&input.severity)
            .bind(&input.target_kind)
            .bind(&input.target_id)
            .bind(&input.summary)
            .bind(input.occurred_at_ms)
            .bind(&input.payload)
            .bind(&input.channel)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Atomically claim due events: the claim increments `attempts`,
    /// stamps `last_attempt_ms` and pushes `next_attempt_at_ms` out
    /// by the lease, so a dispatcher crash mid-batch lets the events
    /// return on lease expiry.
    pub async fn claim_due(
        &self,
        now_ms: i64,
        lease_ms: i64,
        limit: i64,
    ) -> Result<Vec<OutboxEventRow>, StoreError> {
        let rows: Vec<OutboxEventRow> = sqlx::query_as(
            r#"
            UPDATE notification_outbox SET
                attempts = attempts + 1,
                last_attempt_ms = $1,
                next_attempt_at_ms = $1 + $2,
                updated_at_ms = $1
            WHERE event_id IN (
                SELECT event_id FROM notification_outbox
                WHERE status = 'pending' AND next_attempt_at_ms <= $1
                ORDER BY next_attempt_at_ms, event_id
                LIMIT $3
            )
            RETURNING *
            "#,
        )
        .bind(now_ms)
        .bind(lease_ms)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn mark_delivered(
        &self,
        event_id: &str,
        now_ms: i64,
        response_note: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            UPDATE notification_outbox SET
                status = 'delivered', last_response = $2, updated_at_ms = $3
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .bind(response_note)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn schedule_retry(
        &self,
        event_id: &str,
        next_attempt_at_ms: i64,
        now_ms: i64,
        response_note: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            UPDATE notification_outbox SET
                status = 'pending', next_attempt_at_ms = $2,
                last_response = $3, updated_at_ms = $4
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .bind(next_attempt_at_ms)
        .bind(response_note)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn dead_letter(
        &self,
        event_id: &str,
        now_ms: i64,
        response_note: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            UPDATE notification_outbox SET
                status = 'dead', last_response = $2, updated_at_ms = $3
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .bind(response_note)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Recent events for the UI's delivery audit view (any status,
    /// newest first).
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<OutboxEventRow>, StoreError> {
        let rows: Vec<OutboxEventRow> = sqlx::query_as(
            "SELECT * FROM notification_outbox ORDER BY updated_at_ms DESC, event_id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::create_test_pool;

    fn event(event_id: &str, occurred_at_ms: i64) -> NotificationEventInput {
        NotificationEventInput {
            event_id: event_id.into(),
            alert_id: "alert-1".into(),
            incident_key: "rule-1:vm:vm-1:-".into(),
            event_type: EVENT_TYPE_FIRING.into(),
            severity: "warning".into(),
            target_kind: "vm".into(),
            target_id: "vm-1".into(),
            summary: "VM CPU pressure firing".into(),
            occurred_at_ms,
            payload: r#"{"schema_version":1,"event_id":"x"}"#.into(),
            channel: CHANNEL_WEBHOOK.into(),
        }
    }

    #[tokio::test]
    async fn enqueue_is_idempotent_by_event_id() {
        let repo = NotificationOutboxRepository::new(create_test_pool().await);
        assert!(repo.enqueue(&event("evt-1", 1_000)).await.expect("first"));
        assert!(
            !repo.enqueue(&event("evt-1", 2_000)).await.expect("replay"),
            "a replayed enqueue must be ignored"
        );
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notification_outbox")
            .fetch_one(repo.pool())
            .await
            .expect("count");
        assert_eq!(count, 1);
        // The original occurrence time survives the replay.
        let rows = repo.list_recent(10).await.expect("list");
        assert_eq!(rows[0].occurred_at_ms, 1_000);
    }

    #[tokio::test]
    async fn claim_due_respects_lease_and_batch() {
        let repo = NotificationOutboxRepository::new(create_test_pool().await);
        repo.enqueue(&event("evt-now-1", 1_000)).await.expect("e1");
        repo.enqueue(&event("evt-now-2", 1_000)).await.expect("e2");
        let mut future = event("evt-later", 5_000);
        future.event_id = "evt-later".into();
        repo.enqueue(&future).await.expect("e3");

        // Only the two due events claim; attempts increment and the
        // lease pushes the due time forward.
        let claimed = repo.claim_due(2_000, 1_000, 10).await.expect("claim");
        assert_eq!(claimed.len(), 2);
        assert!(claimed.iter().all(|e| e.attempts == 1));
        assert!(claimed.iter().all(|e| e.next_attempt_at_ms == 3_000));

        // Nothing is due while leased.
        let leased = repo.claim_due(2_500, 1_000, 10).await.expect("claim");
        assert!(leased.is_empty());

        // Lease expiry makes them due again (attempts keep counting).
        let reclaimed = repo.claim_due(3_500, 1_000, 10).await.expect("reclaim");
        assert_eq!(reclaimed.len(), 2);
        assert!(reclaimed.iter().all(|e| e.attempts == 2));

        // Delivered events never claim again.
        repo.mark_delivered("evt-now-1", 4_000, "2xx")
            .await
            .expect("delivered 1");
        repo.mark_delivered("evt-now-2", 4_000, "2xx")
            .await
            .expect("delivered 2");

        // The future-dated event only claims once due.
        let early = repo.claim_due(4_500, 1_000, 10).await.expect("claim early");
        assert!(early.is_empty());
        let late = repo.claim_due(5_500, 1_000, 10).await.expect("claim late");
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].event_id, "evt-later");

        // Batch limits bound the claim.
        for i in 0..5 {
            let mut e = event(&format!("evt-batch-{i}"), 6_000);
            e.event_id = format!("evt-batch-{i}");
            repo.enqueue(&e).await.expect("enqueue batch");
        }
        let bounded = repo
            .claim_due(7_000, 1_000, 2)
            .await
            .expect("claim bounded");
        assert_eq!(bounded.len(), 2);

        // Everything still pending claims after lease expiry: the
        // three unclaimed batch events, the two leased batch events,
        // and the leased future event.
        let rest = repo
            .claim_due(100_000, 1_000, 10)
            .await
            .expect("claim rest");
        assert_eq!(rest.len(), 6);
    }

    #[tokio::test]
    async fn delivery_lifecycle_marks() {
        let repo = NotificationOutboxRepository::new(create_test_pool().await);
        repo.enqueue(&event("evt-d", 1_000)).await.expect("enqueue");
        repo.enqueue(&event("evt-r", 1_000)).await.expect("enqueue");
        repo.enqueue(&event("evt-x", 1_000)).await.expect("enqueue");

        let claimed = repo.claim_due(2_000, 60_000, 10).await.expect("claim");
        assert_eq!(claimed.len(), 3);

        repo.mark_delivered("evt-d", 3_000, "2xx")
            .await
            .expect("delivered");
        repo.schedule_retry("evt-r", 30_000, 3_000, "5xx")
            .await
            .expect("retry");
        repo.dead_letter("evt-x", 3_000, "4xx permanent")
            .await
            .expect("dead");

        let rows = repo.list_recent(10).await.expect("list");
        let by_id = |id: &str| {
            rows.iter()
                .find(|r| r.event_id == id)
                .unwrap_or_else(|| panic!("missing {id}"))
        };
        assert_eq!(by_id("evt-d").status, "delivered");
        assert_eq!(by_id("evt-d").last_response.as_deref(), Some("2xx"));
        assert_eq!(by_id("evt-r").status, "pending");
        assert_eq!(by_id("evt-r").next_attempt_at_ms, 30_000);
        assert_eq!(by_id("evt-x").status, "dead");
        // Dead and delivered events never claim again.
        let due = repo.claim_due(100_000, 1_000, 10).await.expect("claim");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].event_id, "evt-r");
    }

    #[tokio::test]
    async fn enqueue_validation_rejects_bad_events() {
        let repo = NotificationOutboxRepository::new(create_test_pool().await);
        let mut bad_type = event("evt-bad-1", 1_000);
        bad_type.event_type = "exploded".into();
        assert!(repo.enqueue(&bad_type).await.is_err());

        let mut bad_payload = event("evt-bad-2", 1_000);
        bad_payload.payload = "not json".into();
        assert!(repo.enqueue(&bad_payload).await.is_err());

        let mut bad_channel = event("evt-bad-3", 1_000);
        bad_channel.channel = "fax".into();
        assert!(repo.enqueue(&bad_channel).await.is_err());

        let mut empty_summary = event("evt-bad-4", 1_000);
        empty_summary.summary = String::new();
        assert!(repo.enqueue(&empty_summary).await.is_err());
    }
}
