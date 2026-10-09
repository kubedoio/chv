# CHV native monitoring implementation plan

**Date:** 2026-10-09  
**Status:** Proposed design campaign; implementation not started by this document  
**Base when authored:** `kubedoio/chv` main `cddb11ebb85e8b44b3446c87c72c84906f9e070e`  
**Goal:** complete native monitoring and optional guest monitoring without undermining the Core runtime.

## Scope and authority

Read ADR-025, ADR-026, ADR-027 and all monitoring contracts before coding. This plan changes no existing accepted authority or VMM qualification claim. Cloud Hypervisor (the VMM) v53.0 is the qualified pin in `docs/specs/ops/cloud-hypervisor-reference.md`. Preserve compatibility and existing `/metrics` Prometheus/ `POST /v1/metrics` BFF surfaces.

Success means: accurate native metrics without guest agent; accurate guest-enhanced metrics when installed; reliable bounded history, service checks, alert workflow, and optional outbound export. Never require Grafana, Checkmk, Netdata, Prometheus, VictoriaMetrics, or a new CHV runtime daemon.

## Design artifacts

| Artifact | Path |
|---|---|
| Native architecture | `docs/specs/adr/025-native-monitoring-architecture.md` |
| Guest agent and identity | `docs/specs/adr/026-optional-monitor-agent-and-guest-identity.md` |
| Retention/alerts/export | `docs/specs/adr/027-monitoring-history-alerts-and-export.md` |
| Node and VM sampler | `docs/specs/component/chv-native-monitoring-spec.md` |
| Optional guest agent | `docs/specs/component/chv-monitor-agent-spec.md` |
| History and alerting | `docs/specs/component/chv-monitoring-history-alerts-spec.md` |
| Metric registry | `docs/specs/contracts/chv-monitoring-metrics-v1.md` |
| Ingest and transport | `docs/specs/contracts/chv-monitoring-ingestion-v1.md` |
| Agent/plugin security | `docs/specs/contracts/chv-monitor-agent-security-plugins-v1.md` |
| Query and alert API | `docs/specs/contracts/chv-monitoring-query-alerts-v1.md` |
| Coding campaign | `docs/prompts/native-monitoring/README.md` |

## Mandatory acceptance gates

