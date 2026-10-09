# CHV monitoring history, alerts, and exports component specification

**Status:** Proposed  
**Decision:** [ADR-027](../adr/027-monitoring-history-alerts-and-export.md)

## Scope

The control plane provides a small native history and alerting capability. It does not bundle Grafana, Prometheus, Alertmanager, or a second monitoring server. Operators can opt into external monitoring without losing native dashboards.

## Persistence separation

Use `/var/lib/chv/monitoring/monitoring.db` for bounded samples and rollups. Control-plane state, user accounts, enrolled agent credentials, policy, and alert incidents (pending, firing, acknowledged, resolved) stay in the existing durable control-plane store — specifically the existing `alerts` table, not a new alert database. Separate connections and workers. Never hold operational DB locks during a monitoring query.

Proposed tables in **monitoring.db**:

```sql
CREATE TABLE monitoring_samples_v1 (
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  metric_id TEXT NOT NULL,
  dimensions_hash TEXT NOT NULL,
  source TEXT NOT NULL,
  observed_at_ms INTEGER NOT NULL,
  received_at_ms INTEGER NOT NULL,
  value REAL NOT NULL,
  quality TEXT NOT NULL,
  boot_id TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (
    target_kind, target_id, metric_id,
    dimensions_hash, source, observed_at_ms, boot_id
  )
);
CREATE INDEX monitoring_samples_lookup_v1
  ON monitoring_samples_v1 (
    target_kind, target_id, metric_id, observed_at_ms
  );
```

This is a **design outline**, not a production migration. Actual schema must include bounded dimension dictionaries, monotonic counter reset markers, integer-safe handling of byte counters, authenticated tenant/project mapping, retention indexes, and migration versioning. Do not store large `u64` counter values in IEEE-754 floats. Prefer a typed sample value in implementation and integer columns where appropriate. `boot_id` participates in the primary key so a source restart cannot collide two samples at the same `observed_at_ms`; series without a boot identity (no restartable counter source) use the empty-string default, and implementations may substitute a cleaner sentinel (for example a generated `identity_epoch` column) as long as restart-distinct samples cannot collide.

Rollups use counters (last-first/reset-safe delta), gauges (min/max/avg/count and last), and quality coverage. The schema must preserve unit and metric kind. Rollup windows use UTC-aligned boundaries and are idempotent. Do not average CPU percentages without a valid time weighting policy.

## Storage budget

Initial targets: local sampling 5/10/15 seconds by family; raw history 48 hours; five-minute rollups 30 days; one-hour rollups 180 days. These are configurable **targets**, not proven resource-cost claims. Apply a hard data-size budget and query quotas. SQLite on a shared filesystem does not isolate disk-full risk. Monitor available filesystem headroom; stop ingesting early, reclaim old partitions/rows, checkpoint safely, and mark monitoring degraded. Test rollback, partial commit and interrupted compaction.

The manager only accepts batches within bounded size, age, and rate budgets. Dropping points is preferred to blocking lifecycle. The drop count and last ingestion error are operator-visible. Retain alert incident metadata even if raw measurements are deleted.

## Query behavior

The authenticated backend-for-frontend (BFF) API exposes current and historical samples, downsample selection, newest timestamp, gap/stale quality, and limited aggregate views. Use typed metric selectors; reject arbitrary expressions and unknown metrics. Enforce project/tenant scope and limit total targets, points, and time range. Reject forbidden targets with the platform's normal access-control semantics. APIs must paginate or explicitly cap results.

The frontend renders source, unit, quality, available time range, and sample age. If history is missing, show no history. Never synthesize random samples. Error, stale, unsupported, and no-data are distinct states.

## Alert evaluator

A small rule engine evaluates only known metric IDs and checks:

- Threshold: `value > limit`, `value < limit`, rate and duration.
- Availability: metric staleness, node disconnect, guest agent unreachable.
- Status: check `warning`/`critical`/`unknown`.
- Group condition: bounded AND/OR across explicitly named checks.
- Per-rule consecutive hold-down / minimum duration and recovery duration.
- Silence schedule, acknowledgment and deduplicated notification states.

State machine:

```text
inactive -> pending -> firing -> resolved -> inactive
                 |         |
                 +-> inactive (cleared before hold)
                           +-> acknowledged (overlay only)
                           +-> silenced (notification overlay only)
```

Acknowledged/silenced do not mean cleared. Persist incidents and transitions. Evaluator restarts restore state and avoid duplicate notifications. Every event includes reason, measured value, evidence window and link to scoped resource detail.

No unbounded alert expressions, arbitrary code, or public PromQL editor in v1.

## Notification integration

A notification dispatcher reads a bounded durable outbox, sends signed outbound HTTPS webhook payloads, retries with capped exponential backoff, records delivery attempt and response class, and dead-letters permanent failures. A Slack webhook adapter can build on the same mechanism. Per-project routing and authorization apply. Never include credentials, process command lines, or sensitive raw labels in payloads.

The system should support standard alert lifecycle events: `firing`, `resolved`, `acknowledged`, `delivery_failed`. Notification adapters must not evaluate alert rules independently.

## External monitoring

Preserve existing Prometheus endpoints and `monitoring/rules/*.yml`. Scope detailed resource export behind authentication/authorization or administrator-configured secure network controls. Optional remote-write/vmagent/VictoriaMetrics deployment must be separately configured and must not become necessary for built-in graphs. Support backpressure and mark export as degraded without affecting local monitoring.

## Acceptance

Prove data retention, query budget, disk-full headroom, WAL checkpoint, storage corruption, manager crash during rollup, replayed samples, metric-source conflict, alert restart, repeated flapping, acknowledgement, silencing, webhook timeout, and tenant isolation. Run tests at 1/10/100/500 VMs with realistic service discovery. Publish measurement results before claiming scale or low resource use.
