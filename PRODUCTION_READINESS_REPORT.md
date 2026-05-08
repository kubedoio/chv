# CHV Production Readiness Status Report

**Date:** 2026-05-07  
**Branch:** main (commit ef2db0bb)  
**Scope:** Multi-node clustering, live migration with disk, production workload readiness

---

## Executive Summary

CHV has a working single-node vertical slice: agent enrollment, mTLS certificates, VM lifecycle (create/start/stop/delete), storage (local file + LVM), networking (bridge + DHCP + DNS + firewall), and a WebUI with BFF. The control plane tracks desired/observed state with a reconciliation loop.

**For 2+ nodes with live migration and disk migration, the system is at ~25% completion.** The enrollment/heartbeat/mTLS plumbing works. Everything else (scheduler, migration orchestration, shared storage, network overlay, HA control plane) does not exist or is stubbed.

---

## Current Architecture (What Works)

```
┌─────────────────────────────────────────────────────────────┐
│                     CONTROL PLANE                            │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │  SQLite DB  │  │  Orchestrator │  │  WebUI BFF       │   │
│  │  (35 migr.) │  │  (dispatch)   │  │  (SvelteKit)     │   │
│  └─────────────┘  └──────────────┘  └──────────────────┘   │
│         ↕ gRPC mTLS                                         │
└─────────────────────────────────────────────────────────────┘
         ↕                              ↕
┌────────────────────┐     ┌────────────────────┐
│    NODE (Agent)    │     │    NODE (Agent)     │
│  ┌──────────────┐  │     │  ┌──────────────┐  │
│  │  chv-stord   │  │     │  │  chv-stord   │  │
│  │  (volumes)   │  │     │  │  (volumes)   │  │
│  ├──────────────┤  │     │  ├──────────────┤  │
│  │  chv-nwd     │  │     │  │  chv-nwd     │  │
│  │  (network)   │  │     │  │  (network)   │  │
│  ├──────────────┤  │     │  ├──────────────┤  │
│  │  Cloud Hyp.  │  │     │  │  Cloud Hyp.  │  │
│  │  (VMs)       │  │     │  │  (VMs)       │  │
│  └──────────────┘  │     │  └──────────────┘  │
└────────────────────┘     └────────────────────┘
          ✓ working               ✓ working
          ✗ no cross-node paths   ✗ no cross-node paths
```

---

## Multi-Node Clustering Status

### What's Implemented (Working)

| Component | File | Description |
|-----------|------|-------------|
| Agent enrollment | `crates/chv-agent-core/src/enrollment.rs:88-121` | Agent sends bootstrap token + inventory, receives node_id + certs |
| Heartbeat loop | `cmd/chv-agent/src/main.rs:467-850` | Every 5s: node state, VM states, volume states, network states |
| Inventory reporting | `crates/chv-agent-core/src/inventory.rs:45-64` | CPU threads, memory bytes, storage classes, hypervisor caps |
| Certificate rotation | `cmd/chv-agent/src/main.rs:180-198` | Every 12h automatic rotation via enrollment RPC |
| Node state machine | `crates/chv-agent-core/src/reconcile.rs:78-196` | Discovered → Bootstrapping → HostReady → StorageReady → NetworkReady → TenantReady |
| Circuit breaker | `crates/chv-controlplane-service/src/node_client.rs:14-146` | 3-state breaker: 5 failures in 30s opens, 30s timeout to half-open |
| Offline message queue | `crates/chv-agent-core/src/cache.rs` | Defers control plane messages when CP unavailable |
| Node client pool | `crates/chv-controlplane-service/src/node_client_pool.rs` | Connection pool with 300s TTL per node |
| Node liveness | `crates/chv-controlplane-service/src/orchestrator.rs` | Marks nodes Unreachable if no heartbeat in 60s |

### What's Missing for 2+ Nodes

| Gap | Priority | Effort | Description |
|-----|----------|--------|-------------|
| **Scheduler/placement** | P0 | L | No bin-packing, resource matching, or affinity rules. VMs go to first enrolled node or explicit node_id. |
| **Node-to-node communication** | P0 | L | Agents cannot talk to each other. All coordination through CP. Migration requires direct node-to-node TCP. |
| **HA control plane** | P1 | XL | Single SQLite DB, single process. No leader election, no replication, no quorum. |
| **Node removal/drain orchestration** | P1 | M | Drain mode exists in proto but no automated workload evacuation. |
| **Cluster membership consensus** | P2 | XL | No split-brain detection. CP assumes it's the only authority. |
| **Auto-discovery** | P3 | S | Nodes require manual bootstrap token. No mDNS/gossip. Acceptable for now. |

