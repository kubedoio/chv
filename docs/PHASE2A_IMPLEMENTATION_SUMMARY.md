# Phase 2A Implementation Summary

## Overview
Implementation of Remote Node Management for CHV, enabling multi-node support with secure agent authentication.

## Backend Changes

### 1. Database Schema (`configs/schema_sqlite.sql`)
- Added `agent_token_hash` column to `nodes` table for secure token storage
- Added `capabilities` column (JSON) for future node capabilities
- Added `roles` table for RBAC foundation

### 2. API Endpoints (`internal/api/nodes.go`)
New endpoints implemented:

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/nodes` | Create new node with agent token |
| PATCH | `/api/v1/nodes/{id}` | Update node details |
| DELETE | `/api/v1/nodes/{id}` | Delete node (cannot delete local) |
| POST | `/api/v1/nodes/{id}/maintenance` | Toggle maintenance mode |
| POST | `/api/v1/agents/register` | Agent registration endpoint |
| POST | `/api/v1/agents/heartbeat` | Agent heartbeat endpoint |

### 3. Agent Authentication (`internal/api/auth.go`)
- `agentAuthMiddleware` - Validates both agent tokens and user tokens
- Agent tokens prefixed with `chv_agent_`
- Constant-time comparison to prevent timing attacks
- SHA-256 hashing of tokens before storage

### 4. Database Methods (`internal/db/sqlite.go`)
Added methods:
- `UpdateNode()` - Update node properties
- `ValidateAgentToken()` - Validate agent token hash
- `GetNodeByAgentToken()` - Get node by token
- `GetNodesByStatus()` - Get nodes by status
- `migrateAddAgentTokenColumns()` - Migration for new columns

### 5. Node Health Monitor (`internal/health/nodemonitor.go`)
- Background goroutine for health monitoring
- Marks stale nodes offline after 2 minutes
- Checks every 30 seconds
- Provides methods: `MarkNodeOnline`, `MarkNodeOffline`, `MarkNodeMaintenance`

### 6. Models (`internal/models/models.go`)
Added:
- `Node.AgentTokenHash` - Secure token storage
- `Node.Capabilities` - JSON capabilities field
- `Role` and `Permission` structs for RBAC
- `AuditLog` struct for audit logging

## Frontend Changes

### 1. API Client (`ui/src/lib/api/client.ts`)
New methods:
- `listNodes()` - Get all nodes with resource counts
- `createNode(data)` - Create new node
- `getNode(id)` - Get single node
- `updateNode(id, data)` - Update node
- `deleteNode(id)` - Delete node
- `setNodeMaintenance(id, enabled)` - Toggle maintenance

### 2. Types (`ui/src/lib/api/types.ts`)
Added node types:
- `Node` - Node interface
- `NodeWithResources` - Node with resource counts
- `CreateNodeInput` - Input for creating node
- `CreateNodeResponse` - Response with agent token
- `UpdateNodeInput` - Input for updating node

### 3. Add Node Modal (`ui/src/lib/components/AddNodeModal.svelte`)
- Form for name, hostname, IP, agent URL
- Displays agent token after creation (one-time only)
- Copy to clipboard functionality
- Next steps guidance

### 4. Nodes Page (`ui/src/routes/nodes/+page.svelte`)
- Grid view of all nodes with status indicators
- Resource counts (VMs, images, pools, networks)
- Last seen timestamp for remote nodes
- Action buttons: Maintenance toggle, Delete
- Add Node button with modal

### 5. Node Utilities (`ui/src/lib/api/nodes.ts`)
Helper functions:
- `generateTreeNodes()` - Generate navigation tree
- `getNodeStatusColor()` - Get status color class
- `getNodeStatusBg()` - Get status background class
- `formatLastSeen()` - Format last seen timestamp

## Security Features

1. **Token Generation**: 32 random bytes, hex encoded with `chv_agent_` prefix
2. **Token Storage**: SHA-256 hashed before storage
3. **Constant-Time Comparison**: Prevents timing attacks
4. **One-Time Token Display**: Token only shown at creation
5. **Local Node Protection**: Cannot delete the local node

## Testing

### Backend Test
```bash
# Start controller
go run ./cmd/chv-controller

# Create a token first
curl -X POST http://localhost:8888/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}'

# Register a new node
curl -X POST http://localhost:8888/api/v1/nodes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "name":"test-node",
    "hostname":"test.local",
    "ip_address":"10.0.0.5",
    "agent_url":"http://10.0.0.5:9090"
  }'

# Agent registration
curl -X POST http://localhost:8888/api/v1/agents/register \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer AGENT_TOKEN" \
  -d '{
    "node_id":"NODE_ID",
    "hostname":"test.local",
    "token":"AGENT_TOKEN"
  }'

# Agent heartbeat
curl -X POST http://localhost:8888/api/v1/agents/heartbeat \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer AGENT_TOKEN" \
  -d '{
    "node_id":"NODE_ID",
    "timestamp":"2026-04-10T10:00:00Z"
  }'
```

## Success Criteria Met

- ✅ Can register new node via API
- ✅ Agent token is generated and returned (one-time display)
- ✅ Node appears in UI list with correct status
- ✅ Node status tracks correctly (online/offline/maintenance)
- ✅ Agent authentication middleware validates tokens
- ✅ Health monitor marks stale nodes offline
- ✅ Database migrations handle new columns

## Next Steps (Phase 2B)

1. Implement heartbeat service on agent side
2. Add resource metrics collection
3. Create node health dashboard component
4. Add health alerts/notifications
5. Implement metrics storage and aggregation
