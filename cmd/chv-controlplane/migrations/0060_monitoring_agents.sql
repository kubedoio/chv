-- Optional guest monitoring agent registry (ADR-026, campaign #602,
-- prompt 03 / gate G3).
--
-- The registry is manager metadata (main control-plane database), never
-- time-series storage. It carries three durable facts the manager owns:
--
--   1. Enrollment claims — hashed single-use bearer secrets scoped to one
--      existing VM, issued by an authorized operator, expiring within the
--      contract's 10-minute ceiling. Only the SHA-256 of the claim token
--      is stored; the plaintext is shown once at issuance and never
--      persisted.
--   2. Enrolled agent identities — the binding between an agent_id and
--      (vm, tenant, install_id, credential_epoch). The manager stores
--      public material only: certificate serial + fingerprint + not_after.
--      It never holds an agent private key (the agent generates its
--      keypair and submits a CSR — security contract "Key provisioning").
--   3. Operational state — last-seen, last boot/sequence, OS identity
--      metadata (privacy allowlist), revocation and cloned-image conflict
--      flags.
--
-- Wire state vocabulary (security contract): unenrolled / enrolling /
-- active / renewal_due (sub-state) / offline / expired / revoked. Stored
-- state is just `active|revoked` plus time fields; the derived vocabulary
-- is computed by the service so thresholds stay code-owned, not
-- database-owned.

CREATE TABLE IF NOT EXISTS monitoring_agent_claims (
    claim_hash text PRIMARY KEY,
    vm_id text NOT NULL REFERENCES vms (vm_id) ON DELETE CASCADE,
    -- Copied from vms.tenant_id at issuance so redemption does not have
    -- to re-join against a VM row that may since have been deleted.
    tenant_id text,
    issued_by text NOT NULL,
    issued_at_ms integer NOT NULL,
    expires_at_ms integer NOT NULL,
    consumed_at_ms integer,
    consumed_install_id text,
    consumed_by_ip text
);

CREATE INDEX IF NOT EXISTS monitoring_agent_claims_vm_idx
    ON monitoring_agent_claims (vm_id);

CREATE TABLE IF NOT EXISTS monitoring_agents (
    agent_id text PRIMARY KEY,
    vm_id text NOT NULL REFERENCES vms (vm_id) ON DELETE CASCADE,
    tenant_id text,
    -- Per-install machine identity presented at claim redemption and
    -- echoed in every batch envelope (cloned-image detection). A mismatch
    -- sets identity_conflict; recovery requires an authorized reset.
    install_id text NOT NULL,
    -- Credential generation. The guest's samples carry
    -- "agent-credential-generation-<epoch>" as their identity_epoch so
    -- counter reset detection fences on credential rotation too.
    credential_epoch integer NOT NULL DEFAULT 1,
    cert_serial text NOT NULL,
    cert_fingerprint text NOT NULL,
    cert_not_after_ms integer NOT NULL,
    -- Previous credential during a rotation's grace window: both the
    -- current and previous cert authenticate until grace elapses, then
    -- only the current one does.
    previous_serial text,
    previous_fingerprint text,
    rotated_at_ms integer,
    -- Operator-forced rotation: set by an authorized BFF action,
    -- cleared when the agent presents a fresh CSR. While set, ingest
    -- responses tell the agent its credential is renewal_due.
    rotation_pending integer NOT NULL DEFAULT 0,
    -- Stored status is minimal; revoked is terminal for this identity.
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active','revoked')),
    identity_conflict integer NOT NULL DEFAULT 0,
    conflict_reason text,
    enrolled_at_ms integer NOT NULL,
    enrolled_by text NOT NULL,
    last_seen_at_ms integer,
    last_boot_id text,
    last_sequence integer,
    -- OS identity metadata (ingestion v1 `os` envelope allowlist).
    os_name text,
    os_version text,
    os_kernel_release text,
    revoked_at_ms integer,
    revoked_by text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS monitoring_agents_vm_idx
    ON monitoring_agents (vm_id);

-- One live identity per VM: claim redemption must fail while an active
-- agent exists (replacement is an explicit revoke-then-re-enroll
-- operation, never a silent takeover).
CREATE UNIQUE INDEX IF NOT EXISTS monitoring_agents_vm_active_uidx
    ON monitoring_agents (vm_id) WHERE status = 'active';
