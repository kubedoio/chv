# CHV monitoring query and alert API contract v1

**Status:** Proposed  
**Authorities:** [ADR-025](../adr/025-native-monitoring-architecture.md), [ADR-027](../adr/027-monitoring-history-alerts-and-export.md)  
**Metrics:** [metric contract v1](chv-monitoring-metrics-v1.md)

## API boundary

The browser uses the authenticated backend-for-frontend (BFF) in `chv-controlplane`. The browser MUST NOT call `chv-agent`, Cloud Hypervisor (the VMM), provider sockets, a monitoring store, or the guest ingestion route. Every API enforces CHV's current role and project/resource authorization. No arbitrary SQL, PromQL, label expressions, or raw agent credentials are accepted.

The current `POST /v1/metrics` inventory summary remains compatible. These are **new proposed endpoints**. Use POST for parameter-rich query operations, consistent with the existing BFF. Write operations use CHV's existing authorization and mutation conventions.

## Read API

| Endpoint | Method | Purpose |
|---|---|---|
| `/v1/monitoring/catalog` | GET | Metric registry, capabilities, units, sources, per-metric source preference, quality |
| `/v1/monitoring/overview` | POST | Fleet/node/VM health and measured resource summaries |
| `/v1/monitoring/current` | POST | Latest authorized resource samples |
| `/v1/monitoring/history` | POST | Time-range samples with bounded resolution |
| `/v1/monitoring/checks` | POST | Guest discovered services/checks, freshness, status |
| `/v1/monitoring/agents` | POST | Paginated agent inventory and enrollment state |
| `/v1/monitoring/alerts` | POST | Paginated incidents, including acknowledged/silenced states |
| `/v1/monitoring/health` | GET | Monitoring subsystem availability, age and data loss |

Example history request:

```json
{
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "metric_ids": ["vm.cpu.cores_used", "vm.cpu.capacity_ratio"],
  "from_ms": 1791572400000,
  "to_ms": 1791576000000,
  "max_points_per_series": 240,
  "resolution": "auto"
}
```

Response:

```json
{
  "schema_version": 1,
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "series": [
    {
      "metric_id": "vm.cpu.cores_used",
      "source": "vm_cgroup",
      "unit": "cores",
      "kind": "gauge",
      "points": [
        {"timestamp_ms": 1791575990000, "value": 1.25, "quality": "valid"}
      ],
      "coverage_ratio": 0.96
    }
  ],
  "generated_at_ms": 1791576001000,
  "truncated": false
}
```

A series may be **partially missing**: valid and non-valid points interleaved. A non-valid point carries `timestamp_ms` and `quality` with `value` absent — never a zero, NaN, or interpolated filler:

```json
{"timestamp_ms": 1791575980000, "quality": "unsupported"}
```

Downsampling MUST exclude non-valid points from both numerator and denominator; `coverage_ratio` counts valid points only. Counter responses carry decimal-string integer values, not imprecise JavaScript numbers.

A **missing series** returns an empty `points` array and a reason. Absence classification is one mapping, not three vocabularies — the ADR prose terms, the sample `quality` enum ([metric contract v1](chv-monitoring-metrics-v1.md)), and the wire series reasons correspond as follows:

| ADR prose | Point `quality` | Series reason (wire) |
|---|---|---|
| unsupported | `unsupported` | `unsupported` |
| missing (source down or never collected) | `unavailable`, `insufficient_samples` | `not_collected` |
| missing (nothing stored in the requested range) | — no points exist | `no_history` |
| stale | `stale` | `stale` |
| — (point-level only) | `invalid` | `not_collected` |

`invalid` is point-level only: an invalid sample appears as a non-valid point, and a series containing only invalid points reports `not_collected`.

It MUST NOT synthesize zero-valued points for any absence class.

Multi-source metrics (for example `vm.cpu.cores_used` from `vmm` and `vm_cgroup`, or `vm.block.*` from `vmm` and `storage_provider`) return **one series per source that has stored data**, labeled by `source`. A request may narrow sources with an optional `sources` filter. When a consumer wants a single series, the server selects the most authoritative available source as declared by the per-metric source-preference order in the metric registry (exposed through `/v1/monitoring/catalog`); the source label is always returned so the UI can display it.

