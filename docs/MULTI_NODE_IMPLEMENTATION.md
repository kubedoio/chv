# Multi-Node Support Implementation

## Overview

This document describes the implementation of multi-node support in CHV MVP-1. While MVP-1 operates as a single-node deployment, the infrastructure is now in place to support multiple nodes in future versions.

## Implementation Summary

### Database Schema Changes

#### New `nodes` Table
```sql
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    hostname TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    is_local INTEGER NOT NULL DEFAULT 1,
    agent_url TEXT NULL,
    agent_token TEXT NULL,
    last_seen_at TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

**Fields:**
- `id` - Unique node identifier (e.g., "local", "node-1")
- `name` - Human-readable node name
- `hostname` - DNS hostname of the node
- `ip_address` - IP address for agent communication
- `status` - Node status: `online`, `offline`, `maintenance`, `error`
- `is_local` - Whether this is the local controller node
- `agent_url` - URL for the node's agent (for future remote nodes)
- `agent_token` - Authentication token for agent communication
- `last_seen_at` - Timestamp of last heartbeat
- `created_at` / `updated_at` - Timestamps

#### Updated Resource Tables

All resource tables now include `node_id` columns:

```sql
-- networks, storage_pools, images, virtual_machines
ALTER TABLE {table} ADD COLUMN node_id TEXT NOT NULL DEFAULT 'local';
CREATE INDEX idx_{table}_node_id ON {table}(node_id);
```

**Constraints changed:**
- Global unique constraints (`UNIQUE(name)`) → Per-node unique constraints (`UNIQUE(node_id, name)`)
- Added foreign key: `FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE`

### Backend Implementation

#### Repository Layer (`internal/db/sqlite.go`)

**New Methods:**
```go
// Node management
CreateNode(ctx, *Node) error
GetNode(ctx, id string) (*Node, error)
GetLocalNode(ctx) (*Node, error)
ListNodes(ctx) ([]Node, error)
UpdateNodeStatus(ctx, id, status string) error
DeleteNode(ctx, id) error

// Node-scoped queries
ListNetworksByNode(ctx, nodeID string) ([]Network, error)
ListStoragePoolsByNode(ctx, nodeID string) ([]StoragePool, error)
ListImagesByNode(ctx, nodeID string) ([]Image, error)
ListVMsByNode(ctx, nodeID string) ([]VirtualMachine, error)

// Resource counting
CountVMsByNode(ctx, nodeID string) (int, error)
CountImagesByNode(ctx, nodeID string) (int, error)
CountStoragePoolsByNode(ctx, nodeID string) (int, error)
CountNetworksByNode(ctx, nodeID string) (int, error)
```

#### Model Updates (`internal/models/models.go`)

**New Types:**
```go
type Node struct {
    ID, Name, Hostname, IPAddress, Status string
    IsLocal bool
    AgentURL, AgentToken string
    LastSeenAt, CreatedAt, UpdatedAt string
}

type NodeResourceCount struct {
    VMs, Images, StoragePools, Networks int
}

type NodeWithResources struct {
    Node
    Resources NodeResourceCount
}

