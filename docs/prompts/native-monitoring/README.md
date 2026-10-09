# Native monitoring implementation campaign

**Status:** Proposed, no implementation claims  
**Design base:** `main` `cddb11ebb85e8b44b3446c87c72c84906f9e070e` at authorship

This is the staged implementation campaign for CHV native monitoring and the optional guest monitoring agent.

## Read first

- [Implementation plan](../../plans/2026-10-09-native-monitoring-implementation.md)
- [ADR-025](../../specs/adr/025-native-monitoring-architecture.md), [ADR-026](../../specs/adr/026-optional-monitor-agent-and-guest-identity.md), [ADR-027](../../specs/adr/027-monitoring-history-alerts-and-export.md)
- [Native spec](../../specs/component/chv-native-monitoring-spec.md), [agent spec](../../specs/component/chv-monitor-agent-spec.md), [history/alerts spec](../../specs/component/chv-monitoring-history-alerts-spec.md)
- [Metrics](../../specs/contracts/chv-monitoring-metrics-v1.md), [ingestion](../../specs/contracts/chv-monitoring-ingestion-v1.md), [agent security](../../specs/contracts/chv-monitor-agent-security-plugins-v1.md), [queries/alerts](../../specs/contracts/chv-monitoring-query-alerts-v1.md)
- `AGENTS.md`, `docs/governance/DOCUMENTATION_STANDARD.md`, `CONTRIBUTING.md`, `docs/release/PIPELINE.md` when packaging changes

## Prompts and gates

| Prompt | Goal | Gate |
|---|---|---|
| [00 Campaign goal](00-campaign-goal.md) | Reconcile latest facts; register PR sequence and exact gates | G0 |
| [01 Native sampling](01-native-sampling.md) | Correct read-only node/VM/provider measurement | G1 |
| [02 History + Web UI](02-history-api-ui.md) | Bounded storage, secure queries, real charts | G2 |
| [03 Agent and enrollment](03-guest-agent-enrollment.md) | Separate Linux agent, packaging, authenticated ingestion | G3 |
| [04 Checks and plugins](04-checks-and-plugins.md) | OS collectors, service discovery, safe custom checks | G4 |
| [05 Alerts and integration](05-alerts-and-export.md) | Alert state, notification outbox, optional export | G4/G5 |
| [06 Vsock and qualification](06-vsock-and-qualification.md) | Opt-in vsock, scale, chaos, production claims | G5 |

Run each prompt with repo access. Each prompt requires a distinct reviewable PR or an explicitly justified split. Do not combine implementation stages into a monolithic PR or claim successful real-host tests without attached evidence. A stage with unqualified functionality ends **BLOCKED**, not PASS.
