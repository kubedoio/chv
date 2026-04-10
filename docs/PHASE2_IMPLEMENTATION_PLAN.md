# CHV Phase 2 Implementation Plan

**Phase:** Production Readiness Features  
**Duration:** 3-4 weeks  
**Goal:** Transform MVP into production-ready platform with remote node support, security, and enhanced VM management

---

## Overview

Phase 2 focuses on four critical areas that transform CHV from a single-node MVP into a production-ready multi-node virtualization platform:

1. **Remote Node Management** - Register, manage, and monitor remote hypervisor nodes
2. **Node Health Monitoring** - Heartbeats, health checks, and automatic failover
3. **VM Power Actions** - Graceful shutdown, force stop, reset with proper state management
4. **RBAC System** - Role-based access control with users, roles, and permissions

---

## Phase 2A: Remote Node Management (Week 1)

### Goals
- Register new remote nodes via API
- Node-agent communication with authentication
- Node status tracking and display

### Implementation Tasks

#### Backend (Go)

**1. Node Registration API** (`internal/api/nodes.go`)
```
POST /api/v1/nodes
{
  "name": "hypervisor-02",
  "hostname": "hv02.example.com",
  "ip_address": "10.0.1.10",
  "agent_url": "http://10.0.1.10:9090"
}
```
- Generate agent token for authentication
- Store node in database with `pending` status
- Return registration credentials

**2. Agent Authentication** (`internal/api/auth.go`)
- Validate agent tokens on agent endpoints
- Separate middleware for agent vs user tokens
- Token refresh mechanism

**3. Agent Registration Flow** (`internal/agent/services/registration.go`)
- Agent startup registers with controller
- Heartbeat endpoint for status updates
- Graceful shutdown notification

**4. Node Status Tracking** (`internal/db/sqlite.go`)
- `UpdateNodeLastSeen()` - heartbeat timestamp
- `UpdateNodeStatus()` - online/offline/maintenance/error
- Background goroutine to mark stale nodes offline

#### Frontend (Svelte)

**1. Node Management Page** (`ui/src/routes/nodes/+page.svelte`)
- List all nodes with status indicators
- Add node modal with registration form
- Node actions: edit, delete, set maintenance mode

**2. Node Detail Page Updates** (`ui/src/routes/nodes/[id]/+page.svelte`)
- Show node connection status
- Last seen timestamp
- Agent version and capabilities

**3. API Client Updates** (`ui/src/lib/api/client.ts`)
```typescript
createNode(data: NodeInput)
updateNode(id: string, data: Partial<NodeInput>)
deleteNode(id: string)
setNodeMaintenance(id: string, enabled: boolean)
```

---

## Phase 2B: Node Health Monitoring (Week 1-2)

### Goals
- Continuous health monitoring of all nodes
- Automatic detection of node failures
- Resource utilization metrics per node

### Implementation Tasks

#### Backend (Go)

**1. Heartbeat System** (`internal/health/heartbeat.go`)
```go
type HeartbeatService struct {
    interval time.Duration
    timeout  time.Duration
    repo     *db.Repository
}

func (s *HeartbeatService) Start()
func (s *HeartbeatService) RecordHeartbeat(nodeID string)
func (s *HeartbeatService) CheckNodeHealth()
```

**2. Agent Heartbeat Endpoint** (`internal/api/agent.go`)
```
POST /api/v1/agents/heartbeat
Authorization: Bearer {agent_token}
{
  "node_id": "node-123",
  "timestamp": "2026-04-10T10:00:00Z",
  "resources": {
    "cpu_percent": 45.2,
    "memory_used_mb": 8192,
    "memory_total_mb": 16384,
    "disk_used_gb": 250,
    "disk_total_gb": 500
  }
}
```

**3. Health Check Aggregation** (`internal/health/checker.go`)
- Check all nodes every 30 seconds
- Mark nodes offline after 2 missed heartbeats
- Alert on node status changes (logging, future: webhooks)

**4. Node Resource Metrics** (`internal/db/sqlite.go`)
```sql
CREATE TABLE node_metrics (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    cpu_percent REAL,
    memory_used_mb INTEGER,
    memory_total_mb INTEGER,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id)
);
```

#### Frontend (Svelte)

**1. Node Health Dashboard** (`ui/src/lib/components/NodeHealthStatus.svelte`)
- Real-time status indicators
- CPU/Memory/Disk utilization bars
- Last heartbeat timestamp

