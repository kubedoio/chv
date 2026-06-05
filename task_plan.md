# Task Plan: gap-cleanup-production

## Goal

Close all 22 CRITICAL findings and the top-priority HIGH findings from the 2026-06-05
comprehensive review — hardening CHV from "production-capable foundation" to
"production-default" across security, data-plane correctness, observability, and test
coverage.

## Branch

`gap-cleanup-production` (forked from `main` at `aa342848`)

---

## What Will Be Implemented

This plan is structured as the 4-sprint sequence from the review report, re-scoped to
what is tractable as individual commits on this branch.  Items are listed in execution
order with their finding IDs, exact file targets, and one-line implementation note.

---

## Sprint 1 — Security Defaults (highest blast radius, ship-blocking)

### S1-1: Fix sqlx feature leak → kills RUSTSEC-2023-0071 (C-4)

**Files**: `crates/chv-webui-bff/Cargo.toml:21`, `crates/chv-controlplane-store/Cargo.toml:21`

**Change**: Add `default-features = false` to both sqlx re-declarations.
The workspace `Cargo.toml` already has `default-features = false` with only the
needed features (`sqlite, migrate, derive, json, uuid, chrono, runtime-tokio-rustls`).
The two crate-level overrides accidentally re-enable `sqlx-mysql` and `sqlx-postgres`,
pulling in `rsa 0.9.10` (Marvin timing-side-channel).

```toml
# before
sqlx = { workspace = true, features = ["runtime-tokio-rustls"] }
# after
sqlx = { workspace = true, default-features = false, features = ["runtime-tokio-rustls"] }
```

**Verification**: `cargo tree -p chv-webui-bff | grep rsa` → no output.

---

### S1-2: Fix default admin/admin — no shipped credentials (C-1)

**Files**:
- `cmd/chv-controlplane/migrations/0008_users.sql`
- `cmd/chv-controlplane/migrations/0033_activate_admin_account.sql`
- `install.sh` (lines 484, 546, 1173)

**Change**:
- Migration 0008: keep table DDL, remove the seeded `INSERT` row (move credential
  seeding entirely into install.sh at runtime).
- Migration 0033: remove the hardcoded activation INSERT; replace with a
  `-- SEEDED AT INSTALL TIME` comment so it doesn't break migration ordering.
- `install.sh`: generate a random 24-char password with `openssl rand -base64 18`,
  bcrypt-hash it via `htpasswd -bnBC 12 "" $PASS | tr -d ':\n' | sed 's/$2y/$2b/'`,
  INSERT it into the DB post-migration, and print the password once to stdout +
  write to `/etc/chv/initial_admin_password` (0600 root).
- Add a `must_change_password BOOLEAN NOT NULL DEFAULT FALSE` column to the users
  table (new forward migration `0034_must_change_password.sql`), set `TRUE` for the
  seeded admin row.

**Verification**: Fresh `install.sh` run produces unique password; no `admin/admin`
hash in any migration file.

---

### S1-3: Fix install.sh TLS wiring — mTLS on by default (C-2, H-6, H-7)

**Files**: `install.sh` (lines 740-757, 765)

**Changes**:
1. Line 765: change `http://127.0.0.1:8443` → `https://127.0.0.1:8443` in generated
   `controlplane.toml`.
2. Lines 740-757: emit `server_cert_path` and `server_key_path` into the generated
   `controlplane.toml` pointing to the certs that install.sh already creates.
3. CA private key: change `chmod 0640` → `chmod 0600` (H-6 — currently group-readable
   to the `chv` group).
4. Systemd unit: inject `CHV_ENCRYPTION_KEY` environment variable (generated once
   during install, stored in `/etc/chv/encryption.key` at 0600) so
   `CredentialEncryption::new()` doesn't silently fall back to plaintext (H-7).

**Verification**: After `install.sh` runs, `grep "http://" /etc/chv/controlplane.toml`
returns nothing; `stat -c %a /etc/chv/ca.key` returns `600`; systemd unit file contains
`CHV_ENCRYPTION_KEY`.

---

### S1-4: Fix vitest CVE (C-3)

**Files**: `ui/package.json`

