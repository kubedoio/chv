# Implementation Plan: Phases 4-7 - Cloud-Init, VMs, UI, Production

## Overview

Complete CHV MVP-1 by implementing cloud-init seed ISO generation, VM lifecycle management, operator UI workflows, and production hardening.

**Completed:**
- ✅ Phase 1: Foundation (Repository, Bootstrap, POST APIs)
- ✅ Phase 2: Agent Contract (HTTP server, install/bootstrap, controller integration)
- ✅ Phase 3: Image Import (Repository, service, download, checksum, API)

**Remaining:**
- Phase 4: Cloud-Init Seed ISO Generation (3 tasks)
- Phase 5: VM Workspace and Hypervisor Launch (4 tasks)
- Phase 6: Real Operator UI (5 tasks)
- Phase 7: Productionization (4 tasks)

---

## Phase 4: Cloud-Init Seed ISO Generation

Implements cloud-init contract for VM guest initialization.

### Task 1: Cloud-Init Document Rendering
**Description:** Build service that prepares `user-data`, `meta-data`, and optional `network-config`.

**Acceptance criteria:**
- [ ] Render cloud-init artifacts under VM workspace
- [ ] Missing required inputs are rejected clearly
- [ ] Generated metadata is deterministic and testable

**Files to create:**
- `internal/cloudinit/service.go` - Document rendering service
- `internal/cloudinit/templates.go` - Cloud-init YAML templates

**Dependencies:** None

**Scope:** Medium

---

### Task 2: Seed ISO Generation
**Description:** Generate seed ISO using host tools (xorrisofs/mkisofs/genisoimage).

**Acceptance criteria:**
- [ ] Agent verifies ISO generation support before attempting
- [ ] VM seed ISO created before any boot attempt
- [ ] `seed_iso_path` persisted in VM record
- [ ] ISO contains user-data, meta-data, network-config

**Files to create:**
- `internal/agent/services/seediso.go` - ISO generation service
- `internal/agent/handlers/cloudinit.go` - ISO generation handler

**API Endpoint:**
```
POST /v1/vms/{id}/seed-iso
Request: { "user_data": "...", "meta_data": {...}, "network_config": {...} }
Response: { "seed_iso_path": "/var/lib/chv/vms/{id}/seed.iso" }
```

**Dependencies:** Task 1

**Scope:** Large

---

### Task 3: Boot Gate Enforcement
**Description:** Refuse VM start if prerequisites are missing.

**Acceptance criteria:**
- [ ] Start fails with structured error when seed.iso missing
- [ ] Start fails when image or storage not ready
- [ ] Start succeeds only after all gate checks pass
- [ ] Gates: image ready, storage ready, network ready, workspace exists, seed ISO exists, cloud-hypervisor available

**Files to modify:**
- `internal/vm/gate.go` - Boot gate checks
- `internal/api/vms.go` - Start handler with gate enforcement

**Dependencies:** Tasks 1-2

**Scope:** Medium

---

### Checkpoint: Phase 4
- [ ] No VM can start before seed ISO exists
- [ ] Cloud-init support visible in state and errors
- [ ] Tests prove boot gate behavior

---

## Phase 5: VM Workspace and Hypervisor Launch

Implements VM lifecycle with Cloud Hypervisor integration.

### Task 4: VM Creation Workflow
**Description:** Create per-VM workspace, clone base disk, render config.

**Acceptance criteria:**
- [ ] `POST /api/v1/vms` creates VM with `provisioning` → `prepared` state
- [ ] Workspace layout: `/var/lib/chv/vms/{id}/`
- [ ] Files: `config.json`, `disk.qcow2` (cloned from image), `seed.iso`
- [ ] VM record persisted with workspace path

**Files to create:**
- `internal/vm/service.go` - VM lifecycle service
- `internal/vm/workspace.go` - Workspace creation
- `internal/storage/qcow2.go` - Disk cloning utilities

**API:**
```
POST /api/v1/vms
Request: {
  "name": "vm-1",
  "image_id": "...",
  "storage_pool_id": "...",
  "network_id": "...",
  "vcpu": 2,
  "memory_mb": 2048,
  "user_data": "#cloud-config..."
}
```

**Dependencies:** Phase 4 checkpoint

**Scope:** Large

---

### Task 5: Cloud Hypervisor Command Construction
**Description:** Build Cloud Hypervisor command line from VM metadata.

