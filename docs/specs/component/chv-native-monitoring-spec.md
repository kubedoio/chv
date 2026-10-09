# CHV native monitoring component specification

**Status:** Proposed  
**Decisions:** [ADR-025](../adr/025-native-monitoring-architecture.md), [ADR-027](../adr/027-monitoring-history-alerts-and-export.md)

## Purpose and ownership

Deliver accurate Proxmox-like monitoring of CHV nodes, VMs, storage, networking, and control-plane services. Work with no optional guest agent or external monitoring stack. `chv-agent` (CellHV Core) remains the sole VM runtime authority.

This spec defines a **read-only** `chv-monitoring-core` library and an optional `chv-monitoring-store` library. These are proposed modules, not existing repository paths. They do not become new runtime daemons. The Core integration uses an internal read-only trait and bounded sampling tasks.

## Current repository seams (reverify at implementation base)

| File | Existing behavior | Required change |
|---|---|---|
| `crates/chv-agent-runtime-ch/src/process.rs` | Calls `/api/v1/vm.counters` and calculates CPU percent | Verify actual pinned-v53.0 JSON and CPU semantics; instrument failed parse; distinguish missing data |
| `crates/chv-agent-core/src/metrics_server.rs` | `sysinfo` collection on scrape; recreated system | Implement interval-based CPU sampler; report sample age and errors |
| `cmd/chv-agent/src/main.rs` | Reports VM counters in state report loop | Add independent sampler; preserve existing event/telemetry compatibility |
| `crates/chv-controlplane-service/src/telemetry.rs` | Persists optional VM counter fields | Stop silent insert errors; decouple high-frequency data from observed VM state |
| `cmd/chv-controlplane/migrations/0018_vm_metrics.sql` | `vm_metrics` table | Treat as compatibility; add new monitoring migrations without destructive history rewrite |
| `crates/chv-webui-bff/src/handlers/metrics.rs` | Inventory and top consumers | Keep response stable; add versioned monitoring queries |
| `ui/src/lib/components/nodes/NodeHealthDashboard.svelte` | Generates synthetic sparkline history | Remove fake history; show live real samples |
| `ui/src/lib/components/vms/VmMetricsTab.svelte` | Displays assigned VM resources | Add measured usage, labeled source, timelines, no-data states |
| `ui/src/routes/observability/+page.svelte` | Polls inventory and builds browser-local history | Replace with server-time, durable history where the metric is named history |

## Collection sources

| Metric family | Primary source | Caveat |
|---|---|---|
| Node CPU | Persistent Linux `/proc/stat` snapshots | First sample is `insufficient_samples`; excludes invalid deltas |
| Node memory/swap | `/proc/meminfo` | Distinguish MemAvailable and used; cached memory is not automatically leak |
| Node load/pressure | `/proc/loadavg`, `/proc/pressure/*` | Pressure support is kernel dependent |
| Node disk space | `statvfs` per configured filesystem | Root is not equivalent to VM storage pool capacity |
| Node block I/O | `/proc/diskstats` or owned provider counters | Handle stacked devices and double counting |
| Node network | `/sys/class/net/*/statistics` | Exclude duplicates at bond/bridge/physical levels when aggregating |
| VM CPU | Verified VM cgroup `cpu.stat` (primary on the qualified pin — G0b fixtures show v53.0 `vm.counters` exposes no CPU counters; the VMM arm stays available if a future pin adds one) | Name `vm.cpu.cores_used` and normalized utilization separately |
| VM memory | Owned VMM cgroup `memory.current`, with documented semantics | Host resident accounting, not guest in-use memory |
| VM network/disk | Pinned VMM counters; mapping of virtio, TAP and provider path | Classify unsupported offloaded/vhost-user paths |
| VM configured vCPU/RAM | Core VM definition | Label as provisioned, never use as utilization |
| Storage pool/volume | `chv-stord` public read-only telemetry | Do not infer Ceph/RBD health from local root filesystem |
| Network fabric/provider | `chv-nwd` read-only metrics | Do not infer wire rate from desired topology |
| CP/node health | Existing `chv_*_metrics`, readiness and heartbeat | Do not promote desired `Running` as observed healthy |

## Sampling engine