**G0 (design baseline, two halves):** **G0a (ratification):** Verify product authority, security, storage, and API contracts against current code; resolve any contradictions by a reviewed design amendment before implementation. Merging the design document package satisfies only the documentary half of G0. **G0b (empirical):** Validate pinned VMM counter schemas with fixtures captured from actual v53.0 responses on a real host (prompt 00's baseline report). Both halves must pass before G1 implementation starts.

**G1 (native sampler):** Host and VM CPU, memory, disk and network samples measure real loads. Source and no-data are truthful. CPU baseline/reset and migration do not produce spikes. Existing VM start/stop/restart passes under sampler pressure. Core-native does not start the legacy authority.

**G2 (history and native UI):** Isolated bounded storage, durable acceptance or explicit failure, rollups, secure history APIs, accurate node/VM graphs, and degraded/no-data states. Monitoring store full/unavailable cannot prevent VM lifecycle.

**G3 (guest agent):** Separate package, secure enrollment, scoped identity, rotation/revoke, Linux guest checks and process/filesystem/service discovery. No guest inbound port, no arbitrary command execution. Non-installed guests remain fully usable.

**G4 (alerts and plugins):** Typed local-check schema, reviewed local plugin execution, durable alert state, dedup, ack/silence, and secured webhook notifications. Test malicious guest data, plugin timeout/SSRF, and manager restart.

**G5 (vsock and external):** Version-gated vsock behind opt-in configuration with host ownership/CID and credential checks. Prometheus/VictoriaMetrics integration optional. Measure multi-node and scale before qualified claims.

## PR sequence

| PR | Task | Gate | Likely files |
|---|---|---|---|
| PR-0 | First execution PR **after the design package merges**: reconcile the design against latest main and capture the G0b fixture baseline | G0a, G0b | `docs/specs/*`, `docs/prompts/native-monitoring/*`, `docs/evidence/*` |
| PR-1 | Implement source adapters and native sampler | G1 | `crates/chv-agent-{runtime-ch,core}`, `crates/chv-hypervisor-api`, `cmd/chv-agent` |
| PR-2 | Versioned node ingest, isolated monitoring DB, robust backlog/retention | G2 | `proto/controlplane`, `crates/chv-controlplane-*`, `cmd/chv-controlplane/migrations` |
| PR-3 | Authenticated query API and real Svelte graphs | G2 | `crates/chv-webui-bff`, `ui/src` |
| PR-4 | Guest agent binary, packaging, enrollment and HTTPS | G3 | `cmd/chv-monitor-agent`, `crates/chv-monitor-*`, `packaging/*`, manager |
| PR-5 | Guest collectors, service discovery, checks and plugin sandbox | G4 | Guest crates, fixtures, UI |
| PR-6 | Alert engine, durable incidents, webhook outbox, UI | G4 | Control-plane store/service, BFF, UI |
| PR-7 | Optional vsock and guest/host mapping | G5 | VMM config, node transport, security, KVM harness |
| PR-8 | Optional exports and full scale/chaos qualification | G5 | Metrics adapters, tests, runbooks |

PRs should remain independently reviewable. G1 and G2 are a useful standalone product milestone. No PR may claim an unpassed gate. Require a frozen tested commit SHA for high-risk changes and record exact real-host evidence. Do not combine vsock, storage migration, or Core runtime authority refactors with the sampler.

## Concrete test matrix

| Scenario | Expected result |
|---|---|
| Idle VM and CPU burn | Correct CPU utilization over intervals; first sample unavailable |
| Guest with no agent | Host-observed metrics available; guest-only fields unsupported |
| Guest agent installed | Filesystems, service and process checks appear with source labels |
| VM start/stop and node reboot | Counter epochs reset, no negative/giant rates |
| VM migration | Old node ownership cannot ingest as new owner; history continuity is explicit |
| Node-to-manager partition | Running VMs unaffected; telemetry stale; buffer bounded |
| Manager SQLite monitoring file full | Ingest rejects/reclaims; lifecycle database and VM operations remain usable |
| Malicious tenant guest | Cannot change VM status, impersonate other target, read other tenant metrics |
| Agent cloned or revoked | Credential cannot be reused to misrepresent a second VM |
| Plugin hangs or exits invalid | Check unknown; agent alive; resource ceiling observed |
| Alert fires, ack, resolves | Correct persisted transition; no duplicate webhook storm |
| 1/10/100/500 VMs | Measured resource, storage and query costs; no unproven scale claim |

## Performance budgets to test, not promises

Target: native host/VM collection adds less than 1% sustained CPU on a normally loaded node at modest scale; guest agent less than 32 MiB RSS at baseline; bounded 256-KiB ingest batches; 5/10/15 second collection families; UI current view p95 under 500 ms for modest clusters. These are hypotheses requiring platform- and workload-specific benchmarks. Publish actual values, confidence, hardware, VM count, sample interval, and acceptance thresholds for scale tiers before enabling them.

## Migration and compatibility

1. Preserve existing `vm_metrics` readers and `POST /v1/metrics` summary. New storage is additive.
2. Introduce versioned samples and BFF APIs; avoid interpreting pre-v1 rows as higher-confidence measurements.
3. Preserve historical agent mode behavior during Core migration; do not reintroduce legacy authority in core-native.
4. Do not ship a guest agent package accidentally in the base installation. Declare supported OS/architecture and optional packages.
5. Rollbacks must leave Core operational if telemetry migrations or the agent fail. Monitoring DB history is never a prerequisite for VM start.

## Immediate next action

Start with [prompt 00](../prompts/native-monitoring/00-campaign-goal.md) to inspect the latest main and reconcile the design. Then execute staged prompts in order. Each PR requires tests, real-host evidence where applicable, security disclosure, updated runbooks, and a clear PASS/FAIL/BLOCKED verdict.
