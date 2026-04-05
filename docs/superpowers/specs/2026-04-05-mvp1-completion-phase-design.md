# CHV MVP-1 Completion Phase Design

**Date:** 2026-04-05  
**Status:** Draft - Pending Approval  
**Approach:** Fix-First (Critical fixes before features)  

## 1. Executive Summary

This design completes CHV MVP-1 by addressing critical production blockers and implementing missing MVP-1 specification items.

### Key Decisions

| Decision | Rationale |
|----------|-----------|
| Fix-First approach | Production stability before feature expansion |
| Full audit trail | Operations track everything (sync + async) for complete visibility |
| Serial console critical | Required for VM troubleshooting and MVP-1 spec compliance |
| WebSocket for console | Industry standard, works through browsers |

### Deliverables

1. **Critical Fixes** (Production blockers)
   - Database health check bug fix
   - CORS middleware for UI connectivity
   - Cloud-init ISO verification
   - Comprehensive operations/audit system

2. **MVP-1 Features** (Spec compliance)
   - VM serial console via WebSocket
   - Image import/download flow
   - OpenAPI/Swagger documentation

---

## 2. Critical Fixes (Phase 1)

### 2.1 Database Health Check Bug

**Problem:**  
Health check shows `database: unhealthy: can't scan into dest[2]: cannot scan inet (OID 869) in binary format into *string`

**Root Cause:**  
PostgreSQL's `inet` type cannot be scanned directly into `*string`. The health check queries node IP addresses using `last_seen_at` which returns `inet` type.

**Solution:**  
Change the scan target from `*string` to `sql.NullString` or handle the type properly.

**Files Modified:**
- `internal/store/store.go` - HealthCheck() method
- `internal/store/store_test.go` - Add health check test

**Implementation:**
```go
// Current (broken):
var lastSeen *string
err := row.Scan(&id, &hostname, &lastSeen, &status)

// Fixed:
var lastSeen sql.NullString
err := row.Scan(&id, &hostname, &lastSeen, &status)
// Convert to string pointer for response
var lastSeenPtr *string
if lastSeen.Valid {
    lastSeenPtr = &lastSeen.String
}
```

**Verification:**
- Health endpoint returns `database: ok`
- All existing tests pass

---

### 2.2 CORS Middleware

**Problem:**  
Vue.js UI running on `localhost:3000` cannot call API on `localhost:8081` due to browser CORS restrictions.

**Solution:**  
Add Chi CORS middleware with configurable origins.

**Configuration:**
```yaml
# configs/controller.yaml
cors:
  enabled: true
  allowed_origins:
    - "http://localhost:3000"
    - "http://localhost:5173"
  allowed_methods:
    - GET
    - POST
    - PUT
    - PATCH
    - DELETE
    - OPTIONS
  allowed_headers:
    - Authorization
    - Content-Type
  allow_credentials: true
  max_age: 300
```

**Environment Variables:**
- `CHV_CORS_ENABLED` - Enable/disable CORS (default: true)
- `CHV_CORS_ORIGINS` - Comma-separated list of origins

**Files Modified:**
- `internal/api/handler.go` - Add CORS middleware setup
- `internal/models/config.go` - Add CORS config struct
- `configs/controller.yaml` - Add CORS section

**Implementation:**
```go
import "github.com/go-chi/cors"

func (h *Handler) SetupRoutes() {
    if h.config.CORS.Enabled {
        h.router.Use(cors.Handler(cors.Options{
            AllowedOrigins:   h.config.CORS.AllowedOrigins,
            AllowedMethods:   h.config.CORS.AllowedMethods,
            AllowedHeaders:   h.config.CORS.AllowedHeaders,
            AllowCredentials: h.config.CORS.AllowCredentials,
            MaxAge:           h.config.CORS.MaxAge,
        }))
    }
    // ... rest of routes
}
```

**Verification:**
- UI can successfully call API from browser
- Preflight OPTIONS requests handled correctly
- Credentials (Authorization header) pass through

---

### 2.3 Cloud-init ISO Verification

**Problem:**  
Unclear if cloud-init ISO generation is fully implemented and integrated.

**Investigation Checklist:**
- [ ] Does `internal/cloudinit/iso.go` exist?
- [ ] Does it generate valid ISO9660 images?
- [ ] Is it integrated into agent's ProvisionVM flow?
- [ ] Does it create proper nocloud datasource?

