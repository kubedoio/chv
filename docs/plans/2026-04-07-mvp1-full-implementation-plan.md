# CHV MVP-1 Full Implementation Plan

## Overview

This plan starts from the current rebuilt baseline, not from the legacy repo state.

Already in place:

- one active MVP-1 direction in the root docs
- Go controller and agent entrypoints
- SQLite schema bootstrap and repository foundation
- token creation and bearer validation
- install status, bootstrap, and repair APIs
- Linux bridge inspection and drift detection
- SvelteKit route skeleton
- containerized control-plane path for `chv-controller` and `chv-webui`

Still missing:

- full CRUD and mutation APIs for networks, storage pools, images, VMs, and operations
- host-native `chv-agent` execution path
- qcow2 image import workflow
- cloud-init seed ISO generation
- VM workspace creation and Cloud Hypervisor launch path
- serious operator UI workflows beyond install scaffolding
- end-to-end tests and deployment hardening

## Architecture Decisions

- Keep one active architecture only: Go + SQLite + SvelteKit + Cloud Hypervisor.
- Run `chv-controller` and `chv-webui` in containers when helpful, but keep `chv-agent` host-native on the node that has hypervisor and bridge access.
- Build the remaining work as thin vertical slices that leave the repo runnable after each phase.
- Do not reintroduce legacy assumptions such as PostgreSQL, Vue, raw-disk-first provisioning, scheduler/node inventory, JWT agent auth, or speculative storage/network backends.

## Current Baseline Gaps

- Backend routing currently exposes read-only list endpoints for several resources, but the resource services and mutation flows are not implemented.
- The agent binary exists as a scaffold, but it does not yet own host-side actions like image preparation, seed ISO generation, or VM lifecycle execution.
- The UI has route placeholders and install-first scaffolding, but the main operator workflows are not yet wired to real mutations.
- Operations logging exists in the schema but is not yet used as the execution ledger for VM and install actions.

## Task List

### Phase 1: Close Out Control-Plane Foundation

Replaces older assumption:
- ad hoc bootstrap/runtime setup with hidden behavior

Implements spec sections:
- 6. Host Installation and Bootstrap Specification
- 12. Auth Contract
- 13. REST API Contract
- 14. SQLite Schema Contract

#### Task 1: Finish repository methods for all MVP-1 tables

**Description:** Complete the SQLite repository surface so every required table has create, get, list, and update methods needed by the controller.

**Acceptance criteria:**
- [ ] `networks`, `storage_pools`, `images`, `virtual_machines`, and `operations` all have explicit repository methods
- [ ] repository methods respect the MVP-1 schema and ISO 8601 UTC timestamp rules
- [ ] default storage and default network records can be created idempotently

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/db`

**Dependencies:** None

**Estimated scope:** Medium

#### Task 2: Persist and expose default system records cleanly

**Description:** Make bootstrap create or verify the default `localdisk` storage pool and the default system-managed bridge-backed network record without duplicating them.

**Acceptance criteria:**
- [ ] bootstrap registers `localdisk` idempotently
- [ ] bootstrap registers the default network for `chvbr0` idempotently
- [ ] install state, storage state, and network state remain consistent across repeated bootstrap runs

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/bootstrap ./internal/db`

**Dependencies:** Task 1

**Estimated scope:** Medium

#### Task 3: Add structured POST APIs for networks and storage pools

**Description:** Implement the missing create endpoints so the control plane matches the REST contract for these foundational resources.

**Acceptance criteria:**
- [ ] `POST /api/v1/networks` exists and validates bridge-backed network requests
- [ ] `POST /api/v1/storage-pools` exists and only supports `localdisk`
- [ ] all failures use the structured error envelope

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/api`

**Dependencies:** Tasks 1-2

**Estimated scope:** Medium

### Checkpoint: Foundation Closed

- [ ] `/usr/local/go/bin/go test ./...`
- [ ] `GET /api/v1/install/status` still returns the expected shape
- [ ] bootstrap remains idempotent

### Phase 2: Build the Host-Native Agent Contract

Replaces older assumption:
- controller-only runtime behavior and implicit host mutation

Implements spec sections:
- 15. Backend Architecture Contract

#### Task 4: Define the controller-agent boundary

**Description:** Decide and codify the exact request/response contract between `chv-controller` and `chv-agent`, including install checks, image import, seed ISO generation, and VM lifecycle actions.

**Acceptance criteria:**
- [ ] an internal package or ADR defines the agent request models and result models
- [ ] controller responsibilities and agent responsibilities are separated cleanly in code
- [ ] no VM host action is performed by controller-only code paths

**Verification:**
- [ ] build passes for both binaries
- [ ] plan and contract reviewed in the codebase docs

**Dependencies:** Phase 1 checkpoint

**Estimated scope:** Small

#### Task 5: Implement agent command surface for install and repair

**Description:** Move host validation and repair execution into `chv-agent` while keeping controller APIs stable.

**Acceptance criteria:**
- [ ] agent can execute install checks and return structured status
- [ ] agent can execute bootstrap and repair actions safely and idempotently
- [ ] controller delegates host actions instead of embedding host-only logic long-term

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/bootstrap ./internal/installstatus ./internal/network`
- [ ] `/usr/local/go/bin/go build ./cmd/chv-controller ./cmd/chv-agent`