- Separate timers for light node metrics (default 5 seconds), VM metrics (default 10 seconds), provider metrics (default 15 seconds), and long-running inventory (default 60 seconds).
- Each source has a timeout and concurrency limit. Use no unbounded per-VM fan-out. Rate limit VMM Unix socket access, particularly during lifecycle or migration.
- Use monotonic time for counter deltas and wall-clock epoch milliseconds for timestamps. Track sample `boot_id`, generation, `collected_at_ms`, `received_at_ms`, and `duration_ms`.
- Counter reset, process replacement, migration, or negative delta yields `insufficient_samples`, not negative traffic or infinite rate.
- VM identity is resolved from Core-owned, fenced observed runtime identity. PID re-use must be fenced using start time and ownership evidence. No arbitrary PID or cgroup path supplied by a guest.
- A partial sample includes valid fields with per-field quality. Missing metrics must be null/absent, never zero by fallback.
- At most one writer per metric scope; values with source `guest_agent` never overwrite `vmm` or `node` values of different meaning.
- Agent-local buffering is bounded and optional; data loss is observable with a drop counter. Control-plane backpressure never blocks VM lifecycle.
- Internal Prometheus endpoint keeps low-cardinality operational metrics. Detailed VM metrics are served by authenticated history and optional scoped exporters.

## Metric model

Every sample identifies `metric_id`, `target_kind`, `target_id`, `source`, `unit`, `kind`, `observed_at_ms`, `value`, `quality`, and optional bounded dimensions. The contract is [monitoring metrics v1](../contracts/chv-monitoring-metrics-v1.md).

**CPU contract:** `vm.cpu.cores_used` represents consumed CPU seconds divided by elapsed wall seconds. It may exceed 1 on multi-vCPU VMs. `vm.cpu.capacity_ratio` divides that by assigned vCPU count and lies in [0,1] when measurements are valid. VM scheduled vCPU count is a provisioned measure. Do not silently clamp real counter anomalies; flag invalid samples.

**Memory contract:** `vm.memory.host_accounted_bytes` means host-side accounted VM memory. `vm.memory.guest_available_bytes` requires trusted guest instrumentation. A VM may have host backing memory without guest application use.

**Network and disk contract:** Store monotonically increasing byte and operation counters. Render rates from reset-safe deltas. Both bytes/sec and bits/sec are explicitly labeled. Do not double count host bridge or VMM devices.

## Ingestion and read path

1. Core captures an immutable snapshot without holding the VM lifecycle lock through network I/O.
2. Core uses the existing authenticated node-to-manager channel with a new versioned, size-limited metrics batch method. If Core-native mode lacks this transport, add an optional management telemetry adapter. Never boot a legacy second Core.
3. The control plane authenticates node identity and proves node ownership for every VM target before accepting a batch. It rejects unknown/stale identities and invalid sizes.
4. Ingestion places validated samples in a bounded queue. A separate worker persists to monitoring storage and records drops/failures.
5. The BFF resolves user RBAC/project scope on every current/history request and returns only permitted metrics.
6. The UI renders no-data/error/stale/unsupported distinctly and uses the server's sample timestamps.

## User interface

Add: overview (fleet health, most-used nodes, events), node detail (CPU, RAM, load, pressure, disks, links, storage), VM detail (CPU, host memory, provisioned RAM, block/network rates, uptime), storage/network resource panels, health and monitoring confidence. Support live refresh and historical 1h/6h/24h/7d/30d with documented downsampling. Do not render a synthetic graph or a browser-held 16-point history as durable history.

Display source labels such as `Hypervisor`, `Guest agent`, or `Storage provider`; display units, sample age, and collection status. The page must remain usable when no metrics exist.

## Operational constraints

A collection failure must never change VM state, desired state, readiness, or placement. Monitoring configuration changes must not require an agent or VMM restart if technically avoidable. Privileged observations stay in `chv-agent` or existing providers. All queries and writes have bounded work, tenant filtering, and production-safe error codes. Logs exclude secrets and guest process command lines.

## Tests and acceptance

- Golden pinned-v53.0 VMM counter fixtures and real-KVM sampled counter comparisons.
- CPU idle/load and burst tests; VM memory distinguished from guest OS available memory.
- Non-zero disk/network traffic with both virtual and backend paths; reset/restart/migration tests.
- Synthetic history code removed; null does not render as zero; 1h/24h history matches stored samples.
- Flood and storage-failure tests show bounded RAM/CPU/queue and no lifecycle regression.
- Legacy, core-managed, and core-native startup paths remain authority-safe; unsupported paths report capability-negative.
- Negative tests: VM ID spoof, lost connectivity, stale VM generation, PID reuse, unknown tenant, deleted VM, permission denial.
- Store exact binary hashes, VMM version, topology, commands, expected/observed metrics, and error evidence in acceptance output.

A completed UI without real measurements does not pass the G2 history-and-UI gate.
