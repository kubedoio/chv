# Task Plan: CHV Implementation Roadmap

## Goal
Bring CHV from ~55% architecture readiness to production-ready (~90%), addressing critical gaps, ADR compliance failures, and feature completeness across 4 phases over 8 weeks.

---

## Phase 1: Data Integrity & Security Hardening (Week 1-2)
**Theme: "Nothing breaks silently"**

- [ ] 1.1 Generation monotonicity at store layer
  - Add `WHERE generation >= ?` to all desired-state writes in controlplane-store
  - Return `StaleGeneration` error when violated
  - Prevents split-brain from stale writes
  - **Files:** `crates/chv-controlplane-store/src/desired_state.rs`
  - **ADR:** 002

- [ ] 1.2 Enforce mTLS (remove optional bypass)
  - Make TLS mandatory for node-controlplane gRPC channels
  - Remove `INSECURE_SKIP_TLS` / plaintext fallback paths
  - Add startup validation that cert/key files exist and are valid
  - **Files:** `crates/chv-controlplane-service/src/server.rs`, `cmd/chv-agent/src/main.rs`
  - **ADR:** 002

- [ ] 1.3 Wire quota enforcement at create-time
  - Check VM CPU/memory/disk quotas before dispatching create task
  - Check volume count/capacity quotas before volume create
  - Return `QuotaExceeded` error (already defined in chv-errors)
  - **Files:** `crates/chv-controlplane-service/src/orchestrator.rs`, `crates/chv-webui-bff/src/handlers/vms.rs`

- [ ] 1.4 Remove SQLite CHECK constraints on status columns
  - Migration 0031: DROP CHECK on backup_jobs.status, vm_snapshots.status
  - Enforce enum values in application layer instead
  - Allows adding new states without table recreation
  - **Files:** `cmd/chv-controlplane/migrations/0031_*.sql`

**Exit Criteria:** `cargo test --workspace` passes; generation violations return proper error; mTLS-only connections work; quota rejection tested.

---

## Phase 2: Network Services & State Machine (Week 3-4)
**Theme: "Complete the contracts"**

- [ ] 2.1 Network mutation endpoint
  - Implement `mutate_network` in BFF (currently returns NotImplemented)
  - Support: rename, update CIDR, add/remove exposures, toggle NAT/DHCP/DNS
  - Dispatch through controlplane orchestrator
  - **Files:** `crates/chv-webui-bff/src/handlers/networks.rs`, `crates/chv-controlplane-service/src/orchestrator.rs`

- [ ] 2.2 nwd firewall enforcement
  - Replace stub logging with actual nftables rule application
  - Create/flush chains per network namespace
  - Apply rules from `FirewallRuleSpec` in fragment
  - **Files:** `crates/chv-nwd-core/src/executor.rs`, new `crates/chv-nwd-core/src/firewall.rs`
  - **ADR:** 005

- [ ] 2.3 nwd DHCP integration
  - Spawn/manage dnsmasq per network with DHCP scope from `DhcpScopeSpec`
  - PID tracking, config file generation, graceful reload on scope change
  - **Files:** `crates/chv-nwd-core/src/executor.rs`, new `crates/chv-nwd-core/src/dhcp.rs`
  - **ADR:** 005

- [ ] 2.4 nwd DNS integration
  - Configure dnsmasq DNS forwarding from `DnsScopeSpec`
  - Static record injection, forwarder configuration
  - **Files:** `crates/chv-nwd-core/src/executor.rs`, new `crates/chv-nwd-core/src/dns.rs`
  - **ADR:** 005

- [ ] 2.5 Node state machine completion
  - Add Discovered state (node registered but not bootstrapped)
  - Add Failed state (unrecoverable error, manual intervention required)
  - Add schedulability check: only place VMs on nodes in TenantReady state
  - **Files:** `crates/chv-agent-core/src/reconcile.rs`, `crates/chv-controlplane-service/src/orchestrator.rs`
  - **ADR:** 003

**Exit Criteria:** Network mutations work end-to-end; firewall rules applied via nftables; DHCP serves addresses; DNS resolves; no VM placed on non-TenantReady node.

---

## Phase 3: Partition Autonomy & Observability (Week 5-6)
**Theme: "Survive and observe"**

- [ ] 3.1 Partition policy gate (ADR-006)
  - During CP outage: nodes preserve state, allow stop/reboot, deny create/migrate
  - Heartbeat-based partition detection in agent
  - Store-layer gate: reject new placements when node reports partition
  - **Files:** `crates/chv-agent-core/src/reconcile.rs`, `crates/chv-controlplane-store/src/desired_state.rs`
  - **ADR:** 006

- [ ] 3.2 Wire histogram metrics to all async task paths
  - Record operation duration for: VM create, VM start, VM stop, snapshot, restore
  - Use `chv_observability::record_operation_duration()`
  - Add metric labels: operation, node_id, outcome
  - **Files:** `crates/chv-controlplane-service/src/orchestrator.rs`, `crates/chv-agent-core/src/reconcile.rs`
  - **ADR:** 009

