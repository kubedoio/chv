# CHV API Specification

## Overview

CHV provides a RESTful API for managing virtualization infrastructure. The API follows standard REST conventions with JSON request/response bodies.

**Base URL:** `/api/v1`

**Authentication:** Bearer token in `Authorization` header

**Content-Type:** `application/json`

---

## Node Management

Nodes represent physical or virtual hypervisor hosts in the CHV cluster. Each node owns its own resources (VMs, images, storage pools, networks).

### List Nodes

Returns all nodes in the cluster with resource counts.

```http
GET /api/v1/nodes
```

**Response:**
```json
[
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
]
```

### Get Node

Returns detailed information about a specific node.

```http
GET /api/v1/nodes/{id}
```

**Parameters:**
- `id` (path) - Node ID

**Response:**
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

**Error Responses:**
- `404 Not Found` - Node does not exist

---

## Node-Scoped Resources

Resources (VMs, images, storage pools, networks) are scoped to nodes. All node-scoped endpoints validate that the node exists before returning resources.

### List Node VMs

Returns all VMs on a specific node.

```http
GET /api/v1/nodes/{id}/vms
```

**Parameters:**
- `id` (path) - Node ID

**Response:**
```json
{
  "node_id": "local",
  "node_name": "Local Node",
  "resources": [
    {
      "id": "vm-123",
      "node_id": "local",
      "name": "web-server-01",
      "image_id": "img-456",
      "storage_pool_id": "pool-789",
      "network_id": "net-abc",
      "desired_state": "running",
      "actual_state": "running",
      "vcpu": 2,
      "memory_mb": 4096,
      "ip_address": "10.0.0.5",
      "created_at": "2026-04-10T10:00:00Z",
      "updated_at": "2026-04-10T10:00:00Z"
    }
  ],
  "count": 1
}
```

**Error Responses:**
- `404 Not Found` - Node does not exist

### List Node Images

Returns all images available on a specific node.

```http
GET /api/v1/nodes/{id}/images
```

**Response:**
```json
{
  "node_id": "local",
  "node_name": "Local Node",
  "resources": [
    {
      "id": "img-456",
      "node_id": "local",
      "name": "ubuntu-22.04",
      "os_family": "ubuntu",
      "architecture": "x86_64",
      "format": "qcow2",
      "status": "ready",
      "cloud_init_supported": true,
      "created_at": "2026-04-10T10:00:00Z"
    }
  ],
  "count": 1
}
```

### List Node Storage Pools

Returns all storage pools on a specific node.

```http
GET /api/v1/nodes/{id}/storage
```

**Response:**
```json
{
  "node_id": "local",
  "node_name": "Local Node",
  "resources": [
    {
      "id": "pool-789",
      "node_id": "local",
      "name": "localdisk",
      "pool_type": "localdisk",
      "path": "/var/lib/chv/vms",
      "is_default": true,
      "status": "ready",
      "capacity_bytes": 1099511627776,
      "allocatable_bytes": 824633720832,
      "created_at": "2026-04-10T10:00:00Z"
    }
  ],
  "count": 1
}
```

### List Node Networks

Returns all networks on a specific node.

```http
GET /api/v1/nodes/{id}/networks
```

**Response:**
```json
{
  "node_id": "local",
  "node_name": "Local Node",
  "resources": [
    {
      "id": "net-abc",
      "node_id": "local",
      "name": "default",
      "mode": "bridge",
      "bridge_name": "chvbr0",
      "cidr": "10.0.0.0/24",
      "gateway_ip": "10.0.0.1",
      "status": "active",
      "created_at": "2026-04-10T10:00:00Z"
    }
  ],
  "count": 1
}
```

---

## Global Resources

These endpoints return resources across all nodes (for backward compatibility and global views).

### List All VMs

```http
GET /api/v1/vms
```

Returns array of VMs (same schema as node-scoped resources).

### List All Images

```http
GET /api/v1/images
```

### List All Storage Pools

```http
GET /api/v1/storage-pools
```

### List All Networks

```http
GET /api/v1/networks
```

---

## Error Responses

All errors follow a consistent format:

```json
{
  "error": {
    "code": "node_not_found",
    "message": "Node not found",
    "resource_type": "node",
    "resource_id": "invalid-id",
    "retryable": false,
    "hint": "Check that the node ID is correct"
  }
}
```

**HTTP Status Codes:**
- `200 OK` - Success
- `201 Created` - Resource created successfully
- `400 Bad Request` - Invalid request parameters
- `401 Unauthorized` - Missing or invalid authentication
- `404 Not Found` - Resource does not exist
- `409 Conflict` - Resource already exists or state conflict
- `422 Unprocessable Entity` - Validation failed
- `500 Internal Server Error` - Server error

---

## Resource Scoping Behavior

### Creating Resources

When creating resources (VMs, images, storage pools, networks), they are automatically associated with the **local node** (the node where the controller is running).

```http
POST /api/v1/vms
{
  "name": "new-vm",
  "image_id": "img-456",
  "storage_pool_id": "pool-789",
  "network_id": "net-abc",
  "vcpu": 2,
  "memory_mb": 4096
}
```

The response will include the `node_id` field set to the local node.

### Future Multi-Node Support

In future versions, creating resources on remote nodes will be supported via:

```http
POST /api/v1/nodes/{node_id}/vms
```

For MVP-1, all resources are created on the local node.

---

## Design Principles

1. **Node-scoped by default** - All resource operations validate node existence
2. **Consistent error format** - All errors use the same structure
3. **Resource counts included** - Node endpoints include resource counts for UI convenience
4. **Backward compatible** - Global endpoints (`/api/v1/vms`) continue to work
5. **Hierarchical URLs** - Node resources follow `/nodes/{id}/{resource}` pattern
