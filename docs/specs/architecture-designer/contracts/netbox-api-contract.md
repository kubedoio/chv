# Contract: NetBox Projection BFF API

> Served by the WebUI BFF inside `chv-controlplane`, following the designer's
> POST-only verb-path convention (`/v1/architectures/<verb>` — see the note in
> `handlers/architectures.rs` for why REST verbs are intentionally not used).
> Design: `docs/design/issue-239-netbox-projection.md` · ADR-023.

## Roles

- **Viewer**: no access to any NetBox projection endpoint (403).
- **Operator**: config read/upsert, dry-run, export, runs — on architectures
  they own (or system rows per the existing ownership model).
- **Admin**: everything, all architectures.
- Architectures with `metadata.environment = production` escalate export (not
  config read) to Admin, mirroring the apply/destroy rule.

## Endpoints

| Endpoint | Role | Purpose |
|---|---|---|
| `POST /v1/architectures/netbox/config/get` | Operator | Fetch projection config (token never returned) |
| `POST /v1/architectures/netbox/config/upsert` | Operator | Create/update projection config (and set token) |
| `POST /v1/architectures/netbox/config/delete` | Operator | Remove projection config (NetBox untouched) |
| `POST /v1/architectures/netbox/export/dry-run` | Operator | Compute + return the projection plan (no writes) |
| `POST /v1/architectures/netbox/export` | Operator (Admin if production) | Enqueue an export run |
| `POST /v1/architectures/netbox/runs/list` | Operator | List projection runs for a topology |
| `POST /v1/architectures/netbox/runs/get` | Operator | Fetch one run with plan + results |
| `POST /v1/architectures/netbox/runs/retry` | Operator | Re-enqueue a failed run |

## Config get

```http
POST /v1/architectures/netbox/config/get
```

```json
{ "id": "arch_01HX..." }
```

Response (note: `token_set` boolean only — the token and its ciphertext are
never returned):

```json
{
  "architecture_id": "arch_01HX...",
  "endpoint": "https://netbox.example.internal",
  "token_secret_ref": "netbox-arch-01HX",
  "token_set": true,
  "retention_policy": "mark_stale",
  "enable_post_apply": true,
  "custom_field_prefix": "chv_",
  "site_name": "dc1",
  "updated_at": "2026-10-08T09:00:00Z"
}
```

404 with `NETBOX_NOT_CONFIGURED` when no config exists.

## Config upsert

```http
POST /v1/architectures/netbox/config/upsert
```

```json
{
  "id": "arch_01HX...",
  "expected_version": 4,
  "endpoint": "https://netbox.example.internal",
  "token": "PAbCd...|null",
  "token_secret_ref": "netbox-arch-01HX",
  "retention_policy": "mark_stale",
  "enable_post_apply": true,
  "site_name": "dc1"
}
```

- `token` is optional on update (omitted/null keeps the existing secret).
- `expected_version` follows the topology's optimistic-concurrency rule
  (rejected 409 on `expected_version` mismatch, same behavior as
  `/v1/architectures/update`).
- `retention_policy ∈ {"mark_stale", "delete"}`; `"delete"` requires Admin.
- Non-HTTPS endpoints are rejected: 400 `NETBOX_HTTPS_REQUIRED`.

Response: the config summary shape above.

## Dry-run

```http
POST /v1/architectures/netbox/export/dry-run
```

```json
{ "id": "arch_01HX..." }
```

**Projected version:** the `architecture_version_id` of the most recent
`succeeded` apply run for the architecture — never the editable
`latest_yaml` draft. If the architecture has never been applied, the request
fails with 400 `NETBOX_NOT_APPLIED` (there is nothing applied to project).

Response — the deterministic, secret-free plan (computed live against NetBox
read endpoints; no writes):