- [ ] 3.3 Structured health checks
  - `/healthz` (liveness): process alive
  - `/readyz` (readiness): DB connected, controlplane reachable
  - Include dependency status in readiness response
  - **Files:** `cmd/chv-agent/src/main.rs`, `cmd/chv-controlplane/src/main.rs`

- [ ] 3.4 Agent server lock restructuring
  - Split tokio::sync::Mutex into per-resource locks (per-VM, per-volume)
  - Use message-passing (mpsc channels) for operations that cross I/O boundaries
  - Eliminate full-handler serialization
  - **Files:** `crates/chv-agent-core/src/agent_server.rs`
  - **ADR:** 010

- [ ] 3.5 Add down-migrations for migrations 0025-0031
  - Write `.down.sql` for each recent migration
  - Test rollback path
  - **Files:** `cmd/chv-controlplane/migrations/`

**Exit Criteria:** Agent detects partition and restricts operations; operation durations visible in Prometheus; health endpoints respond correctly; agent handles concurrent VM operations without serialization.

---

## Phase 4: Storage Backends & Production Hardening (Week 7-8)
**Theme: "Scale beyond one node"**

- [ ] 4.1 iSCSI backend for chv-stord
  - Implement `StorageBackend` trait for iSCSI targets
  - Target discovery, LUN management, session lifecycle
  - Integration with open-iscsi (iscsiadm)
  - **Files:** new `crates/chv-stord-backends/src/iscsi.rs`
  - **ADR:** 004

- [ ] 4.2 Ceph RBD backend for chv-stord
  - Implement `StorageBackend` trait for Ceph RBD
  - Pool/image management via librbd
  - Snapshot and clone support
  - **Files:** new `crates/chv-stord-backends/src/ceph_rbd.rs`
  - **ADR:** 004

- [ ] 4.3 buf breaking check in CI
  - Add `buf` CLI to GitHub Actions workflow
  - Run `buf breaking` against main branch on PR
  - Prevent proto contract regressions
  - **Files:** `.github/workflows/ci.yml`, `buf.yaml`

- [ ] 4.4 Frontend production refactor
  - Complete Tailwind-first migration (remove inline styles)
  - Fix svelte-check warnings
  - Fix command palette (Ctrl+K) Playwright test
  - Accessibility audit and ARIA improvements
  - **Files:** `ui/src/`

- [ ] 4.5 E2E integration tests
  - Add test harness that boots agent + controlplane + BFF
  - Test: VM lifecycle, quota rejection, snapshot, network mutation
  - Run in CI on merge to main
  - **Files:** new `tests/integration/`

**Exit Criteria:** iSCSI and Ceph backends pass integration tests; proto changes checked for compatibility; frontend svelte-check clean; E2E suite green in CI.

---

## Dependencies Between Phases

```
Phase 1 ──┬──> Phase 2 (state machine needs generation check)
           │
           └──> Phase 3 (partition policy needs mTLS + generation)
                    │
                    └──> Phase 4 (storage backends need health checks + metrics)
```

Phase 1 is prerequisite for all others. Phases 2 and 3 can overlap slightly (weeks 4-5). Phase 4 requires Phase 3's observability to validate.

---

## Architectural Decisions Required

| Decision | Options | Recommendation | When |
|----------|---------|----------------|------|
| BFF direct-SQLite vs gRPC | Keep SQLite / Switch to gRPC | Hybrid: SQLite for reads, gRPC for mutations | Phase 2 |
| Multi-node overlay network | VXLAN / Geneve / flat routing | Geneve (modern, extensible, kernel support) | Phase 4+ |
| PostgreSQL migration trigger | Node count / data volume | Defer until >3 nodes or >100 VMs | Phase 4+ |
| Feature flags | Env vars / config file / external | Config file (simple, no external deps) | Phase 3 |

---

## Success Metrics

| Metric | Current | After Phase 1 | After Phase 2 | After Phase 3 | After Phase 4 |
|--------|---------|---------------|---------------|---------------|---------------|
| ADR Compliance (PASS) | 2/10 | 3/10 | 5/10 | 7/10 | 9/10 |
| Architecture Readiness | 55% | 65% | 75% | 85% | 90% |
| Test Count | 24 | 35+ | 50+ | 65+ | 80+ |
| Critical Gaps | 6 | 2 | 1 | 0 | 0 |
| Production Blockers | 6 | 3 | 1 | 0 | 0 |

---

## Risk Mitigation

| Risk | Phase | Mitigation |
|------|-------|------------|
| nwd firewall breaks existing VMs | 2 | Feature-flag nftables enforcement; test with dry-run mode first |
| iSCSI/Ceph unavailable in CI | 4 | Mock backend trait in tests; real integration tests run on tagged hardware |
| Lock restructuring introduces races | 3 | Add loom-based concurrency tests; gradual rollout per-handler |
| Partition policy false positives | 3 | Conservative timeout (30s); require 3 consecutive heartbeat misses |

---

## Status
**Plan complete** — Ready for phase-by-phase execution. Start with Phase 1.1 (generation monotonicity).