**Change**: Bump `vitest` from `^2.1.4` to `^3.2.0` (latest stable that resolves the
arbitrary-file-read vuln in `@vitest/mocker`).  Run `npm install` in `ui/` to update
lockfile.  Run `npm run test` to confirm no regressions.

**Verification**: `npm audit --prefix ui` reports 0 critical vulns for vitest.

---

### S1-5: SECURITY.md + supply-chain gates (H-30)

**Files**: `SECURITY.md` (new), `.github/dependabot.yml` (new),
`.github/workflows/security.yml` (new)

**Changes**:
- `SECURITY.md`: disclosure policy (email to maintainers, 90-day coordinated disclosure,
  CVE process), scope, out-of-scope, hall-of-fame template. Standard GitHub advisory
  format.
- `.github/dependabot.yml`: enable for `cargo` (weekly) + `npm`/`ui/` (weekly).
- `.github/workflows/security.yml`: new workflow running `cargo audit --deny warnings`
  and `cargo deny check` on PRs + schedule (daily). Blocked on advisory DB feed.

**Verification**: CI workflow parses without errors; `cargo audit` passes on current
tree after S1-1 lands.

---

## Sprint 2 — Data-Plane Correctness

### S2-1: Quota TOCTOU fix (C-6)

**Files**: `crates/chv-controlplane-store/src/desired_state.rs:444`

**Change**: Replace the read-then-insert two-step with a single atomic
`INSERT ... SELECT` that checks the quota inline:

```sql
INSERT INTO vm_desired_state (vm_id, ...)
SELECT ?, ...
WHERE (SELECT COUNT(*) FROM vm_desired_state WHERE tenant_id = ? AND deleted_at IS NULL) < ?
```

SQLite guarantees the WHERE check and the INSERT are atomic within the same statement.
No need for `BEGIN IMMEDIATE` (which would serialize all writes).

**Verification**: Unit test that fires 20 concurrent `CreateVm` at quota=10 via
`tokio::spawn` — exactly 10 succeed.

---

### S2-2: Agent reconciler — per-VM parallelism (C-8)

**Files**: `crates/chv-agent-core/src/reconcile.rs` (lines 1012, 1096, 1116, 825, 878)

**Change**: Replace the sequential `for vm in vms { ... .await }` loop with
`futures::stream::iter(vms).buffer_unordered(8).collect::<Vec<_>>().await`.
Each VM's work (open_volume, start, stop, delete) becomes an independent future.
Buffer size 8 is configurable via a const `VM_RECONCILE_CONCURRENCY: usize = 8`.

**Dependencies**: Add `futures = "0.3"` to `chv-agent-core/Cargo.toml` if not present.

**Verification**: `cargo test -p chv-agent-core` passes; no deadlock under
`tokio::test` with 20 simulated VMs.

---

### S2-3: Orchestrator N+1 — inline node_id (C-9)

**Files**: `crates/chv-controlplane-service/src/orchestrator.rs` (lines 209-242)

**Change**: Fold the per-operation `SELECT COALESCE(...)` into the claim query using a
JOIN so node_id is resolved in the same SQL round-trip as the claim:

```sql
UPDATE operations SET status = 'claimed', claimed_at = ? ...
RETURNING o.*, COALESCE(vds.target_node_id, vol.node_id, net.node_id) AS node_id
-- (JOIN vm_desired_state, volumes, networks in the same UPDATE...RETURNING)
```

SQLite supports `RETURNING` with JOINed columns.  The N separate SQL calls collapse to
the original 2 (claim + retryable fetch).

**Verification**: `cargo test -p chv-controlplane-service` passes; tracing spans show
only 2 DB calls per tick instead of 2+N.

---

### S2-4: Replace std::sync::Mutex with tokio::sync::RwLock (C-10)

**Files**: `crates/chv-agent-core/src/vm_runtime.rs:19-20`,
`crates/chv-agent-runtime-ch/src/process.rs:71`

**Change**: Replace `Arc<std::sync::Mutex<HashMap<...>>>` with
`Arc<tokio::sync::RwLock<HashMap<...>>>`.  Read paths (the dominant case: reconcile
reads VM state) use `read()` and no longer block the tokio worker.  Write paths
(create/delete) use `write()`.  `tokio::sync::RwLock` does not poison on panic.