**Expected Implementation:**
```go
// internal/cloudinit/iso.go
package cloudinit

import (
    "os"
    "os/exec"
    "path/filepath"
)

type ISOGenerator struct {
    dataDir string
}

func NewISOGenerator(dataDir string) *ISOGenerator {
    return &ISOGenerator{dataDir: dataDir}
}

// Generate creates a cloud-init ISO for a VM
func (g *ISOGenerator) Generate(vmID string, config Config) (string, error) {
    // Create temp directory for iso contents
    workDir := filepath.Join(g.dataDir, "tmp", vmID)
    if err := os.MkdirAll(workDir, 0755); err != nil {
        return "", err
    }
    defer os.RemoveAll(workDir)
    
    // Write user-data
    userDataPath := filepath.Join(workDir, "user-data")
    if err := os.WriteFile(userDataPath, []byte(config.UserData), 0644); err != nil {
        return "", err
    }
    
    // Write meta-data
    metaDataPath := filepath.Join(workDir, "meta-data")
    if err := os.WriteFile(metaDataPath, []byte(config.MetaData), 0644); err != nil {
        return "", err
    }
    
    // Write network-config (if provided)
    if config.NetworkConfig != "" {
        netConfigPath := filepath.Join(workDir, "network-config")
        if err := os.WriteFile(netConfigPath, []byte(config.NetworkConfig), 0644); err != nil {
            return "", err
        }
    }
    
    // Generate ISO using xorrisofs
    isoPath := filepath.Join(g.dataDir, "volumes", vmID+"-cloudinit.iso")
    cmd := exec.Command("xorrisofs",
        "-input-charset", "utf-8",
        "-o", isoPath,
        "-V", "cidata",
        "-J", "-R",
        workDir,
    )
    
    if output, err := cmd.CombinedOutput(); err != nil {
        return "", fmt.Errorf("xorrisofs failed: %w (output: %s)", err, output)
    }
    
    return isoPath, nil
}
```

**Files to Verify/Create:**
- `internal/cloudinit/iso.go` - ISO generation
- `internal/cloudinit/config.go` - Config structs
- `cmd/chv-agent/service.go` - Integration point

**Verification:**
- ISO file created at `/var/lib/chv/volumes/{vm-id}-cloudinit.iso`
- ISO mounts correctly with `mount -o loop`
- Contains `user-data`, `meta-data`, `network-config`
- Label is "cidata"

---

### 2.4 Comprehensive Operations & Audit System

**Problem:**  
`internal/operations/` is empty. No visibility into:
- Async operation progress
- Historical audit trail
- What happened when something fails

**Solution:**  
Full operations tracking for ALL API operations (sync and async) creating a complete audit trail.

#### 2.4.1 Database Schema

```sql
-- Operations table for audit trail
CREATE TABLE operations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Classification
    type VARCHAR(50) NOT NULL,           -- 'vm_create', 'vm_start', 'image_import', etc.
    category VARCHAR(20) NOT NULL,       -- 'sync', 'async'
    
    -- Status tracking
    status VARCHAR(20) NOT NULL,         -- 'pending', 'running', 'completed', 'failed', 'cancelled'
    status_message TEXT,                 -- Human-readable status
    
    -- Resource reference
    resource_type VARCHAR(50),           -- 'vm', 'image', 'node', 'network', 'storage_pool'
    resource_id UUID,                    -- Reference to the resource
    
    -- Actor tracking
    actor_type VARCHAR(20) NOT NULL,     -- 'user', 'system', 'scheduler', 'reconciler'
    actor_id VARCHAR(255),               -- User ID, system component name, etc.
    
    -- Execution context
    node_id UUID REFERENCES nodes(id) ON DELETE SET NULL,
    
    -- Payload and result
    request_payload JSONB,               -- The API request body
    result_payload JSONB,                -- Result data on completion
    error_details JSONB,                 -- Error info on failure
    
    -- Progress (for async operations)
    progress_percent INT DEFAULT 0 CHECK (progress_percent >= 0 AND progress_percent <= 100),
    progress_message TEXT,
    
    -- Timing
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX idx_operations_resource ON operations(resource_type, resource_id);
CREATE INDEX idx_operations_status ON operations(status) WHERE status IN ('pending', 'running');
CREATE INDEX idx_operations_created_at ON operations(created_at DESC);
CREATE INDEX idx_operations_type ON operations(type);
CREATE INDEX idx_operations_actor ON operations(actor_type, actor_id);

-- Operation logs for detailed step-by-step progress
CREATE TABLE operation_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation_id UUID NOT NULL REFERENCES operations(id) ON DELETE CASCADE,
    level VARCHAR(10) NOT NULL,          -- 'info', 'warning', 'error', 'debug'
    message TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_operation_logs_operation ON operation_logs(operation_id, created_at);
```

#### 2.4.2 API Endpoints

