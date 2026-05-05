# CHV Project Status Report

**Date:** 2026-05-05
**Version:** 0.0.0.4
**Phase:** Early-to-MVP transitioning to stability

---

## 1. System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Browser (SvelteKit)                    │
└──────────────────────────────┬──────────────────────────────┘
                               │ HTTP/WS
┌──────────────────────────────▼──────────────────────────────┐
│                    chv-webui-bff (axum)                       │
│              BFF Layer — JWT Auth, Caching, Quotas            │
└──────────────────────────────┬──────────────────────────────┘
                               │ gRPC-over-mTLS
┌──────────────────────────────▼──────────────────────────────┐
│                  chv-controlplane                             │
│     Orchestrator — Desired State, Tasks, Scheduling          │
└──────────────────────────────┬──────────────────────────────┘
                               │ gRPC-over-mTLS (monotonic generations)
┌──────────────────────────────▼──────────────────────────────┐
│                      chv-agent (per node)                     │
│          Reconciler — State Machine, VM Lifecycle             │
├──────────────┬───────────────┬───────────────────────────────┤
│  chv-stord   │    chv-nwd    │     Cloud Hypervisor (VMM)    │
│  Storage     │    Network    │     KVM-based VMs             │
└──────────────┴───────────────┴───────────────────────────────┘
```

**Key Principles:**
- Control plane owns desired state; nodes own observed state
- All node communication via gRPC-over-mTLS through chv-agent
- Browser talks only to BFF; never direct to node services
- Partition autonomy: nodes self-heal during CP outages

---

## 2. Codebase Metrics

| Metric | Value |
|--------|-------|
| Rust crates | 21 (14 internal + 4 binaries + 3 generated) |
| Total LOC | ~309,000 |
| Source files | 783 .rs files |
| Tests | 5,358 test attributes |
| Proto files | 6 |
| Migrations | 30 |
| TODOs remaining | 3 |
| Recent commits | 10 (all fix/quality focused) |

---

## 3. ADR Compliance Status

| ADR | Title | Status | Compliance |
|-----|-------|--------|------------|
| 001 | Node Runtime Split | Accepted | **PASS** |
| 002 | Control Plane to Node Boundary | Accepted | PARTIAL — mTLS optional; generation not enforced at store |
| 003 | Node State Machine | Accepted | PARTIAL — Missing Discovered/Failed states; no schedulability check |
| 004 | Storage Datapath Model | Accepted | **FAIL** — iSCSI + Ceph RBD backends not implemented |
| 005 | Network Service Model | Accepted | PARTIAL — 5 nwd daemon stubs unimplemented |
| 006 | Partition and Autonomy Policy | Accepted | **FAIL** — No partition policy gate at store layer |
| 007 | Upgrade and Rollback Policy | Accepted | PARTIAL — Bundle-tested concept defined, not fully automated |
| 008 | Error Handling Patterns | Accepted | **PASS** — Single crate, `Into<tonic::Status>`, sanitized boundaries |
| 009 | Logging and Observability | Accepted | PARTIAL — Histograms added; metrics not wired to all paths |
| 010 | Async Runtime Safety | Accepted | PARTIAL — Poison-safe recovery added; some lock scope issues remain |

**Summary:** 2 PASS, 6 PARTIAL, 2 FAIL

---

## 4. Implementation Completeness by Component

### chv-controlplane (Orchestrator)
| Feature | Status | Notes |
|---------|--------|-------|
| Desired state management | Done | SQLite-backed, generation-tracked |
| Task dispatch & tracking | Done | Queued→Running→Succeeded/Failed |
| VM create/start/stop/reboot/delete | Done | Full lifecycle via reconciliation |
| Hot-plug (resize, attach) | Done | Generation-aware after fix |
| Scheduling | Partial | No schedulability check (TenantReady only) |
| Quota enforcement | Partial | Schema exists, not enforced at create time |
| Network mutations | Stub | Returns NotImplemented |
| Partition policy gate | Missing | ADR-006 not implemented |

### chv-agent (Node Reconciler)
| Feature | Status | Notes |
|---------|--------|-------|
| State machine (10 states) | Partial | Core states work; Discovered/Failed missing |
| VM reconciliation loop | Done | 5-second tick, desired→observed convergence |
| Cloud Hypervisor management | Done | Start, stop, pause, resume, snapshot, restore |
| Volume attach/detach | Done | Via chv-stord gRPC |
| NIC lifecycle | Done | Create/delete via chv-nwd |
| Snapshot create/restore | Done | Local disk per VM |
| Serial console (WebSocket→PTY) | Done | Working end-to-end |
| Enrollment + mTLS bootstrap | Done | Certificate-based |

### chv-stord (Storage Daemon)
| Feature | Status | Notes |
|---------|--------|-------|
| Local raw/qcow2 volumes | Done | Create, resize, delete |
| LVM backend | Done | Thin provisioning |
| iSCSI backend | **Missing** | ADR-004 MVP-1 mandatory |
| Ceph RBD backend | **Missing** | ADR-004 MVP-1 mandatory |
| Snapshots (volume-level) | Done | Copy-on-write for qcow2 |
| Pool management | Done | Discover, report capacity |

### chv-nwd (Network Daemon)
| Feature | Status | Notes |
|---------|--------|-------|
| Linux bridge creation | Done | Per-network isolation |
| Namespace + veth/tap | Done | VM network plumbing |
| NAT/masquerade | Partial | Policy logged, not fully enforced |
| Firewall (nftables) | **Stub** | Logs only, no rule enforcement |
| DHCP scope | **Stub** | Logs only, no dnsmasq integration |
| DNS scope | **Stub** | Logs only, no resolver integration |
| Service exposure (port forward) | Done | iptables DNAT rules |

### chv-webui-bff (API Gateway)
| Feature | Status | Notes |
|---------|--------|-------|
| JWT authentication | Done | Login, token creation/revocation |
| VM CRUD + lifecycle | Done | All mutations return spec-compliant shape |
| Network CRUD | Partial | Create/delete work; mutate NotImplemented |
| Volume management | Done | Create, attach, resize |
| Backup jobs | Done | Create, schedule, restore |
| Snapshots | Done | Create, delete, restore (VM must be stopped) |
| Quota checking | Partial | Schema in place, enforcement not wired |
| Caching (Moka) | Done | 60s TTL with invalidation on mutations |
| Metrics middleware | Done | Request duration, status codes |
| RBAC | Partial | Viewer/Operator/Admin roles; middleware basic |

### Frontend (SvelteKit)
| Feature | Status | Notes |
|---------|--------|-------|
| Dashboard/Overview | Done | Cluster health, resource summary |
| VM list + detail | Done | State indicators, actions |
| Network list + detail | Done | Bridge topology |
| Volume list + detail | Done | Pool capacity |
| Task tracking | Done | Real-time status |
| Serial console | Done | xterm.js WebSocket |
| Settings/Hypervisor | Done | Configuration management |
| Design system | Partial | Tailwind migration incomplete |
| Command palette (Ctrl+K) | Broken | Playwright test failing |
| Accessibility | Partial | ARIA basics, not full audit |

---

## 5. Gap Analysis

### Critical Gaps (Blocking Production)

| # | Gap | Impact | Effort | Component |
|---|-----|--------|--------|-----------|
| 1 | Generation monotonicity not enforced at store | Stale writes can overwrite current state (split-brain) | Medium | controlplane-store |
| 2 | mTLS optional at runtime | Node communication unencrypted in some configs | Medium | controlplane, agent |
| 3 | iSCSI + Ceph RBD backends missing | Limited to local storage only | High | stord-backends |
| 4 | Network daemon stubs (DHCP, DNS, firewall) | VMs lack network services; firewall not enforced | High | nwd-core |
| 5 | Partition policy gate missing | No protection against stale scheduling during CP outage | Medium | controlplane-store |
| 6 | Quota enforcement not wired | Resource limits defined but not checked at create-time | Low | orchestrator, BFF |

### Important Gaps (Pre-GA)

| # | Gap | Impact | Effort | Component |
|---|-----|--------|--------|-----------|
| 7 | Network mutation endpoint | Users cannot modify network config after creation | Medium | BFF, orchestrator |
| 8 | Node state machine incomplete | Discovered/Failed states not handled | Medium | agent-core |
| 9 | Observability metrics not fully wired | Histogram recording exists but not on all paths | Low | all crates |
| 10 | tokio Mutex held across I/O | Agent handler serialization under load | High | agent-core |
| 11 | No down-migrations | Cannot rollback schema changes | Medium | migrations |
| 12 | SQLite CHECK constraints | Adding enum variants requires table recreation | Low | migrations |

### Deferred (Architectural Decision Needed)

| Gap | Decision Required |
|-----|-------------------|
| BFF direct-SQLite access | Should BFF call controlplane gRPC or keep direct DB? |
| Multi-node networking | Overlay (VXLAN/Geneve) vs flat routing? |
| PostgreSQL migration path | When to leave SQLite? At what scale? |
| Feature flag mechanism | How to gate partial features in production? |
| buf breaking in CI | Add proto compatibility checking? |

---

## 6. Architectural Maturity Assessment

```
                    DEFINED    IMPLEMENTED    TESTED    PRODUCTION-READY
                    ───────    ───────────    ──────    ────────────────
