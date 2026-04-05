# Implementation Plan: CHV MVP-1 Completion Phase

**Date:** 2026-04-05  
**Design Reference:** `2026-04-05-mvp1-completion-phase-design.md`  
**Approach:** Fix-First (Critical fixes before features)

## Overview

This plan implements the remaining CHV MVP-1 requirements through 7 focused tasks grouped into 2 phases:

- **Phase 1:** Critical production fixes (DB health, CORS, cloud-init, operations)
- **Phase 2:** MVP-1 feature completion (serial console, image import, OpenAPI)

Each task is sized for a single focused implementation session with clear acceptance criteria.

## Architecture Decisions

- **Bottom-up implementation:** Database schema → Models → Store → API → Integration
- **Vertical slicing:** Each task delivers working, testable functionality
- **Fail fast:** High-risk tasks (CORS, operations schema) are early in the plan
- **Isolation:** Tasks minimize file overlap to reduce merge conflicts

## Task List

---

### Phase 1: Critical Fixes

#### Task 1: Fix Database Health Check Bug

**Description:** Fix the `inet` type scanning error in the health check that causes false "unhealthy" status for the database.

**Acceptance criteria:**
- [ ] Health endpoint returns `database: ok` instead of error
- [ ] Existing store tests still pass
- [ ] New test added for HealthCheck method

**Verification:**
- [ ] Run `curl http://localhost:8081/health | jq '.checks.database'` → returns `"ok"`
- [ ] Run `go test ./internal/store/... -v` → all pass
- [ ] Check code: `internal/store/store.go` uses `sql.NullString` or proper type

**Dependencies:** None

**Files touched:**
- `internal/store/store.go` - Fix HealthCheck method
- `internal/store/store_test.go` - Add HealthCheck test

**Estimated scope:** Small (1-2 files)

**Implementation notes:**
```go
// Change from: var lastSeen *string
// To: var lastSeen sql.NullString
// Handle Valid check when converting to response
```

---

#### Task 2: Add CORS Middleware

**Description:** Add Chi CORS middleware to allow the Vue.js UI to connect to the API from browser origins.

**Acceptance criteria:**
- [ ] CORS middleware added to API handler
- [ ] Configurable via `configs/controller.yaml`
- [ ] Defaults allow localhost:3000 and localhost:5173
- [ ] Environment variable `CHV_CORS_ORIGINS` overrides config
- [ ] Preflight OPTIONS requests handled correctly

**Verification:**
- [ ] Run `curl -H "Origin: http://localhost:3000" -I http://localhost:8081/api/v1/vms` → see `Access-Control-Allow-Origin`
- [ ] UI login succeeds from browser without CORS errors
- [ ] Check config: `configs/controller.yaml` has `cors:` section

**Dependencies:** None

**Files touched:**
- `internal/api/handler.go` - Add CORS middleware setup
- `internal/models/config.go` - Add CORS config struct
- `configs/controller.yaml` - Add CORS configuration
- `configs/controller.Dockerfile` - Document CORS env vars (if needed)

**Estimated scope:** Small (2-3 files)

---

#### Task 3: Verify Cloud-init ISO Generation

**Description:** Verify cloud-init ISO generation exists and works, or implement it if missing. This is critical for VM provisioning.

**Acceptance criteria:**
- [ ] Check if `internal/cloudinit/iso.go` exists and is functional
- [ ] If missing, implement ISO generation using `xorrisofs`
- [ ] Integration with agent's VM provisioning confirmed
- [ ] ISO contains user-data, meta-data, network-config
- [ ] ISO has label "cidata"

**Verification:**
- [ ] File exists: `ls internal/cloudinit/iso.go`
- [ ] If implemented: Generate test ISO and verify with `isoinfo -d -i test.iso`
- [ ] Check label: `isoinfo -d -i test.iso | grep "Volume id"` → "cidata"
- [ ] Integration check: Agent code calls cloudinit.Generate()

**Dependencies:** None

**Files touched (if implementing):**
- `internal/cloudinit/iso.go` - Create ISO generation
- `internal/cloudinit/config.go` - Config structs
- `internal/cloudinit/iso_test.go` - Unit tests
- `cmd/chv-agent/service.go` - Integration point