**Acceptance criteria:**
- [ ] Launcher builds CH command with qcow2 disk + readonly seed ISO
- [ ] Network attachment via TAP device on bridge
- [ ] Unit tested without real hypervisor
- [ ] Configurable CH binary path, memory, vCPU

**Files to create:**
- `internal/hypervisor/launcher.go` - Command construction
- `internal/hypervisor/config.go` - CH configuration structs

**Command Structure:**
```bash
cloud-hypervisor \
  --kernel /path/to/vmlinux \
  --disk path=/var/lib/chv/vms/{id}/disk.qcow2 \
  --disk path=/var/lib/chv/vms/{id}/seed.iso,readonly=on \
  --net tap=,mac=,ip=,mask= \
  --cpus boot=2 \
  --memory size=2048M \
  --api-socket /var/lib/chv/vms/{id}/ch-api.sock
```

**Dependencies:** Task 4

**Scope:** Medium

---

### Task 6: VM Start/Stop/Delete Operations
**Description:** VM runtime mutation with state tracking.

**Acceptance criteria:**
- [ ] `POST /api/v1/vms/{id}/start` → `starting` → `running`
- [ ] `POST /api/v1/vms/{id}/stop` → `stopping` → `stopped`
- [ ] `DELETE /api/v1/vms/{id}` → `deleting` → cleanup
- [ ] PID tracking for running VMs
- [ ] Operations logged

**Files to modify:**
- `internal/vm/service.go` - Start/stop/delete methods
- `internal/api/vms.go` - Handlers

**API:**
```
POST /api/v1/vms/{id}/start
POST /api/v1/vms/{id}/stop
DELETE /api/v1/vms/{id}
```

**Dependencies:** Tasks 4-5

**Scope:** Large

---

### Task 7: VM Detail and Operations APIs
**Description:** VM detail view and operations filtering.

**Acceptance criteria:**
- [ ] `GET /api/v1/vms/{id}` returns full VM details
- [ ] Operations filtered by resource_type="vm", resource_id
- [ ] Response includes last_error, created_at, updated_at

**Files to modify:**
- `internal/api/vms.go` - Get VM handler
- `internal/db/sqlite.go` - GetVMByID, ListOperationsByResource

**Dependencies:** Task 6

**Scope:** Medium

---

### Checkpoint: Phase 5
- [ ] Complete create/start/stop/delete path
- [ ] Per-VM workspace contract honored
- [ ] Cloud Hypervisor launch test-covered

---

## Phase 6: Real Operator UI

Completes UI with real VM workflows.

### Task 8: Install Page Actions
**Description:** Complete install page with real bootstrap/repair actions.

**Acceptance criteria:**
- [ ] `/install` shows all install fields from spec
- [ ] Bootstrap action triggers backend bootstrap
- [ ] Repair actions (bridge, directories) work
- [ ] Success/failure messages from backend responses

**Files to modify:**
- `ui/src/routes/install/+page.svelte`
- `ui/src/lib/api/client.ts` - Add bootstrap/repair methods

**Dependencies:** Phases 1-2 (already complete)

**Scope:** Medium

---

### Task 9: Storage and Networks Pages
**Description:** Replace placeholders with real data and create flows.

**Acceptance criteria:**
- [ ] `/storage` shows real localdisk pools with create modal
- [ ] `/networks` shows bridge-backed networks with create modal
- [ ] System-managed badges shown
- [ ] Create flows already implemented (verify working)

**Files to verify/modify:**
- `ui/src/routes/storage/+page.svelte`
- `ui/src/routes/networks/+page.svelte`

**Dependencies:** Phase 1 (already complete)

**Scope:** Medium

---

### Task 10: Images Page Workflow
**Description:** Add image import form and status display.

**Acceptance criteria:**
- [ ] Import form with name, URL, checksum fields
- [ ] Import progress/status visible (polling)
- [ ] Error handling for failed imports
- [ ] Only qcow2 format shown

**Files to modify:**
- `ui/src/routes/images/+page.svelte`
- `ui/src/lib/api/client.ts` - Add importImage method
- `ui/src/lib/components/ImportImageModal.svelte` - NEW

**Dependencies:** Phase 3 (already complete)

**Scope:** Small

---

### Task 11: VM List and Create Flow
**Description:** Build VM management UI.

