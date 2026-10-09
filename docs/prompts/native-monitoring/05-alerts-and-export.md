# Prompt 05 — Native alerts, notifications and optional observability export (G4/G5)

Prerequisites: G2 storage/UI, G3 guest identity and G4 guest checks available. Follow [ADR-027](../../specs/adr/027-monitoring-history-alerts-and-export.md), [history/alerts spec](../../specs/component/chv-monitoring-history-alerts-spec.md), and [query/alert API v1](../../specs/contracts/chv-monitoring-query-alerts-v1.md).

## Goal

Provide lightweight built-in alert lifecycle and useful notifications without an Alertmanager dependency. Preserve the current Prometheus surface and allow optional external VictoriaMetrics/Prometheus pipelines.

## Tasks

1. Implement a typed, bounded alert evaluator: thresholds, sustained thresholds, reset-safe rate, missing data, guest check status, and limited AND/OR groups. No arbitrary PromQL/SQL or code execution.
2. Persist rule revision, `pending`/`firing`/`resolved` incident state, acknowledgment, silence, evaluator cursor, notification dedup key and audit events. Reuse existing CHV alert persistence where compatible; otherwise introduce additive migrations.
3. Guarantee time-based hold and recovery evaluation. Do not treat missing samples as numeric zero. Acknowledgment suppresses repeated notifications under policy but does not resolve the incident.
4. Add rule CRUD and incident read/action APIs through authorized BFF. Use revision preconditions and per-project RBAC. Prevent viewer rule modification and cross-tenant incident access.
5. Build a bounded durable notification outbox, at-least-once delivery, dedup event IDs, exponential backoff, retries/dead-letter and delivery status. Support configured signed HTTPS webhooks plus a Slack webhook adapter. Do not embed credentials in payloads or logs.
6. Add UI rules, incidents, actions, last observed metric/check value, evidence window, and delivery audit, with links to node/VM history. Start with node unreachable, collector stale, VM CPU pressure, VM storage near full, agent disconnected, guest filesystem full and service down. Defaults must not alert on unsupported metrics.
7. Preserve existing `/metrics` instrumentation and recording/alert rule files. Add optional secure export of selected data to an external monitoring backend, with explicit opt-in, bounded queues, data filtering and per-project policy. Never make remote export required for local graphs.
8. Document alert delivery secret rotation, webhook allowlist, notification outage, evaluator restart, and disabling the optional guest agent.

## Tests

- Threshold hold, threshold crossing and recovery, missing/unsupported data, agent disappearance, source disagreement, rule revision conflict.
- Crash between alert persistence and webhook send, duplicate webhook, timeout/429/500, permanent 400, restart, silence expiry, acknowledge-while-firing, replayed metric batches.
- Cross-tenant BFF and webhook payload redaction, SSRF/redirect bypass, stored XSS attempt in check summary.
- Optional exporter down, slow or full queue: native monitoring and VM lifecycle unaffected.
- Unit, integration, frontend and real-guest service-stop-to-alert acceptance tests.

## Gate

G4 PASS requires persistent incident behavior with deterministic alerts, no duplicate notification storms, and secure project-scoped actions. Optional exporter must be disabled by default and non-blocking. Do not claim Prometheus alert parity.
