-- Native alerting (ADR-027, campaign #602, prompt 05 / gate G4
-- part 2): typed alert rules, the monitoring-incident model on the
-- existing `alerts` table, transition history, and the durable
-- notification outbox.
--
-- Placement rule (ADR-027 decision 7): alert workflow state is
-- control-plane authority and lives in the operational database —
-- never in the disposable monitoring store. Monitoring incidents
-- reuse the existing `alerts` table via additive columns, exactly as
-- the query/alert contract anticipated ("the current schema predates
-- several of these fields ... must not assume the table as-is").
--
-- Coexistence with operational alerts: existing rows keep their
-- `'open'` status and default to source 'operational'. Monitoring
-- incidents carry source 'monitoring' and use only the
-- pending | firing | resolved status vocabulary (enforced in the
-- store layer, not by a column CHECK, so operational rows are
-- untouched). Readers that count unresolved alerts count
-- open + firing; `pending` is pre-notification churn and must not
-- flap the overview badge.

-- ---------------------------------------------------------------------------
-- Monitoring-incident columns on `alerts` (all additive, all nullable
-- or defaulted — an operational row simply never sets them).
-- ---------------------------------------------------------------------------
ALTER TABLE alerts ADD COLUMN source TEXT NOT NULL DEFAULT 'operational';
-- The rule that produced this incident (monitoring rows only).
ALTER TABLE alerts ADD COLUMN rule_id TEXT;
-- Rule revision at incident creation — rule edits never rewrite
-- history retroactively.
ALTER TABLE alerts ADD COLUMN rule_revision INTEGER;
-- Incident identity for dedup: rule + target + dimension match. One
-- ACTIVE incident per key (partial unique index below): flapping
-- re-opens a fresh row, it never duplicates a live one.
ALTER TABLE alerts ADD COLUMN dedup_key TEXT;
ALTER TABLE alerts ADD COLUMN acknowledged_by TEXT;
-- Condition-true bookkeeping (epoch ms; integer columns per the
-- campaign's timestamp convention, not the legacy text strftime).
ALTER TABLE alerts ADD COLUMN pending_since_ms INTEGER;
ALTER TABLE alerts ADD COLUMN first_occurrence_ms INTEGER;
ALTER TABLE alerts ADD COLUMN last_occurrence_ms INTEGER;
-- Condition-false bookkeeping for the recovery window (firing only).
ALTER TABLE alerts ADD COLUMN clear_since_ms INTEGER;
-- Last observed measurement, rendered and redacted (e.g.
-- "0.94 (vm.cpu.capacity_ratio)"). Display text, never a computed
-- input.
ALTER TABLE alerts ADD COLUMN last_observed TEXT;
-- Evidence window the evaluator judged against — links the incident
-- to the history charts.
ALTER TABLE alerts ADD COLUMN evidence_from_ms INTEGER;
ALTER TABLE alerts ADD COLUMN evidence_to_ms INTEGER;
-- Silence is a notification overlay only: it never resolves the
-- incident (spec state machine).
ALTER TABLE alerts ADD COLUMN silenced_until_ms INTEGER;
ALTER TABLE alerts ADD COLUMN silenced_by TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS alerts_active_dedup_idx
    ON alerts (dedup_key)
    WHERE dedup_key IS NOT NULL AND status IN ('pending','firing');
CREATE INDEX IF NOT EXISTS alerts_rule_idx ON alerts (rule_id);
CREATE INDEX IF NOT EXISTS alerts_source_status_idx ON alerts (source, status);

-- ---------------------------------------------------------------------------
-- Typed alert rules. v1 rules bind exactly one target (no fleet
-- wildcards): evaluation stays bounded and the contract's typed-rule
-- shape is honored. The typed condition set is a JSON `spec` column
-- validated at every load — threshold, reset-safe rate, availability
-- (staleness), guest-check status, and one-level bounded AND/OR
-- groups. No PromQL, no SQL, no code execution ever crosses this
-- column.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS alert_rules (
    rule_id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('node','vm')),
    target_id TEXT NOT NULL,
    rule_type TEXT NOT NULL CHECK (rule_type IN ('threshold','rate','availability','check_status','group')),
    spec TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('critical','warning','info')),
    -- Hold-down before firing, and how long the condition must stay
    -- false before a firing incident resolves.
    for_seconds INTEGER NOT NULL DEFAULT 300,
    recovery_seconds INTEGER NOT NULL DEFAULT 120,
    -- What missing data means for this rule. NEVER a numeric zero.
    missing_data TEXT NOT NULL DEFAULT 'unknown' CHECK (missing_data IN ('unknown','fire','ignore')),
    -- Optimistic-concurrency revision: updates/deletes carry the
    -- caller's expected revision; a mismatch is a conflict.
    revision INTEGER NOT NULL DEFAULT 1,
    created_by TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS alert_rules_enabled_idx
    ON alert_rules (enabled, target_kind);

-- ---------------------------------------------------------------------------
-- Incident transition history (pending -> firing -> resolved). Rows
-- die with their incident (a pending incident cleared before hold is
-- deleted, not stored as 'inactive' — the spec state machine).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS alert_transitions (
    transition_id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_id TEXT NOT NULL REFERENCES alerts (alert_id) ON DELETE CASCADE,
    -- NULL for the creation transition.
    from_state TEXT,
    to_state TEXT NOT NULL,
    occurred_at_ms INTEGER NOT NULL,
    reason TEXT NOT NULL,
    measured TEXT
);

CREATE INDEX IF NOT EXISTS alert_transitions_alert_idx
    ON alert_transitions (alert_id, occurred_at_ms);

-- ---------------------------------------------------------------------------
-- Durable notification outbox (at-least-once, idempotent by event_id).
-- The event_id PRIMARY KEY plus INSERT OR IGNORE gives the
-- crash-between-persistence-and-send guarantee: enqueueing twice is a
-- no-op, so an evaluator crash can never duplicate a notification.
-- `payload` is the pre-rendered, redacted contract envelope — the
-- dispatcher never re-derives content, and nothing beyond the
-- contract's fields is ever stored or sent.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS notification_outbox (
    event_id TEXT PRIMARY KEY,
    alert_id TEXT NOT NULL,
    incident_key TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN ('firing','resolved','acknowledged','delivery_failed','test')),
    severity TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    occurred_at_ms INTEGER NOT NULL,
    payload TEXT NOT NULL,
    channel TEXT NOT NULL DEFAULT 'webhook' CHECK (channel IN ('webhook','slack')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','delivered','dead')),
    attempts INTEGER NOT NULL DEFAULT 0,
    -- Due-time doubles as the claim lease: claiming pushes it forward,
    -- so a crashed dispatcher's claims expire and get retried.
    next_attempt_at_ms INTEGER NOT NULL,
    last_attempt_ms INTEGER,
    last_response TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS notification_outbox_due_idx
    ON notification_outbox (status, next_attempt_at_ms);