```go
// List operations with filtering
GET /api/v1/operations
Query params:
  - resource_type: vm, image, node, etc.
  - resource_id: specific resource
  - status: pending, running, completed, failed
  - type: vm_create, vm_start, etc.
  - actor_id: who initiated
  - from, to: time range
  - limit, offset: pagination

Response:
{
  "data": [
    {
      "id": "op-uuid",
      "type": "vm_create",
      "category": "async",
      "status": "completed",
      "status_message": "VM created and started successfully",
      "resource_type": "vm",
      "resource_id": "vm-uuid",
      "actor_type": "user",
      "actor_id": "user@example.com",
      "progress_percent": 100,
      "started_at": "2026-04-05T10:00:00Z",
      "completed_at": "2026-04-05T10:00:15Z",
      "created_at": "2026-04-05T10:00:00Z"
    }
  ],
  "pagination": {
    "total": 150,
    "limit": 20,
    "offset": 0
  }
}

// Get operation details
GET /api/v1/operations/:id
Response includes full payload, result, error details

// Get operation logs
GET /api/v1/operations/:id/logs
Response:
{
  "data": [
    {
      "id": "log-uuid",
      "level": "info",
      "message": "Creating VM volume...",
      "created_at": "2026-04-05T10:00:01Z"
    },
    {
      "id": "log-uuid",
      "level": "info",
      "message": "VM volume created (2.5s)",
      "created_at": "2026-04-05T10:00:04Z"
    }
  ]
}
```

#### 2.4.3 Integration Points

**Controller API Handlers:**
Every handler creates an operation record:

```go
func (h *VMHandler) CreateVM(w http.ResponseWriter, r *http.Request) {
    // Create operation record
    op := &models.Operation{
        Type:         models.OpVMCreate,
        Category:     models.OpCategoryAsync,
        Status:       models.OpStatusPending,
        ActorType:    models.ActorTypeUser,
        ActorID:      getUserID(r),
        RequestPayload: toJSON(req),
    }
    op, _ = h.opsStore.Create(r.Context(), op)
    
    // Pass operation ID to service layer
    vm, err := h.service.CreateVM(r.Context(), req, op.ID)
    
    // Update operation on completion
    if err != nil {
        h.opsStore.Fail(r.Context(), op.ID, err)
    } else {
        h.opsStore.Complete(r.Context(), op.ID, vm)
    }
}
```

**Agent gRPC Integration:**
Agent reports progress back to controller:

```protobuf
message OperationProgress {
    string operation_id = 1;
    int32 progress_percent = 2;
    string progress_message = 3;
    repeated OperationLog logs = 4;
}

message OperationLog {
    string level = 1;
    string message = 2;
    google.protobuf.Timestamp timestamp = 3;
}
```

**Reconciler Integration:**
Reconciler updates operations when converging state:

```go
// When reconciler takes action
op := operations.Start(ctx, models.OpVMStart, vm.ID)
err := agentClient.StartVM(ctx, nodeID, vm.ID)
if err != nil {
    operations.Fail(ctx, op.ID, err)
} else {
    operations.Complete(ctx, op.ID, nil)
}
```

#### 2.4.4 Files to Create/Modify

**New Files:**
- `internal/models/operation.go` - Operation model with constants
- `internal/store/operations.go` - Database operations
- `internal/operations/service.go` - Business logic
- `internal/api/operations.go` - HTTP handlers

**Modified Files:**
- `configs/schema.sql` - Add operations tables
- All API handlers - Integrate operation tracking
- `internal/pb/agent/agent.proto` - Add operation progress messages

---

## 3. MVP-1 Features (Phase 2)

### 3.1 VM Serial Console (WebSocket)

**Requirement:** MVP-1 Section 5 - "Serial console / console access path"

**Design:**
- WebSocket endpoint: `/api/v1/vms/:id/console`
- Proxy between browser and cloud-hypervisor API socket
- Authentication via token in WebSocket subprotocol or query param
- Support for both read and write (full console access)

**Architecture:**
```
Browser --WebSocket--> Controller --HTTP--> Cloud-Hypervisor API
                         |
                         |--Token validation
                         |--VM ownership check
```

**API:**
```
WS /api/v1/vms/:id/console?token=<jwt>

Messages (JSON):
Client -> Server: {"type": "input", "data": "base64-encoded-data"}
Server -> Client: {"type": "output", "data": "base64-encoded-data"}
Server -> Client: {"type": "error", "message": "..."}
Server -> Client: {"type": "status", "connected": true}
```

**Files:**
- `internal/api/console.go` - WebSocket handler
- `internal/hypervisor/console.go` - Console proxy to CH API
- `chv-ui/src/components/vms/VMConsole.vue` - UI component

**Security:**
- Token validation on connection
- Check user has access to VM
- Rate limiting: max 1 connection per VM per user
- Audit log all console sessions

