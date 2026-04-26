# CHV Phased Implementation Plan

Based on the comprehensive repository review and existing roadmap (`docs/plans/2026-04-24-gap-analysis-and-implementation-plan.md`), the following is a detailed, phased implementation plan to bring the CHV platform to production readiness.

## Phase 1: Stability, Bug Fixes & Agent Parity

**Goal**: Ensure the foundation is solid, fix failing tests, and replace mock agent implementations with real daemon calls.

### 1.1 UI & Frontend Fixes
*   **Fix `Ctrl+K` Command Palette Test**: Address the failing Playwright e2e test in `ui/tests/e2e/navigation.spec.ts:29`. The command palette currently doesn't appear when `Ctrl+K` is triggered programmatically via `KeyboardEvent`.
*   **Clean Up Unused CSS**: Remove unused CSS selectors across Svelte components (e.g., `.user-icon`, `.username-cell` in `settings/users/+page.svelte`, `.action-btn` in `templates/+page.svelte`, etc.) to clear `svelte-check` warnings.
*   **Fix A11y Warnings**: Add `tabindex` and keyboard event handlers to the `dialog` roles in `CloudInitViewer.svelte` and `CloudInitEditor.svelte`. Fix associated labels in `settings/users/+page.svelte`.

### 1.2 Agent Daemon Integration (Removing Stubs)
*   **`chv-stord` Integration (`A1`, `A3`)**: Implement missing `stord` gRPC calls in `crates/chv-agent-core/src/reconcile.rs`. Replace `unimplemented!()` in the mock with real daemon clients for:
    *   `get_volume_health`
    *   `resize_volume`
    *   `prepare_snapshot`
    *   `prepare_clone`
    *   `restore_snapshot`
    *   `delete_snapshot`
    *   `set_device_policy` (`A7`)
*   **`chv-nwd` Integration (`A2`, `A3`)**: Implement missing `nwd` gRPC calls in `crates/chv-agent-core/src/reconcile.rs`.
    *   `get_network_health`
    *   `set_firewall_policy`
    *   `set_nat_policy`
    *   `ensure_dhcp_scope`
    *   `ensure_dns_scope`
    *   `expose_service`
    *   `withdraw_service_exposure` (Needs wiring for exposures removed from desired state).
*   **Console Server Hardening (`A4`, `A5`)**:
    *   Implement one-time-use LRU cache and replay prevention for Console tokens.
    *   Wire `ioctl(TIOCSWINSZ)` to allow PTY resizing.

### 1.3 Control Plane Infrastructure (`I1`, `I2`)
*   Fix `dev-install.sh` / `install.sh` to ensure the SQLite database has proper `chown chv:chv` ownership to prevent read-only DB errors on deploy.
*   Fix agent console port collision (PID holding 8444 after restart) by configuring `KillMode=mixed` and `TimeoutStopSec=5` in the agent systemd unit.

---

## Phase 2: Feature Completion (Orchestration, Quotas, & Networking)

**Goal**: Fulfill the remaining core features outlined in the gap analysis.

### 2.1 Backend / BFF Completion
*   **Network Mutations (`B1`)**: Implement `mutate_network` in `crates/chv-webui-bff/src/handlers/mutations.rs` (or `networks.rs`) which currently returns `NotImplemented`.
*   **Hypervisor Orchestrator Merge (`B5`)**: Ensure the `vm.create` payload injected into the agent includes all hypervisor fields, falling back to defaults cleanly.
*   **Quota Enforcement (`B3`)**: Implement quota checking at the orchestrator layer (`create_vm` path) to prevent exceeding limits.
*   **Agent API Token Auth (`B10`)**: Enforce gRPC auth for the agent using the existing `tokens` table instead of relying solely on mTLS or local network trust.
*   **Metadata Propagation (`A6`)**: Propagate `operation_id` via gRPC metadata in `daemon_clients.rs`.

### 2.2 Storage & Network Daemons
*   **`chv-nwd` Linux Executor (`A8`)**: Implement the `LinuxExecutor` logic (`ip`, `nft`) in `cmd/chv-nwd/main.rs`.
*   **Storage Pool Provisioning (`B7`)**: Extend `chv-stord` backend to handle actual directory/LVM provisioning and validation, rather than just DB inserts.
*   **Image Import Validation (`B8`)**: Ensure `qcow2` headers are validated correctly before copying.

---

## Phase 3: Production Readiness & Observability

**Goal**: Prepare the system for long-term maintainability, backup strategies, and scale.

### 3.1 UI Production Refactor (`U1`, `U6`, `U7`)
*   **Tailwind-first Migration**: Strip `app.css` and fully migrate primitives/shell components to Tailwind.
*   **Component Reorganization**: Reorganize `ui/src/lib/components` into strictly feature-based folders.
*   **DataTable Refactoring**: Split the large `DataTable` (688 lines) into smaller modules handling selection, sorting, and visibility.
*   **Overview Logic Extraction**: Extract logic from the overview page (526 lines) into dedicated helpers.

### 3.2 Backups & Role-Based Access
*   **Backup/DR (`B2`)**: Remove the backup job and history stubs in `api/stub.rs`. Design the backup schema and implement the backend orchestration for taking VM snapshots and shipping them.
*   **RBAC Middleware (`B6`)**: Add middleware enforcing role-based access control on all BFF routes, moving beyond just `login`/`me`.

### 3.3 CI/CD & Deployments (`I3`, `I4`, `I5`, `I6`)
*   **GitHub Actions**: Set up automated CI (`cargo test`, `cargo clippy`, `npm run build`, Playwright e2e tests).
*   **DB Migration Backups**: Implement a pre-migration SQLite backup hook.
*   **Nginx Routing**: Configure multi-node WebSocket routing (`/ws/vms/`) using a dynamic upstream based on `node_id`.
*   **Versioning**: Automate release versioning.

---

## Success Criteria
1.  All Playwright E2E tests pass reliably.
2.  `svelte-check` reports 0 errors and 0 warnings.
3.  All `unimplemented!()` stubs in the agent `reconcile.rs` are replaced with real daemon clients.
4.  No `BffError::NotImplemented` returned from any frontend API call.
5.  CI pipeline is green and automated backups are working.