**Dependencies:** Task 4

**Estimated scope:** Large

#### Task 6: Add operations logging for install and repair actions

**Description:** Every bootstrap and repair action should create an auditable `operations` row with request, result, and error payloads.

**Acceptance criteria:**
- [ ] install bootstrap and repair actions write `operations` rows
- [ ] operation state transitions are recorded correctly
- [ ] failed actions capture structured error payloads

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/api ./internal/db`

**Dependencies:** Tasks 1, 5

**Estimated scope:** Medium

### Checkpoint: Agent Boundary Landed

- [ ] controller still serves install APIs correctly
- [ ] agent owns host mutation paths
- [ ] operations list begins reflecting real actions

### Phase 3: Implement Image Import

Replaces older assumption:
- raw-runtime-disk-first provisioning and fake multi-backend image support

Implements spec sections:
- 9. Storage and Image Contract
- 13.5 Images

#### Task 7: Build image metadata model and import service

**Description:** Implement the controller and agent path for importing qcow2 cloud images into `/var/lib/chv/images`.

**Acceptance criteria:**
- [ ] `POST /api/v1/images/import` creates an image record with `importing` state
- [ ] import flow supports remote URL fetch and local copy path handling if intentionally supported
- [ ] imported image records store name, os family, architecture, format, source URL, checksum, and local path

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/images ./internal/db ./internal/api`

**Dependencies:** Phase 2 checkpoint

**Estimated scope:** Large

#### Task 8: Add checksum validation and failure handling

**Description:** Ensure the image import path validates checksum when provided and surfaces `ready` or `failed` states explicitly.

**Acceptance criteria:**
- [ ] checksum validation supports the `sha256:` format from the spec
- [ ] failed downloads or mismatched checksums mark image state as `failed`
- [ ] the API returns stable resource state after import attempts

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/images`

**Dependencies:** Task 7

**Estimated scope:** Medium

#### Task 9: Wire image list and import into the UI

**Description:** Turn the `/images` page from a display placeholder into an operator workflow with import form, status table, and error surface.

**Acceptance criteria:**
- [ ] `/images` shows real image rows
- [ ] import form submits to the backend and shows pending, ready, and failed states
- [ ] the page reflects qcow2-only MVP-1 language

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run build`

**Dependencies:** Tasks 7-8

**Estimated scope:** Medium

### Checkpoint: Image Workflow Complete

- [ ] controller can import and list qcow2 images
- [ ] UI reflects import state honestly
- [ ] no raw-disk or non-MVP-1 storage language remains in the active path

### Phase 4: Implement Cloud-Init Seed ISO Generation

Replaces older assumption:
- boot before preparation and implicit guest initialization

Implements spec sections:
- 10. Cloud-init Contract

#### Task 10: Implement cloud-init document rendering

**Description:** Build the service that prepares `user-data`, `meta-data`, and optional `network-config` into a VM-specific working directory.

**Acceptance criteria:**
- [ ] rendered cloud-init artifacts are created under the VM workspace or cloudinit staging area
- [ ] missing required cloud-init inputs are rejected clearly
- [ ] generated metadata is deterministic and testable

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/cloudinit`

**Dependencies:** Phase 3 checkpoint

**Estimated scope:** Medium

#### Task 11: Implement seed ISO generation support

**Description:** Generate a real seed ISO and record its path in VM metadata.

**Acceptance criteria:**
- [ ] agent verifies ISO generation support before attempting generation
- [ ] VM seed ISO is created before any boot attempt
- [ ] `seed_iso_path` is persisted in the VM record

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/cloudinit ./internal/vm`

**Dependencies:** Task 10

**Estimated scope:** Large

#### Task 12: Add boot gate enforcement

**Description:** Refuse VM start if image, storage, network, workspace, seed ISO, or Cloud Hypervisor prerequisites are missing.