**Estimated scope:** Medium (3-4 files, if implementing)

**Decision point:** First check if files exist. If they do and work, this task is just verification. If missing, implement.

---

#### Task 4: Operations and Audit System - Database Schema

**Description:** Create the database schema for operations tracking and audit trail.

**Acceptance criteria:**
- [ ] `operations` table created with all columns
- [ ] `operation_logs` table created
- [ ] All indexes defined for common queries
- [ ] Foreign key constraints properly set
- [ ] Migration applied to existing databases

**Verification:**
- [ ] Run `psql -h localhost -p 5433 -U chv -c "\dt"` → see `operations` and `operation_logs`
- [ ] Run `psql -h localhost -p 5433 -U chv -c "\d operations"` → verify columns
- [ ] Check indexes: `psql ... -c "SELECT indexname FROM pg_indexes WHERE tablename = 'operations';"`
- [ ] Run `docker compose restart controller` → no migration errors

**Dependencies:** None

**Files touched:**
- `configs/schema.sql` - Add operations tables

**Estimated scope:** Small (1 file)

---

#### Task 5: Operations and Audit System - Models and Store

**Description:** Create Go models and database store methods for operations.

**Acceptance criteria:**
- [ ] Operation model with constants for types, statuses, categories
- [ ] Operation store with CRUD methods
- [ ] Operation log store methods
- [ ] Unit tests for store methods

**Verification:**
- [ ] Run `go test ./internal/store/... -v -run Operation` → all pass
- [ ] Code review: `internal/models/operation.go` has all type constants
- [ ] Code review: `internal/store/operations.go` has Create, Get, List, Update methods

**Dependencies:** Task 4 (database schema)

**Files touched:**
- `internal/models/operation.go` - Operation model
- `internal/store/operations.go` - Store methods
- `internal/store/operations_test.go` - Unit tests

**Estimated scope:** Medium (3 files)

---

#### Task 6: Operations and Audit System - API Integration

**Description:** Integrate operations tracking into API handlers for complete audit trail.

**Acceptance criteria:**
- [ ] Operations API endpoints: `GET /api/v1/operations`, `GET /api/v1/operations/:id`, `GET /api/v1/operations/:id/logs`
- [ ] All VM handlers create operation records
- [ ] All Node handlers create operation records
- [ ] Operations service for business logic

**Verification:**
- [ ] Run `curl http://localhost:8081/api/v1/operations` → returns list
- [ ] Create VM via API → check operation created
- [ ] Run `curl http://localhost:8081/api/v1/operations/:id` → returns details
- [ ] Run `go test ./internal/api/... -v` → all pass

**Dependencies:** Task 5 (models and store)

**Files touched:**
- `internal/operations/service.go` - Business logic
- `internal/api/operations.go` - HTTP handlers
- `internal/api/vms.go` - Integrate operation tracking
- `internal/api/nodes.go` - Integrate operation tracking
- `internal/api/handler.go` - Wire up operations routes

**Estimated scope:** Medium (4-5 files)

---

### Checkpoint: Phase 1 Complete

**Verify before proceeding:**
- [ ] All tests pass: `go test ./...`
- [ ] Health check shows `database: ok`
- [ ] UI can authenticate and call API (CORS working)
- [ ] Operations endpoints return data
- [ ] Build succeeds: `make build`

---

### Phase 2: MVP-1 Features

#### Task 7: VM Serial Console via WebSocket

**Description:** Implement WebSocket endpoint for VM serial console access, proxying to cloud-hypervisor's API socket.

**Acceptance criteria:**
- [ ] WebSocket endpoint `/api/v1/vms/:id/console` implemented
- [ ] Token authentication on WebSocket connection
- [ ] Proxy to cloud-hypervisor API socket
- [ ] Message protocol defined (input/output/status)
- [ ] Rate limiting: max 1 connection per VM per user
- [ ] Audit log entry for each console session

**Verification:**
- [ ] Test with `wscat -c "ws://localhost:8081/api/v1/vms/:id/console?token=..."`
- [ ] Verify connection rejected without valid token
- [ ] Verify only 1 connection per VM allowed
- [ ] Check audit: operation record created for console session
- [ ] Run `go test ./internal/api/... -v -run Console` → all pass

