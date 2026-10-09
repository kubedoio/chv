# ADR-027: Monitoring storage, alerting, and optional export

**Date:** 2026-10-09  
**Status:** Proposed  
**Authority:** ADR-009, ADR-025, ADR-026

## Context

The existing control-plane database is an operational authority and must not become an unlimited time-series store. CHV also needs useful built-in alerts with recovery and acknowledgment, rather than requiring an Alertmanager service. External monitoring remains valuable but should not be a dependency.

## Decision

1. The native single-instance installation uses a bounded SQLite file `/var/lib/chv/monitoring/monitoring.db`. It has a separate SQL connection pool, migrations, maintenance tasks, and failure signals. Its contents are disposable telemetry, not authoritative VM state.
2. The deployment must enforce disk budgets with backpressure and eviction. **A separate file on the same filesystem does not provide disk-full isolation.** Reserve headroom, use bounded WAL/checkpointing, and test a full filesystem. Prefer a dedicated filesystem or strict project quota for tighter production isolation.
3. Keep metrics registration, agent binding, tenant identity, alert rules, and alert workflow metadata in durable control-plane storage. Keep high-volume samples and downsampled series in monitoring storage.
4. For the first release, sample every 5–15 seconds locally, batch at 15 seconds where cost permits, retain raw points for 48 hours, five-minute rollups for 30 days, and one-hour rollups for 180 days. Defaults are adjustable only within enforced storage and query budgets.
5. Store absolute counters and calculate rates from monotonic deltas. Record source and measurement quality. Do not turn process restarts, counter wrap, missing periods, migration, or reboot into giant rate spikes.
6. The alert evaluator is native and small. Supported rule types: absolute threshold, sustained threshold, missing/stale target, change rate, service/check status, and simple composite AND/OR on bounded expressions. No PromQL reimplementation in v1.
7. Persist alert state as `pending -> firing -> resolved`, plus acknowledgment and silence metadata. Deduplicate by rule and authorized target identity. Acknowledgment does not clear the underlying problem.
8. Native channels: UI alert center and signed, outbound HTTPS webhook. Slack may use a configured webhook adapter. Email and ticket-system adapters are later extensions. Never retry notifications without idempotency, backoff, bounded queues, and audit.
9. Export is optional. Existing Prometheus instrumentation continues. An authenticated, scoped scrape surface and optional remote-write-compatible exporter may be introduced separately. Do not expose tenant-specific series on unauthenticated `/metrics`.
10. The BFF provides paged, bounded history APIs and aggregates. No arbitrary SQL, arbitrary PromQL, or arbitrary time-series selector crosses the public API.
11. Retain event correlation IDs and source quality in all views. Dashboard failures do not mutate VM state. If monitoring storage is unavailable, service remains alive and UI shows monitoring `degraded`.
12. Scale-up beyond proven SQLite capacity is a measured decision. VictoriaMetrics remains an optional backend, selected by configuration after compatibility testing. Do not promise seamless backend conversion without a migration contract.

## Storage strategy

| Class | Owner | Example | Retention |
|---|---|---|---|
| Operational VM state | Core/control plane | Desired state, operation journal | Existing durable policy |
| Agent credentials and policy | Control plane | Claim hashes, target binding, rules | Until deleted/revoked plus audit policy |
| Recent raw time series | Monitoring store | CPU ratio, bytes and counters | 48 hours target |
| Downsampled time series | Monitoring store | 5-minute and hourly summaries | 30/180 days targets |
| Alert incidents | Durable alert store | Pending, firing, ack, resolved | Configurable incident policy |
| External export | External operator | Metrics written to existing monitoring | External retention policy |

## Explicit non-goals

- Distributed metrics database in the default installation.
- Guest log capture or command execution by default.
- Arbitrary queries over cross-tenant raw metrics.
- Billing-grade or quota-grade measurements based only on best-effort monitoring data.
- Changing `chv-stord`, `chv-nwd`, or runtime authority.

## Qualification

Validate 1/10/100/500 VM scenarios with realistic labels and enabled guest agents; measure memory, CPU, storage growth, query latency, and churn. Treat all targets as acceptance candidates, not achieved benchmarks. Prove monitoring-db unavailable, disk-full, interrupted rollup, evaluator restart, webhook timeout/retry, and manager/agent network partition while VM lifecycle still works.

See [history and alerts spec](../component/chv-monitoring-history-alerts-spec.md) and [query/alert contract](../contracts/chv-monitoring-query-alerts-v1.md).
