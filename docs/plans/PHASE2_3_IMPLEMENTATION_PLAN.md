# Implementation Plan: Phase 2 & 3 - Agent Contract and Image Import

## Overview

Implement the host-native agent contract (Phase 2) and image import workflow (Phase 3) for CHV MVP-1. This establishes the controller-agent boundary and enables qcow2 image import functionality.

**Phase 2 Goal:** Define and implement the controller-agent contract, moving host-side operations into the agent binary.

**Phase 3 Goal:** Implement qcow2 image import with URL fetch, checksum validation, and state management.

---

## Current State Assessment

**Existing:**
- Controller with SQLite repository
- Bootstrap and install status APIs
- Basic agent binary scaffold (main.go exists)
- Network and storage management in controller
- UI with create flows for networks and storage

**Missing:**
- Controller-agent communication contract
- Agent HTTP server surface
- Image import service and API
- Operations logging to database
- Checksum validation

---

## Architecture Decisions

1. **Agent as HTTP Server** - Agent runs HTTP server on localhost (configurable port), controller makes HTTP calls
2. **Unix Domain Socket Option** - Support UDS for single-host deployments (security + performance)
3. **JSON Request/Response** - Simple, debuggable format
4. **Controller Orchestrates, Agent Executes** - Controller decides what to do, agent performs host mutations
5. **Operations as Audit Log** - Every agent action creates an operations row

---

## Task List

### Phase 2: Agent Contract

#### Task 1: Define Agent Contract Types
**Description:** Create the shared types package defining controller-agent request/response models.

**Acceptance criteria:**
- [ ] Agent request types for: InstallCheck, Bootstrap, Repair, ImageImport, ImageValidate
- [ ] Agent response types with structured results and errors
- [ ] Common error type with code, message, retryable flag
- [ ] HTTP transport types (optional auth, headers)

**Files to create:**
- `internal/agentapi/types.go` - Shared types between controller and agent

**Verification:**
- [ ] `go build ./...` passes
- [ ] Types used by both controller and agent compile

**Dependencies:** None

**Scope:** Small

---

#### Task 2: Create Agent HTTP Server
**Description:** Implement the agent's HTTP server surface with routing and middleware.

**Acceptance criteria:**
- [ ] Agent starts HTTP server on configurable address (default: `:9090`)
- [ ] Health endpoint: GET /health
- [ ] Chi router with JSON middleware
- [ ] Structured logging
- [ ] Graceful shutdown on SIGTERM

**Files to create/modify:**
- `cmd/chv-agent/main.go` - HTTP server setup
- `internal/agent/server.go` - Server structure and routing

**Verification:**
- [ ] `go run ./cmd/chv-agent` starts server
- [ ] `curl http://localhost:9090/health` returns `{"ok": true}`

**Dependencies:** Task 1

**Scope:** Medium

---

#### Task 3: Implement Agent Install Check Handler
**Description:** Agent endpoint for install status checks (bridge, directories, prerequisites).

**Acceptance criteria:**
- [ ] GET /v1/install/check returns structured install status
- [ ] Checks bridge existence, IP assignment, up state
- [ ] Checks data root directories exist
- [ ] Finds cloud-hypervisor binary
- [ ] Finds cloud-init tools (xorrisofs/mkisofs/genisoimage)
- [ ] Returns same format as controller's install/status

**Files to create:**
- `internal/agent/handlers/install.go` - Install check handler
- `internal/agent/services/install.go` - Install check logic (moved from controller)

**Verification:**
- [ ] `curl http://localhost:9090/v1/install/check` returns valid JSON
- [ ] Tests pass: `go test ./internal/agent/...`

**Dependencies:** Task 2

**Scope:** Medium

---

#### Task 4: Implement Agent Bootstrap Handler
**Description:** Agent endpoint for bootstrap actions (directory creation, bridge setup).

**Acceptance criteria:**
- [ ] POST /v1/install/bootstrap creates directories
- [ ] Creates bridge if missing
- [ ] Assigns IP to bridge
- [ ] Returns actions taken
- [ ] Idempotent (safe to run multiple times)

