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

`latest` includes `observed_at_ms`, `received_at_ms`, `source`, `quality`, `age_seconds`, `unit`, and `value` only when valid. `stale` is decided server-side from metric-specific thresholds and boot/incarnation state. Defaults: target-kind thresholds are 60 s (node) and 90 s (other targets); metric families on the guest agent's 60-second collection cadence (`vm.guest.fs.*`, `vm.guest.service.*`, `check.*`) use a 180 s window so a single missed collection does not read as stale.

## Alert rule API

All endpoints are POST with JSON bodies, gated by CHV's role tiers (viewer reads, operator mutations, administrator delivery test). CHV v1 roles are fleet-scoped — there are no projects yet, so rule and incident visibility follows the operator's fleet authorization. This is the same recorded v1 limitation as every other monitoring read.

| Endpoint | Purpose | Minimum role |
|---|---|---|
| `/v1/monitoring/alerts` | List incidents (paginated, filters) | Viewer |
| `/v1/monitoring/alerts/detail` | One incident with its transition history | Viewer |
| `/v1/monitoring/alert-rules` | List rules (paginated, filters) | Viewer |
| `/v1/monitoring/notifications/deliveries` | Recent notification delivery audit | Viewer |
| `/v1/monitoring/alert-rules/create` | Create typed rule | Operator |
| `/v1/monitoring/alert-rules/update` | Update with revision precondition | Operator |
| `/v1/monitoring/alert-rules/delete` | Delete with revision precondition | Operator |
| `/v1/monitoring/alerts/acknowledge` | Acknowledge incident (overlay) | Operator |
| `/v1/monitoring/alerts/silence` | Time-bound notification silence (overlay) | Operator |
| `/v1/monitoring/notifications/test` | Enqueue a delivery test event | Administrator |

Incidents persist in the existing operational `alerts` table (rows with `source = 'monitoring'`); rules, transitions, and the notification outbox live in the additive operational tables `alert_rules`, `alert_transitions`, and `notification_outbox` (migration `0061_alerts_incidents.sql`). None of this touches monitoring.db — incidents are workflow state, not telemetry.

### Flat rule wire shape

A rule body is **flat**: the common fields and the typed spec fields ride in one JSON object at the top level. `rule_type` is derived from the spec shape and returned; clients never send it.

Common fields:

| Field | Type | Constraints / default |
|---|---|---|
| `name` | string | required, 1..=128 printable bytes |
| `target_kind` | string | required, `node` or `vm` (per-target rules only in v1 — no fleet wildcards) |
| `target_id` | string | required, 1..=128 bytes |
| `severity` | string | required, `critical`, `warning`, or `info` |
| `for_seconds` | integer | hold before firing; default 300, 0..=86400 (0 fires immediately) |
| `recovery_seconds` | integer | condition-false window before resolving; default 120, 0..=86400 |
| `missing_data` | string | `unknown`, `fire`, or `ignore`; default `unknown` |
| `enabled` | boolean | optional, defaults to `true`; the create-from-template flow creates disabled rules (templates never auto-enable) |
| `rule_id`, `expected_revision` | — | update/delete only: the revision precondition |

The five typed spec shapes (every field at the top level of the same object):