**Note**: `DashMap` is an alternative (lock-free per-shard), but `tokio::sync::RwLock`
avoids a new dep and is sufficient for the reconcile read/write ratio.

**Verification**: `cargo clippy -p chv-agent-core -p chv-agent-runtime-ch` clean;
`cargo test` passes.

---

### S2-5: Memory-migration cancel path (C-5)

**Files**: `crates/chv-controlplane-service/src/migration.rs:530-560`,
`crates/chv-agent-runtime-ch/src/process.rs:1329+`

**Change**: Add a best-effort cancel path for the `PreCopy` phase:
1. On network failure/timeout during `send-migration`, call `vm.resume` on source
   (Cloud Hypervisor supports this even while migration is in-flight for precopy).
2. Transition migration state to `Failed` with reason `"migration_cancelled_mid_precopy"`.
3. Agent on source side: on receiving `CancelMigration` while in `PreCopy`, issue
   `POST /api/v1/vm.resume` to local CH and update migration state.
4. Add `MigrationState::CancelRequested` variant + `valid_transition` guard.
5. Update the comment from "cannot rollback cleanly" to document the actual behavior.

**Limitation documented**: FinalCopy phase (paused source) truly cannot be rolled back
without data loss risk — document this in a `// SAFETY:` comment and in the migration
runbook. PreCopy cancel is safe; FinalCopy cancel is not implemented.

**Verification**: Unit test for `PreCopy → CancelRequested → Failed` transition; test
for `FinalCopy → CancelRequested` returns `Err(ChvError::InvalidTransition)`.

---

## Sprint 3 — Observability + Contracts

### S3-1: VM lifecycle RED metrics on agent (C-11)

**Files**: `crates/chv-agent-runtime-ch/src/process.rs`,
`crates/chv-observability/src/lib.rs`

**Change**: Add to `chv-observability`:
```rust
pub static VM_OPS_TOTAL: LazyLock<CounterVec>;     // labels: op, result (ok|err)
pub static VM_OP_DURATION_SECONDS: LazyLock<HistogramVec>; // labels: op
```
Instrument `create_vm`, `start_vm`, `stop_vm`, `delete_vm`, `pause_vm`, `resume_vm`
in `process.rs` with `.observe()` / `.inc()` wrapping each CH API call.

**Verification**: `curl localhost:8080/metrics | grep vm_ops_total` returns labeled
counters after a VM lifecycle event.

---

### S3-2: gRPC server-side latency interceptor (C-13)

**Files**: `crates/chv-controlplane-service/src/main.rs` (or bootstrap),
`crates/chv-agent-core/src/grpc_server.rs` (or equivalent),
`crates/chv-observability/src/lib.rs`

**Change**: Add a `tonic` tower `Layer` that wraps each gRPC method call:
- Records `grpc_server_duration_seconds{service, method, grpc_status}` histogram.
- Records `grpc_server_requests_total{service, method, grpc_status}` counter.
Wire into every `.add_service()` call in each binary's server setup.

**Verification**: `cargo test` passes; `grpc_server_duration_seconds` visible in
`/metrics` after a gRPC call.

---

### S3-3: Request-ID in every BFF error response (H-23, H-24)

**Files**: `crates/chv-webui-bff/src/error.rs:31`,
`crates/chv-webui-bff/src/middleware/` (or main.rs)

**Changes**:
1. Add `request_id: Option<String>` field to `BffError::Internal`.
2. Axum middleware: generate `X-Request-ID` UUID on every request, store in
   `Extension<RequestId>`.
3. All `BffError::Internal` construction sites (242): extract `RequestId` from
   extensions and embed it.
4. Error response JSON body: `{ "error": "Internal server error", "request_id": "..." }`.

**Scope note**: The 242 callsites are macro-generated or near-identical; a
`impl From<ChvError> for BffError` change propagates automatically to most.

**Verification**: `curl -X POST .../vms -d '{bad json}'` response body contains
`"request_id"`.

---

### S3-4: Proto `reserved` discipline + API-evolution ADR (C-16)

