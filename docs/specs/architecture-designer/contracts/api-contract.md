# Contract: Architecture Designer API

## Resource: ArchitectureTopology

The API manages saved topology objects.

## Endpoints

```text
GET    /architectures
POST   /architectures
GET    /architectures/{id}
PUT    /architectures/{id}
DELETE /architectures/{id}

POST   /architectures/{id}/validate
POST   /architectures/{id}/check-fleet
POST   /architectures/{id}/generate-yaml
GET    /architectures/{id}/export.yaml

POST   /architectures/{id}/plan
POST   /architectures/{id}/apply
POST   /architectures/{id}/destroy-plan
POST   /architectures/{id}/destroy

GET    /architectures/{id}/versions
GET    /architectures/{id}/runs
GET    /architectures/{id}/drift
```

## Create architecture

```http
POST /architectures
Content-Type: application/json
```

```json
{
  "name": "customer-a-production",
  "description": "Production topology for Customer A",
  "environment": "production",
  "design_graph": {
    "nodes": [],
    "edges": []
  }
}
```

## Validate

```http
POST /architectures/{id}/validate
```

Response:

```json
{
  "status": "invalid",
  "summary": {
    "errors": 2,
    "warnings": 1,
    "info": 0
  },
  "findings": [
    {
      "severity": "error",
      "code": "NETWORK_CIDR_OVERLAP",
      "message": "Network tenant-prod overlaps with mgmt.",
      "path": "networks[1].cidr",
      "blocking": true
    }
  ]
}
```

## Check against current fleet

```http
POST /architectures/{id}/check-fleet
```

Response:

```json
{
  "status": "warning",
  "checked_at": "2026-06-13T09:00:00Z",
  "findings": [
    {
      "severity": "warning",
      "code": "HOST_SOFT_CAPACITY_EXCEEDED",
      "message": "Host chv-node-01 would exceed 80% memory allocation.",
      "resource_ref": "servers/chv-node-01"
    }
  ]
}
```

## Plan

```http
POST /architectures/{id}/plan
```

Request:

```json
{
  "mode": "apply",
  "requested_by": "webui",
  "options": {
    "allow_warnings": false,
    "refresh_inventory": true
  }
}
```

Response:

```json
{
  "plan_id": "plan_01HX...",
  "status": "requires_confirmation",
  "summary": {
    "create": 4,
    "update": 1,
    "delete": 0,
    "warnings": 1
  },
  "changes": []
}
```

## Apply

```http
POST /architectures/{id}/apply
```

Request:

```json
{
  "plan_id": "plan_01HX...",
  "confirmation": {
    "acknowledged_warnings": true,
    "typed_name": "customer-a-production"
  },
  "requested_by": "webui"
}
```

Response:

```json
{
  "run_id": "run_01HX...",
  "task_id": "task_01HX...",
  "status": "queued"
}
```

## Error response

```json
{
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "Architecture contains blocking validation errors.",
    "details": []
  }
}
```
