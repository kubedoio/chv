# CHV API Reference

Complete API reference for the CHV Controller HTTP API.

**Base URL**: `http://localhost:8080/api/v1`

## Authentication

All API requests (except health checks) require authentication using a Bearer token.

```
Authorization: Bearer <token>
```

### Obtaining a Token

```bash
curl -X POST http://localhost:8080/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-token",
    "expires_in": "24h"
  }'
```

**Response**:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "token": "chv_abc123xyz789",
  "name": "my-token",
  "expires_at": "2026-04-06T12:00:00Z"
}
```

> **Important**: Save the token immediately - it is only shown once.

## Health Check

### GET /health

Check controller health status.

**No authentication required.**

**Example**:
```bash
curl http://localhost:8080/health
```

**Response**:
```json
{
  "status": "healthy",
  "timestamp": "2026-04-05T10:30:00Z",
  "version": "0.1.0"
}
```

## Tokens

### POST /tokens

Create a new API token.

**Request Body**:
```json
{
  "name": "token-name",
  "expires_in": "24h"
}
```

**Expires In Format**:
- `1h` - 1 hour
- `24h` - 24 hours
- `7d` - 7 days
- `30d` - 30 days
- `never` - No expiration

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{
    "name": "automation-token",
    "expires_in": "7d"
  }'
```

## Nodes

### POST /nodes/register

Register a new node (agent) with the controller.

**Request Body**:
```json
{
  "hostname": "node1",
  "management_ip": "192.168.1.10",
  "total_cpu_cores": 8,
  "total_ram_mb": 16384
}
```

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/nodes/register \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "hostname": "node1",
    "management_ip": "192.168.1.10",
    "total_cpu_cores": 8,
    "total_ram_mb": 16384
  }'
```

### GET /nodes

List all registered nodes.

**Example**:
```bash
curl http://localhost:8080/api/v1/nodes \
  -H "Authorization: Bearer $TOKEN"
```

**Response**:
```json
[
  {
    "id": "node-uuid",
    "hostname": "node1",
    "management_ip": "192.168.1.10",
    "state": "online",
    "total_cpu_cores": 8,
    "total_ram_mb": 16384,
    "allocatable_cpu_cores": 6,
    "allocatable_ram_mb": 12288
  }
]
```

### GET /nodes/{id}

Get details for a specific node.

**Example**:
```bash
curl http://localhost:8080/api/v1/nodes/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer $TOKEN"
```

### POST /nodes/{id}/maintenance

Toggle maintenance mode for a node.

**Request Body**:
```json
{
  "enabled": true
}
```

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/nodes/550e8400-e29b-41d4-a716-446655440000/maintenance \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"enabled": true}'
```

## Networks

### POST /networks

Create a new network.

**Request Body**:
```json
{
  "name": "default",
  "bridge_name": "br0",
  "cidr": "192.168.100.0/24",
  "gateway_ip": "192.168.100.1"
}
```

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/networks \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "default",
    "bridge_name": "br0",
    "cidr": "192.168.100.0/24",
    "gateway_ip": "192.168.100.1"
  }'
```

### GET /networks

List all networks.

**Example**:
```bash
curl http://localhost:8080/api/v1/networks \
  -H "Authorization: Bearer $TOKEN"
```

## Storage Pools

### POST /storage-pools

Create a new storage pool.

**Request Body**:
```json
{
  "name": "local",
  "pool_type": "local",
  "path_or_export": "/var/lib/chv/volumes",
  "supports_online_resize": true
}
```

**Pool Types**:
- `local` - Local filesystem storage
- `nfs` - NFS mount

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/storage-pools \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "local",
    "pool_type": "local",
    "path_or_export": "/var/lib/chv/volumes",
    "supports_online_resize": true
  }'
```

### GET /storage-pools

List all storage pools.

**Example**:
```bash
curl http://localhost:8080/api/v1/storage-pools \
  -H "Authorization: Bearer $TOKEN"
```

## Images

### POST /images/import

Import a cloud image.

**Request Body**:
```json
{
  "name": "ubuntu-22.04",
  "os_family": "ubuntu",
  "source_url": "https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img",
  "source_format": "qcow2",
  "architecture": "x86_64",
  "cloud_init_supported": true
}
```

