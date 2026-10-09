# ADR-025: Native monitoring without a mandatory monitoring stack

**Date:** 2026-10-09  
**Status:** Proposed  
**Authority:** ADR-009, ADR-016, ADR-017, ADR-002-WebUI

## Context

CHV needs Proxmox-style metrics for nodes, VMs, storage, and networking. Users must see accurate metrics without installing an agent inside every VM. External Prometheus, VictoriaMetrics, Netdata, and Grafana must remain optional.

Existing code includes `chv-agent-core/src/metrics_server.rs`, `chv-agent-runtime-ch/src/process.rs::vm_counters`, VM telemetry fields, `vm_metrics`, the BFF `/v1/metrics` summary, and Svelte monitoring pages. These are not a complete monitoring system. The node dashboard can generate synthetic history. The VM memory-used value is zero in the real runtime adapter. Some persistence failures are ignored. The existing `/metrics` Prometheus surface covers service health, not a complete native monitoring experience.

ADR-016 defines `chv-agent` (CellHV Core) as the sole VM runtime authority. ADR-017 forbids fabricated capability or statistics claims. Monitoring must respect both decisions.

## Decision

1. **Native monitoring is always available.** The installed `chv-agent` samples the node and its owned VMs. It does not require a guest agent, Prometheus, or another mandatory service.
2. **No second runtime authority.** Monitoring reads snapshots through bounded, read-only interfaces. It does not create, mutate, supervise, stop, or adopt VM processes.
3. **Separate lifecycle and measurement flows.** A periodic sampling task does not reuse desired-state reconciliation or VM state transitions to report high-frequency counters. It never blocks the reconciler.
4. **Measurements carry source and quality.** The system distinguishes configured resources, host-observed usage, VMM counters, and guest-reported usage. It shows `unsupported`, `missing`, or `stale`, never manufactured zeroes.
5. **Separate monitoring storage.** CHV uses a bounded `monitoring.db` for native history on small installations. Durable VM authority, journal, enrollment, and desired state stay outside it. Database and filesystem pressure cannot affect lifecycle availability.
6. **Single user interface.** The existing SvelteKit UI and authenticated backend-for-frontend (BFF) provide dashboards and historical queries. Browsers do not reach VMM sockets, Core internals, or agent ingestion endpoints.
7. **Compatibility.** Preserve existing Prometheus `/metrics` endpoints and the current `POST /v1/metrics` summary response. New `/v1/monitoring/*` routes supplement them, with explicit versioned data contracts.
8. **Optional expansion.** A separate agent adds guest operating-system metrics under ADR-026. External time-series exporters remain optional under ADR-027.
9. **Isolation.** Sampling is bounded in CPU, time, memory, labels, queues, and storage. Sampling failures report degraded monitoring health, not VM health.
10. **Availability.** Core continues running VMs during manager loss. Monitoring reports stale data and may retain a bounded local buffer for reconnection. Dropped samples do not change VM authority.

## Data flow

```text
Cloud Hypervisor (the VMM), Linux /proc/cgroup, chv-stord, chv-nwd
   -> read-only collectors within chv-agent (single authority)
   -> bounded snapshot/batch over existing authenticated node channel
   -> monitoring ingestion + monitoring.db (separate pool, backpressure)
   -> authorized BFF monitoring query API -> SvelteKit charts
                                   \
                                    -> optional external exporter
```

Core-native mode MUST receive its own monitoring read adapter. Implementers must not restore a legacy startup path to obtain metrics. Observed-state ownership must be proven; a desired VM state is not running-state evidence.

## Architectural boundaries

| Owner | Responsibility | Forbidden |
|---|---|---|
| `chv-agent` | Read-only host, VMM, and local provider sampling; bounded buffering | Second VM operation engine or privilege expansion |
| `chv-stord` and `chv-nwd` | Export provider-owned performance and health readings | Authority transfer to monitoring |
| `chv-controlplane` | Ingestion, registry, retention, history, alert evaluator | Telemetry changes to desired/observed VM lifecycle |
| BFF | Authentication, tenant/project filtering, queries | Direct browser-to-node calls |
| UI | Accurate state, charts, absent/stale indication | Fake generated utilization |
| Guest agent | Optional guest metrics and checks | VM lifecycle control |

## Alternatives rejected

- Bundle Grafana and Prometheus by default: exceeds the lightweight install goal.
- Install Netdata or a Checkmk server on each node: duplicates ownership and operational dependencies.
- Write all high-frequency metrics to the existing control-plane SQLite tables: threatens lifecycle performance and database growth.
- Reuse the VM state event channel as a 5-second stream: conflates health/state with performance telemetry.
- Put `vm_id` or `agent_id` on every global Prometheus internal metric: creates uncontrolled series growth.

## Compatibility and release gate

This ADR is a proposal. It does not assert that native monitoring is implemented. The qualified Cloud Hypervisor pin is v53.0. The implementation must verify each counter against that pinned API and real KVM.

Milestone **G1+G2** — the first qualified product milestone: native sampling (G1) plus real history and UI (G2) — requires real Linux node and VM CPU, memory, network, and disk observations; a restart; an induced monitoring-db outage; and an unchanged lifecycle test. Mark unavailable metrics unavailable, not zero. The first qualified topology remains the repository's currently qualified topology. Multi-node support requires a separate gate.

## Rationale

- Sampling inside the existing `chv-agent` (read-only) is the only design that adds no new runtime authority, daemon, or node failure path; a separate collector daemon was rejected for exactly that reason.
- Source and quality on every sample: without them, "no data" and "zero" are indistinguishable, and every downstream graph, alert, and export silently fabricates.
- A separate bounded monitoring store: high-frequency telemetry in the control-plane SQLite threatens the lifecycle database the platform must keep healthy; full external stacks (Prometheus/Grafana) violate the lightweight-install goal.
- Absolute counters with boot-epoch semantics: rates from raw deltas fabricate spikes on restart and migration; the design refuses to average that away.

## Consequences

- The platform owns a metric registry, ingestion, retention, rollup, and query surface — a real ongoing maintenance and qualification cost (G0–G5), not a bundled third-party stack's.
- Every new metric must declare source, kind, unit, and quality handling up front; the registry is a reviewed contract, not an open label space.
- The UI must render absence honestly (no-data, unsupported, and stale as distinct states), which is more work than drawing lines.
- The first qualified topology is the repository's current one; multi-node and vsock support require their own gates before any claim.

See [native monitoring spec](../component/chv-native-monitoring-spec.md), [metric contract](../contracts/chv-monitoring-metrics-v1.md), and [implementation plan](../../plans/2026-10-09-native-monitoring-implementation.md).
