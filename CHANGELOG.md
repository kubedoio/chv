# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Architecture Designer (Phases 0–7)**: a first-class surface for declaring desired CHV topologies (servers, networks, datastores, instances, backups, RBAC) as YAML or via a Svelte Flow canvas, with fleet validation, plan generation, idempotent apply, and drift detection. Six SQLite tables, 18 BFF endpoints under `/v1/architectures/*`, two new crates (`chv-architecture-validate`, `chv-architecture-reconcile`), full UI under `/architectures` with 6 detail tabs (overview, canvas, yaml, plan, runs, drift). Production environments require `Admin` role; non-production stays operator-applyable. 798 cargo tests, 23/23 architecture E2E specs, permission matrix asserting 54 (route × role) cases plus an exhaustiveness meta-test, release-only perf gate (269µs vs 2s budget on 800-NIC-edge topology), TTL boundary tests pinning `>` semantics at T0+15m. ADRs 001–006-Designer all `Accepted`. Tracking issues filed for periodic retention pruner and pre-existing E2E redirect flakes. Release notes: [`docs/release/architecture-designer-release-notes.md`](docs/release/architecture-designer-release-notes.md). GO/NO-GO disposition: [`docs/specs/architecture-designer/go-no-go-2026-06-16.md`](docs/specs/architecture-designer/go-no-go-2026-06-16.md). Phase PRs: [#112](https://github.com/kubedoio/chv/pull/112), [#113](https://github.com/kubedoio/chv/pull/113), [#114](https://github.com/kubedoio/chv/pull/114), [#124](https://github.com/kubedoio/chv/pull/124), [#125](https://github.com/kubedoio/chv/pull/125), [#126](https://github.com/kubedoio/chv/pull/126), [#127](https://github.com/kubedoio/chv/pull/127), [#128](https://github.com/kubedoio/chv/pull/128), [#130](https://github.com/kubedoio/chv/pull/130).