**Files**: `proto/**/*.proto` (all 7 files), `docs/specs/adr/019-api-evolution.md` (new)

**Changes**:
- Audit all messages: for any field that was ever removed or renamed (check git log),
  add `reserved N; reserved "field_name";` entries.
- For fields planned for future removal: add `[deprecated = true]` option.
- New ADR-019 covering: never reuse field numbers, `reserved` on deletion,
  `[deprecated = true]` one minor before removal, cursor pagination policy,
  proto enum evolution rules.
- Add a CI lint step: `buf breaking --against '.git#branch=main'` to catch proto
  regressions on every PR.

**Verification**: `buf lint proto/` and `buf breaking` pass in CI.

---

### S3-5: Observability docs — SLIs, SLOs, Prometheus rules (H-29)

**Files**: `docs/OBSERVABILITY.md` (new), `monitoring/rules/` (new dir)

**Changes**:
- `docs/OBSERVABILITY.md`: define SLIs (VM start p99, API error rate, migration
  success rate), SLOs (99th percentile VM start < 30s, API error rate < 0.1%),
  alert thresholds, runbook links.
- `monitoring/rules/chv.yml`: Prometheus alerting rules for the defined SLOs.
- `monitoring/rules/recording.yml`: pre-computed recording rules for expensive
  multi-label queries.

**Verification**: `promtool check rules monitoring/rules/*.yml` passes.

---

## Sprint 4 — Tests + Cleanup

### S4-1: chv-agent-core reconcile + migration unit tests (C-17, C-18)

**Files**: `crates/chv-agent-core/src/reconcile.rs` (add `#[cfg(test)] mod tests`),
`crates/chv-agent-core/src/migration.rs` (add tests)

**Target**: ~30 unit tests covering:
- Node FSM transitions (all valid + all invalid transition guards)
- Reconcile tick: VM not-present → creates; VM present matches desired → no-op;
  VM present differs → updates; operation-journal idempotency
- Migration state machine: `Initiated → PreCopy`, `PreCopy → CancelRequested → Failed`,
  `PreCopy → FinalCopy → Completed`
- Error paths: CH API timeout, stord volume not found

**Pattern**: Use `MockCloudHypervisorAdapter` (if it exists) or add one via
`#[cfg_attr(test, mockall::automock)]`.

**Verification**: `cargo test -p chv-agent-core` — target ≥25 new test functions.

---

### S4-2: JWT negative-case tests (C-20)

**Files**: `crates/chv-webui-bff/src/auth.rs` (add `#[cfg(test)] mod tests`)

**Target**: Test suite covering:
- Expired token (`exp` in the past) → 401
- Token signed with wrong secret → 401
- Token missing `sub` claim → 401
- Token with wrong `aud` → 401 (if aud validation added as part of H-1)
- Tampered payload (valid header+sig, modified body) → 401
- API token with `exp = u64::MAX/2` — document this is intentional or fix it (H-2)

**Verification**: `cargo test -p chv-webui-bff` — 6+ new negative-case test functions.

---

### S4-3: Credential crypto tamper tests (C-21)

**Files**: `crates/chv-controlplane-store/src/credential_crypto.rs`

**Target**: Add to existing test module:
- Tampered ciphertext (flip one bit) → `Err(ChvError::...)` not silent plaintext
- Wrong key decryption → `Err`, not garbled output
- AAD tamper → `Err` (AES-GCM authentication tag fails)
- Empty ciphertext input → `Err`, not panic

**Verification**: `cargo test -p chv-controlplane-store` — 4+ new test functions.

---

### S4-4: Svelte component decomposition (C-22)

**Files**: The 6 worst-offender Svelte files:
- `ui/src/routes/backup-jobs/+page.svelte` (577 LOC)
- `ui/src/routes/templates/+page.svelte` (542 LOC)
- `ui/src/lib/components/MobileNav.svelte` (480 LOC)
- `ui/src/lib/components/QuickActions.svelte` (477 LOC)
- `ui/src/routes/images/+page.svelte` (470 LOC)
- `ui/src/lib/components/CloudInitEditor.svelte` (441 LOC)

