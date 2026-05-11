# Product Readiness Gap Analysis — Final Report

**Date**: 2026-05-11  
**Branch**: main (post PR #45 merge)  
**Methodology**: Spec-vs-implementation gap analysis across 13 backend ADRs, 5 WebUI ADRs, 6 component specs, 6 WebUI specs, 4 ops specs  
**Previous Assessment**: 2026-05-08 (5.4/10). This is the updated assessment after PR #43-#45 merges.

---

## Overall Completeness: 67% (6.7 / 10)

| Domain | Score | Weight | Weighted | Change |
|--------|-------|--------|----------|--------|
| Control Plane (orchestrator, reconcile, BFF) | 7.0 | 30% | 2.10 | +2.0 |
| Node Components (agent, stord, nwd) | 6.5 | 25% | 1.63 | +1.5 |
| WebUI / BFF | 7.5 | 25% | 1.88 | +1.0 |
| CLI / Operations | 5.0 | 20% | 1.00 | +0.0 |
| **Total** | | **100%** | **6.60** | **+1.2** |

**Verdict**: Late alpha / early beta. The happy path works end-to-end for single-node deployments. Multi-node scenarios (migration, overlay networking) are partially wired. Operational tooling and production hardening remain the largest gaps.

**Phase Assessment**:
| Phase | Target | Status |
|-------|--------|--------|
| Phase 1: Stability | Solid control plane + agent lifecycle | 85% complete |
| Phase 2: Features | Migration, storage, networking | 55% complete |
| Phase 3: Production | mTLS, monitoring, upgrades, operational tooling | 30% complete |

---

## ADR Compliance Matrix

| ADR | Title | Compliance | Detail |
|-----|-------|-----------|--------|
| ADR-001 | Component boundaries | 80% | Boundaries correct; mTLS now wired in config but not enforced end-to-end without cert provisioning |
| ADR-002 | Desired-state reconciliation | 70% | Loop exists with per-operation timeouts; convergence metrics absent |
| ADR-003 | Agent autonomy (partition tolerance) | 40% | ConnectivityTracker exists but not wired into RPC gates; create_vm does NOT check CP reachability |
| ADR-004 | Error handling | 90% | chv-errors crate used consistently |
| ADR-005 | Logging with tracing | 90% | Structured tracing throughout |
| ADR-006 | Async safety | 90% | tokio used correctly, no blocking in async |
| ADR-007 | Inter-service security | 50% | TLS channel construction exists; enrollment not enforced; no cert rotation |
| ADR-008 | State machines | 85% | 14-state VM FSM in agent; transitions validated |
| ADR-009 | Storage abstraction | 45% | StorageBackend trait exists; only local+LVM backends; iSCSI/Ceph absent |
| ADR-010 | Network overlay | 35% | VXLAN interface creation works; eBPF map writes are stubs; FDB management partial |
| ADR-011 | Live migration protocol | 60% | Memory transfer works; disk pre-copy sender exists but agent never calls it; dirty tracking disable is no-op |
| ADR-012 | Upgrade strategy | 5% | Not implemented beyond version field in config |
| ADR-013 | Observability | 55% | Logging good; metrics endpoint absent; health checks exist for CP |

---

## Component Spec Compliance

| Spec | Completeness | Key Gaps |
|------|-------------|----------|
| chv-agent-spec | 65% | No host resource monitoring, persistent cache partial, partition policy not enforced |
| chv-nwd-spec | 45% | eBPF map stubs, no BFD/link monitoring, FDB updates partial |
| chv-stord-spec | 50% | Only local+LVM backends, dirty tracking disable is no-op, pre-copy not called from agent |
| disk-migration-protocol-spec | 55% | Sender/receiver code exists, flow control works, but agent orchestration skips disk phase |
| live-migration-spec | 60% | Memory migration works, iterative convergence exists, disk pre-copy unwired |
| vxlan-overlay-spec | 35% | VXLAN iface creation works, encap/decap handled by kernel, eBPF policy/rate-limit stubs |

---

## WebUI Spec Compliance

| Spec | Completeness | Key Gaps |
|------|-------------|----------|
| webui-api-bff-spec | 75% | list_volumes/list_storage_pools return empty; images delete bypasses BFF client |
| webui-design-system-spec | 85% | Design tokens applied; some components don't use shell utilities |
| webui-implementation-spec | 70% | VM detail missing events/metrics tabs; snapshot tab works via BFF now |
| webui-information-architecture | 80% | All 13 pages exist; navigation correct |
| webui-product-spec | 70% | Core flows work; filter dropdowns incomplete; task linking partial |
| webui-state-and-tasks-spec | 65% | Task store exists; SSE streaming works; mutation-to-task linkage incomplete |

---

## Findings by Severity

### CRITICAL (2)

| # | Finding | File:Line | ADR/Spec | Impact |
|---|---------|-----------|----------|--------|
| C1 | `create_vm` does not check `ConnectivityState::Disconnected` — violates partition policy | `crates/chv-agent-core/src/agent_server.rs:544-553` | ADR-003, ADR-006 | VMs created during network partition may diverge from CP desired state; split-brain |
| C2 | `disable_source_dirty_tracking()` is explicit no-op — migration cannot safely cutover | `crates/chv-controlplane-service/src/migration.rs:768` | disk-migration-protocol-spec §FinalSync | Dirty blocks accumulate indefinitely; final sync never converges |

### HIGH (8)

| # | Finding | File:Line | Spec | Fix Complexity |
|---|---------|-----------|------|---------------|
| H1 | iSCSI StorageBackend absent | `crates/chv-stord-backends/src/` | ADR-009 | 2-3 weeks |
| H2 | Ceph RBD StorageBackend absent | `crates/chv-stord-backends/src/` | ADR-009 | 2-3 weeks |
| H3 | eBPF `update_rules()`, `update_rate_limit()`, `read_stats()` are log-only stubs | `crates/chv-nwd-core/src/ebpf.rs:321-370` | vxlan-overlay-spec | 1-2 weeks |
| H4 | No upgrade/rollback orchestration framework | N/A | ADR-012 | 3-4 weeks |
| H5 | chvctl not packaged in release tarball | `scripts/build-release.sh` | chvctl-cli-spec §Distribution | 1 hour |
| H6 | No host resource monitoring (CPU/mem/disk) | agent crate | chv-agent-spec §Monitoring | 1 week |
| H7 | VM detail page missing events/metrics tabs | `ui/src/routes/vms/[id]/+page.svelte:181-215` | webui-product-spec §VM-Detail | 2-3 days |
| H8 | Images delete bypasses BFF client module | `ui/src/routes/images/+page.svelte:97` | webui-api-bff-spec §Boundary | 1 hour |

### MEDIUM (11)

| # | Finding | Spec |
|---|---------|------|
| M1 | No BFD/link health monitoring in nwd | chv-nwd-spec §LinkMonitoring |
| M2 | No convergence metrics from reconciler | ADR-002 §Convergence |
| M3 | Agent metrics HTTP endpoint (port 9100) absent | ADR-013 |
| M4 | No failure matrix automation (manual circuit breakers only) | ops/failure-matrix |
| M5 | No compatibility matrix enforcement at runtime | ops/compatibility-matrix |
| M6 | list_volumes BFF handler returns empty array | webui-api-bff-spec §Volumes |
| M7 | list_storage_pools BFF handler returns empty array | webui-api-bff-spec §Storage |
| M8 | Filter dropdowns incomplete (type/resource missing) | webui-product-spec §Events |
| M9 | ConnectivityTracker state not wired into any RPC gate | ADR-003 |
| M10 | No cert rotation mechanism for mTLS | ADR-007 |
| M11 | Migration timeout flat 7200s, not progressive | live-migration-spec §Timeouts |

---

## Operational Readiness

| Area | Coverage | Detail |
|------|----------|--------|
| chvctl CLI commands | 5/10 | Binary compiles, subcommands exist; not packaged; no Unix socket mode |
| Failure handling | 5/13 | Happy path recovery works; cascading failures, split-brain, storage exhaustion not handled |
| Runtime sequences | 4/4 | All 4 core sequences (boot, migrate, stop, reboot) implemented |
| Monitoring | 2/10 | CP health endpoint exists; agent metrics absent; no Grafana dashboards |
| Security | 4/10 | JWT auth, RBAC; no mTLS enforcement, no cert management |

---

## Priority Recommendations

### P0: Blocks Production Deployment

1. **Fix C1**: Wire `ConnectivityTracker` into `create_vm`/`migrate_vm` RPC gates (1-2 days)
2. **Fix C2**: Implement real dirty tracking disable via `ioctl` or bitmap flush (3-5 days)
3. **Package chvctl** in release tarball (H5, 1 hour)
4. **Fix H8**: Route images delete through BFF client (1 hour)

### P1: Blocks Beta/Multi-tenant

5. **eBPF map writes** (H3): Implement real BPF map update calls (1-2 weeks)
6. **Host resource monitoring** (H6): CPU/mem/disk reporting for capacity scheduling (1 week)
7. **Agent metrics endpoint**: Prometheus-compatible `/metrics` on port 9100 (2-3 days)
8. **VM detail events/metrics tabs** (H7): Add remaining tab content (2-3 days)

### P2: Blocks GA

9. iSCSI + Ceph RBD backends (H1, H2)
10. Upgrade/rollback orchestration (H4)
11. BFD link monitoring (M1)
12. Convergence metrics (M2)
13. Compatibility matrix enforcement (M5)

### P3: Polish

14. Filter dropdowns for events page (M8)
15. Progressive migration timeouts (M11)
16. Cert rotation (M10)

---

## Changes Since Previous Assessment (2026-05-08)

| Item | Previous Status | Current Status |
|------|----------------|----------------|
| mTLS config wiring | Not implemented | TLS paths wired to client constructors (PR #45) |
| Disk pre-copy sender | Never called | Sender code complete, agent orchestration partially wired (PR #45) |
| VXLAN FDB management | Absent | Reconciler creates VXLAN iface + partial FDB (PR #45) |
| Partition autonomy | No connectivity tracking | ConnectivityTracker exists, not yet gating RPCs (PR #45) |
| deleteVm type mismatch | Broken | Fixed (PR #44) |
| Orchestrator timeouts | Flat 60s/7200s | 4-tier timeouts (PR #44) |
| BFF cache | Hand-rolled O(n) | moka-based lock-free cache (PR #44) |
| Dead code | 773+ lines orphaned | Cleaned (PR #44) |
| @sveltejs/kit CVE | 2.56.1 | Updated to latest (PR #44) |

---

## Methodology Notes

- Each domain analyzed by dedicated agent reading ALL relevant spec documents AND corresponding implementation
- Scores reflect "percentage of spec that has working implementation," not code quality
- 67% means roughly two-thirds of specified behaviors exist and function correctly
- Happy path (create/start/stop/delete VM on single node with local storage) works end-to-end
- Multi-node, multi-backend, failure-recovery, and operational scenarios concentrate the remaining gaps
- Assessment conducted against HEAD of main branch (commit 9aebe901)
