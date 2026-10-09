# Prompt 00 — Deliver complete CHV monitoring without compromising VM authority

You are the lead Rust systems engineer for `kubedoio/chv`. Implement native Proxmox-like observability and an optional Checkmk-style Linux monitoring agent, in independently reviewable PRs. Read this campaign and the authority documents. **Do not implement a parallel VM runtime.**

## Before code

1. Read `AGENTS.md`, `CONTRIBUTING.md`, `docs/governance/DOCUMENTATION_STANDARD.md`, and the current branch's architecture.
2. Read `docs/plans/2026-10-09-native-monitoring-implementation.md`, ADR-025/026/027, all three monitoring component specs, and all four v1 contracts.
3. Read accepted ADR-009, ADR-016 and ADR-017, the current Core migration map, production-readiness analysis, and `docs/specs/ops/cloud-hypervisor-reference.md`.
4. Audit implementation files: `crates/chv-agent-runtime-ch/src/process.rs`, `crates/chv-agent-core/src/metrics_server.rs`, `cmd/chv-agent/src/main.rs`, `crates/chv-controlplane-service/src/telemetry.rs`, `crates/chv-controlplane-store/src/observed_state.rs`, existing SQLite migrations, `crates/chv-webui-bff/src/handlers/metrics.rs`, `ui/src/routes/observability/+page.svelte`, `NodeHealthDashboard.svelte`, and `VmMetricsTab.svelte`.
5. Confirm current `main` HEAD and pin VMM version from docs. Source audit facts may have changed since `cddb11e`; record changes and do not invent missing interfaces.
6. Check upstream pinned v53.0 OpenAPI/counter response, not only generic latest documentation. Capture real response fixtures before assuming field paths.

## Outcome

Implement these user-facing capabilities:
- No guest agent needed: node CPU, memory, load, physical and logical storage, networking; VM CPU, host-side memory, configured RAM, block I/O and network counters; node/VM freshness, available/unavailable states, and history.
- Optional `chv-monitor-agent`: Linux guest CPU/memory, filesystems/inodes, process/service checks, network errors and interfaces, service discovery and locally approved safe plugins.
- Native SvelteKit dashboards, time ranges, alert state, acknowledgment/silencing, webhook/Slack integration.
- Local bounded SQLite history, optional secure Prometheus/VictoriaMetrics export.
- Eventually opt-in virtio-vsock for isolated guests, with separate qualification.

## Invariants

- `chv-agent` stays the only Core authority. Monitoring cannot mutate VMM, VM journal, reconciliation, storage, or network.
- A manager or monitoring database crash cannot stop running VMs.
- No mandatory Prometheus, VictoriaMetrics, Grafana, Checkmk, Netdata, sidecar or guest agent.
- The guest agent has no lifecycle privilege, even over vsock.
- Metrics are real and qualified: no synthetic history, zero-as-missing, counter overflow, inaccurate unit, invented guest memory, or desired-running as observed-running.
- Guest telemetry has scoped authentication and target ownership checks. The UI is authorization-filtered.
- All writes and queries have CPU, memory, time, retention, batch, and series limits. No uncontrolled plugin or remote-code path.
- Backward compatibility for Prometheus `/metrics`, the current BFF `POST /v1/metrics`, and supported legacy modes.
- Never silently upgrade the VMM or override accepted ADRs.

## Implementation procedure

Create a design baseline report with a fact/evidence table and exact code citations. If a contract contradicts code or current policy, propose a narrow correction to the **proposed** documents, not a hidden workaround.

Execute prompts 01–06 in order, with one or more PRs per stage. For every PR record: scope; exact base and head commits; files changed; contract clauses satisfied; added tests; command outputs; real-host evidence; known limitations; security implications; and PASS/FAIL/BLOCKED conclusion. Do not merge an unqualified feature as production-ready. Keep old paths compatible or fail explicitly.

Follow the repository's high-risk disclosure rules for mTLS, credentials, process ownership, systemd privileges, schema migrations, and VMM device configuration. Use staged feature flags where appropriate. Write a runbook for installing/removing the optional agent and recovering monitoring storage.

## Completion definition

Monitoring is **fully implemented** only after G0–G5 and acceptance suites pass on supported topology/OS/VMM versions. If G5 vsock or scale is incomplete, report native+guest features that passed and list G5 separately as BLOCKED. A UI screenshot, mock test, or green cargo build does not demonstrate KVM or production readiness.
