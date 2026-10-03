# Contract: Architecture Designer API

The Architecture Designer API is served by the backend-for-frontend (BFF) inside `chv-controlplane`. The BFF uses POST-only verb paths, not REST verbs. Every endpoint takes a JSON request body and returns a JSON response body.

## Resource: ArchitectureTopology

The API manages saved topology objects. Each topology carries a `version_number`. Update and archive calls send `expected_version`; the BFF rejects stale versions with `409 Conflict`.

## Endpoints

| Endpoint | Role | Purpose |
|---|---|---|
| `POST /v1/architectures/list` | Viewer | List topologies; excludes archived by default |
| `POST /v1/architectures/get` | Viewer | Fetch one topology with graph and YAML |
| `POST /v1/architectures/versions/list` | Viewer | List saved versions of a topology |
| `POST /v1/architectures/validate` | Operator | Validate the persisted YAML of a topology |
| `POST /v1/architectures/validate-yaml` | Operator | Validate an ad-hoc YAML body |
| `POST /v1/architectures/create` | Operator | Create a topology |
| `POST /v1/architectures/update` | Operator | Update a topology |
| `POST /v1/architectures/archive` | Operator | Archive a topology |
| `POST /v1/architectures/check-fleet` | Operator | Run fleet-consistency checks |
| `POST /v1/architectures/generate-yaml` | Operator | Return the topology's YAML |
| `POST /v1/architectures/import-yaml` | Operator | Import YAML into a topology |
| `POST /v1/architectures/plan` | Operator | Generate an apply-mode plan |
| `POST /v1/architectures/destroy-plan` | Operator | Generate a destroy-mode plan |
| `POST /v1/architectures/discard-plan` | Operator | Discard a plan |
| `POST /v1/architectures/apply` | Operator | Apply a confirmed plan |
| `POST /v1/architectures/destroy` | Operator | Execute a destroy-mode plan |
| `POST /v1/architectures/runs/list` | Operator | List apply runs |
| `POST /v1/architectures/drift` | Operator | Fetch a drift report |

Notes:

- `apply` and `destroy` escalate to Admin for `production` and `prod` environments. Other roles receive `403` with `code: PRODUCTION_REQUIRES_ADMIN`.
- There is no `export.yaml` endpoint. `generate-yaml` returns the YAML document in the response body.
- The plan mode is fixed by the endpoint: `plan` produces `apply` mode, `destroy-plan` produces `destroy` mode.

## Create architecture

```http
POST /v1/architectures/create
Content-Type: application/json
```

```json
{
  "name": "customer-a-production",
  "display_name": "Customer A Production",
  "description": "Production topology for Customer A",
  "environment": "production",
  "design_graph_json": null,
  "latest_yaml": null
}
```

The response returns an `architecture` summary object with `id`, `name`, `status`, `version_number`, and timestamps.

## Validate

```http
POST /v1/architectures/validate
```

Request:

```json
{
  "id": "arch_01HX..."
}
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
POST /v1/architectures/check-fleet
```

Request:

```json
{
  "id": "arch_01HX..."
}
```

Response:

```json
{
  "status": "invalid",
  "inventory_snapshot_id": "inv_01HX...",
  "checked_at": "2026-06-13T09:00:00Z",
  "findings": [
    {
      "severity": "error",
      "code": "INSUFFICIENT_MEMORY",
      "message": "Host chv-node-01 does not have enough free memory for the requested instances.",
      "resource_ref": "servers/chv-node-01",
      "blocking": true
    }
  ]
}
```

## Plan

```http
POST /v1/architectures/plan
```

Request:

```json
{
  "id": "arch_01HX...",
  "allow_warnings": false,
  "refresh_inventory": true
}
```

`refresh_inventory` defaults to `true`. `allow_warnings` is a forward-compatibility hook consumed by the apply path.

Response:

```json
{
  "plan_id": "plan_01HX...",
  "architecture_id": "arch_01HX...",
  "architecture_version": 3,
  "architecture_version_id": "archver_01HX...",
  "status": "requires_confirmation",
  "mode": "apply",
  "summary": {
    "create": 4,
    "update": 1,
    "delete": 0,
    "replace": 0,
    "no_op": 0,
    "warnings": 1
  },
  "changes": [],
  "warnings": [],
  "expires_at": "2026-06-13T09:15:00Z",
  "created_at": "2026-06-13T09:00:00Z"
}
```

## Apply

```http
POST /v1/architectures/apply
```

Request:

```json
{
  "id": "arch_01HX...",
  "plan_id": "plan_01HX...",
  "confirmation": {
    "typed_name": "customer-a-production"
  },
  "acknowledged_warnings": true
}
```

Response:

```json
{
  "run_id": "run_01HX...",
  "task_id": "task_01HX...",
  "status": "queued",
  "started_at": null,
  "architecture_id": "arch_01HX...",
  "architecture_version_id": "archver_01HX...",
  "plan_id": "plan_01HX..."
}
```

## Drift

```http
POST /v1/architectures/drift
```

Request:

```json
{
  "id": "arch_01HX...",
  "force_refresh": false
}
```

The response carries `drift_report_id`, `status`, `findings`, `summary`, `baseline_version_id`, `computed_at`, `cache_hit`, and `error_message`. A cached report is returned when the most recent report is younger than the cache TTL.

## Error response

Errors use a flat JSON shape:

```json
{
  "code": "PLAN_EXPIRED",
  "message": "plan plan_01HX... expired at 2026-06-13T09:15:00Z",
  "plan_id": "plan_01HX..."
}
```

Stable error codes include `GRAPH_EMPTY`, `PLAN_EXPIRED`, `PLAN_NOT_DISCARDABLE`, `MISSING_CONFIRMATION`, `WARNINGS_NOT_ACKNOWLEDGED`, `PLAN_NOT_APPLICABLE`, `PLAN_MODE_MISMATCH`, `INVALID_RESOURCE_NAME`, `PRODUCTION_REQUIRES_ADMIN`, and `DRIFT_CHECK_FAILED`.
