# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Architecture Designer (Phases 0–7)**: a first-class surface for declaring desired CHV topologies (servers, networks, datastores, instances, backups, RBAC) as YAML or via a Svelte Flow canvas, with fleet validation, plan generation, idempotent apply, and drift detection. Six SQLite tables, 18 BFF endpoints under `/v1/architectures/*`, two new crates (`chv-architecture-validate`, `chv-architecture-reconcile`), full UI under `/architectures` with 6 detail tabs (overview, canvas, yaml, plan, runs, drift). Production environments require `Admin` role; non-production stays operator-applyable. 798 cargo tests, 23/23 architecture E2E specs, permission matrix asserting 54 (route × role) cases plus an exhaustiveness meta-test, release-only perf gate (269µs vs 2s budget on 800-NIC-edge topology), TTL boundary tests pinning `>` semantics at T0+15m. ADRs 001–006-Designer all `Accepted`. Tracking issues filed for periodic retention pruner and pre-existing E2E redirect flakes. Release notes: [`docs/release/architecture-designer-release-notes.md`](docs/release/architecture-designer-release-notes.md). GO/NO-GO disposition: [`docs/specs/architecture-designer/go-no-go-2026-06-16.md`](docs/specs/architecture-designer/go-no-go-2026-06-16.md). Phase PRs: [#112](https://github.com/kubedoio/chv/pull/112), [#113](https://github.com/kubedoio/chv/pull/113), [#114](https://github.com/kubedoio/chv/pull/114), [#124](https://github.com/kubedoio/chv/pull/124), [#125](https://github.com/kubedoio/chv/pull/125), [#126](https://github.com/kubedoio/chv/pull/126), [#127](https://github.com/kubedoio/chv/pull/127), [#128](https://github.com/kubedoio/chv/pull/128), [#130](https://github.com/kubedoio/chv/pull/130).

### Fixed
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
