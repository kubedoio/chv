# Product Readiness Gap Analysis — Final Report

**Date**: 2026-05-11  
**Branch**: main (post PR #50 merge)  
**Methodology**: Spec-vs-implementation gap analysis across 13 backend ADRs, 5 WebUI ADRs, 6 component specs, 6 WebUI specs, 4 ops specs  
**Previous Assessment**: 2026-05-11 (7.5/10 / 75%). This is the updated assessment after PR #50 merge (eBPF, NodeUpgrader, mTLS, partition flush fixes).

---

## Overall Completeness: 69% (6.9 / 10)

| Domain | Score | Weight | Weighted | Previous | Change |
|--------|-------|--------|----------|----------|--------|
| Control Plane (orchestrator, reconcile, upgrade) | 56% | 20% | 1.12 | 65% | -9%* |
| Agent + Observability | 80% | 20% | 1.60 | 83% | -3% |
| Storage + Migration | 71% | 20% | 1.42 | 72% | -1% |
| Network + eBPF | 68% | 15% | 1.02 | 77% | -9%* |
| WebUI / BFF | 74% | 15% | 1.11 | 76% | -2% |
| Operations / CLI | 64% | 10% | 0.64 | 51% | +13% |
| **Total** | | **100%** | **6.91** | **7.21** | **-0.30** |

*Control Plane and Network scores dropped because this assessment reads implementation line-by-line against spec requirements, catching enforcement gaps the previous assessment missed (e.g., generation check defined but never called in SQL queries, eBPF load failures swallowed as warnings).*

**Verdict**: Solid beta for single-node. Multi-node scenarios have structural gaps in convergence feedback (dirty_blocks_remaining never reported), overlay reliability (link recovery not wired), and upgrade orchestration (NodeUpgrader exists but isn't called from production code paths). Operations tooling improved but still lacks 6 CLI subcommands.

**Phase Assessment**:
| Phase | Target | Previous | Current |
|-------|--------|----------|---------|
| Phase 1: Stability | Solid control plane + agent lifecycle | 90% | 85% |
| Phase 2: Features | Migration, storage, networking | 72% | 68% |
| Phase 3: Production | mTLS, monitoring, upgrades, operational tooling | 50% | 45% |

---

## ADR Compliance Matrix

| ADR | Title | Previous | Current | Key Gap |
|-----|-------|----------|---------|---------|
| ADR-001 | Component boundaries | 85% | 82% | BffError lacks From<ChvError>; audit logging only on mutate_vm |
| ADR-002 | Desired-state reconciliation | 85% | 72% | Generation check defined but never enforced in SQL WHERE clauses |
| ADR-003 | Agent autonomy (partition tolerance) | 78% | 70% | No state transition guards; compound readiness not enforced for TenantReady |
| ADR-004 | Storage datapath | 80% | 68% | IOPS/bandwidth runtime limits never enforced in any backend |
| ADR-005 | Network service model | 88% | 75% | peer_vteps not persisted to SQLite; with_vtep_ip() never called |
| ADR-006 | Partition policy | 40% | 35% | No partition detection mode; no CP-side reconnect flush |
| ADR-007 | Upgrade/rollback | 55% | 50% | SystemdNodeUpgrader exists but never wired to production orchestrator |
| ADR-008 | Error handling | 85% | 80% | BffError lacks From<ChvError> impl; error codes inconsistent |
| ADR-009 | Logging with tracing | 82% | 75% | Metric constants defined but never emitted; operation_id absent from spans |
| ADR-010 | Async safety | 88% | 88% | No change — solid |
| ADR-011 | Single-node controlplane | 80% | 75% | NodeCache missing volume/network fragments; no pre-flight backup validation |
| ADR-012 | Disk migration pre-copy | 75% | 62% | FinalSync not coordinated with VM pause; bitmap non-atomic |
| ADR-013 | Network overlay VXLAN+eBPF | 72% | 58% | Link recovery not wired; eBPF failure swallowed; CP VTEP registry absent |

---

## Findings by Severity

### CRITICAL (7)

| # | Finding | Domain | Spec Reference |
|---|---------|--------|----------------|
| C1 | Stale generation check defined in code but never enforced in SQL queries — reconciler can overwrite newer state | Control Plane | ADR-002 |
| C2 | Agent reconnect flush absent — deferred messages lost on reconnection | Control Plane | ADR-006, ADR-003 |
| C3 | Source stord never reports dirty_blocks_remaining to CP — convergence loop will always timeout | Storage | ADR-012, disk-migration-protocol-spec |
| C4 | chv-stord binary only wires LocalFileBackend — iSCSI/Ceph code exists but unreachable | Storage | chv-stord-spec |
| C5 | VXLAN link failure recovery absent — link_monitor::check_all_links() never wired to event loop | Network | ADR-013, vxlan-overlay-spec |
| C6 | eBPF load failure swallowed as warning — traffic passes unfiltered on failure | Network | ADR-013, vxlan-overlay-spec |
| C7 | No host disk full detection — node continues accepting VMs until crash | Operations | chv-agent-spec failure matrix |

### HIGH (43)

#### Control Plane (9)

| # | Finding | Spec |
|---|---------|------|
| H1 | No cert rotation caller-identity check — any client cert accepted post-rotation | ADR-002 |
| H2 | No state transition guards — invalid state transitions silently succeed | ADR-003 |
| H3 | Compound readiness not enforced for TenantReady (skips StorageReady/NetworkReady check) | ADR-003 |
| H4 | No partition detection mode — CP cannot distinguish slow network from dead node | ADR-006 |
| H5 | No CP-side reconnect-flush mechanism — messages queued during partition lost | ADR-006 |
| H6 | Compat matrix warn-only, not blocking boot | ADR-007 |
| H7 | SystemdNodeUpgrader not wired to production UpgradeOrchestrator | ADR-007 |
| H8 | No per-node rollback-target tracking — rollback goes to unknown state | ADR-007 |
| H9 | NodeCache missing volume/network state fragments | ADR-011 |

#### Agent + Observability (4)

| # | Finding | Spec |
|---|---------|------|
| H10 | Metrics constants defined (METRIC_RECONCILE_DRIFT etc.) but never emitted to endpoint | ADR-009 |
| H11 | operation_id not propagated in tracing spans — cross-service correlation broken | ADR-009 |
| H12 | chvctl has no local agent socket for debug commands | chv-agent-spec |
| H13 | BffError lacks From<ChvError> — error conversion is ad-hoc | ADR-008 |

#### Storage + Migration (10)

| # | Finding | Spec |
|---|---------|------|
| H14 | IOPS/bandwidth never enforced in any storage backend | ADR-004 |
| H15 | FinalSync not coordinated with VM pause — data race window | ADR-012 |
| H16 | Dirty bitmap snapshot non-atomic — blocks can flip during capture | ADR-012 |
| H17 | CRC mismatch aborts transfer instead of retransmitting block | disk-migration-protocol-spec |
| H18 | Destination TLS server config absent — migration channel one-way auth only | disk-migration-protocol-spec |
| H19 | Partial dest volume not cleaned up on migration failure | disk-migration-protocol-spec |
| H20 | No security sandboxing for storage backends | chv-stord-spec |
| H21 | No dest resource check before migration starts | live-migration-spec |
| H22 | Cloud Hypervisor memory migration not explicitly orchestrated by CP | live-migration-spec |
| H23 | CP doesn't trigger final dirty flush before resume at destination | live-migration-spec |

#### Network + eBPF (8)

| # | Finding | Spec |
|---|---------|------|
| H24 | peer_vteps not persisted to SQLite — lost on nwd restart | ADR-005 |
| H25 | LinuxExecutor::with_vtep_ip() defined but never called | ADR-005 |
| H26 | VNI range validation absent — invalid VNIs accepted | ADR-013 |
| H27 | eBPF denied-packets counter never emitted to metrics | ADR-013 |
| H28 | eBPF program crash/detach detection absent | ADR-013 |
| H29 | FDB cleanup incomplete on topology deletion — stale entries remain | ADR-013 |
| H30 | VTEP IP validation absent — invalid IPs accepted | ADR-013 |
| H31 | No overlay integration tests | vxlan-overlay-spec |

#### WebUI + BFF (9)

| # | Finding | Spec |
|---|---------|------|
| H32 | Audit logging only fires on mutate_vm, not all mutations | webui-api-bff-spec |
| H33 | Competing sidebar navigation components (desktop vs mobile) | webui-design-system-spec |
| H34 | Node detail page missing events/metrics/VMs tabs | webui-implementation-spec |
| H35 | Task lifecycle not visually distinct (no progress states) | webui-state-and-tasks-spec |
| H36 | Cluster detail page is a stub (no content) | webui-implementation-spec |
| H37 | Maintenance window "Create" button is dead (no handler) | webui-implementation-spec |
| H38 | No /tasks/[id] detail route for individual task inspection | webui-product-spec |
| H39 | Event filters incomplete (missing severity filter) | webui-product-spec |
| H40 | Storage pools page has no create/delete actions | webui-implementation-spec |

#### Operations + CLI (3)

| # | Finding | Spec |
|---|---------|------|
| H41 | 6 chvctl subcommands missing (stor, nw, ops, migrate, upgrade, health) | chv-agent-spec ops |
| H42 | No host memory pressure detection | chv-agent-spec failure matrix |
| H43 | No cert expiration rotation automation | ops-security-spec |

### MEDIUM (28)

| # | Finding | Domain |
|---|---------|--------|
| M1 | Reconcile tick metrics not exposed on Prometheus endpoint | Control Plane |
| M2 | No leader election for multi-CP deployments | Control Plane |
| M3 | Draining state has no evacuation logic | Agent |
| M4 | DaemonSupervisor starts stord/nwd sequentially, not parallel | Agent |
| M5 | No graceful shutdown propagation to child daemons | Agent |
| M6 | Backpressure slow_down_factor logged but not applied to send rate | Migration |
| M7 | Volume checksum empty on FinalizeComplete | Migration |
| M8 | No migration bandwidth throttling | Migration |
| M9 | delete_topology does not tear down VXLAN interfaces | Network |
| M10 | ebpf_programs_loaded always returns 0 in OverlayStatus | Network |
| M11 | No DHCP lease cleanup on VM destroy | Network |
| M12 | Node lifecycle collapsed to 4 states in UI (spec has 10) | WebUI |
| M13 | No WebSocket for real-time task updates | WebUI |
| M14 | No dark mode despite design system tokens | WebUI |
| M15 | Console viewer has no clipboard paste support | WebUI |
| M16 | chvctl routes through BFF HTTP, not local Unix socket | CLI |
| M17 | No mutation confirmation prompts in chvctl | CLI |
| M18 | Compat matrix missing nwd and stord components | Operations |
| M19 | No shipped default compat.toml file | Operations |
| M20 | No host network uplink failure detection | Operations |
| M21 | Operation latency histograms absent from metrics | Observability |
| M22 | No OpenTelemetry export scaffolding | Observability |
| M23 | No log rotation configuration in agent | Observability |
| M24 | Health endpoint missing storage/network sub-checks | Agent |
| M25 | No rate limiting on BFF API endpoints | WebUI |
| M26 | No CSRF protection on mutation endpoints | WebUI |
| M27 | Images page missing upload progress indicator | WebUI |
| M28 | No pagination on VM list (unbounded query) | WebUI |

### LOW (8)

| # | Finding | Domain |
|---|---------|--------|
| L1 | Test-code uses std::sync::Mutex in async context | Agent |
| L2 | No cluster overview page in WebUI | WebUI |
| L3 | Version endpoint returns hardcoded string | Agent |
| L4 | No favicon or PWA manifest | WebUI |
| L5 | Cargo.toml workspace members not alphabetically sorted | Build |
| L6 | Some error messages lack context (bare "failed") | Error handling |
| L7 | No --json output flag on chvctl commands | CLI |
| L8 | UI build warnings (unused CSS selectors) | WebUI |

---

## Operational Readiness

| Area | Previous | Current | Detail |
|------|----------|---------|--------|
| chvctl CLI commands | 5/10 | 4/10 | Only vm/node/image/event subcommands; stor/nw/ops/migrate/upgrade/health absent |
| Failure handling | 6/13 | 5/13 | Circuit breaker works; host resource pressure gaps (disk, memory, network) |
| Runtime sequences | 4/4 | 4/4 | All 4 core sequences implemented |
| Monitoring | 5/10 | 4/10 | Metrics constants defined but not emitted; histograms absent |
| Security | 5/10 | 5/10 | mTLS enforced on migration; cert rotation exists; no expiration automation |

---

## Score Comparison: Previous vs Current Assessment

The current assessment is **stricter** than the previous one. The previous assessment (post PR #49) checked whether code/structs/methods existed. This assessment checks whether they are **wired into production code paths** and **actually execute at runtime**.

Examples of the stricter lens:
- Generation check: struct exists, method exists, but SQL queries don't use it = **not implemented**
- SystemdNodeUpgrader: full trait impl exists, but UpgradeOrchestrator uses a no-op = **not wired**
- eBPF programs: load functions exist, but failures are swallowed = **security gap**
- Metrics constants: defined in code, but never passed to Prometheus endpoint = **not emitting**
- iSCSI/Ceph backends: full code exists, but chv-stord binary doesn't instantiate them = **unreachable**

---

## Priority Recommendations (Next Sprint)

### P0: Blocks Production (Fix This Week)

| # | Finding | Fix | Effort |
|---|---------|-----|--------|
| 1 | C1: Generation check not enforced in SQL | Add `WHERE generation <= ?` to all update/insert queries in store | 2 days |
| 2 | C3: dirty_blocks_remaining never reported | Wire stord DirtyTracker count into migration status RPC response | 1 day |
| 3 | C5: Link monitor not wired to event loop | Call `link_monitor.check_all_links()` from nwd tick loop | 1 day |
| 4 | C6: eBPF failure swallowed | Change warn to error + set NetworkReady=false on load failure | 1 day |
| 5 | C7: No disk full detection | Add disk usage check in agent tick; transition to Degraded at 90% | 1 day |

### P1: Blocks Multi-Node Beta

| # | Finding | Fix | Effort |
|---|---------|-----|--------|
| 6 | C2: Reconnect flush absent | Implement deferred message queue flush on state transition Disconnected→Connected | 3 days |
| 7 | C4: iSCSI/Ceph unreachable | Wire backend selection via config in chv-stord main.rs | 1 day |
| 8 | H2: No state transition guards | Add valid_transitions map; reject invalid in set_node_state | 2 days |
| 9 | H7: NodeUpgrader not wired | Replace no-op upgrader with SystemdNodeUpgrader in UpgradeOrchestrator::new() | 1 day |
| 10 | H15: FinalSync not paused | Coordinate: pause VM → flush dirty → resume at dest | 3 days |
| 11 | H24: peer_vteps not persisted | Add SQLite table + INSERT/DELETE in add_peer/remove_peer | 2 days |

### P2: Blocks GA

| # | Finding | Fix | Effort |
|---|---------|-----|--------|
| 12 | H14: IOPS not enforced | Implement cgroup blkio throttle in storage backends | 1 week |
| 13 | H41: 6 CLI subcommands missing | Scaffold stor/nw/ops/migrate/upgrade/health in chvctl | 1 week |
| 14 | H10: Metrics not emitted | Wire METRIC_* constants into metrics_handler format string | 2 days |
| 15 | H11: operation_id missing | Add operation_id field to tracing spans in agent/CP | 2 days |
| 16 | H26-H30: eBPF hardening | VNI validation, denied counter, crash detection, FDB cleanup | 1 week |
| 17 | H32-H40: WebUI gaps | Node tabs, task detail, audit logging, maintenance window | 1 week |

### P3: Polish

| # | Finding | Fix | Effort |
|---|---------|-----|--------|
| 18 | M3: Draining evacuation | Implement VM migration requests on Draining entry | 3 days |
| 19 | M12: Node lifecycle in UI | Expose all 10 states in node status component | 2 days |
| 20 | M16: chvctl Unix socket | Add --socket flag for local agent communication | 2 days |
| 21 | M21-M23: Observability | Histograms, OTEL scaffold, log rotation | 1 week |

---

## Score Progression

| Assessment | Date | Score | Phase 1 | Phase 2 | Phase 3 | Methodology |
|-----------|------|-------|---------|---------|---------|-------------|
| Initial | 2026-05-08 | 5.4/10 | 70% | 35% | 10% | Quick scan |
| Post PR #43-#45 | 2026-05-11 | 6.6/10 | 85% | 55% | 30% | Existence check |
| Post PR #49 | 2026-05-11 | 7.2/10 | 90% | 72% | 50% | Existence check |
| **Post PR #50 (strict)** | **2026-05-11** | **6.9/10** | **85%** | **68%** | **45%** | **Runtime wiring verification** |

*Note: The apparent score decrease from 7.2 to 6.9 reflects a stricter methodology (checking runtime wiring, not just code existence), not a regression in code quality. The codebase has more code than before PR #50 — the new assessment is simply more honest about what actually executes in production.*

---

## Methodology Notes

- Each domain analyzed by dedicated subagent reading ALL relevant spec documents AND corresponding implementation files line-by-line
- Scores reflect "percentage of spec that has **working, wired, runtime-reachable** implementation"
- Code that exists but is never called from production paths counts as 0% implemented
- Structs/traits defined but with no-op methods count as 25% (design intent captured)
- Methods implemented but with swallowed errors/warnings count as 50% (partial)
- 69% means roughly two-thirds of specified behaviors are both implemented AND reachable at runtime
- Single-node happy path (create/start/stop/delete VM with local storage) remains production-quality
- Multi-node migration, overlay networking, and partition recovery have structural wiring gaps
- Assessment conducted against HEAD of main branch (post PR #50 merge)
