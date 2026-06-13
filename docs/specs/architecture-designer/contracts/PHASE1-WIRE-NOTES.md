# Phase 1 Wire Notes — Architecture Designer Validation

These are the BFF wire shapes the UI agent can rely on for Phase 1. They live
alongside `api-contract.md` because the existing CHV BFF uses POST-only
verb-paths instead of REST verbs (see the `architectures.rs` module-level note);
this document records the concrete request/response JSON shapes that ship in
Phase 1.

All endpoints accept JSON request bodies and return JSON response bodies.
All endpoints sit behind the operator-or-admin role gate (validate is
documented as `operator+` in api-contract.md). Authentication is the standard
bearer token used by every other BFF endpoint; CSRF protection applies to
POST, same as the rest of the surface.

## Validation result shape (shared)

```json
{
  "status": "valid" | "warning" | "invalid",
  "summary": {
    "errors": 0,
    "warnings": 0,
    "info": 0
  },
  "findings": [
    {
      "severity": "error" | "warning" | "info",
      "code": "INVALID_CIDR",
      "message": "Network tenant-prod has invalid CIDR: 999.0.0.0/24",
      "path": "networks[0].cidr",
      "resource_ref": "networks/tenant-prod",
      "blocking": true,
      "suggestion": "Provide a syntactically valid IPv4 or IPv6 CIDR."
    }
  ]
}
```

`code` is one of the stable strings registered in `codes.rs`. Stable codes
shipped in Phase 1:

- `SCHEMA_INVALID`
- `DUPLICATE_NAME`
- `MISSING_REFERENCE`
- `INVALID_CIDR`
- `NETWORK_CIDR_OVERLAP`
- `DUPLICATE_IP`
- `IP_OUTSIDE_NETWORK`
- `GATEWAY_OUTSIDE_NETWORK`
- `DHCP_RANGE_INVALID`
- `RAW_SECRET_FORBIDDEN`
- `INVALID_PERMISSION`
- `INVALID_EDGE`
- `USER_NAMESPACE_COLLISION`
- `STATIC_IP_IN_DHCP_RANGE`

`USER_NAMESPACE_COLLISION` and `STATIC_IP_IN_DHCP_RANGE` extend the
`architecture-designer-validation.md` list to cover the spec rules in items 12
and 13. Both are stable strings — they will not change.

## POST `/v1/architectures/validate`

Validate the **persisted** topology's `latest_yaml`. Persists the result by
setting `last_validation_status` on the topology row.

Request:

```json
{ "id": "<architecture id>" }
```

Response: `ValidationResult` shape above.

Errors:

- `400 BAD_REQUEST` — id is blank.
- `403 FORBIDDEN` — caller is a viewer.
- `404 NOT_FOUND` — id does not match any topology.
- `409 CONFLICT` — topology was modified concurrently while persisting status.

## POST `/v1/architectures/validate-yaml`

Validate an **ad-hoc** YAML body without touching persistent state. Useful for
the editor's "validate before save" path.

Request:

```json
{ "yaml": "apiVersion: chv.kubedo.io/v1alpha1\nkind: CHVArchitecture\n..." }
```

Response: `ValidationResult` shape above.

Errors: `400 BAD_REQUEST` (`yaml` blank), `403 FORBIDDEN` (viewer).

## POST `/v1/architectures/generate-yaml`

Phase 1 returns the **persisted** `latest_yaml` verbatim. The graph→YAML mapper
is a Phase 2 deliverable (lives with the canvas).

Request:

```json
{ "id": "<architecture id>" }
```

Response (200):

```json
{ "yaml": "apiVersion: chv.kubedo.io/v1alpha1\n..." }
```

Response (422):

```json
{
  "message": "topology graph is empty; YAML generation requires a non-empty graph (Phase 2)",
  "code": "GRAPH_EMPTY"
}
```

`GRAPH_EMPTY` is returned when both `latest_yaml` and `design_graph_json` are
absent or empty. Until the Phase 2 canvas wires the graph→YAML mapper, the
endpoint is best-effort: a topology that already has YAML in `latest_yaml` will
re-emit it; topologies that only have a graph payload will still see 422
because the mapper isn't built yet. Note: 422 here flows through the existing
`BffError::QuotaExceeded` path is **NOT** the right channel — we surface it as
a dedicated 422 with `code: "GRAPH_EMPTY"`, which means the BFF returns
`BffError::BadRequest` (400) is unsuitable; we use a custom 422 inside the
handler. See implementation for the exact branch.

## POST `/v1/architectures/import-yaml`

Replace a topology's `latest_yaml` with caller-supplied YAML. Validates the
YAML, persists `latest_yaml` and `last_validation_status` together (in one
optimistic-concurrency-checked update). Allowed even when validation fails —
the row is marked `last_validation_status: failed` and the YAML is stored so
the user can iterate.

Request:

```json
{
  "id": "<architecture id>",
  "yaml": "apiVersion: chv.kubedo.io/v1alpha1\n..."
}
```

Response (200):

```json
{ "result": { "status": "...", "summary": {...}, "findings": [...] } }
```

Errors: `400 BAD_REQUEST` (id/yaml blank), `403 FORBIDDEN` (viewer),
`404 NOT_FOUND` (unknown id), `409 CONFLICT` (concurrent edit).

## Severity → status table

| any error | any warning | result status |
| --------- | ----------- | ------------- |
| ≥ 1       | any         | `invalid`     |
| 0         | ≥ 1         | `warning`     |
| 0         | 0           | `valid`       |

`blocking: true` on a finding means the apply gate must refuse. In Phase 1
every `error`-severity finding is `blocking: true`; warnings and info are
`blocking: false`.

## Schema validation behavior

When the YAML fails JSON Schema validation, every individual schema violation
becomes one `Finding` (no aggregation), and **static checks are skipped** —
running graph-shape checks against a model that didn't pass schema
validation produces noisy duplicates. The result has `status: "invalid"` and
`summary.errors == findings.len()` for the schema-fail path.

## Stability guarantees

- `code` values are stable; they will not change once shipped.
- New codes may appear; UI must tolerate codes it doesn't know.
- Retired codes are kept commented in `codes.rs` and never re-used (treat
  them like proto field numbers per ADR-014).