```json
{
  "mapping_version": "v1",
  "architecture_id": "arch_01HX...",
  "architecture_version": 3,
  "retention": "mark_stale",
  "summary": { "create": 4, "update": 1, "no_op": 7, "conflict": 1, "stale": 0 },
  "entries": [
    {
      "action": "create",
      "kind": "virtual_machine",
      "chv_resource_ref": "instances/app-01",
      "netbox_natural_key": { "name": "app-01" },
      "external_id": "arch:arch_01HX...:instance/app-01:3",
      "reason": "no object with this external id; natural key free",
      "changes": []
    },
    {
      "action": "conflict",
      "kind": "prefix",
      "chv_resource_ref": "networks/tenant-prod",
      "netbox_natural_key": { "prefix": "10.0.20.0/24" },
      "external_id": "arch:arch_01HX...:network/tenant-prod:3",
      "reason": "prefix exists and is not owned by chv (chv_managed_by absent)",
      "changes": []
    }
  ]
}
```

If any entry is `conflict`, export is allowed but the response of the run will
carry the conflicts; the UI must render them before apply (the plan is
advisory, not gating — parity with the designer's plan model is deliberate,
but conflicts never write).

## Export

```http
POST /v1/architectures/netbox/export
```

```json
{ "id": "arch_01HX..." }
```

Response:

```json
{
  "run_id": "netrun_01HX...",
  "architecture_id": "arch_01HX...",
  "status": "queued"
}
```

- Rejected with 409 `NETBOX_RUN_ACTIVE` if a run is already queued/running.
- 400 `NETBOX_NOT_CONFIGURED` when no config exists.
- Production gating: 403 `PRODUCTION_REQUIRES_ADMIN` (same code as apply).

## Runs list / get / retry

`runs/list` request `{ "id": "arch_01HX...", "limit": 20 }` → `{ "runs": [...] }`
(each run: id, trigger, status, mode, summary, error_message, timestamps).

`runs/get` request `{ "id": "arch_01HX...", "run_id": "netrun_01HX..." }` →
full run including `plan_json` entries and per-entry `result_json` outcomes.
<<<<<<< HEAD
The worker persists `result_json` inside a provenance envelope
(`resolved_architecture_version_id` + `result`); the BFF unwraps it, serving
the flat outcome as `result_json` and the resolved version id as a nullable
`resolved_architecture_version_id` field (null for rows without an envelope).
=======
>>>>>>> origin/main

`runs/retry` request `{ "id": "arch_01HX...", "run_id": "netrun_01HX..." }` →
`{ "run_id": "netrun_01HX...", "status": "queued" }` — only from `failed`
and below the attempt cap; otherwise 409 `PROJECTION_RUN_NOT_RETRYABLE`.

## Error codes (stable wire surface)

| Code | HTTP | Meaning |
|---|---|---|
| `NETBOX_NOT_CONFIGURED` | 400/404 | No projection config for the architecture |
| `NETBOX_NOT_APPLIED` | 400 | Architecture has no succeeded apply run — nothing applied to project |
| `NETBOX_HTTPS_REQUIRED` | 400 | Endpoint is not HTTPS |
| `NETBOX_TOKEN_MISSING` | 400 | Config has no usable token |
| `NETBOX_RUN_ACTIVE` | 409 | A run is already queued/running |
| `PROJECTION_RUN_NOT_RETRYABLE` | 409 | Run not failed or attempts exhausted |
| `PRODUCTION_REQUIRES_ADMIN` | 403 | Production export by non-admin |
| `NETBOX_UNREACHABLE` | 502 | Dry-run could not reach NetBox |
| `NETBOX_AUTH_FAILED` | 502 | Token rejected by NetBox (dry-run only; runs record it on the run) |
| `PLAN_EXPIRED` | 409 | (reuse) topology version changed under a stale request |

Errors use the designer's flat JSON shape: `{ "code": "...", "message": "..." }`.
Error messages never contain the token or endpoint credentials.

## Events

Every config mutation and run terminal transition emits an audit event
(`architecture_netbox_config_updated`, `architecture_netbox_dry_run`,
`architecture_netbox_export_succeeded`, `architecture_netbox_export_failed`,
`architecture_netbox_export_retried`) via the existing `EventRepository`,
carrying architecture id, run id, trigger, and summary — never the token.