Threshold — latest valid value against a bound:

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
  "severity": "warning"
}
```

Rate — reset-safe counter rate over a window (`window_seconds` 30..=3600, `threshold_per_second` finite ≥ 0; counter resets and epoch crossings are skipped, never fabricated into spikes, and a window covered less than half by real deltas reads as missing):

```json
{
  "name": "Guest NIC receive errors",
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "metric_id": "vm.guest.net.rx_errors_total",
  "operator": "greater_than",
  "threshold_per_second": 10,
  "window_seconds": 300
}
```

Availability — inverted: the condition is TRUE when the series is stale or not collected:

```json
{
  "name": "Node unreachable",
  "target_kind": "node",
  "target_id": "node-1",
  "metric_id": "node.cpu.capacity_ratio",
  "severity": "critical"
}
```

Check status — guest check inventory state:

```json
{
  "name": "Service down",
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "check_id": "service:g4-http.service",
  "status_match": "critical"
}
```

Group — bounded one-level AND/OR (2..=5 conditions, each one of the four simple shapes above; nested groups are rejected):

```json
{
  "name": "CPU pressure with the service degraded",
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "op": "and",
  "conditions": [
    {"metric_id": "vm.cpu.capacity_ratio", "operator": "greater_than", "threshold": 0.9},
    {"check_id": "service:g4-http.service", "status_match": "critical"}
  ]
}
```

`dimension_match` (optional on threshold, rate, and availability shapes) selects an exact series: at most 2 keys, every entry must be present-and-equal in the sample's dimensions, and the key order does not matter (the canonical form is part of the incident dedup key). All identifier-ish strings are bounded (1..=128 printable bytes, no control characters).

### Honesty rules

- **Unknown metrics are refused, not parked.** Every `metric_id` must exist in the published registry ([metric contract v1](chv-monitoring-metrics-v1.md)); create and update return `400 unknown_metric` instead of storing a rule that can never fire. `check_id` is not registry-validated (checks are per-guest discoveries) but is charset- and length-bounded.
- **Spec parsing is strict.** Each typed shape accepts exactly its own field set: unknown fields, missing fields, or a typo'd operator are `400 invalid_rule`, never a silent reinterpretation as another shape. A stored `spec` that no longer parses is a loud load error, never a silent rule skip.
- **Updates and deletes carry a revision precondition.** A mismatched `expected_revision` changes nothing and returns **409**; the client reloads the rule and retries. Revisions advance monotonically. Target identity is immutable — an update never moves a rule between targets (delete and recreate instead). Deleting a rule never deletes the incidents it produced; they are historical record.
- **The rule ceiling is a loud limit.** Creating a rule beyond the configured `monitoring.alerting.max_rules` (default 200) returns **409**, not a silent clamp.
- **Rule mutations are audited.** Create, update, delete, acknowledge, silence, and the delivery test append audit events with the acting user.

### Request/response shapes

Incidents list — body `{status?, target_kind?, target_id?, rule_id?, include_resolved?, limit?, offset?}` (`status` `pending|firing|resolved`; `include_resolved` default false; `limit` default 50, capped at 100) → `{incidents, total}`. `pending` is always included — pre-notification is not pre-visibility. Each incident carries `alert_id`, `status`, `severity`, `rule_id`, `rule_revision`, `dedup_key`, `target_kind`, `target_id`, `node_id`, `message`, `last_observed`, `evidence_from_ms`/`evidence_to_ms`, `pending_since_ms`, `first_occurrence_ms`, `last_occurrence_ms`, `opened_at`, `resolved_at`, and the `acknowledged_*`/`silenced_*` overlays.

Incident detail — body `{alert_id}` → `{incident, transitions}` (transitions carry `from_state`, `to_state`, `occurred_at_ms`, `reason`, `measured`; bounded to the most recent 100).

Rules list — body `{enabled_only?, target_kind?, limit?, offset?}` → `{rules, total}`; each rule is the flat wire shape plus `rule_id`, `rule_type`, `revision`, `created_by`, `created_at_ms`, `updated_at_ms`.

Create — flat rule body → `{rule}`. Update — flat rule body plus `{rule_id, expected_revision}` (a replacement of name, spec, severity, holds, and `missing_data`; `enabled` is optional — omitted keeps the current state, so a partial update never silently re-enables a disabled rule) → `{rule}` with the incremented revision. Delete — `{rule_id, expected_revision}` → `{deleted: true}`.

Acknowledge — `{alert_id}` → `{acknowledged: true}`; only active (pending/firing) incidents can be acknowledged, otherwise 404.

Silence — `{alert_id, duration_minutes}` XOR `{alert_id, until_ms}` (sending both, neither, a non-future `until_ms`, or `duration_minutes` outside 1..=10080 is `400 invalid_rule`) → `{silenced: true, until_ms}`; only active incidents, otherwise 404.

Delivery test — `{}` → `{enqueued: true, event_id}`; **409** when no notification destination is configured — an honest error, not a silent no-op.

Delivery audit — `{limit?}` (default 20, capped at 50) → `{deliveries}`: recent outbox events in any status, newest first, each with `event_id`, `alert_id`, `event_type`, `severity`, `target_kind`, `target_id`, `summary`, `channel`, `status`, `attempts`, `next_attempt_at_ms`, `last_attempt_ms`, `last_response`.

## Incident states

Incident states: `pending`, `firing`, `resolved`. Separate `acknowledged_at`, `acknowledged_by`, `silenced_until` fields. Do not conflate acknowledgment with recovery. Alerts carry resource identity, rule version, first/last occurrence, last good measurement, effective severity, and runbook link.

Implemented lifecycle (migration `0061_alerts_incidents.sql`, rows with `source = 'monitoring'` in the operational `alerts` table):

- A met condition first opens **pending**; it promotes to **firing** only after the condition has held for `for_seconds` (a zero hold fires immediately, recording both transitions). A pending incident whose condition clears before the hold is **deleted**, not stored — inactive is not a stored state. Resolved incidents persist with their transition history (`alert_transitions`).
- Active incident identity is the dedup key `{rule_id}:{target_kind}:{target_id}:{canonical-dimension-match}` — unique among pending/firing incidents, so a racing evaluation can never open a duplicate.
- **Acknowledgment and silence are overlays on active incidents only.** Acknowledgment records who and when; it never resolves the incident and never stops firing/resolved notifications. Silence suppresses notification **enqueue** while `silenced_until_ms` is in the future — the transition itself still happens and is still visible. Neither overlay clears on its own; only the recovery window resolves.
- Missing data never evaluates as a numeric zero. `missing_data: unknown` (default) records the gap on the incident without firing; `fire` treats absence as condition-true; `ignore` makes absence a no state change. Availability rules invert: a stale or uncollected series IS the condition. A firing incident is never resolved by absence — only by the condition being observably false for `recovery_seconds`.
- On evaluator restart, active incidents and their notification dedup keys are restored from the operational database; nothing depends on monitoring.db surviving.
- Alert counts on the overview, node, network, and cluster read surfaces count `status IN ('open','firing')` — operational `open` rows and monitoring `firing` rows — so pending churn does not flap the badge. In the events feed's state column, `resolved` wins over `acknowledged`.

## Notifications and integrations

Outgoing webhook payloads use a versioned envelope with exactly these ten fields, in this order — the field set is closed, which is the structural redaction guarantee (nothing beyond these fields can ever be emitted):

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

`event_type` is the lifecycle vocabulary `firing`, `resolved`, `acknowledged`, `delivery_failed`, `test` (`acknowledged` is reserved in the vocabulary — acknowledgment is an overlay and currently emits no notification). `summary` is rule name, observation, and severity — never raw payloads, agent claims, or command lines. `resource_url` is a relative UI path. Do not include tenant-secret fields, agent enrollment claims, process command lines, arbitrary plugin output or raw SQL.

Every webhook delivery is signed: the `x-chv-signature` header carries `v1=<HMAC-SHA256 of the raw request body keyed by the operator-configured secret, as 64 lowercase hex digits>`. Receivers verify the signature over the exact raw body bytes before parsing (see the [alerting runbook](../../runbooks/alerting.md) for a copy-pasteable recipe).

Delivery semantics:

- **Destinations come only from operator configuration** (`[monitoring.notifications]` in `controlplane.toml`) — that configuration IS the destination allowlist; alert rules and the UI can never direct a notification anywhere. Destinations are HTTPS-only, follow no redirects, carry no credentials in the URL, and are boot-validated against link-local and unspecified IP literals (the cloud-metadata hazard). Loopback and private ranges are allowed deliberately for internal receivers.
- Events are enqueued **per configured channel** (signed webhook, Slack adapter) and only when a destination exists — an unconfigured system enqueues nothing and accumulates no outbox rows; the UI is always a channel. The enqueue rides the same operational-database transaction as the incident transition it reports (both rows or neither).
- Delivery is **at-least-once** with idempotent event IDs (`INSERT OR IGNORE`): a crash between alert persistence and webhook send can replay an enqueue but never duplicate a notification. A claimed-but-unfinished batch returns on lease expiry (5 minutes), so a dispatcher crash resumes without a reaper.
- Outcomes: **2xx** marks the event delivered; **429, 5xx, and network failures** (connect, timeout, TLS) schedule a retry with capped exponential backoff (5 s · 2^attempt, capped at 1 h, ±20% jitter) up to the configured attempt limit (default 8); **any other 4xx** dead-letters immediately. Redirects are never followed.
- A dead-lettered event is audited and emits **one** `delivery_failed` courtesy event (itself never recursing) so the outage is visible on the surviving channel. A destination removed from configuration after events were enqueued dead-letters those events honestly instead of silently dropping them.
- The Slack adapter posts a single-line `{"text": "[<event_type>] <summary> (<severity>) — <resource_url>"}` payload, unsigned — Slack's incoming-webhook URL is its own credential — with the same HTTPS-only, no-redirect transport.
- The delivery audit (`POST /v1/monitoring/notifications/deliveries`) lists recent outbox events with status, attempts, and the last response class.

Use HTTPS and a strict destination allowlist to prevent server-side request forgery.

## Error model

Responses use typed error codes `invalid_range`, `unknown_metric`, `invalid_rule`, `unsupported_source`, `permission_denied`, `query_too_large`, `monitoring_unavailable`, `revision_conflict`. Alerting adds two conflict (409) classes: a rule revision precondition mismatch (the client reloads and retries) and the configured rule ceiling; the delivery test also answers 409 when no destination is configured. Alerting reads and mutations are backed by the operational database and do not return `monitoring_unavailable`. Return a request ID for operator correlation. Monitoring unavailable does not mean node or VM unhealthy.

## Acceptance

Test field-typed JSON roundtrips, tenant visibility, stale and missing points, large counter precision, uneven samples, resolution selection, excessive queries, alert pending/firing/recovery, ack/silence, webhook idempotency, failed delivery, and restart persistence. UI must pass accessibility, keyboard and no-fake-history tests.
