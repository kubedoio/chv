-- Native monitoring store, schema v1 (#602, ADR-027,
-- docs/specs/component/chv-monitoring-history-alerts-spec.md).
--
-- This database is DISPOSABLE TELEMETRY, never operational state: VM
-- lifecycle must work with this file deleted. It lives in a separate
-- SQLite file (/var/lib/chv/monitoring/monitoring.db) with its own pool
-- so monitoring queries and maintenance can never hold operational DB
-- locks.
--
-- Value columns are typed, not a float grab-bag: counters use
-- value_integer (exact; SQLite INTEGER is i64 and ingestion saturates
-- u64 counters at i64::MAX — byte counters reach 2^63 only after
-- centuries at 10 Gbit/s line rate), gauges use value_real. Both are
-- NULL exactly when quality != 'valid': missing data is stored as an
-- absent value plus its quality, never as a zero placeholder.

-- Key/value metadata (rollup cursors, maintenance timestamps).
CREATE TABLE monitoring_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Dimension dictionary: a bounded, hash-addressed set of dimension
-- key/value maps. Samples reference the hash, keeping per-row storage
-- O(1); unreferenced sets are garbage-collected by maintenance.
CREATE TABLE monitoring_dimension_sets (
  dimensions_hash TEXT PRIMARY KEY,
  dimensions_json TEXT NOT NULL,
  last_seen_at_ms INTEGER NOT NULL
);

-- Raw samples. Retention target: 48 hours (maintenance deletes older).
-- boot_id + identity_epoch participate in the primary key so a restarted
-- counter source can never collide with its previous incarnation at the
-- same observed_at_ms.
CREATE TABLE monitoring_samples (
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  metric_id TEXT NOT NULL,
  source TEXT NOT NULL,
  dimensions_hash TEXT NOT NULL,
  observed_at_ms INTEGER NOT NULL,
  received_at_ms INTEGER NOT NULL,
  value_integer INTEGER,
  value_real REAL,
  quality TEXT NOT NULL,
  kind TEXT NOT NULL,
  unit TEXT NOT NULL,
  boot_id TEXT NOT NULL DEFAULT '',
  identity_epoch TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (target_kind, target_id, metric_id, dimensions_hash, source, observed_at_ms, boot_id, identity_epoch),
  CHECK (
    (quality = 'valid' AND ((value_integer IS NOT NULL) != (value_real IS NOT NULL)))
    OR (quality != 'valid' AND value_integer IS NULL AND value_real IS NULL)
  )
);
CREATE INDEX monitoring_samples_series
  ON monitoring_samples (target_kind, target_id, metric_id, source, dimensions_hash, observed_at_ms);
CREATE INDEX monitoring_samples_retention
  ON monitoring_samples (observed_at_ms);
CREATE INDEX monitoring_samples_received
  ON monitoring_samples (received_at_ms);

-- Durable deduplication window for the ingestion contract's
-- (authenticated sender, boot_id, sequence) key. Retention bounded by
-- maintenance (received_at_ms); a replay outside the retained window
-- fails closed via the sequence high-water mark, never re-inserts.
CREATE TABLE monitoring_ingest_dedup (
  sender_node_id TEXT NOT NULL,
  boot_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  batch_digest TEXT NOT NULL,
  accepted_samples INTEGER NOT NULL,
  received_at_ms INTEGER NOT NULL,
  PRIMARY KEY (sender_node_id, boot_id, sequence)
);
CREATE INDEX monitoring_ingest_dedup_retention
  ON monitoring_ingest_dedup (received_at_ms);

-- Per-sender high-water mark: the newest sequence durably accepted for
-- a (sender, boot_id). A sequence <= high water without a retained dedup
-- row is an out-of-window replay and is rejected (resync_required
-- semantics from the ingestion contract) rather than re-inserted.
CREATE TABLE monitoring_ingest_watermarks (
  sender_node_id TEXT NOT NULL,
  boot_id TEXT NOT NULL,
  high_water_sequence INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (sender_node_id, boot_id)
);

-- Series registry: one row per distinct (target, metric, source,
-- dimension set). Enforces the per-target series cap at ingestion and
-- gives the query path series metadata without scanning samples.
CREATE TABLE monitoring_series (
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  metric_id TEXT NOT NULL,
  source TEXT NOT NULL,
  dimensions_hash TEXT NOT NULL,
  kind TEXT NOT NULL,
  unit TEXT NOT NULL,
  last_observed_at_ms INTEGER NOT NULL,
  PRIMARY KEY (target_kind, target_id, metric_id, source, dimensions_hash)
);

-- UTC-aligned rollups, tier '5m' (30-day retention) and '1h' (180-day).
-- Gauge windows carry min/max/sum/count over valid points; counter
-- windows carry first/last/delta where delta is NULL whenever the
-- window contains a reset (non-monotonic step or epoch change) — the
-- rate is then honestly absent, never a negative or fabricated spike.
-- Idempotent: a window is recomputed from raw data wholesale, so an
-- interrupted pass or late raw arrival simply rewrites the row.
CREATE TABLE monitoring_rollups (
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  metric_id TEXT NOT NULL,
  source TEXT NOT NULL,
  dimensions_hash TEXT NOT NULL,
  tier TEXT NOT NULL,
  window_start_ms INTEGER NOT NULL,
  window_ms INTEGER NOT NULL,
  value_min REAL,
  value_max REAL,
  value_sum REAL,
  value_count INTEGER,
  counter_first INTEGER,
  counter_last INTEGER,
  counter_delta INTEGER,
  valid_points INTEGER NOT NULL,
  total_points INTEGER NOT NULL,
  PRIMARY KEY (target_kind, target_id, metric_id, source, dimensions_hash, tier, window_start_ms)
);
CREATE INDEX monitoring_rollups_retention
  ON monitoring_rollups (tier, window_start_ms);
