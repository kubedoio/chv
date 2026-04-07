# CHV MVP-1 Clean Rebuild Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace CHV's mixed PostgreSQL/Vue/raw-disk/node-scheduler architecture with one coherent MVP-1 implementation: Go controller/agent, SQLite, explicit bootstrap/install status, qcow2 plus seed ISO VM provisioning, and a SvelteKit operator UI.

**Architecture:** Perform an in-place clean rebuild. First remove conflicting active product paths, docs, and runtime assumptions. Then introduce a thin but coherent controller/agent split with a SQLite-backed repository layer, structured install/bootstrap APIs, and a SvelteKit UI that reflects only the supported MVP-1 surface.

**Tech Stack:** Go 1.26, SQLite (`modernc.org/sqlite`), Chi, SvelteKit, TypeScript, Tailwind CSS, Vitest, Playwright or component tests as needed later.

---

### Task 1: Normalize the active repo surface

**Files:**
- Modify: `/Users/scolak/Projects/chv/README.md`
- Modify: `/Users/scolak/Projects/chv/go.mod`
- Delete or replace: `/Users/scolak/Projects/chv/chv-ui/**`
- Delete or replace conflicting docs/configs under `/Users/scolak/Projects/chv/docs/**`, `/Users/scolak/Projects/chv/configs/**`, `/Users/scolak/Projects/chv/internal/**`, `/Users/scolak/Projects/chv/cmd/**`
- Create: `/Users/scolak/Projects/chv/ui/**`

**Step 1: Write the failing verification**

Run: `rg -n "PostgreSQL|postgres|Vue|PrimeVue|Pinia|raw runtime|NFS|scheduler|node inventory|JWT" README.md docs configs internal cmd chv-ui`

Expected: Many matches proving the repo still exposes conflicting architecture.

**Step 2: Remove the conflicting active paths**

- Delete the Vue UI and its build files.
- Delete controller/agent packages that encode nodes, quotas, snapshots, scheduler, reconciliation, JWT agent auth, and raw-runtime-disk assumptions.
- Keep only low-level pieces that remain valid for the new contract or rebuild them cleanly.

**Step 3: Replace active documentation**

- Rewrite `/Users/scolak/Projects/chv/README.md` to describe only the consolidated MVP-1 contract.
- Keep `DESIGN.md` as the UI design source where compatible.
- Remove or replace obsolete docs that would otherwise present parallel architecture paths.

**Step 4: Verify normalization**

Run: `rg -n "PostgreSQL|postgres|Vue|PrimeVue|Pinia|NFS|raw runtime|scheduler|node inventory|JWT" README.md docs configs internal cmd ui`

Expected: No remaining active matches except intentional historical references in plan-only files.

**Step 5: Commit**

```bash
git add README.md docs configs cmd internal ui go.mod go.sum
git commit -m "refactor: normalize repo to single MVP-1 direction"
```

### Task 2: Build the new backend foundation

**Files:**
- Create: `/Users/scolak/Projects/chv/cmd/chv-controller/main.go`
- Create: `/Users/scolak/Projects/chv/cmd/chv-agent/main.go`
- Create: `/Users/scolak/Projects/chv/internal/config/config.go`
- Create: `/Users/scolak/Projects/chv/internal/models/*.go`
- Create: `/Users/scolak/Projects/chv/internal/db/sqlite.go`
- Create: `/Users/scolak/Projects/chv/internal/db/migrate.go`
- Create: `/Users/scolak/Projects/chv/internal/auth/service.go`
- Create: `/Users/scolak/Projects/chv/internal/operations/service.go`
- Create: `/Users/scolak/Projects/chv/configs/schema_sqlite.sql`
- Test: `/Users/scolak/Projects/chv/internal/db/*.go`, `/Users/scolak/Projects/chv/internal/auth/*.go`

**Step 1: Write failing repository tests**

- Add tests for bootstrapping the SQLite schema.
- Add tests for token hashing and validation.
- Add tests that create and read install status, storage pools, networks, images, VMs, and operations.

**Step 2: Run tests to verify failure**

Run: `go test ./internal/db ./internal/auth`

Expected: Fails because the new repository layer does not exist yet.

**Step 3: Implement the minimal repository layer**

- Use only the required tables from the consolidated spec.
- Store timestamps as ISO 8601 UTC text and booleans as `0/1`.
- Expose repository methods only for the supported MVP-1 resources.

**Step 4: Re-run backend foundation tests**

Run: `go test ./internal/db ./internal/auth`

Expected: PASS

**Step 5: Commit**

```bash
git add cmd/chv-controller cmd/chv-agent internal/config internal/models internal/db internal/auth internal/operations configs/schema_sqlite.sql
git commit -m "feat: add MVP-1 SQLite control plane foundation"
```

### Task 3: Implement install status, bootstrap, and repair

**Files:**
- Create: `/Users/scolak/Projects/chv/internal/bootstrap/service.go`
- Create: `/Users/scolak/Projects/chv/internal/installstatus/service.go`
- Create: `/Users/scolak/Projects/chv/internal/network/bridge.go`
- Create: `/Users/scolak/Projects/chv/internal/api/handler.go`
- Create: `/Users/scolak/Projects/chv/internal/api/install.go`
- Test: `/Users/scolak/Projects/chv/internal/bootstrap/*.go`, `/Users/scolak/Projects/chv/internal/network/*.go`, `/Users/scolak/Projects/chv/internal/api/*.go`

**Step 1: Write failing tests**