**Acceptance criteria:**
- [ ] start requests fail with a structured error when `seed.iso` does not exist
- [ ] start requests fail cleanly when image or storage state is not ready
- [ ] start requests succeed only after all gate checks pass

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/vm ./internal/api`

**Dependencies:** Tasks 7, 11

**Estimated scope:** Medium

### Checkpoint: Pre-Boot Preparation Enforced

- [ ] no VM can start before seed ISO exists
- [ ] cloud-init support is visible in state and errors
- [ ] tests prove the boot gate behavior

### Phase 5: Implement VM Workspace and Hypervisor Launch Path

Replaces older assumption:
- speculative scheduler/node orchestration and broad legacy VM surface

Implements spec sections:
- 8.5 VirtualMachine
- 9.3 VM Disk Policy
- 13.6 VMs

#### Task 13: Implement VM creation workflow

**Description:** Create the per-VM workspace, copy or clone the base qcow2 disk into `disk.qcow2`, render config, and persist the VM record.

**Acceptance criteria:**
- [ ] `POST /api/v1/vms` creates a VM with `provisioning` then `prepared` state transitions
- [ ] workspace layout matches the filesystem contract
- [ ] `config.json`, `disk.qcow2`, and `seed.iso` exist in the expected locations after preparation

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/vm ./internal/storage ./internal/api`

**Dependencies:** Phase 4 checkpoint

**Estimated scope:** Large

#### Task 14: Implement Cloud Hypervisor command construction

**Description:** Add a launcher that builds the Cloud Hypervisor command line from VM metadata and host defaults.

**Acceptance criteria:**
- [ ] launcher uses explicit qcow2 disk plus readonly seed ISO
- [ ] launcher includes network attachment for the bridge-backed model
- [ ] command generation is unit tested without requiring a real hypervisor in tests

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/hypervisor ./internal/vm`

**Dependencies:** Task 13

**Estimated scope:** Medium

#### Task 15: Implement VM start, stop, and delete operations

**Description:** Complete the runtime mutation path with state transitions, pid tracking, cleanup behavior, and operation logging.

**Acceptance criteria:**
- [ ] `POST /api/v1/vms/{id}/start` updates state to `starting` and then `running`
- [ ] `POST /api/v1/vms/{id}/stop` updates state to `stopping` and then `stopped`
- [ ] `DELETE /api/v1/vms/{id}` updates state to `deleting` and handles workspace cleanup policy safely

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/vm ./internal/api ./internal/operations`

**Dependencies:** Tasks 6, 12, 14

**Estimated scope:** Large

#### Task 16: Implement VM detail and operations APIs

**Description:** Add `GET /api/v1/vms/{id}` and ensure the operations feed supports both list and per-resource filtering needed by the VM detail view.

**Acceptance criteria:**
- [ ] VM detail response includes last error, created_at, and updated_at
- [ ] operations API can show the VM execution history needed by the UI
- [ ] the API response shapes match the consolidated contract

**Verification:**
- [ ] `/usr/local/go/bin/go test ./internal/api ./internal/db`

**Dependencies:** Task 15

**Estimated scope:** Medium

### Checkpoint: VM Lifecycle Running

- [ ] one complete create/start/stop/delete path exists
- [ ] per-VM workspace contract is honored
- [ ] Cloud Hypervisor launch path is test-covered

### Phase 6: Build the Real Operator UI

Replaces older assumption:
- placeholder or imagined SaaS UI instead of an operator console tied to backend truth

Implements spec sections:
- 16. Web UI Contract

#### Task 17: Finish install page actions and state rendering

**Description:** Complete the install page so it is the truthful operations console for bootstrap, repair, warnings, and errors.

**Acceptance criteria:**
- [ ] `/install` shows all install fields from the spec
- [ ] bootstrap and repair actions mutate real backend state
- [ ] success and failure messages are based on backend responses only

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run build`

**Dependencies:** Phases 1-2

**Estimated scope:** Medium

#### Task 18: Finish storage and networks pages

**Description:** Replace placeholders with real tables, empty states, and system-managed badges.

**Acceptance criteria:**
- [ ] `/storage` shows real localdisk information
- [ ] `/networks` shows bridge name, CIDR, gateway, status, and system-managed state
- [ ] create flows are present only where the backend actually supports them

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`

**Dependencies:** Tasks 2-3

**Estimated scope:** Medium

#### Task 19: Finish images page workflow

**Description:** Add real import forms, status refresh, and error handling on the images page.

**Acceptance criteria:**
- [ ] operators can submit an image import from the UI
- [ ] import progress and failure state are visible
- [ ] no unsupported format options are shown

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`

**Dependencies:** Task 9

**Estimated scope:** Small

#### Task 20: Build VM list and VM create flow

**Description:** Add the core VM workflow with create form, review step, list table, and action buttons.

**Acceptance criteria:**
- [ ] `/vms` shows name, states, image, storage, network, CPU, memory, IP, and last error
- [ ] create flow requires name, image, storage pool, network, vCPU, memory, and cloud-init user-data
- [ ] review step explicitly shows qcow2 image, seed ISO, bridge-backed network, and localdisk target

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run build`