**Dependencies:** Task 6 (operations system for audit)

**Files touched:**
- `internal/api/console.go` - WebSocket handler
- `internal/hypervisor/console.go` - Console proxy
- `internal/api/handler.go` - Wire up console route

**Estimated scope:** Medium (3 files)

---

#### Task 8: Image Import Flow

**Description:** Implement async image import with progress tracking via operations.

**Acceptance criteria:**
- [ ] `POST /api/v1/images/import` endpoint (async, returns operation ID)
- [ ] Image download with progress updates
- [ ] Format conversion (qcow2 → raw)
- [ ] Progress tracked via operations API
- [ ] Support resume for interrupted downloads
- [ ] Storage in specified storage pool

**Verification:**
- [ ] Call import API → get 202 Accepted with operation ID
- [ ] Poll operation endpoint → see progress increasing
- [ ] Verify image file created in storage pool
- [ ] Verify image record created in database
- [ ] Run `go test ./internal/images/... -v` → all pass

**Dependencies:** Task 6 (operations system)

**Files touched:**
- `internal/images/importer.go` - Import orchestration
- `internal/images/downloader.go` - HTTP download with progress
- `internal/images/converter.go` - qemu-img wrapper
- `internal/api/images.go` - Add import endpoint

**Estimated scope:** Medium (4 files)

---

#### Task 9: OpenAPI/Swagger Documentation

**Description:** Add OpenAPI annotations and serve Swagger UI for interactive API documentation.

**Acceptance criteria:**
- [ ] `swaggo/swag` annotations added to all handlers
- [ ] `docs/swagger.json` and `docs/swagger.yaml` generated
- [ ] Swagger UI served at `/swagger/index.html`
- [ ] All endpoints documented with request/response schemas
- [ ] Error responses documented

**Verification:**
- [ ] Open `http://localhost:8081/swagger/index.html` → UI loads
- [ ] Verify all endpoints listed
- [ ] Test "Try it out" feature with auth token
- [ ] Run `make build` → includes swagger docs

**Dependencies:** None (can run parallel to 7,8 after checkpoint)

**Files touched:**
- `cmd/chv-controller/main.go` - Add swag init comment
- `internal/api/handler.go` - Add swagger endpoint
- All handler files (`vms.go`, `nodes.go`, etc.) - Add godoc comments
- `docs/swagger.yaml` - Generated

**Estimated scope:** Medium (5+ files, mostly annotations)

---

### Checkpoint: Phase 2 Complete

**Final verification:**
- [ ] All tests pass: `go test ./...`
- [ ] UI console access works
- [ ] Image import works end-to-end
- [ ] Swagger UI accessible and functional
- [ ] Full build: `make build && make docker-up`

---

## Parallelization Opportunities

**Sequential (must be in order):**
- Task 4 → Task 5 → Task 6 (operations stack)

**Can parallelize after Phase 1 checkpoint:**
- Task 7 (console) and Task 8 (image import) can run in parallel
- Task 9 (swagger) can run parallel to 7,8 (no dependencies)

**Suggested parallel execution:**
```
Phase 1: Tasks 1-6 (sequential)
Checkpoint
Phase 2: Tasks 7,8,9 (parallel if multiple agents available)
```

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Cloud-hypervisor console API differs from docs | High | Check actual CH version, adapt proxy code |
| Large image downloads timeout/crash | Medium | Implement chunked download, resume capability |
| Operations table grows too large | Medium | Add 90-day retention policy (future task) |
| WebSocket connection leaks | Medium | Add connection limits, timeouts, proper cleanup |
| Swag annotations break build | Low | Test generation in CI, pin swag version |

---

## Open Questions

None - design approved.

---

## Summary

| Phase | Tasks | Est. Time | Key Deliverable |
|-------|-------|-----------|-----------------|
| Phase 1 | 1-6 | 2 days | Production-ready core (health, CORS, audit) |
| Phase 2 | 7-9 | 2 days | MVP-1 spec compliance |
| **Total** | **9 tasks** | **~4 days** | **Complete CHV MVP-1** |