### Changed
- **Storage (stord, behavior change — issue #395)**: the migration *client* TLS config is now gated on `migration.enabled` exactly like the receiver half: setting any client field (`client_cert_path`, `client_key_path`, `ca_cert_path`, `dest_server_name`) while `migration.enabled = false` is a **startup error** instead of a silently ignored half-configuration. Such configs were almost certainly mistakes (an operator who believes migration is off must not discover a configured identity only when it silently does nothing); they now fail fast at startup, which is the point. `enabled = false` with no client fields still starts cleanly as a migration-unavailable stord.
- **Fabric provider pin: `o3kio/fabric` v0.1.2 → v0.1.5** (ADR-021). Hardening only; no provider API changes — `chv-nwd-core` compiles against the same `fabric-plan`/`fabric-linux` surface and the in-CI conformance suite (now 25 cases) passes unchanged. v0.1.3: endpoint values and journaled plans re-validated on read (fail-closed on parseable-but-invalid journals). v0.1.4: `remove_network` converges when the fabric namespace survives but the WireGuard link does not (previously wedged with an orphaned ownership entry); public-key shape (43 base64 characters + one trailing `'='`) validated on both the parsed and the deserialized path; the private-key publish never replaces an existing destination. Upstream details: `o3kio/fabric` CHANGELOG for [0.1.3]–[0.1.5].

### Fixed
- **BFF (idempotency — issue #406)**: a retried `vm delete` of a retained VM returned `HTTP 500 INTERNAL_ERROR` instead of a clean idempotent success. The BFF derives per-resource idempotency keys (`delete-vm-<vm_id>`, `resize-vm-<vm_id>-<cpu>-<mem>`) and recorded them as plain UNIQUE INSERTs into the shared `operations` table; with the M2.5 authority-side retention keeping the VM rows after a delete, the retry re-entered the handler with the same key and the UNIQUE violation surfaced as an unhandled 500 (found live by the M4.7 fault-matrix run 1, F7). A new shared helper (`handlers/operations`) now checks for a recorded operation *before* any mutation — inside the handler's existing `BEGIN IMMEDIATE` transaction, mirroring the controlplane store's `ON CONFLICT (idempotency_key) DO NOTHING` + re-select — and a collision replays the recorded original outcome (200 with the recorded `task_id` and its current `recorded_status`) without re-executing the mutation. The delete key stays per-VM (one delete is the intent); the resize key is args-hashed so a genuinely *different* resize is a fresh operation (a per-VM key would silently replay the first outcome and swallow the new intent) while a same-args retry still replays. Covers `vm delete` and `vm resize` (the two retry-reachable surfaces with client-supplied VM ids); `vm start/stop/reboot` go through the controlplane's already-idempotent path, and `create`/`clone`/`import` embed server-generated VM ids so their keys cannot collide. First-operation semantics and retention behavior are unchanged; a genuinely unrecoverable insert failure fails closed with a 409 naming the idempotent-retry condition instead of a 500 (classified by a re-select heuristic, not sqlx error inspection — unreachable defense-in-depth under `BEGIN IMMEDIATE` + the in-tx pre-check). (#406)
- **Storage (stord, data integrity — issue #392)**: migration finalize now verifies the destination's *data*, not just the protocol. Previously the receiver replied `FinalizeAck{verified:true}` unconditionally and the sender sent an empty `volume_checksum`, so the only integrity mechanism was the per-chunk CRC32 — a corrupted, truncated, or externally modified destination volume still finalized as `Completed`. At finalize the sender now streams a full-volume SHA-256 digest over the source (via the same `read_block` path as bulk copy, never holding the volume in memory) and carries it in `FinalizeComplete.volume_checksum` in a versioned, self-describing format (`"sha256:"` + 32 raw bytes, documented in the proto); the receiver re-computes the same digest over the destination after its final flush and sets `verified` accordingly. Fail-closed on every leg: digest mismatch ⇒ `verified=false` with a precise, content-free error ("destination digest mismatch: expected sha256:…, got sha256:…" — never volume data); unknown/unsupported digest format ⇒ `verified=false` naming the format (forward-compatible detection, no silent misinterpretation); unreadable destination ⇒ `verified=false`; and the sender treats `verified=false` as a migration failure — task → `Failed` with `Status::data_loss` (same integrity class as a chunk CRC mismatch), so `Completed` genuinely means "destination verified". Per-chunk CRC32, ack semantics, and the protocol are otherwise unchanged. Cost: one additional sequential full-volume read on each side at finalize. The new e2e leg corrupts the destination file out-of-band mid-migration (past every CRC check) and asserts the migration fails with the digest-mismatch error and the task ends `Failed`. (#392)
- **Storage (stord)**: the storage-migration acknowledgment protocol now works end-to-end — previously *no* migration could ever complete (issue #391, plus two further latent deadlocks found by the first end-to-end test; all pinned by the new `tests/migration_e2e.rs`). Four data-path fixes: (1) **round-ack deadlock** — the receiver only logged `RoundComplete` and sent nothing, so the sender blocked forever waiting for a round acknowledgment after the first dirty sync round; the receiver now always answers `RoundComplete` with an `Ack` carrying its highest processed sequence number, which both acknowledges the round and flushes its ack window. (2) **ack-window flush mismatch** — the receiver acknowledged only every 64 chunks with no final flush, so any phase whose chunk count was not a multiple of 64 (at 4 MiB blocks: any volume under 256 MiB) timed out after 30 s; the receiver now flushes outstanding acks at `FinalSync` and before `FinalizeAck`, the sender drains after `RoundComplete`/`FinalSync` instead of at a point the receiver can never observe (end of bulk copy), and the FinalizeAck wait tolerates in-flight boundary acks. (3) **Init handshake deadlock** — the sender awaited the stream response before sending `InitMigration`, but the tonic server handler blocks reading the first message before responding, so the stream could never start; the sender now queues `InitMigration` before opening the stream. (4) **gRPC message-size limit** — a 4 MiB `BlockChunk` plus protobuf overhead exceeds tonic's default 4 MiB decode limit, so the receiver rejected every real chunk with `OutOfRange`; both serving paths (mTLS TCP listener and Unix socket) now accept migration messages up to 8 MiB. Fail-closed semantics are unchanged: CRC mismatches still fail the migration, out-of-bounds chunks are still rejected, sparse-zero handling is untouched, and no mTLS path was weakened. The no-TLS `FailedPrecondition` also now cites the real config keys (`migration.client_cert_path`/`client_key_path`/`ca_cert_path`/`dest_server_name`) instead of the nonexistent `migration.tls.*` section. The new end-to-end test runs the real `MigrationSender` against the real receiver served by `serve_migration_tls` on a loopback mTLS listener with `LocalFileBackend` on both sides, covering: a small (2-chunk, sub-ack-interval) volume completing with byte-exact destination; dirty sync rounds transferring concurrently written blocks; and the `PausedFinalSync` pause handshake driven through a `MigrationTaskTable` entry. (#391)
- **Control plane (dev-mode operator contract)**: `CHV_ALLOW_INSECURE=1` on a production build fails with `InsecureModeLockedOut` telling the operator to "rebuild with: cargo build --features dev" — but `chv-controlplane` had no `dev` feature to enable (it existed only on the `chv-controlplane-service` dependency and was never forwarded), making the documented dev-build path impossible. The bin crate now forwards the feature (`dev = ["chv-controlplane-service/dev"]`), CI gained a compile guard (`cargo check -p chv-controlplane -p chv-agent --features dev`), and the dev path was verified to pass both security validators and proceed into bootstrap. (prompt-03 post-merge review)
- **Storage (stord)**: the migration receiver now listens on TCP with mandatory mTLS (issue #390). Previously only the *sender* half of migration TLS existed: a stord configured as a migration destination still never opened a TCP listener, so cross-node migrations could not connect — the destination served the migration service only on its local Unix socket. New all-or-nothing `[migration]` receiver fields (`listen_addr`, `server_cert_path`, `server_key_path`, `client_ca_path`) with fail-closed startup validation (partial config, unreadable files, mismatched keypair, invalid/empty client CA bundle, or a bad listen address is a startup error; no fields = source-only stord with no TCP listener). `migration.enabled` is the master switch for the receiver too: receiver fields set with `enabled = false` are a startup error rather than a silently ignored (or silently opened) listener. When configured, `StorageServer` additionally serves `StorageMigrationService` on a TLS TCP listener via tonic `ServerTlsConfig` with `client_ca_root` and *no* `client_auth_optional` — rustls rejects peers without a valid client certificate during the TLS handshake (loopback-tested: no-cert and untrusted-CA clients are rejected; valid-identity clients complete a gRPC round trip). The Unix-socket serving surface (both services, 0600 socket, group ownership) is unchanged and no mTLS path was weakened. (#390)
- **Storage (stord)**: wired the storage-migration mTLS path end-to-end (previously hard-coded `None`). Added a `[migration]` config section (`enabled` plus `client_cert_path`, `client_key_path`, `ca_cert_path`, `dest_server_name`) with fail-closed startup validation: missing fields, unreadable files, a malformed/empty CA bundle, or a mismatched keypair is a *startup error*; there is no plaintext fallback and no skip-verify option; PEM/key material is never logged. `chv-stord` now installs the rustls ring crypto provider and passes the validated `MigrationTlsConfig` into `StorageServer`. Example config documents file ownership, permission, and rotation expectations. (#232)
- **Control plane**: replaced the `CHV_ALLOW_INSECURE=1` startup `panic!` in `PeerIdentityInterceptor::new` with typed, non-panicking startup validation. `validate_security_mode` runs during bootstrap *before* any listener/interceptor is constructed; a build without the `dev` feature that requests insecure mode now exits with a clean, operator-greppable `InsecureModeLockedOut` error instead of panicking. Defense-in-depth: the interceptor constructor returns the same typed error. (#233)
- **Architecture Designer dashboard logged users out on mount.** `getArchitectureDrift` accepted a placeholder `_fetch` parameter where a token argument should have been; the dashboard fan-out at `/architectures` called it without a token, the BFF returned 401, and the global `bffFetch` 401 handler redirected the user to `/login`. The `.catch(() => null)` at the call site suppressed the rethrown `BFFError` but not the redirect. Function signature now takes `token?: string` and forwards it into `bffFetch`; both call sites (dashboard fan-out + drift store) pass `getStoredToken() ?? undefined`. Three Vitest regression tests pin the token-forward, signal-forward, and explicit-undefined contracts.

## [0.2.0] - 2026-05-29

### Added
- Serial console backend: PTY lifecycle, JWT token gating, WebSocket proxy via BFF (`/ws/vms/{id}`)
- Hypervisor settings: DB schema, BFF CRUD, and orchestrator default-merge logic
- GitHub Actions CI pipeline: Rust check/clippy/test + UI build on push/PR
- Design system revision: aligned `DESIGN.md` with actual CSS implementation (warm earthy palette, IBM Plex typography)
- UI pages: Snapshots, Metrics (Chart.js), Export/Import, User management, API tokens
- Network firewall rule viewer and storage pool list in UI
- VM list enhancements: status filters, bulk actions, and improved state indicators
- Network mutations end-to-end (B1): `StartNetwork`, `StopNetwork`, `RestartNetwork` lifecycle RPCs across proto → BFF → control plane → agent → NWD
- Hypervisor settings UI page (`/settings/hypervisor`): global defaults editing, profile management, apply-profile
- CreateVMModal Advanced section: per-VM hypervisor overrides (cpu_nested, cpu_kvm_hyperv, memory_shared, memory_hugepages, iommu, watchdog, serial_mode, console_mode)
- Console token LRU cache: bounded replay prevention (2048 entries) replacing unbounded `HashMap`
- Orchestrator merge tests: 5 unit tests covering VM override precedence, global fallback, defaults on failure, and post-merge validation
- Agent daemon parity wiring: `get_volume_health` and `get_network_health` wired into reconcile loop; TODOs added for remaining methods pending desired-state schema extensions
- UI component reorganization: 10 feature folders (`vms/`, `nodes/`, `networks/`, `storage/`, `settings/`, `tasks/`, `events/`, `shell/`, `primitives/`, `shared/`) with barrel exports
- Command palette (`Ctrl+K`): fuzzy-search navigation modal with 16 commands grouped by category
- DataTable modularization: extracted `Selection`, `Sorting`, `Visibility` into `shared/datatable/` sub-modules
- Dashboard refactor: extracted `dashboard.ts` helpers and `dashboard.svelte.ts` store; reduced `+page.svelte` from 635 → 292 lines
- Quota enforcement (B3): atomic quota checks at VM-create time with structured `QUOTA_EXCEEDED` errors
- Backup backend (B2): `backup_jobs`, `backup_schedules`, `backup_restores` tables + `BackupRepository` + BFF REST handlers
- RBAC middleware (B6): role-based access control (`Viewer`/`Operator`/`Admin`) on all BFF routes
- Operation ID propagation (A6): `x-operation-id` gRPC metadata across control plane → agent → stord/nwd with tracing spans
- LVM device policy (A7): `io_scheduler` and `read_only` wired in `set_device_policy`; `cache_mode` warned as creation-time only
- Pre-migration SQLite backup hook (I5): automatic DB backup before migrations with 10-backup rotation
- Automated version bump (I6): `scripts/bump-version.sh` + `make bump-version` syncing `VERSION`, `Cargo.toml`, `package.json`, docs
- Nginx WebSocket proxy (I3): `/ws/vms/` location with upgrade headers and timeout config
- Dark mode: full implementation with `UserMenu` toggle, completed `[data-theme="dark"]` tokens, fixed `Button`, `Card`, `Modal`, `Input`, `Select`, `SearchModal` for dark backgrounds
- Client-side API cache (`api-cache.svelte.ts`): vanilla Svelte 5 runes cache with TTL (30s lists, 60s details), stale-while-revalidate, and mutation invalidation; integrated into Dashboard, VMs, Nodes, Networks pages
- Playwright E2E expansion: 3 new test files (`navigation.spec.ts`, `vms.spec.ts`, `settings.spec.ts`) covering sidebar nav, command palette, logout, VM list, create modal, hypervisor settings
- Volume snapshot/clone schema (migrations `0025`): `snapshot_op`, `snapshot_name`, `clone_source_volume_id` in `volume_desired_state`; `parent_volume_id` in `volumes`; wired in reconcile loop and agent server
- Network services schema (migrations `0026`): `firewall_rules_json`, `nat_rules_json`, `dhcp_scope_json`, `dns_enabled`, `dns_scope_json` in `network_desired_state`; wired `set_firewall_policy`, `set_nat_policy`, `ensure_dhcp_scope`, `ensure_dns_scope` in reconcile loop
- Backup worker: scheduled VM backups with S3/NFS shipping, retention enforcement (count + days), retry logic
- S3 credential encryption at rest: AES-256-GCM with key from `CHV_ENCRYPTION_KEY` or `CHV_JWT_SECRET`
- Atomic schedule claim: optimistic locking on `last_run_at` prevents duplicate scheduled jobs
- Disaster recovery runbooks: 5 operational runbooks covering VM snapshot restore, volume snapshot restore, backup artifact restore, control plane DR, and full site recovery

### Changed
- `WEBUI_CHANGES.md` deprecated in favor of CHANGELOG; see [docs/WEBUI_CHANGES.md](./docs/WEBUI_CHANGES.md) for historical reference only
- Systemd service files: all services now use `KillMode=mixed` and `TimeoutStopSec=5` for clean shutdown
- UI design token alignment: `app.css`, `tailwind.config.cjs`, and 8 components aligned to earthy palette (`#8f5a2a` primary, `#3f6b45` success, `#9a6a1f` warning, `#9b4338` danger)
- BFF hypervisor settings router: RESTful GET/PATCH/POST routing with backward-compatible POST fallbacks
- Agent `cpu_kvm_hyperv` conflation removed: field now independent of `cpu_nested`
- `app.css` Tailwind migration: removed global utility duplicates (box-sizing, headings, sr-only, etc.); reduced from 387 → 306 lines
- UI design tokens fully aligned: earthy palette applied to `app.css`, `tailwind.config.cjs`, and all 8 drifted components
- `tailwind.config.cjs`: mapped custom colors to CSS custom properties with dark-mode support
- **License:** Changed from MIT to Apache-2.0 across all crates and the UI package

### Fixed
- Agent console port collision on restart (systemd `KillMode=mixed`)
- Database ownership on fresh deploy (`chown chv:chv` in install scripts)
- Design token drift in `Button.svelte` and `VMMetricsWidget`
- Hypervisor settings HTTP methods in BFF router (`.post()` → `.get()` for reads)
- Serial console design doc filename reference (`console.rs` → `console_server.rs`)
- `tokio-tungstenite` unnecessary dependency note in serial console implementation plan
- Inter-ADR cross-reference gaps (partition policy ↔ state machine, drain semantics, supervision during upgrades)
- All ADRs missing dates
- Dead code removal: deleted unused `hypervisor_settings_validator.rs` from control-plane-service
- Post-merge validation: orchestrator now validates `iommu=true` requires `memory_shared=true` before dispatch
- Serial console PTY resize: verified end-to-end wired (frontend `VmConsole.svelte` → WebSocket JSON → `ioctl(TIOCSWINSZ)`)
- Race condition in scheduled job creation eliminated via optimistic locking (`try_claim_schedule_run`)
- Count-based retention pruning now cleans up remote artifacts before deleting DB rows
- BFF duplicate JSON keys removed from backup response builders

## [0.1.0] - 2026-05-10

### Added
- SemVer versioning policy (`docs/release/versioning-policy.md`)
- `VERSION` file standardized to SemVer (`0.1.0`)
- `scripts/bump-version.sh` updated for SemVer (MAJOR.MINOR.PATCH)
- Rich CLI version output in `chvctl` with git SHA, build date, and release channel
- Build metadata injection via `cmd/chvctl/build.rs`
- `scripts/version.sh` for runtime version querying
- `scripts/smoke-version.sh` for automated version validation
- CI version validation: `VERSION` format check, `Cargo.toml` sync check, `chvctl --version` smoke test

### Changed
- Migrated from four-segment version scheme (`0.0.0.4`) to Semantic Versioning (`0.1.0`)

## [0.0.0.2] - 2026-04-14

### Added
- Rust control plane Phase 1 foundation with inbound gRPC and HTTP admin APIs
- `chv-controlplane` binary with optional mTLS for gRPC and axum-based admin server
- `ControlPlaneService`, `LifecycleService`, `ReconcileService`, `EnrollmentService`, and `TelemetryService` implementations
- SQLite-backed repositories: nodes, desired state, observed state, bootstrap tokens, network exposures
- Structured error mapping to tonic::Status with sanitized user-facing messages
- Operation journal for VM lifecycle with idempotency via resource fingerprinting
- Desired-state fragment parsers with strict validation and `deny_unknown_fields`
- Certificate enrollment with optional CA-backed issuer and bootstrap token validation
- HTTP admin endpoints: health, ready, nodes list, and Prometheus metrics
- Expanded integration tests for store, service, and API layers

### Removed
- Legacy Go control plane (`legacy/go-controlplane`) and stale references

## [0.0.0.1] - 2026-04-10

### Changed
- Simplified docker-compose configurations by removing agent service (runs on bare-metal hosts)
- Changed controller port mapping from 8080:8080 to 8088:8080 to avoid conflicts
- Removed agent dependency from controller service