---

## Live Migration Status

### Current State: NOT IMPLEMENTED

The only migration-adjacent code is `snapshot_vm` / `restore_vm` which calls Cloud Hypervisor's `/api/v1/vm.snapshot` and `/api/v1/vm.restore`. This is cold migration (stop → snapshot → copy → restore on new node). There is no live migration path.

### What Live Migration Requires

```
LIVE MIGRATION FLOW (what must be built):

Source Node                    Control Plane                Destination Node
    │                               │                            │
    │  1. MigrateVm(vm_id, dest)    │                            │
    │◄──────────────────────────────│                            │
    │                               │  2. PrepareMigration(vm)   │
    │                               │───────────────────────────►│
    │                               │                            │
    │                               │  3. Ready(socket_addr)     │
    │                               │◄───────────────────────────│
    │  4. StartMigration(dest_addr) │                            │
    │◄──────────────────────────────│                            │
    │                               │                            │
    │  5. CH: vm.send-migration ════════════════════════════════►│ CH: vm.receive-migration
    │     (TCP: memory pages,       │                            │
    │      CPU state, device state) │                            │
    │                               │                            │
    │  6. Converged (dirty < thresh)│                            │
    │     CH: vm.pause              │                            │
    │     Final page flush ═════════════════════════════════════►│
    │                               │                            │ CH: vm.resume
    │  7. MigrationComplete         │                            │
    │──────────────────────────────►│  8. UpdatePlacement(dest)  │
    │                               │───────────────────────────►│
    │  9. CleanupSource             │                            │
    │◄──────────────────────────────│                            │
```

### Gap Breakdown for Live Migration

| Component | Status | Effort | What's Needed |
|-----------|--------|--------|---------------|
| **Migration orchestrator** | NOT PRESENT | L | New dispatch branch in orchestrator.rs, state machine (Preparing → PreCopy → Converging → Paused → Completed/Failed), timeout handling, rollback |
| **Node-to-node TCP socket** | NOT PRESENT | M | Agent opens listening socket on dest, source connects. Port allocation, firewall rules, TLS wrapping. |
| **CH send-migration call** | NOT PRESENT | S | HTTP PUT to CH socket `/api/v1/vm.send-migration` with `{"receiver_url": "tcp://dest:port"}` |
| **CH receive-migration call** | NOT PRESENT | S | HTTP PUT to CH socket `/api/v1/vm.receive-migration` with `{"receiver_url": "tcp://0.0.0.0:port"}` |
| **Dirty page tracking** | NOT PRESENT | S | Call CH `/api/v1/vm.get-dirty-pages` to decide when to enter stop-and-copy |
| **VM state "Migrating"** | NOT PRESENT | S | Add migration states to VM desired/observed state enums in proto |
| **Post-migration validation** | NOT PRESENT | M | Verify VM running on dest, network reachable, then cleanup source |
| **Rollback on failure** | NOT PRESENT | M | If dest fails, resume on source. If source crashes mid-migration, recovery path. |

### Disk Migration (the hard part)

| Approach | Effort | Tradeoff |
|----------|--------|----------|
| **A) Shared storage (NFS/Ceph)** | L | Disk doesn't move; both nodes access same volume. Requires shared storage infra. Simplest for live migration. |
| **B) Block-level replication** | XL | Mirror disk to dest in background, then switch. Works with local storage. Complex, needs dirty block tracking. |
| **C) Storage migration post-VM** | L | Move VM first (memory-only), then live-migrate storage via CH's virtio-block migration support. Longer total time. |

**Recommended approach:** Shared storage (A) for first iteration. Ceph RBD backend is already stubbed in stord. Wire it up, and migration becomes memory-only (no disk copy needed).

### Network Continuity During Migration

| Gap | Effort | Description |
|-----|--------|-------------|
| **VXLAN overlay** | L | Required so VMs on different nodes share L2 segment. Without this, migrated VM loses network. |
| **MAC preservation** | S | MAC must follow VM. Currently stored in DHCP host files per-node. |
| **Gratuitous ARP** | S | After migration, dest node sends GARP to update switch MAC tables. |
| **Firewall state sync** | M | nftables rules on source must be replicated to dest. |

