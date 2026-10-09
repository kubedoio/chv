# Prompt 02 — Bounded history, authenticated API and honest UI (G2)

Prerequisite: G1 passed. Implement [ADR-027](../../specs/adr/027-monitoring-history-alerts-and-export.md), [history/alerts spec](../../specs/component/chv-monitoring-history-alerts-spec.md), [ingestion v1](../../specs/contracts/chv-monitoring-ingestion-v1.md), and [query/alerts v1](../../specs/contracts/chv-monitoring-query-alerts-v1.md).

## Goal

Persist and render real monitoring history inside CHV without a mandatory external service. Preserve the VM authority and main control-plane database.

## Tasks

1. Add a versioned node metric batch method under `proto/controlplane` on the existing authenticated channel. Use `tonic-prost-build`; do not hand-edit `gen/rust`. Enforce source node identity and current VM ownership/incarnation.
2. Implement durable deduplication `sender+boot_id+sequence`, replay-conflict checks, size/rate/series caps and safe batch errors. ACK only a documented durable outcome. Do not allow manager backpressure to pause reconciliation.
3. Add separate SQLite `monitoring.db`, pool, schema migrations and retention. Include exact integer counters, bounded series/dimension dictionaries, timestamps, source, kind, unit and quality. Existing `vm_metrics` is a compatibility table; do not destructively repurpose it.
4. Implement idempotent raw and 5m/1h rollups, reset-safe rates, headroom/cap enforcement, bounded query work, periodic WAL checkpoint and clear degraded signals. Document same-filesystem disk-full risk and test it.
5. Implement the v1 BFF read APIs with RBAC/project filters, quality and source fields, correct timestamp handling, no arbitrary SQL/PromQL, and pagination/point limits. Preserve `POST /v1/metrics` and Prometheus `/metrics`.
6. Update overview, node details, VM details, storage and network charts. Remove all `Math.random` sample generation and false durable-history claims. Render unsupported, no history, stale, and agent-disabled states distinctly.
7. Add view selection 1h/6h/24h/7d/30d. Render units correctly: memory provisioning versus host-accounted versus guest available, CPU cores vs percentage, bytes/sec vs bits/sec. Add labels for source and sample age.
8. Add operator runbook for retention, damage recovery and monitoring-disabled operation.

## Tests

- Protobuf compatibility, malformed batch/replay, wrong target, cross-project read, missing data, counter exactness, partial raw writes, manager restart, storage corruption, disk-full and WAL growth.
- Frontend unit/E2E: real time-range data, 0 vs absent, source labels, accessible charts, tab refresh, no synthetic points, failed network, unauthorized target.
- Multi-node remains unqualified until separate qualification. Run native G1 smoke after each storage/UI change.

## Gate

G2 PASS requires usable standalone native dashboards and real, bounded, persisted history on a qualified real-host VM. Trigger a storage failure and show both monitoring degraded and VM lifecycle still operational. No guest agent or external server may be needed.
