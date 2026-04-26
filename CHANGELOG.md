# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Serial console backend: PTY lifecycle, JWT token gating, WebSocket proxy via BFF (`/ws/vms/{id}`)
- Hypervisor settings: DB schema, BFF CRUD, and orchestrator default-merge logic
- GitHub Actions CI pipeline: Rust check/clippy/test + UI build on push/PR
- Design system revision: aligned `DESIGN.md` with actual CSS implementation (warm earthy palette, IBM Plex typography)
- UI pages: Snapshots, Metrics (Chart.js), Export/Import, User management, API tokens
- Network firewall rule viewer and storage pool list in UI
- VM list enhancements: status filters, bulk actions, and improved state indicators

### Changed
- `WEBUI_CHANGES.md` deprecated in favor of CHANGELOG; see [docs/WEBUI_CHANGES.md](./docs/WEBUI_CHANGES.md) for historical reference only

### Fixed
- Agent console port collision on restart (systemd `KillMode=mixed`)
- Database ownership on fresh deploy (`chown chv:chv` in install scripts)
- Design token drift in `Button.svelte` and `VMMetricsWidget`
- Hypervisor settings HTTP methods in BFF router (`.post()` → `.get()` for reads)
- Serial console design doc filename reference (`console.rs` → `console_server.rs`)
- `tokio-tungstenite` unnecessary dependency note in serial console implementation plan
- Inter-ADR cross-reference gaps (partition policy ↔ state machine, drain semantics, supervision during upgrades)
- All ADRs missing dates

## [Unreleased] — Sprint 11 & 12 Implementation

### Added
- **Network mutations end-to-end** (B1): `StartNetwork`, `StopNetwork`, `RestartNetwork` lifecycle RPCs across proto → BFF → control plane → agent → NWD
- **Hypervisor settings UI page** (`/settings/hypervisor`): global defaults editing, profile management, apply-profile
- **CreateVMModal Advanced section**: per-VM hypervisor overrides (cpu_nested, cpu_kvm_hyperv, memory_shared, memory_hugepages, iommu, watchdog, serial_mode, console_mode)
- **Console token LRU cache**: bounded replay prevention (2048 entries) replacing unbounded `HashMap`
- **Orchestrator merge tests**: 5 unit tests covering VM override precedence, global fallback, defaults on failure, and post-merge validation
- **Agent daemon parity wiring**: `get_volume_health` and `get_network_health` wired into reconcile loop; TODOs added for remaining methods pending desired-state schema extensions

### Changed
- **Systemd service files**: all services now use `KillMode=mixed` and `TimeoutStopSec=5` for clean shutdown
- **UI design token alignment**: `app.css`, `tailwind.config.cjs`, and 8 components aligned to earthy palette (`#8f5a2a` primary, `#3f6b45` success, `#9a6a1f` warning, `#9b4338` danger)
- **BFF hypervisor settings router**: RESTful GET/PATCH/POST routing with backward-compatible POST fallbacks
- **Agent `cpu_kvm_hyperv` conflation removed**: field now independent of `cpu_nested`

### Fixed
- **Dead code removal**: deleted unused `hypervisor_settings_validator.rs` from control-plane-service
- **Post-merge validation**: orchestrator now validates `iommu=true` requires `memory_shared=true` before dispatch
- **Serial console PTY resize**: verified end-to-end wired (frontend `VmConsole.svelte` → WebSocket JSON → `ioctl(TIOCSWINSZ)`)

## [Unreleased] — Sprint 13–15 Implementation

### Added
- **UI component reorganization**: 10 feature folders (`vms/`, `nodes/`, `networks/`, `storage/`, `settings/`, `tasks/`, `events/`, `shell/`, `primitives/`, `shared/`) with barrel exports
- **Command palette** (`Ctrl+K`): fuzzy-search navigation modal with 16 commands grouped by category
- **DataTable modularization**: extracted `Selection`, `Sorting`, `Visibility` into `shared/datatable/` sub-modules
- **Dashboard refactor**: extracted `dashboard.ts` helpers and `dashboard.svelte.ts` store; reduced `+page.svelte` from 635 → 292 lines
- **Quota enforcement** (B3): atomic quota checks at VM-create time with structured `QUOTA_EXCEEDED` errors
- **Backup backend** (B2): `backup_jobs`, `backup_schedules`, `backup_restores` tables + `BackupRepository` + BFF REST handlers
- **RBAC middleware** (B6): role-based access control (`Viewer`/`Operator`/`Admin`) on all BFF routes
- **Operation ID propagation** (A6): `x-operation-id` gRPC metadata across control plane → agent → stord/nwd with tracing spans
- **LVM device policy** (A7): `io_scheduler` and `read_only` wired in `set_device_policy`; `cache_mode` warned as creation-time only
- **Pre-migration SQLite backup hook** (I5): automatic DB backup before migrations with 10-backup rotation
- **Automated version bump** (I6): `scripts/bump-version.sh` + `make bump-version` syncing `VERSION`, `Cargo.toml`, `package.json`, docs
- **Nginx WebSocket proxy** (I3): `/ws/vms/` location with upgrade headers and timeout config

### Changed
- **`app.css` Tailwind migration**: removed global utility duplicates (box-sizing, headings, sr-only, etc.); reduced from 387 → 306 lines
- **UI design tokens fully aligned**: earthy palette applied to `app.css`, `tailwind.config.cjs`, and all 8 drifted components
- **BFF hypervisor settings router**: RESTful GET/PATCH/POST routing

### Fixed
- **Console token LRU cache**: bounded 2048-entry replay prevention
- **Agent `cpu_kvm_hyperv` conflation**: field now independent of `cpu_nested`
- **Post-merge validation**: orchestrator validates `iommu=true` requires `memory_shared=true`
- **Systemd services**: all services use `KillMode=mixed` + `TimeoutStopSec=5`
- **Network mutations** (B1): full lifecycle RPC chain across proto → BFF → control plane → agent → NWD

## [Unreleased] — P3 & Schema-Dependent Gaps

### Added
- **Dark mode**: full implementation with `UserMenu` toggle, completed `[data-theme="dark"]` tokens, fixed `Button`, `Card`, `Modal`, `Input`, `Select`, `SearchModal` for dark backgrounds
- **Client-side API cache** (`api-cache.svelte.ts`): vanilla Svelte 5 runes cache with TTL (30s lists, 60s details), stale-while-revalidate, and mutation invalidation; integrated into Dashboard, VMs, Nodes, Networks pages
- **Playwright E2E expansion**: 3 new test files (`navigation.spec.ts`, `vms.spec.ts`, `settings.spec.ts`) covering sidebar nav, command palette, logout, VM list, create modal, hypervisor settings
- **Volume snapshot/clone schema** (migrations `0025`): `snapshot_op`, `snapshot_name`, `clone_source_volume_id` in `volume_desired_state`; `parent_volume_id` in `volumes`; wired in reconcile loop and agent server
- **Network services schema** (migrations `0026`): `firewall_rules_json`, `nat_rules_json`, `dhcp_scope_json`, `dns_enabled`, `dns_scope_json` in `network_desired_state`; wired `set_firewall_policy`, `set_nat_policy`, `ensure_dhcp_scope`, `ensure_dns_scope` in reconcile loop

### Changed
- **`tailwind.config.cjs`**: mapped custom colors to CSS custom properties with dark-mode support

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