**Acceptance criteria:**
- [ ] `/vms` shows table: name, state, image, pool, network, vCPU, memory, IP, last error
- [ ] Create VM form: name, image, pool, network, vCPU, memory, user-data
- [ ] Review step showing: qcow2 image, seed ISO, bridge network, localdisk
- [ ] Stats cards at top

**Files to create/modify:**
- `ui/src/routes/vms/+page.svelte` - Enhanced with create button
- `ui/src/lib/components/CreateVMModal.svelte` - NEW

**Dependencies:** Phase 5 checkpoint

**Scope:** Large

---

### Task 12: VM Detail and Operations Pages
**Description:** Complete remaining routes.

**Acceptance criteria:**
- [ ] `/vms/[id]` shows: disk path, seed ISO path, workspace, state, cloud-init summary, errors, operations history
- [ ] Start/stop/delete buttons
- [ ] `/operations` shows recent actions
- [ ] `/settings` shows MVP-1 settings only

**Files to create/modify:**
- `ui/src/routes/vms/[id]/+page.svelte` - Enhance
- `ui/src/routes/operations/+page.svelte` - Enhance
- `ui/src/routes/settings/+page.svelte` - Enhance

**Dependencies:** Tasks 7, 11

**Scope:** Large

---

### Checkpoint: Phase 6
- [ ] Every route renders backend truth
- [ ] No decorative features presented as real
- [ ] Token login works

---

## Phase 7: Productionization

CI, testing, and documentation.

### Task 13: Expand Backend Tests
**Description:** Add tests for all new functionality.

**Acceptance criteria:**
- [ ] Tests: bridge drift, localdisk registration, image import, seed ISO, VM workspace, VM launch
- [ ] Controller and agent integration tests
- [ ] >80% coverage for new packages

**Files:**
- Various `*_test.go` files

**Dependencies:** Phases 3-5

**Scope:** Medium

---

### Task 14: Expand Frontend Tests
**Description:** Add UI tests for workflows.

**Acceptance criteria:**
- [ ] Install page tests
- [ ] Image import flow tests
- [ ] VM create flow tests
- [ ] API error rendering tests

**Files:**
- `ui/src/**/*.test.ts`

**Dependencies:** Phase 6

**Scope:** Medium

---

### Task 15: Add CI Pipeline
**Description:** GitHub Actions for backend and UI.

**Acceptance criteria:**
- [ ] Backend tests run on PR/push
- [ ] UI tests and build run on PR/push
- [ ] Container builds syntax-checked
- [ ] Go linting (golangci-lint)
- [ ] TypeScript type checking

**Files to create:**
- `.github/workflows/ci.yml`

**Dependencies:** Tasks 13-14

**Scope:** Medium

---

### Task 16: Final Documentation
**Description:** Update all docs for shipped MVP-1.

**Acceptance criteria:**
- [ ] README reflects final behavior
- [ ] Install docs: compose for controller/webUI, host-native agent
- [ ] API documentation
- [ ] Remove obsolete docs

**Files to update:**
- `README.md`
- `docs/` (various)

**Dependencies:** All previous phases

**Scope:** Small

---

### Checkpoint: MVP-1 Ready
- [ ] All tests pass
- [ ] UI builds
- [ ] Docker compose works
- [ ] End-to-end runbook exists

---

## Timeline Estimate

| Phase | Tasks | Effort |
|-------|-------|--------|
| Phase 4: Cloud-Init | 3 | 4 sessions |
| Phase 5: VMs | 4 | 6 sessions |
| Phase 6: UI | 5 | 5 sessions |
| Phase 7: Production | 4 | 3 sessions |
| **Total** | **16** | **18 sessions** |

---

## Risk Mitigation (from original plan)

| Risk | Mitigation |
|------|------------|
| Cloud Hypervisor differences | Launcher isolated, tested separately |
| ISO generation tooling | Verify support, fail with structured hints |
| Containerized controller limits | Keep host-native support |
| UI/backend drift | Gate UI on real API capabilities |

---

## Recommended Execution Order

1. **Phase 4** → Seed ISO generation (prerequisite for VMs)
2. **Phase 5** → VM lifecycle (core feature)
3. **Checkpoint** → Full VM create/start/stop/delete works
4. **Phase 6** → UI for VM management
5. **Checkpoint** → End-to-end user flow complete
6. **Phase 7** → Testing and documentation
7. **Final Checkpoint** → MVP-1 ready