## Query limits and constraints

Defaults: at most 8 metric IDs, one target for detailed history, 1000 points/series hard ceiling, 30 days detailed query and 180 days aggregated view; configured pagination for fleet lists, and no more than 100 targets in one overview. The server selects or validates resolution based on requested period, retention tier and point cap. It rejects an excessive range with `400 invalid_range` or `413 query_too_large`. Time inputs are integer epoch milliseconds. Client-clock time is not trusted. User permissions are checked **before** reading the target or presenting metric existence.

`latest` includes `observed_at_ms`, `received_at_ms`, `source`, `quality`, `age_seconds`, `unit`, and `value` only when valid. `stale` is decided server-side from metric-specific thresholds and boot/incarnation state.

## Alert rule API

| Endpoint | Method | Purpose | Minimum role |
|---|---|---|---|
| `/v1/monitoring/alert-rules` | POST | List authorized rules | Viewer |
| `/v1/monitoring/alert-rules/create` | POST | Create typed rule | Project admin/appropriate operator |
| `/v1/monitoring/alert-rules/update` | POST | Update with revision precondition | Project admin/appropriate operator |
| `/v1/monitoring/alert-rules/delete` | POST | Delete with revision precondition | Project admin/appropriate operator |
| `/v1/monitoring/alerts/acknowledge` | POST | Acknowledge incident | Authorized operator |
| `/v1/monitoring/alerts/silence` | POST | Time-bound silence | Authorized operator |
| `/v1/monitoring/notifications/test` | POST | Authorized delivery test | Administrator |

These are proposed paths. They do not supersede the existing `alerts` table and the node/overview alert-count read surfaces until a compatible migration and route review succeeds.

Typed rule example:

```json
{
  "name": "Guest filesystem nearly full",
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "metric_id": "vm.guest.fs.available_bytes",
  "dimension_match": {"mount_id": "/var"},
  "operator": "less_than",
  "threshold": 2147483648,
  "for_seconds": 300,
  "recovery_seconds": 120,
  "missing_data": "unknown",
  "severity": "warning",
  "enabled": true
}
```

`missing_data` is `unknown`, `fire`, or `ignore`, with defaults chosen per rule class. Missing data never evaluates as a numeric zero. Rule changes require a version/revision precondition and audit trail.

## Incident states

Incident states: `pending`, `firing`, `resolved`. Separate `acknowledged_at`, `acknowledged_by`, `silenced_until` fields. Do not conflate acknowledgment with recovery. On evaluator restart, restore active incident and notification dedup keys. Alerts carry resource identity, rule version, first/last occurrence, last good measurement, effective severity, and runbook link. Storing this incident model in the existing `alerts` table requires a compatible additive migration and column review — the current schema predates several of these fields, so the implementing PR must not assume the table as-is.

## Notifications and integrations

Outgoing webhook payloads use a versioned envelope:

```json
{
  "schema_version": 1,
  "event_id": "d0b2a2f7-2a77-4f86-ae55-53f0c8bf4aac",
  "incident_id": "f4528846-4925-435e-a69a-a4a147501527",
  "event_type": "firing",
  "severity": "warning",
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "summary": "Guest filesystem nearly full",
  "occurred_at_ms": 1791576000000,
  "resource_url": "/vms/83dab870-4903-48a9-9d37-486e100ed009"
}
```

Sign requests with an operator-configured secret (for example HMAC-SHA256) and send idempotent event IDs. Do not include tenant-secret fields, agent enrollment claims, process command lines, arbitrary plugin output or raw SQL. Use HTTPS and a strict destination allowlist to prevent server-side request forgery.

## Error model

Responses use typed error codes `invalid_range`, `unknown_metric`, `unsupported_source`, `permission_denied`, `query_too_large`, `monitoring_unavailable`, `revision_conflict`. Return a request ID for operator correlation. Monitoring unavailable does not mean node or VM unhealthy.

## Acceptance

Test field-typed JSON roundtrips, tenant visibility, stale and missing points, large counter precision, uneven samples, resolution selection, excessive queries, alert pending/firing/recovery, ack/silence, webhook idempotency, failed delivery, and restart persistence. UI must pass accessibility, keyboard and no-fake-history tests.