**2. Health Alerts** (`ui/src/lib/components/HealthAlerts.svelte`)
- Toast notifications for node offline/online
- Persistent alerts for maintenance mode

**3. Node List Updates**
- Color-coded status (green=online, red=offline, yellow=maintenance)
- Resource utilization sparklines
- Quick health summary

---

## Phase 2C: VM Power Actions (Week 2)

### Goals
- Graceful shutdown with timeout
- Force stop for hung VMs
- Reset/restart actions
- Boot logs capture

### Implementation Tasks

#### Backend (Go)

**1. Graceful Shutdown** (`internal/vm/service.go`)
```go
func (s *Service) ShutdownVM(ctx context.Context, vmID string, timeout time.Duration) error
```
- Send ACPI shutdown signal via agent
- Wait for VM to reach stopped state
- Force stop if timeout exceeded

**2. Force Stop** (`internal/vm/service.go`)
```go
func (s *Service) ForceStopVM(ctx context.Context, vmID string) error
```
- Kill Cloud Hypervisor process immediately
- Update VM state to stopped

**3. Reset/Restart** (`internal/vm/service.go`)
```go
func (s *Service) ResetVM(ctx context.Context, vmID string) error
func (s *Service) RestartVM(ctx context.Context, vmID string, graceful bool) error
```

**4. API Endpoints** (`internal/api/vms.go`)
```
POST /api/v1/vms/{id}/shutdown?timeout=60
POST /api/v1/vms/{id}/force-stop
POST /api/v1/vms/{id}/reset
POST /api/v1/vms/{id}/restart?graceful=true
```

**5. Boot Logs** (`internal/vm/logs.go`)
- Capture serial console output during boot
- Store in database or files
- API endpoint to retrieve logs
```
GET /api/v1/vms/{id}/boot-logs
```

#### Frontend (Svelte)

**1. VM Power Menu** (`ui/src/lib/components/VMPowerMenu.svelte`)
- Dropdown with power actions
- Confirmation dialogs for destructive actions
- Visual feedback for in-progress actions

**2. Boot Logs Viewer** (`ui/src/lib/components/BootLogViewer.svelte`)
- Scrollable log display
- Auto-refresh during boot
- Download logs button

**3. VM List Actions** (`ui/src/routes/vms/+page.svelte`)
- Quick action buttons (start, stop, restart)
- Progress indicators for shutdown operations
- Status tooltips showing detailed state

---

## Phase 2D: RBAC System (Week 3-4)

### Goals
- User roles (admin, operator, viewer)
- Resource-level permissions
- Audit logging of all actions

### Implementation Tasks

#### Backend (Go)

**1. Role Model** (`internal/models/models.go`)
```go
type Role struct {
    ID          string
    Name        string
    Permissions []Permission
}

type Permission struct {
    Resource string // "vms", "images", "nodes", etc.
    Action   string // "create", "read", "update", "delete", "*"
}

const (
    RoleAdmin     = "admin"
    RoleOperator  = "operator"
    RoleViewer    = "viewer"
)
```

**2. Permission Middleware** (`internal/api/auth.go`)
```go
func (h *Handler) requirePermission(resource, action string) Middleware
```
- Check user role against required permission
- Return 403 Forbidden if insufficient permissions

**3. User Management API** (`internal/api/users.go`)
```
GET    /api/v1/users
POST   /api/v1/users
GET    /api/v1/users/{id}
PATCH  /api/v1/users/{id}
DELETE /api/v1/users/{id}
POST   /api/v1/users/{id}/reset-password
```

**4. Audit Logging** (`internal/audit/logger.go`)
```go
type AuditLog struct {
    ID          string
    Timestamp   time.Time
    UserID      string
    UserName    string
    Action      string
    Resource    string
    ResourceID  string
    Details     string
    IPAddress   string
    Success     bool
    Error       string
}
```
- Log all API actions
- Background worker to write logs
- API endpoint to query logs (admin only)

**5. Database Schema Updates** (`configs/schema_sqlite.sql`)
```sql
CREATE TABLE roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL, -- JSON array
    created_at TEXT NOT NULL
);

CREATE TABLE audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_name TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    details TEXT,
    ip_address TEXT,
    success INTEGER NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX idx_audit_logs_created ON audit_logs(created_at);
```

#### Frontend (Svelte)

**1. User Management Page** (`ui/src/routes/users/+page.svelte`)
- List all users
- Create/edit user modal
- Role assignment dropdown
- Disable/enable user accounts