---

### 3.2 Image Import Flow

**Requirement:** MVP-1 Section 6 - Cloud-image provisioning

**Current State:** Images must be pre-placed on nodes

**Target State:** API-driven image import with progress tracking

**API:**
```
POST /api/v1/images/import
{
  "name": "ubuntu-22.04",
  "source_url": "https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img",
  "source_format": "qcow2",
  "target_storage_pool_id": "pool-uuid",
  "architecture": "x86_64",
  "os_family": "ubuntu"
}

Response (202 Accepted):
{
  "operation_id": "op-uuid",
  "message": "Image import started",
  "status_url": "/api/v1/operations/op-uuid"
}
```

**Flow:**
1. API receives import request
2. Creates operation record (status: pending)
3. Queues import job (or starts goroutine)
4. Returns 202 with operation ID
5. Background process:
   - Download image from URL
   - Update operation progress (0-50%)
   - Convert qcow2 -> raw (if needed)
   - Update operation progress (50-100%)
   - Store in storage pool
   - Create image record
   - Mark operation completed

**Files:**
- `internal/images/importer.go` - Import logic
- `internal/images/downloader.go` - HTTP download with progress
- `internal/images/converter.go` - qemu-img convert wrapper

---

### 3.3 OpenAPI/Swagger Documentation

**Requirement:** Documentation - interactive API docs

**Tools:** `swaggo/swag` - Generates OpenAPI from Go annotations

**Implementation:**

1. **Add annotations to all handlers:**
```go
// CreateVM godoc
// @Summary      Create a new VM
// @Description  Creates a VM and schedules it on a node
// @Tags         vms
// @Accept       json
// @Produce      json
// @Param        vm  body      CreateVMRequest  true  "VM to create"
// @Success      201  {object}  VM
// @Failure      400  {object}  ErrorResponse
// @Failure      401  {object}  ErrorResponse
// @Failure      500  {object}  ErrorResponse
// @Router       /api/v1/vms [post]
func (h *VMHandler) CreateVM(w http.ResponseWriter, r *http.Request) { ... }
```

2. **Generate docs:**
```bash
go install github.com/swaggo/swag/cmd/swag@latest
swag init -g cmd/chv-controller/main.go
```

3. **Serve Swagger UI:**
```go
// Add route
h.router.Get("/swagger/*", httpSwagger.Handler(
    httpSwagger.URL("/swagger/doc.json"),
))
```

**Files Modified:**
- All handler files - Add godoc comments
- `cmd/chv-controller/main.go` - Add swag init
- `internal/api/handler.go` - Add swagger endpoint

---

## 4. Implementation Order

### Phase 1: Critical Fixes (Must have for production)

| # | Task | Est. Time | Dependencies |
|---|------|-----------|--------------|
| 1 | Fix database health check | 2h | None |
| 2 | Add CORS middleware | 3h | None |
| 3 | Verify cloud-init ISO | 4h | None |
| 4 | Operations/audit system | 8h | None |

**Phase 1 Total: ~2 days**

### Phase 2: MVP-1 Features (Spec compliance)

| # | Task | Est. Time | Dependencies |
|---|------|-----------|--------------|
| 5 | VM serial console | 6h | None |
| 6 | Image import flow | 6h | Operations system |
| 7 | OpenAPI/Swagger | 4h | None |

**Phase 2 Total: ~2 days**

---

## 5. Success Criteria

### Phase 1 Complete When:
- [ ] Health endpoint shows `database: ok`
- [ ] UI can successfully authenticate and call API
- [ ] Cloud-init ISOs are generated and VMs boot with config
- [ ] All API operations are tracked with audit trail
- [ ] Can query operation history for any resource

### Phase 2 Complete When:
- [ ] Can open VM console from UI and see boot messages
- [ ] Can import cloud image via API with progress tracking
- [ ] Swagger UI shows all endpoints with request/response schemas

---

## 6. Testing Strategy

### Unit Tests
- Operations store CRUD
- CORS middleware behavior
- ISO generation (mock xorrisofs)
- Console proxy (mock CH API)

### Integration Tests
- Full VM create with cloud-init
- Image import flow
- Console connection lifecycle

### Manual Verification
- Health check via `curl /health`
- UI login and VM operations
- Console access via browser
- Image import progress in UI

---

## 7. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Cloud-hypervisor console API changes | High | Abstract console interface, version check |
| Large image downloads timeout | Medium | Implement chunked download with resume |
| Operations table grows unbounded | Medium | Add retention policy (90 days default) |
| WebSocket connections leak | Medium | Implement connection limits and timeouts |

---

**Next Step:** Upon approval, create implementation plan with writing-plans skill.
