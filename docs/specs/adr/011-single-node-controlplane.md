# ADR-011 — Single-Node Control Plane with SQLite

## Status
Accepted

## Date
2026-05-07

## Context
CHV targets sovereign edge environments with small cluster sizes of approximately 20 nodes. Operational simplicity is a primary goal: single binary, single database file, minimal dependencies.

The system already implements partition autonomy (ADR-006): agents preserve runtime state during control-plane outage and VMs continue running. HA control planes introduce consensus complexity (leader election, split-brain, quorum) that is disproportionate for the target scale.

SQLite is already configured with WAL mode, Synchronous=Normal, foreign_keys=ON, busy_timeout=5s, max 16 connections, acquire_timeout=5s. Pre-migration backup already exists: copies DB to `/var/lib/chv/backups/{db}.{timestamp}.bak`, retaining the last 10 backups.

## Decision
- One control-plane process per cluster, managing up to approximately 20 nodes (soft target, not enforced at enrollment)
- SQLite in WAL mode as the sole persistence layer (single-writer model)
- No HA: no leader election, no replication, no multi-instance deployment
- The control plane is SPOF for management operations only (create VM, migrate, delete, resource changes)
- Running workloads are unaffected by control-plane outage (partition autonomy, ADR-006)
- Agents cache desired state locally (NodeCache with vm_fragments, volume_fragments, network_fragments as JSON on disk) and continue executing cached intent during outage
- Agents queue messages for the control plane during outage and flush them in order after reconnection
- DR strategy: periodic SQLite file backup to external storage; restore = stop CP, replace DB file, restart, agents reconnect
- Scale-out: if more than approximately 20 nodes are needed, deploy a second independent control plane as a separate failure domain

## Consequences
Pros:
- Zero distributed systems complexity (no Raft, no Paxos, no etcd dependency)
- Single file backup and restore (SQLite is a single file)
- Deterministic behavior (no split-brain, no quorum loss scenarios)
- Minimal resource footprint on the management node
- Already implemented and tested

Cons:
- Control-plane outage blocks ALL management operations (no redundancy)
- Single-writer model limits concurrent API throughput (acceptable at 20-node scale)
- Manual failover: operator must restore backup to a new host if the management node fails
- No real-time replication: backup lag means potential data loss between last backup and crash

## Guardrails
- Enrollment MUST NOT hard-reject nodes beyond the 20-node target (soft limit only)
- Agent NodeCache MUST persist to disk so it survives an agent restart during partition
- Backup automation SHOULD be configured by the operator (cron + external storage)
- Control-plane startup MUST validate SQLite integrity before accepting connections
- Agent reconnection MUST flush all queued messages in order after control-plane recovery

## Related ADRs
- **ADR-006** (partition-policy): defines agent autonomy during control-plane outage
- **ADR-003** (node-state-machine): defines node states that continue during partition
- **ADR-012** (disk-migration): relies on the control plane being available for migration orchestration
- **ADR-013** (network-overlay): relies on the control plane for VTEP registry coordination