**2. Audit Log Viewer** (`ui/src/routes/audit-logs/+page.svelte`)
- Filter by user, resource type, date range
- Search by action or resource ID
- Export to CSV

**3. Permission Guards** (`ui/src/lib/auth/permissions.ts`)
```typescript
export function canCreateVM(user: User): boolean
export function canDeleteVM(user: User): boolean
export function canManageUsers(user: User): boolean
```

**4. UI Permission Hiding**
- Hide action buttons user can't perform
- Show "read-only" indicator for viewers
- Disable inputs based on permissions

---

## API Changes Summary

### New Endpoints

```
# Nodes
POST   /api/v1/nodes
PATCH  /api/v1/nodes/{id}
DELETE /api/v1/nodes/{id}
POST   /api/v1/nodes/{id}/maintenance

# Agent
POST   /api/v1/agents/heartbeat
POST   /api/v1/agents/register

# VM Power Actions
POST   /api/v1/vms/{id}/shutdown
POST   /api/v1/vms/{id}/force-stop
POST   /api/v1/vms/{id}/reset
POST   /api/v1/vms/{id}/restart
GET    /api/v1/vms/{id}/boot-logs

# Users
GET    /api/v1/users
POST   /api/v1/users
GET    /api/v1/users/{id}
PATCH  /api/v1/users/{id}
DELETE /api/v1/users/{id}

# Audit Logs
GET    /api/v1/audit-logs
```

---

## Database Migrations

### Migration 1: Node Improvements
```sql
-- Add columns for remote node support
ALTER TABLE nodes ADD COLUMN agent_token TEXT;
ALTER TABLE nodes ADD COLUMN capabilities TEXT; -- JSON

-- Create node_metrics table
CREATE TABLE node_metrics (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    cpu_percent REAL,
    memory_used_mb INTEGER,
    memory_total_mb INTEGER,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id)
);
```

### Migration 2: RBAC
```sql
-- Roles table
CREATE TABLE roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Insert default roles
INSERT INTO roles (id, name, permissions, created_at) VALUES
('role-admin', 'admin', '[{"resource": "*", "action": "*}]', datetime('now')),
('role-operator', 'operator', '[{"resource": "vms", "action": "*"}, {"resource": "images", "action": "*"}, ...]', datetime('now')),
('role-viewer', 'viewer', '[{"resource": "*", "action": "read"}]', datetime('now'));

-- Add role to users
ALTER TABLE users ADD COLUMN role_id TEXT DEFAULT 'role-viewer';

-- Audit logs
CREATE TABLE audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_name TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    details TEXT,
    ip_address TEXT,
    success INTEGER NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL
);
```

---

## Testing Strategy

### Unit Tests
- Node registration validation
- Permission checking logic
- VM state transitions
- Audit log formatting

### Integration Tests
- Node-agent communication flow
- VM power action sequences
- RBAC middleware
- Audit log recording

### E2E Tests (Playwright)
- Register new node flow
- VM shutdown/restart actions
- User creation and role assignment
- Audit log viewing

---

## Success Criteria

### Phase 2A Complete
- [ ] Can register new remote node via API
- [ ] Agent authenticates with token
- [ ] Node status updates in real-time
- [ ] UI shows all nodes with correct status

### Phase 2B Complete
- [ ] Heartbeats recorded every 30 seconds
- [ ] Nodes marked offline after timeout
- [ ] Resource metrics displayed in UI
- [ ] Health alerts shown for node failures

### Phase 2C Complete
- [ ] Graceful shutdown works with timeout
- [ ] Force stop immediately kills VM
- [ ] Boot logs captured and viewable
- [ ] Power actions have confirmation dialogs

### Phase 2D Complete
- [ ] Users can be created with roles
- [ ] Permissions enforced on all endpoints
- [ ] All actions logged to audit log
- [ ] UI hides actions user can't perform

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Agent token security | Store hashed tokens, use JWT with short expiry |
| Node split-brain | Implement leader election for controller HA |
| VM state inconsistency | Reconciliation loop runs every 30s |
| Audit log performance | Async logging with buffer, background flush |
| RBAC complexity | Start with 3 simple roles, expand later |

---

## Dependencies

### New Go Dependencies
```go
// JWT for agent tokens
github.com/golang-jwt/jwt/v5

// Rate limiting
github.com/go-chi/httprate
```

### No New Frontend Dependencies
- Use existing Svelte/TypeScript stack
- Leverage existing modal, toast, form components