**Change**: Extract logical sub-sections into `<ComponentName>Section.svelte` or
`<ComponentName>Panel.svelte` siblings, keeping each file under 300 LOC.  Props +
events stay typed.  Zero behavior changes — layout extraction only.

**Verification**: `npm run check --prefix ui` (svelte-check) passes, 0 errors,
0 warnings. Visual smoke check of each affected route.

---

### S4-5: Stord backend skeleton tests (C-19)

**Files**: `crates/chv-stord-backends/src/{ceph,iscsi,local,lvm}.rs`

**Target**: Add a trait-level mock and at minimum one "construct + call probe" test per
backend to establish the testing pattern:
- `LocalBackend::probe()` — calls the actual path (temp dir)
- `LvmBackend` — mock the `lvm` binary invocation
- `CephBackend` + `IscsiBackend` — unit test against mock socket / env, document
  "full integration test requires real cluster"

**Verification**: `cargo test -p chv-stord-backends` — 4+ new test functions, all pass.

---

## What Is NOT in Scope for This Branch

The following HIGH findings require architectural changes that deserve their own branches
and review cycles:

| Finding | Reason deferred |
|---|---|
| H-14 Pagination AIP-158 cursor | Breaking API change — needs migration guide + SDK versioning |
| H-15 google.rpc.Status rich errors | Multi-crate proto change, language SDK impact |
| H-16 Client-side Idempotency-Key | HTTP contract change |
| H-17 Stringly-typed proto enums | Proto breaking change, needs coordinated rollout |
| H-18 God-crate split | Multi-sprint refactor, own ADR needed |
| H-19 Layering inversion | Requires H-18 first |
| C-7 STONITH / node fencing | Requires distributed systems design work |
| C-12 OpenTelemetry OTLP | Infra dependency (OTLP collector), own branch |
| H-20/H-21/H-22 Newtype propagation | Large cross-crate refactor, own branch |

---

## Phases

- [x] Phase 1 (S1): Security defaults — sqlx fix, admin/admin, install.sh TLS, vitest CVE, SECURITY.md ✅ landed in 5 commits (df6f2d42, 0ba3e649, 69bdb40b, b4238ec4, fb260481)
- [ ] Phase 2 (S2): Data-plane correctness — quota, reconciler parallel, orchestrator N+1, mutex, migration cancel
- [ ] Phase 3 (S3): Observability + contracts — VM metrics, gRPC interceptor, request-ID, proto reserved, SLO docs
- [ ] Phase 4 (S4): Tests + cleanup — reconcile tests, JWT tests, crypto tests, Svelte decomp, stord tests

## Key Questions

1. Migration 0033 currently activates the admin user — if we remove the INSERT, does
   a new migration 0034 (adding `must_change_password`) need to also seed the admin row,
   or does install.sh handle 100% of initial-user seeding? **Decision: install.sh seeds.**
2. For S2-2 (reconciler parallelism) — is the current per-VM operation lock
   (`cache.lock()`) per-VM or global? If global, buffer_unordered won't help until
   the lock is narrowed. **Needs code read before implementing.**
3. For S1-3 (vitest bump) — does `@vitest/mocker` ship with vitest 3.x or require
   a separate install? **Check npm registry before bumping.**

## Decisions Made

- **Scope**: 22 CRITICALs + targeted HIGHs only. Architectural rewrites deferred.
- **Branch**: All work on `gap-cleanup-production`, single PR to main.
- **Order**: Security first (S1), then data-plane (S2), then observability (S3), then tests (S4).
- **Mutex choice**: `tokio::sync::RwLock` over `DashMap` — avoids new dep.
- **Migration cancel scope**: PreCopy only. FinalCopy documented as non-rollbackable.
- **Stord backends**: Skeleton tests only — full integration tests require real cluster.

## Errors Encountered

(none yet)

## Status

**Phase 1 (S1) complete.** All 5 sub-items landed as bisectable commits on
`gap-cleanup-production`. Working tree clean, `cargo check --workspace` passes,
svelte-check 0/0, all 129 vitest tests pass.

Closes findings: **C-1, C-2, C-3, C-4, H-6, H-7, H-30 (partial)**.

Ready to begin Phase 2 (S2: data-plane correctness) when authorized.