- Test bootstrap idempotency.
- Test directory creation under `/var/lib/chv/`.
- Test bridge detection and drift reporting.
- Test localdisk registration and install status persistence.
- Test HTTP responses for:
  - `GET /api/v1/install/status`
  - `POST /api/v1/install/bootstrap`
  - `POST /api/v1/install/repair`

**Step 2: Run tests to verify failure**

Run: `go test ./internal/bootstrap ./internal/network ./internal/api`

Expected: FAIL

**Step 3: Implement minimal services**

- Abstract shell execution for bridge operations so tests can fake host state safely.
- Ensure bootstrap never overwrites unrelated bridge state silently.
- Persist structured install status on checks and bootstrap/repair actions.
- Return the structured error envelope required by the spec.

**Step 4: Re-run the install-path tests**

Run: `go test ./internal/bootstrap ./internal/network ./internal/api`

Expected: PASS

**Step 5: Commit**

```bash
git add internal/bootstrap internal/installstatus internal/network internal/api cmd/chv-controller cmd/chv-agent
git commit -m "feat: add install status and bootstrap APIs"
```

### Task 4: Implement images, cloud-init, and VM lifecycle

**Files:**
- Create or modify: `/Users/scolak/Projects/chv/internal/images/service.go`
- Create or modify: `/Users/scolak/Projects/chv/internal/cloudinit/*.go`
- Create: `/Users/scolak/Projects/chv/internal/storage/service.go`
- Create: `/Users/scolak/Projects/chv/internal/hypervisor/launcher.go`
- Create: `/Users/scolak/Projects/chv/internal/vm/service.go`
- Create: `/Users/scolak/Projects/chv/internal/api/images.go`
- Create: `/Users/scolak/Projects/chv/internal/api/networks.go`
- Create: `/Users/scolak/Projects/chv/internal/api/storage.go`
- Create: `/Users/scolak/Projects/chv/internal/api/vms.go`
- Test: matching `_test.go` files

**Step 1: Write failing tests**

- qcow2 image import metadata handling
- cloud-init seed ISO generation
- VM workspace creation
- VM start boot-gate enforcement so no VM starts before `seed.iso` exists
- VM start/stop/delete API behavior

**Step 2: Run tests to verify failure**

Run: `go test ./internal/images ./internal/cloudinit ./internal/storage ./internal/hypervisor ./internal/vm ./internal/api`

Expected: FAIL

**Step 3: Implement the minimal supported path**

- Import qcow2 images into `/var/lib/chv/images`
- Prepare per-VM workspace under `/var/lib/chv/vms/<vm-id>/`
- Generate `disk.qcow2`, `seed.iso`, and `config.json`
- Launch Cloud Hypervisor with explicit qcow2 disk plus readonly seed ISO

**Step 4: Re-run tests**

Run: `go test ./internal/images ./internal/cloudinit ./internal/storage ./internal/hypervisor ./internal/vm ./internal/api`

Expected: PASS

**Step 5: Commit**

```bash
git add internal/images internal/cloudinit internal/storage internal/hypervisor internal/vm internal/api
git commit -m "feat: add qcow2 image and VM lifecycle path"
```

### Task 5: Implement the SvelteKit operator UI

**Files:**
- Create: `/Users/scolak/Projects/chv/ui/package.json`
- Create: `/Users/scolak/Projects/chv/ui/svelte.config.js`
- Create: `/Users/scolak/Projects/chv/ui/vite.config.ts`
- Create: `/Users/scolak/Projects/chv/ui/src/routes/**`
- Create: `/Users/scolak/Projects/chv/ui/src/lib/api/**`
- Create: `/Users/scolak/Projects/chv/ui/src/lib/components/**`
- Create: `/Users/scolak/Projects/chv/ui/src/app.html`
- Create: `/Users/scolak/Projects/chv/ui/src/app.css`
- Test: `/Users/scolak/Projects/chv/ui/src/**/*.test.ts`

**Step 1: Write failing UI tests**

- install status page rendering
- bootstrap action
- repair action
- image import form rendering
- VM create flow review state
- API error rendering

**Step 2: Run tests to verify failure**

Run: `cd /Users/scolak/Projects/chv/ui && npm test`

Expected: FAIL because the new SvelteKit app does not exist yet.

**Step 3: Implement the UI in route order**

- `/login`
- `/install`
- `/`, `/storage`, `/networks`, `/images`
- `/vms`, `/vms/[id]`
- `/operations`, `/settings`

Use only the virtualization-console styling defined in `DESIGN.md`.

**Step 4: Re-run UI verification**

Run: `cd /Users/scolak/Projects/chv/ui && npm test && npm run build`

Expected: PASS

**Step 5: Commit**

```bash
git add ui
git commit -m "feat: add SvelteKit operator console for MVP-1"
```

### Task 6: Final project verification

**Files:**
- Modify as needed based on verification failures

**Step 1: Run backend verification**

Run: `go test ./...`

Expected: PASS

**Step 2: Run frontend verification**

Run: `cd /Users/scolak/Projects/chv/ui && npm test && npm run build`

Expected: PASS

**Step 3: Run normalization verification**

Run: `rg -n "PostgreSQL|postgres|Vue|PrimeVue|Pinia|NFS|raw runtime|scheduler|node inventory|JWT" README.md docs configs internal cmd ui`

Expected: No active architecture conflicts remain.

**Step 4: Commit final fixes**

```bash
git add -A
git commit -m "test: verify clean MVP-1 rebuild"
```
