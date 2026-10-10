-- Check inventory, schema v1 (#602, G4; agent spec
-- docs/specs/component/chv-monitor-agent-spec.md: "Discovery creates
-- service/check inventory in the manager").
--
-- Like every table in 0001_initial.sql, this is DISPOSABLE TELEMETRY,
-- never operational state: VM lifecycle must work with this file
-- deleted. It lives in the same isolated monitoring SQLite database
-- with its own pool so monitoring queries can never hold operational
-- DB locks.
--
-- One row per (target, check): the LATEST record for that check, not a
-- history. The upsert only moves rows forward — a delayed batch with an
-- older observed_at_ms must never regress the inventory. `status` is
-- the check's typed state string (ok|warning|critical|unknown), never
-- a float; `received_at_ms` is manager-stamped, never sender-supplied.

-- Latest-record-per-check inventory (ingestion contract v1 `checks`).
CREATE TABLE monitoring_checks (
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  check_id TEXT NOT NULL,
  service_key TEXT,
  status TEXT NOT NULL,
  summary TEXT,
  observed_at_ms INTEGER NOT NULL,
  received_at_ms INTEGER NOT NULL,
  agent_id TEXT NOT NULL,
  PRIMARY KEY (target_kind, target_id, check_id)
);
CREATE INDEX monitoring_checks_target
  ON monitoring_checks (target_kind, target_id, received_at_ms);