**Dependencies:** Phase 5 checkpoint

**Estimated scope:** Large

#### Task 21: Build VM detail, operations, login, and settings pages

**Description:** Complete the remaining required routes with backend truth and operator-safe token handling.

**Acceptance criteria:**
- [ ] `/vms/[id]` shows disk path, seed ISO path, workspace path, state, cloud-init summary, errors, and operations history
- [ ] `/operations` shows recent auditable actions
- [ ] `/login` stores bearer token without logging it
- [ ] `/settings` exposes only real MVP-1 settings

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`

**Dependencies:** Task 16, Task 20

**Estimated scope:** Large

### Checkpoint: UI Matches Backend Truth

- [ ] every required route exists and renders backend truth
- [ ] no decorative or unsupported features are presented as real
- [ ] token login works with opaque bearer tokens only

### Phase 7: Productionization, CI, and Documentation

Replaces older assumption:
- mixed install lore and manual verification only

Implements spec sections:
- 17. Implementation Order Contract
- 18. Testing Contract

#### Task 22: Expand automated backend coverage

**Description:** Add the remaining backend tests required by the MVP-1 testing contract.

**Acceptance criteria:**
- [ ] tests cover bridge drift detection, localdisk registration, image import, seed ISO generation, VM workspace creation, and VM launch path
- [ ] controller and agent packages both have meaningful integration coverage

**Verification:**
- [ ] `/usr/local/go/bin/go test ./...`

**Dependencies:** Phases 3-5

**Estimated scope:** Medium

#### Task 23: Expand automated frontend coverage

**Description:** Add the required install, image import, VM create, and API error rendering tests.

**Acceptance criteria:**
- [ ] install page rendering and actions are covered
- [ ] image import flow is covered
- [ ] VM create flow is covered
- [ ] API error rendering is covered

**Verification:**
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`

**Dependencies:** Phase 6

**Estimated scope:** Medium

#### Task 24: Add CI for backend and UI

**Description:** Introduce a CI pipeline that runs backend tests plus UI tests and build on every push and PR.

**Acceptance criteria:**
- [ ] backend tests run in CI
- [ ] UI tests and UI build run in CI
- [ ] container build paths are exercised or at least syntax-checked in CI

**Verification:**
- [ ] CI workflow files are valid
- [ ] local dry-run commands match the CI steps

**Dependencies:** Tasks 22-23

**Estimated scope:** Medium

#### Task 25: Final documentation pass

**Description:** Rewrite docs so the repo explains only the coherent MVP-1 that actually shipped.

**Acceptance criteria:**
- [ ] README reflects the final shipped behavior
- [ ] install and run docs explain compose for controller/webUI and host-native agent deployment
- [ ] any remaining obsolete docs are removed or clearly marked historical

**Verification:**
- [ ] documentation reviewed against the actual code paths

**Dependencies:** All previous phases

**Estimated scope:** Small

### Final Checkpoint: MVP-1 Ready

- [ ] `/usr/local/go/bin/go test ./...`
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run test`
- [ ] `cd /Users/scolak/Projects/chv/ui && npm run build`
- [ ] `docker compose build`
- [ ] one documented end-to-end runbook exists for controller + webUI containers and host-native agent startup

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Cloud Hypervisor command details differ across hosts | High | keep launcher isolated and test command construction separately from execution |
| Seed ISO generation tooling differs by distro | High | verify support explicitly in agent and fail with structured hints |
| Containerized controller may not be appropriate for every host network setup | Medium | keep host-native controller support viable alongside compose |
| UI drifts ahead of backend behavior | Medium | gate UI routes and actions on real API capabilities only |
| Long implementation phases create too-large diffs | Medium | treat each task above as its own commit-sized slice |

## Recommended Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Checkpoint: Foundation Closed
5. Task 4
6. Task 5
7. Task 6
8. Checkpoint: Agent Boundary Landed
9. Task 7
10. Task 8
11. Task 9
12. Checkpoint: Image Workflow Complete
13. Task 10
14. Task 11
15. Task 12
16. Checkpoint: Pre-Boot Preparation Enforced
17. Task 13
18. Task 14
19. Task 15
20. Task 16
21. Checkpoint: VM Lifecycle Running
22. Task 17
23. Task 18
24. Task 19
25. Task 20
26. Task 21
27. Checkpoint: UI Matches Backend Truth
28. Task 22
29. Task 23
30. Task 24
31. Task 25
32. Final Checkpoint: MVP-1 Ready