---

## Production Workload Readiness

### Scorecard

| Dimension | Grade | Blocking? | Summary |
|-----------|-------|-----------|---------|
| **HA / Failover** | RED | YES | Single CP, no failover, SPOF |
| **Data Durability** | YELLOW | SOFT | SQLite only, manual backups, no replication |
| **Networking** | YELLOW | YES (multi-node) | Single-node bridge only, no L2 across nodes |
| **Storage** | YELLOW | YES (migration) | Local only in production, shared backends stubbed |
| **Security** | YELLOW | NO | mTLS works, RBAC partial, secrets on disk |
| **Observability** | GREEN | NO | Prometheus metrics, structured logging, health endpoints |
| **Resource Management** | YELLOW | SOFT | Inventory tracked, no scheduling or quota enforcement |
| **Backup / DR** | YELLOW | SOFT | Schema exists, execution stubbed |

### Detail per Dimension

**HA / Failover (RED)**
- Control plane is single process + single SQLite file
- If CP dies, no new VMs can be created, no operations dispatched
- Existing VMs continue running (partition autonomy, ADR-006) but unmanaged
- No leader election, no consensus, no standby CP
- **Fix:** PostgreSQL + streaming replication + haproxy, or embedded Raft

**Data Durability (YELLOW)**
- SQLite with 35 migrations, pre-migration backup (last 10 kept)
- No WAL mode configured explicitly (SQLite default is journal mode)
- No automated offsite backup
- **Fix:** Enable WAL, add scheduled backup to S3/MinIO, document restore

**Networking (YELLOW, blocks multi-node)**
- chv-nwd does bridge, DHCP, DNS, nftables per-node
- No VXLAN, no VLAN tagging, no distributed networking
- VMs on different nodes cannot share L2 segment
- **Fix:** VXLAN encapsulation with control plane coordinating VTEPs

**Storage (YELLOW, blocks migration)**
- stord has local file + LVM backends working
- iSCSI and Ceph RBD backends exist as test stubs
- No volume migration, no cross-node access
- **Fix:** Finish Ceph RBD backend, add shared pool concept to placement

**Security (YELLOW)**
- mTLS between agent and CP (good)
- JWT auth with bcrypt (good)
- RBAC roles exist but middleware not on all routes
- CA keys and JWT secret stored as plaintext files
- No audit logging
- **Fix:** Complete RBAC middleware, add Vault integration, add audit log

**Observability (GREEN)**
- Prometheus metrics at `/metrics` (chv_vms_total, chv_nodes_ready, etc.)
- Structured logging via tracing crate
- Health + readiness endpoints
- **Fix (nice-to-have):** OpenTelemetry tracing, Grafana dashboards, AlertManager rules

**Resource Management (YELLOW)**
- Inventory: CPU, memory, storage classes collected and reported
- No admission control (can overbook a node)
- No scheduler (placement is "first node" or explicit)
- Quotas table exists but validation not wired
- **Fix:** Wire quota check at create_vm, implement basic fit-based scheduler

**Backup / DR (YELLOW)**
- Backup job scheduler exists with cron, retry, status tracking
- Actual execution returns `NotImplemented`
- No retention policy, no verification, no cross-region
- **Fix:** Wire execution to snapshot RPCs, add retention policy

---

## Implementation Roadmap: Multi-Node + Live Migration

### Phase 1: Shared Storage (Weeks 1-3)

**Goal:** Two nodes can run VMs from the same Ceph storage pool.

| Task | Effort | Depends On |
|------|--------|------------|
| Finish Ceph RBD stord backend | M | Ceph cluster available |
| Add "shared" flag to storage pools | S | — |
| Storage pool discovery reports shared pools | S | Ceph backend |
| VM create validates target pool accessibility | S | Shared flag |

### Phase 2: Network Overlay (Weeks 2-4)

**Goal:** VMs on different nodes can communicate on same L2 network.

| Task | Effort | Depends On |
|------|--------|------------|
| VXLAN tunnel management in chv-nwd | L | — |
| Control plane VTEP registry (which node has which VNI) | M | — |
| Topology sync: when VM is placed, dest nwd creates VXLAN port | M | VTEP registry |
| Gratuitous ARP after migration | S | VXLAN working |
| Firewall state sync between nodes | M | — |