**Files to create:**
- `internal/agent/handlers/bootstrap.go` - Bootstrap handler
- `internal/agent/services/bootstrap.go` - Bootstrap logic

**Verification:**
- [ ] POST to endpoint creates directories
- [ ] Second POST is idempotent
- [ ] Tests pass

**Dependencies:** Task 3

**Scope:** Medium

---

#### Task 5: Create Agent Client in Controller
**Description:** Controller-side client for making calls to agent.

**Acceptance criteria:**
- [ ] AgentClient struct with configurable base URL
- [ ] Methods: CheckInstall(), Bootstrap(), Repair()
- [ ] Timeout handling (default 30s)
- [ ] Error translation (HTTP errors → typed errors)
- [ ] Retry logic for transient failures

**Files to create:**
- `internal/agentclient/client.go` - HTTP client for agent

**Verification:**
- [ ] Client can connect to running agent
- [ ] Tests with mock agent server

**Dependencies:** Task 2

**Scope:** Medium

---

#### Task 6: Update Controller to Use Agent Client
**Description:** Modify controller bootstrap service to delegate to agent instead of direct execution.

**Acceptance criteria:**
- [ ] Bootstrap service uses AgentClient
- [ ] Install check delegates to agent
- [ ] Repair delegates to agent
- [ ] Fallback to direct execution if agent unavailable (configurable)
- [ ] All existing tests pass

**Files to modify:**
- `internal/bootstrap/service.go` - Use agent client
- `internal/api/install.go` - No changes needed (API stable)

**Verification:**
- [ ] `go test ./internal/bootstrap/...` passes
- [ ] `go test ./internal/api/...` passes
- [ ] Controller delegates to agent when available

**Dependencies:** Tasks 4, 5

**Scope:** Large

---

### Checkpoint: Phase 2
- [ ] Agent server runs independently
- [ ] Controller delegates host operations to agent
- [ ] Install APIs work through agent
- [ ] All tests pass

---

### Phase 3: Image Import

#### Task 7: Extend Image Repository Methods
**Description:** Add CRUD operations for images table.

**Acceptance criteria:**
- [ ] CreateImage(ctx, image) - creates image record
- [ ] GetImageByID(ctx, id) - fetch single image
- [ ] UpdateImage(ctx, image) - update status, path, etc
- [ ] ListImages(ctx) - already exists, verify complete
- [ ] DeleteImage(ctx, id) - soft delete

**Files to modify:**
- `internal/db/sqlite.go` - Add repository methods

**Verification:**
- [ ] `go test ./internal/db/...` passes

**Dependencies:** None

**Scope:** Small

---

#### Task 8: Create Image Import Service
**Description:** Service layer for image import workflow with state machine.

**Acceptance criteria:**
- [ ] ImageImportService struct
- [ ] ImportImage(ctx, input) - initiates import, creates "importing" record
- [ ] State machine: importing → validating → ready/failed
- [ ] Support URL fetch (HTTP/HTTPS)
- [ ] Store images in `/var/lib/chv/images/`
- [ ] Generate unique filename based on image ID

**Files to create:**
- `internal/images/service.go` - Import service
- `internal/images/fetcher.go` - HTTP fetch logic

**Verification:**
- [ ] Unit tests for service
- [ ] `go test ./internal/images/...` passes

**Dependencies:** Task 7

**Scope:** Large

---

#### Task 9: Implement Agent Image Download Handler
**Description:** Agent endpoint for downloading images from URLs.

**Acceptance criteria:**
- [ ] POST /v1/images/download accepts URL and destination path
- [ ] Streams download to disk (memory efficient)
- [ ] Progress tracking (bytes downloaded)
- [ ] Timeout handling (large files)
- [ ] Returns final path and size
- [ ] Resume support (optional for MVP)

**Files to create:**
- `internal/agent/handlers/images.go` - Image handlers
- `internal/agent/services/imagedownload.go` - Download service