**Source Formats**:
- `qcow2` - QEMU Copy-On-Write
- `raw` - Raw disk image
- `vmdk` - VMware disk (converted to raw)

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/images/import \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "ubuntu-22.04",
    "os_family": "ubuntu",
    "source_url": "https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img",
    "source_format": "qcow2",
    "architecture": "x86_64",
    "cloud_init_supported": true
  }'
```

### GET /images

List all images.

**Example**:
```bash
curl http://localhost:8080/api/v1/images \
  -H "Authorization: Bearer $TOKEN"
```

**Response**:
```json
[
  {
    "id": "image-uuid",
    "name": "ubuntu-22.04",
    "os_family": "ubuntu",
    "status": "ready",
    "architecture": "x86_64",
    "size_bytes": 536870912
  }
]
```

## VMs

### POST /vms

Create a new VM.

**Request Body**:
```json
{
  "name": "vm1",
  "cpu": 2,
  "memory_mb": 4096,
  "image_id": "550e8400-e29b-41d4-a716-446655440000",
  "disk_size_bytes": 10737418240,
  "networks": [
    {"network_id": "network-uuid"}
  ],
  "cloud_init": {
    "user_data": "#cloud-config\nusers:\n  - name: admin\n    sudo: ALL=(ALL) NOPASSWD:ALL\n"
  }
}
```

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "vm1",
    "cpu": 2,
    "memory_mb": 4096,
    "image_id": "550e8400-e29b-41d4-a716-446655440000",
    "disk_size_bytes": 10737418240,
    "networks": [{"network_id": "network-uuid"}]
  }'
```

### GET /vms

List all VMs.

**Example**:
```bash
curl http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer $TOKEN"
```

**Response**:
```json
[
  {
    "id": "vm-uuid",
    "name": "vm1",
    "desired_state": "running",
    "actual_state": "running",
    "node_id": "node-uuid",
    "created_at": "2026-04-05T10:00:00Z"
  }
]
```

### GET /vms/{id}

Get VM details.

**Example**:
```bash
curl http://localhost:8080/api/v1/vms/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer $TOKEN"
```

### POST /vms/{id}/start

Start a VM.

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/vms/550e8400-e29b-41d4-a716-446655440000/start \
  -H "Authorization: Bearer $TOKEN"
```

### POST /vms/{id}/stop

Stop a VM.

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/vms/550e8400-e29b-41d4-a716-446655440000/stop \
  -H "Authorization: Bearer $TOKEN"
```

### POST /vms/{id}/reboot

Reboot a VM.

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/vms/550e8400-e29b-41d4-a716-446655440000/reboot \
  -H "Authorization: Bearer $TOKEN"
```

### POST /vms/{id}/resize-disk

Resize a VM's disk.

**Request Body**:
```json
{
  "new_size_bytes": 21474836480
}
```

**Example**:
```bash
curl -X POST http://localhost:8080/api/v1/vms/550e8400-e29b-41d4-a716-446655440000/resize-disk \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"new_size_bytes": 21474836480}'
```

### DELETE /vms/{id}

Delete a VM.

**Example**:
```bash
curl -X DELETE http://localhost:8080/api/v1/vms/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer $TOKEN"
```

## Error Responses

All errors follow this format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error message",
    "details": {}
  }
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `UNAUTHORIZED` | 401 | Missing or invalid token |
| `FORBIDDEN` | 403 | Insufficient permissions |
| `NOT_FOUND` | 404 | Resource not found |
| `ALREADY_EXISTS` | 409 | Resource already exists |
| `INVALID_REQUEST` | 400 | Bad request parameters |
| `INVALID_STATE` | 400 | VM in wrong state for operation |
| `INTERNAL_ERROR` | 500 | Server error |

### Example Error Response

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "VM not found",
    "details": {
      "vm_id": "550e8400-e29b-41d4-a716-446655440000"
    }
  }
}
```

## Rate Limiting

> **Note**: Rate limiting is not implemented in MVP-1. It is planned for v0.2.0.

## Pagination

> **Note**: Pagination is not implemented in MVP-1. All list endpoints return complete results.

## WebSocket / Real-time Updates

> **Note**: Real-time updates via WebSocket are not implemented in MVP-1. Use polling for status updates.

Example polling:
```bash
while true; do
  curl http://localhost:8080/api/v1/vms/$VM_ID \
    -H "Authorization: Bearer $TOKEN"
  sleep 5
done
```