### Phase 3: Scheduler (Weeks 3-4)

**Goal:** Control plane picks optimal node for VM placement.

| Task | Effort | Depends On |
|------|--------|------------|
| Fit-based scheduler (CPU + memory + disk) | M | Inventory data |
| Quota enforcement at placement | S | Scheduler |
| Affinity/anti-affinity labels | S | Scheduler |
| Schedulable node filter (only TenantReady, not draining) | S | Node states |

### Phase 4: Live Migration (Weeks 4-7)

**Goal:** Migrate a running VM between two nodes with minimal downtime.

| Task | Effort | Depends On |
|------|--------|------------|
| Add MigrateVm operation type + proto messages | S | — |
| Migration state machine in orchestrator | L | — |
| Agent: open receiving socket for migration | M | — |
| Agent: call CH send-migration/receive-migration | M | Receiving socket |
| Dirty page tracking + convergence detection | M | CH API calls |
| Stop-and-copy final phase (pause → flush → resume) | M | Dirty page tracking |
| Post-migration validation + source cleanup | M | Resume on dest |
| Rollback path if destination fails | M | — |
| Migration progress reporting to UI | S | State machine |

### Phase 5: HA Control Plane (Weeks 6-10)

**Goal:** Control plane survives single-node failure.

| Task | Effort | Depends On |
|------|--------|------------|
| Migrate SQLite → PostgreSQL | L | — |
| PostgreSQL streaming replication setup | M | PG migration |
| Load balancer (haproxy) for CP endpoint | M | Replication |
| Agent reconnection logic (try multiple CP addresses) | M | Load balancer |
| Backup automation + restore testing | M | PG migration |

### Phase 6: Production Hardening (Weeks 8-12)

**Goal:** Safe for production workloads.

| Task | Effort | Depends On |
|------|--------|------------|
| Complete RBAC on all routes | M | — |
| Secrets management (Vault) | M | — |
| Audit logging for mutations | M | — |
| Backup execution (wire to snapshot RPCs) | M | — |
| Retention policy + verification | S | Backup execution |
| Disaster recovery runbook + testing | M | All above |
| Node drain orchestration (auto-evacuate VMs) | M | Migration working |

---

## Minimum Viable Multi-Node (Fastest Path)

If the goal is "2 nodes, one live migration" as a demo/proof:

| What | Effort | Calendar |
|------|--------|----------|
| Finish Ceph RBD backend in stord | 2-3 days | Week 1 |
| Add MigrateVm RPC + basic orchestration | 3-4 days | Week 1-2 |
| Agent migration socket (send/receive) | 2-3 days | Week 2 |
| VXLAN tunnel between 2 nodes (hardcoded) | 2-3 days | Week 2-3 |
| Basic scheduler (pick dest with capacity) | 1-2 days | Week 3 |
| End-to-end test: migrate VM between nodes | 2 days | Week 3 |

**Total:** ~3 weeks for a working demo. Not production-hardened (no HA, no rollback, no drain).

---

## Known P0 Bugs Blocking Any Production Use

| Bug | File | Impact |
|-----|------|--------|
| `/api/v1/nodes` returns empty array (stub) | `stub.rs:11-13` | UI shows no nodes even when enrolled |
| CPU inventory returns 0 on some hosts | `inventory.rs:46-47` (P3 from design doc) | Scheduler would think node has 0 CPU |
| Backup execution returns NotImplemented | backup handlers | No data protection |
| Quota enforcement not wired | quota handlers | Infinite overbooking possible |

---

## Recommendation

**For a "2 nodes + live migration with disk" demo:** 3-week sprint. Focus on Ceph backend, migration orchestrator, VXLAN between 2 specific nodes, basic scheduler. Skip HA, skip hardening.

**For production workloads:** 10-12 week roadmap (Phases 1-6 above). The hard parts are HA control plane and network overlay; the migration itself is straightforward once shared storage and node-to-node networking exist.

**Biggest architectural risk:** The SQLite single-CP design. Every other gap is additive (build new code). The DB migration is a rewrite of the persistence layer. Consider doing it first so all subsequent work builds on the production foundation.