**Verification:**
- [ ] Can download test file from URL
- [ ] Progress reported
- [ ] Tests pass

**Dependencies:** Task 2

**Scope:** Large

---

#### Task 10: Implement Checksum Validation
**Description:** SHA256 checksum validation for downloaded images.

**Acceptance criteria:**
- [ ] Parse `sha256:abc123...` format
- [ ] Stream calculate SHA256 while downloading (or after)
- [ ] Compare with expected checksum
- [ ] Mark image failed on mismatch
- [ ] Error message includes "checksum mismatch"

**Files to create:**
- `internal/images/checksum.go` - Checksum validation

**Verification:**
- [ ] Valid checksum passes
- [ ] Invalid checksum fails with clear error
- [ ] Tests pass

**Dependencies:** Task 8

**Scope:** Medium

---

#### Task 11: Add Image Import API Endpoint
**Description:** Controller API endpoint for image import.

**Acceptance criteria:**
- [ ] POST /api/v1/images/import accepts import request
- [ ] Request body: `{ "name": "...", "source_url": "...", "checksum": "sha256:..." }`
- [ ] Returns image record with "importing" status
- [ ] Delegates download to agent
- [ ] Updates image status on completion/failure
- [ ] Auth required

**Files to create/modify:**
- `internal/api/images.go` - Image handlers (new file)
- `internal/api/handler.go` - Register route

**Verification:**
- [ ] `go test ./internal/api/...` passes
- [ ] API returns correct response

**Dependencies:** Tasks 8, 9, 10

**Scope:** Medium

---

#### Task 12: Add Operations Logging
**Description:** Log all image import operations to database.

**Acceptance criteria:**
- [ ] CreateOperation(ctx, op) repository method
- [ ] Operation created on import start (state: pending)
- [ ] Operation updated on progress (state: running)
- [ ] Operation updated on completion (state: completed) or failure (state: failed)
- [ ] Error payload captured on failure
- [ ] Visible in operations list API

**Files to modify:**
- `internal/db/sqlite.go` - Add CreateOperation, UpdateOperation
- `internal/operations/service.go` - Operations service
- `internal/images/service.go` - Log operations

**Verification:**
- [ ] Operations appear in API
- [ ] State transitions correct
- [ ] Tests pass

**Dependencies:** Task 11

**Scope:** Medium

---

### Checkpoint: Phase 3
- [ ] Image import API works end-to-end
- [ ] URL download functional
- [ ] Checksum validation works
- [ ] Operations logged
- [ ] All tests pass

---

## Final Verification

### Build & Test
```bash
/usr/local/go/bin/go test ./...
/usr/local/go/bin/go build ./cmd/chv-controller ./cmd/chv-agent
cd ui && npm run build
```

### E2E Test Scenarios
1. **Agent Communication:** Start agent → Controller delegates install check → Returns status
2. **Bootstrap via Agent:** POST /install/bootstrap → Agent creates dirs/bridge → Status updated
3. **Image Import:** POST /images/import → Agent downloads → Checksum validates → Status "ready"
4. **Failed Import:** POST with invalid checksum → Status "failed" → Error logged
5. **Operations List:** Operations API shows import history with states

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Large file downloads timeout | High | Implement streaming, progress tracking, configurable timeout |
| Agent-controller version mismatch | Medium | Version handshake, backward compatibility in contract |
| Checksum calculation slow for large files | Low | Stream calculate during download, don't re-read |
| Disk space during download | High | Check available space before download, cleanup on failure |

---

## Open Questions

1. **Agent authentication?** - For MVP, localhost-only. Future: mTLS or token auth.
2. **Concurrent imports?** - Queue or allow parallel? Recommend: queue for MVP.
3. **Resume partial downloads?** - HTTP Range support? Defer to post-MVP.

---

## Timeline Estimate

| Phase | Tasks | Est. Effort |
|-------|-------|-------------|
| Phase 2: Agent Contract | 6 | 6 sessions |
| Phase 3: Image Import | 6 | 6 sessions |
| **Total** | **12** | **12 sessions** |
