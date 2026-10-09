-- NetBox projection store (issue #239).
-- Per-architecture integration configuration and the projection run queue
-- for projecting applied CHV architectures into a downstream NetBox
-- instance. See docs/specs/component/architecture-designer-netbox-projection.md
-- and docs/plans/2026-10-08-netbox-projection-implementation-plan.md (PR 3).
--
-- The API token is stored only as CredentialEncryption ciphertext
-- (`token_ciphertext`); plaintext never leaves the store layer's write path.
-- The `trigger_kind` column intentionally avoids the SQL keyword
-- "trigger" and carries the wire enum manual | post_apply.

CREATE TABLE IF NOT EXISTS netbox_projection_config (
    architecture_id text PRIMARY KEY REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    endpoint text NOT NULL,
    token_secret_ref text NOT NULL,
    token_ciphertext text NOT NULL,
    retention_policy text NOT NULL DEFAULT 'mark_stale'
        CHECK (retention_policy IN ('mark_stale','delete')),
    enable_post_apply integer NOT NULL DEFAULT 0,
    custom_field_prefix text NOT NULL DEFAULT 'chv_',
    site_name text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS netbox_projection_runs (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    architecture_version_id text NOT NULL REFERENCES architecture_versions (id) ON DELETE CASCADE,
    trigger_kind text NOT NULL
        CHECK (trigger_kind IN ('manual','post_apply')),
    mode text NOT NULL
        CHECK (mode IN ('dry_run','export')),
    status text NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued','running','succeeded','failed')),
    plan_json text,
    result_json text,
    summary_json text,
    error_message text,
    attempt_count integer NOT NULL DEFAULT 0,
    requested_by text,
    started_at text,
    finished_at text,
    -- Earliest time a requeued (auto-retried) run may be claimed again;
    -- NULL for freshly enqueued runs and terminal states. Set by the
    -- repository's `requeue` to `now + backoff` (exponential, capped at
    -- 30 minutes); `claim_next_queued` skips queued runs whose backoff
    -- has not elapsed yet.
    next_attempt_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS netbox_projection_runs_architecture_id_created_at_idx
    ON netbox_projection_runs (architecture_id, created_at DESC);

CREATE INDEX IF NOT EXISTS netbox_projection_runs_status_idx
    ON netbox_projection_runs (status);

-- One active (queued or running) run per architecture: a manual enqueue
-- while one is active is rejected (NETBOX_RUN_ACTIVE) and the post-apply
-- trigger coalesces instead of stacking a second run.
CREATE UNIQUE INDEX IF NOT EXISTS netbox_projection_runs_one_active
    ON netbox_projection_runs (architecture_id)
    WHERE status IN ('queued','running');