const (
    NodeStatusOnline      = "online"
    NodeStatusOffline     = "offline"
    NodeStatusMaintenance = "maintenance"
    NodeStatusError       = "error"
)
```

**Updated Types:**
- All resource types now include `NodeID string` field

#### API Layer (`internal/api/nodes.go`)

**New Endpoints:**
```go
GET    /api/v1/nodes              -> listNodes
GET    /api/v1/nodes/{id}         -> getNode
GET    /api/v1/nodes/{id}/vms     -> listNodeVMs
GET    /api/v1/nodes/{id}/images  -> listNodeImages
GET    /api/v1/nodes/{id}/storage -> listNodeStoragePools
GET    /api/v1/nodes/{id}/networks-> listNodeNetworks
```

**Features:**
- Node validation on all node-scoped endpoints (404 if node not found)
- Resource counts included in node responses
- Consistent error format with `code`, `message`, `retryable`, `hint`

#### Service Layer Updates

**VM Service (`internal/vm/service.go`):**
- `CreateVM()` now fetches local node and assigns `NodeID`

**Image Service (`internal/images/service.go`):**
- `ImportImage()` now fetches local node and assigns `NodeID`

**API Handlers (`internal/api/`):**
- `createNetwork()` - assigns to local node
- `createStoragePool()` - assigns to local node

### Frontend Implementation

#### API Client (`ui/src/lib/api/client.ts`)

**New Methods:**
```typescript
getNode(nodeId: string)
listNodeVMs(nodeId: string)
listNodeImages(nodeId: string)
listNodeStoragePools(nodeId: string)
listNodeNetworks(nodeId: string)
```

#### Updated Pages

**Node-scoped pages now use node-specific APIs:**
- `/nodes/[id]/+page.svelte` - Uses `listNode{Resource}()` methods
- `/nodes/[id]/vms/+page.svelte` - Uses `listNodeVMs()`
- `/nodes/[id]/images/+page.svelte` - Uses `listNodeImages()`
- `/nodes/[id]/storage/+page.svelte` - Uses `listNodeStoragePools()`
- `/nodes/[id]/networks/+page.svelte` - Uses `listNodeNetworks()`

### Database Migrations

**Automatic Migrations on Startup:**
1. Create `nodes` table if not exists
2. Add `node_id` columns to resource tables
3. Create indexes for performance
4. Create default "local" node if not exists
5. Associate existing resources with local node

**Migration Safety:**
- Idempotent - can run multiple times safely
- Non-destructive - preserves existing data
- Default values - existing rows get `node_id = 'local'`

## API Specification

See [API_SPEC.md](API_SPEC.md) for detailed endpoint documentation.

### Response Format

**Node-scoped list response:**
```json
{
  "node_id": "local",
  "node_name": "Local Node",
  "resources": [...],
  "count": 5
}
```

**Node detail response:**
```json
{
  "id": "local",
  "name": "Local Node",
  "hostname": "localhost",
  "ip_address": "127.0.0.1",
  "status": "online",
  "is_local": true,
  "resources": {
    "vms": 5,
    "images": 3,
    "storage_pools": 2,
    "networks": 1
  },
  "created_at": "2026-04-10T10:00:00Z",
  "updated_at": "2026-04-10T10:00:00Z"
}
```

## Design Decisions

### 1. Node-scoped by Default

**Decision:** All new resources are automatically associated with the local node.

**Rationale:**
- Maintains single-node simplicity for MVP-1
- No changes required to existing workflows
- Foundation for multi-node without breaking changes

### 2. Global Endpoints Preserved

**Decision:** Global endpoints (`/api/v1/vms`) continue to work alongside node-scoped endpoints.

**Rationale:**
- Backward compatibility
- Convenience for global views
- Existing integrations continue to work

### 3. Per-Node Unique Constraints

**Decision:** Resource names are unique per-node, not globally.

**Rationale:**
- Different nodes may have similarly named VMs (e.g., "web-server-01")
- Natural isolation between nodes
- Matches Proxmox/VMware behavior

### 4. Node Validation at API Layer

**Decision:** All node-scoped endpoints validate node existence and return 404 if not found.

**Rationale:**
- Clear error semantics
- Fail fast - don't query resources for non-existent nodes
- Consistent with REST conventions

## Future Work

### Phase 2: Remote Node Management

**Planned features:**
- `POST /api/v1/nodes` - Register new remote node
- Node heartbeat mechanism
- Agent token rotation
- Node health checks and automatic failover

### Phase 3: Resource Scheduling

**Planned features:**
- VM placement policies
- Resource quotas per node
- Load balancing across nodes
- Live migration between nodes

### Phase 4: Distributed Storage

**Planned features:**
- Shared storage pools across nodes
- VM migration with shared storage
- Distributed image repository

## Testing

### Manual Testing Checklist

- [ ] List nodes endpoint returns local node with resource counts
- [ ] Get node endpoint returns 404 for invalid node ID
- [ ] Node-scoped VM list returns only VMs for that node
- [ ] Node-scoped image list returns only images for that node
- [ ] Creating VM automatically assigns to local node
- [ ] Creating network automatically assigns to local node
- [ ] UI node detail page shows correct resource counts
- [ ] UI node-scoped pages load without errors

### Database Migration Testing

1. Start with old schema (no nodes table)
2. Start controller
3. Verify migrations applied:
   - `nodes` table exists
   - `node_id` columns exist on resource tables
   - Indexes created
   - Local node created
   - Existing resources associated with local node

## Deployment Notes

### Fresh Installation

No special actions required. Schema is created with all multi-node support.

### Upgrade from Single-Node

1. Stop controller: `systemctl stop chv-controller`
2. Backup database: `cp /var/lib/chv/chv.db /var/lib/chv/chv.db.backup`
3. Deploy new binary
4. Start controller: `systemctl start chv-controller`
5. Verify migrations applied in logs
6. Test node endpoints: `curl http://localhost:8888/api/v1/nodes`

### Rollback

If issues occur:
1. Stop controller
2. Restore database: `cp /var/lib/chv/chv.db.backup /var/lib/chv/chv.db`
3. Deploy previous binary
4. Start controller

## References

- [API Specification](API_SPEC.md)
- [Architecture Decisions](ARCHITECTURE_DECISIONS.md) - ADR 5, ADR 6
- [Design System](../DESIGN.md) - Node status indicators, navigation patterns