VM Lifecycle        [████████████████████████████████████████░░░░░░░░]  80%
State Machine       [████████████████████████████░░░░░░░░░░░░░░░░░░░]  55%
Storage (local)     [████████████████████████████████████████████░░░░]  85%
Storage (remote)    [████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]  15%
Networking (basic)  [████████████████████████████████████░░░░░░░░░░░░]  70%
Networking (services)[████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 25%
Error Handling      [████████████████████████████████████████████████]  95%
Observability       [████████████████████████░░░░░░░░░░░░░░░░░░░░░░░]  50%
Security (auth)     [████████████████████████████████░░░░░░░░░░░░░░░]  65%
Security (mTLS)     [████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░]  40%
Partition Autonomy  [████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]  15%
Frontend UI         [████████████████████████████████████░░░░░░░░░░░░]  70%
CI/CD               [████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]  30%
```

**Overall Architecture Readiness: ~55%** (MVP functional, production hardening needed)

---

## 7. Recent Activity (Last 2 Weeks)

All recent work has been quality/correctness focused:

| PR | Description | Findings Fixed |
|----|-------------|----------------|
| #21 | Spec-gap remediation — ADR compliance, BFF contract, security | ~120 |
| #20 | Comprehensive review wave 3 — critical + high findings | 19 |
| — | Deferred findings — JSON logging, stord blocking I/O, rustls CVE | 3 |
| — | Comprehensive review phase 4 — 54 findings across 26 files | 54 |
| — | Comprehensive review wave 0/1/2 merge | 24 |

**Total findings remediated in recent sprint: ~220**

---

## 8. Recommended Roadmap

### Immediate (Week 1-2)
1. Enforce generation monotonicity at store layer
2. Wire quota enforcement at VM/volume create
3. Make mTLS mandatory (remove optional bypass)
4. Complete network mutation endpoint

### Short-term (Week 3-4)
5. Implement DHCP/DNS/firewall in nwd (not just stubs)
6. Add partition policy gate (ADR-006)
7. Complete node state machine (Discovered/Failed transitions)
8. Wire histogram metrics to all async task paths

### Medium-term (Month 2)
9. iSCSI backend implementation
10. Multi-node network topology (overlay decision)
11. Production UI refactor (Tailwind-first)
12. CI/CD pipeline hardening (buf breaking, E2E tests)

### Long-term (Month 3+)
13. Ceph RBD backend
14. PostgreSQL migration evaluation
15. HA control plane (multi-instance)
16. Live migration support

---

## 9. Technology Stack

| Layer | Technology | Status |
|-------|------------|--------|
| Language | Rust (workspace) | Stable |
| Database | SQLite (embedded) | Production-ready for single-node |
| Frontend | SvelteKit + Tailwind | Functional, needs polish |
| RPC | gRPC / tonic / protobuf | Stable |
| HTTP API | axum | Stable |
| VMM | Cloud Hypervisor | Stable |
| Async | Tokio | Stable |
| Logging | tracing | Stable |
| Metrics | Prometheus (pull) | Partial |
| Auth | JWT + mTLS | Functional |

---

## 10. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Split-brain from stale generation writes | Medium | Critical | Implement store-layer generation check |
| Network isolation bypass (no firewall enforcement) | High | High | Implement nftables rules in nwd |
| Data loss during partition (no policy gate) | Low | Critical | Implement ADR-006 |
| Storage limitation (local-only) | Certain | Medium | Implement iSCSI backend |
| Performance under load (mutex contention) | Medium | Medium | Restructure agent_server lock scopes |
