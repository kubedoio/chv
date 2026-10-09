# CHV monitoring metric contract v1

**Status:** Proposed  
**Authority:** [ADR-025](../adr/025-native-monitoring-architecture.md)

## Scope and terminology

This contract defines typed measurements from `chv-agent`, provider daemons, and optional `chv-monitor-agent`. It does not define VM lifecycle state, billing evidence, or a Prometheus scrape format. A metric value is always a **measurement**, not an authoritative desired state.

**Metric** is a named, versioned phenomenon with a unit, type and allowed source. **Sample** is a timestamped observation. **Target** is an authorized `node`, `vm`, `volume`, `network`, or `agent_check`. **Source** records which observation layer generated the number.

## Required sample fields

| Field | Type | Requirement |
|---|---|---|
| `schema_version` | integer | Exactly `1` |
| `target_kind` | enum | `node`, `vm`, `volume`, `network`, `check` |
| `target_id` | UUID/string | Resolved against authenticated inventory; never arbitrary |
| `metric_id` | registry string | Must exist in explicit metric allowlist |
| `source` | enum | `node_os`, `vmm`, `vm_cgroup`, `storage_provider`, `network_provider`, `guest_agent`, `derived` |
| `kind` | enum | `gauge`, `counter`, `state` |
| `unit` | enum | `ratio`, `cores`, `bytes`, `bytes_per_second`, `seconds`, `count`, `operations`, `celsius`, `boolean` |
| `observed_at_ms` | int64 | Source observation time in Unix milliseconds |
| `value` | typed value | Finite f64 for ratios/gauges; signed/unsigned integer for exact counters |
| `quality` | enum | `valid`, `insufficient_samples`, `unsupported`, `unavailable`, `invalid`, `stale` |
| `dimensions` | map | Bounded name/value pairs from registered dimensions |
| `boot_id` | string | Required for counter series from a restartable source |
| `identity_epoch` | string | Stable incarnation/version for ownership and migration fencing |

For `quality != valid`, `value` MUST be absent. Do not encode missing as `0` or NaN. `received_at_ms` is stamped by the manager, never trusted from the sender. `source` and `quality` are immutable for a stored sample. Unknown field/version handling is described by [ingestion v1](chv-monitoring-ingestion-v1.md).

## Registry of initial metrics

| Metric ID | Kind and unit | Allowed source | Semantics |
|---|---|---|---|
| `node.cpu.capacity_ratio` | gauge ratio | node_os | CPU time busy divided by total CPU time over sample interval |
| `node.cpu.load1`, `load5`, `load15` | gauge count | node_os | Linux load average, not CPU percent |
| `node.memory.total_bytes` | gauge bytes | node_os | Total physical memory |
| `node.memory.available_bytes` | gauge bytes | node_os | Linux MemAvailable |
| `node.swap.used_bytes` | gauge bytes | node_os | Used swap |
| `node.memory.psi_some_ratio` | gauge ratio | node_os | Window-defined Linux memory PSI; require `window` dimension |
| `node.fs.available_bytes` | gauge bytes | node_os | Available filesystem bytes to unprivileged processes |
| `node.fs.total_bytes` | gauge bytes | node_os | Filesystem total, keyed by mount identity |
| `node.net.rx_bytes_total`, `tx_bytes_total` | counter bytes | node_os | Interface counters, not summed blindly across bridges |
| `node.block.read_bytes_total`, `write_bytes_total` | counter bytes | node_os | Device counters keyed by stable device |
| `vm.cpu.cores_used` | gauge cores | vmm, vm_cgroup | CPU seconds delta divided by monotonic wall seconds |
| `vm.cpu.capacity_ratio` | gauge ratio | derived | `cores_used / assigned_vcpus`, only when both valid |
| `vm.cpu.assigned_vcpus` | gauge count | derived | Provisioned vCPU count, not usage |
| `vm.memory.provisioned_bytes` | gauge bytes | derived | Configured guest memory |
| `vm.memory.host_accounted_bytes` | gauge bytes | vm_cgroup | Host cgroup accounted memory; not guest working set |
| `vm.memory.guest_available_bytes` | gauge bytes | guest_agent | Memory available inside guest OS |
| `vm.block.read_bytes_total`, `write_bytes_total` | counter bytes | vmm, storage_provider | Per-VM virtual block bytes when attributable |
| `vm.net.rx_bytes_total`, `tx_bytes_total` | counter bytes | vmm, network_provider | Per-VM virtual NIC bytes when attributable |
| `vm.guest.fs.available_bytes`, `total_bytes` | gauge bytes | guest_agent | Guest filesystem data keyed by mount |
| `vm.guest.service.up` | state boolean | guest_agent | Configured/discovered service known running |
| `vm.guest.process.count` | gauge count | guest_agent | Count for approved process selector |
| `vm.guest.net.rx_errors_total` | counter count | guest_agent | Guest interface error counter |
| `check.duration_seconds` | gauge seconds | guest_agent | Bounded local check runtime |
| `check.status` | state enum | guest_agent | `ok`, `warning`, `critical`, `unknown` as state, not float |
| `monitoring.agent.last_seen_age_seconds` | gauge seconds | derived | Manager-computed age of last authenticated report |

Canonical IDs use `.` within CHV storage APIs. Prometheus export uses a separately maintained `chv_*` underscore name registry. Do not generate unlimited Prometheus label combinations by naively promoting all dimensions.

## Normalization

- CPU `cores_used` may exceed 1 for multi-vCPU VMs. `capacity_ratio` is 0..1 for valid measurements of an assigned capacity; overshoot is a validation anomaly, not blindly clamped.
- Ratios are fractional, not percentages. Frontend multiplies by 100 when rendering a percent.
- Byte counters are **integer precision**. JavaScript transports them as decimal strings in JSON when they can exceed JS-safe integer range.
- Calculate rates using positive deltas across two observations with the same `boot_id`, `identity_epoch`, counter key and source. A reset emits no rate for that interval.
- Unit conversion belongs to UI/query formatting. Never change metric unit depending on value magnitude.
- Guest available memory differs from VMM backing memory; show both with distinct labels.
- Guest reported `check.status` is not VMM observed status. It must not transition a VM state machine.
- Unknown or unsupported metrics appear as absent/unsupported, not successful with zero.

## Dimension allowlist and limits

Allowed dimensions include `interface_id`, `block_device_id`, `mount_id`, `service_key`, `process_selector`, `window`, `direction`, and `check_id`. Each registry entry declares allowed dimensions. Default max 4 dimensions/sample, 64 bytes per key and 128 bytes per value, 128 discovered check objects per agent. Enforce a per-target series limit and a global cap. Never accept arbitrary `tenant`, `project`, `vm_id`, `pid`, `command_line`, `token`, or `secret` as a label. Target IDs are indexed storage keys, not public Prometheus labels by default.

## Example v1 sample

```json
{
  "schema_version": 1,
  "target_kind": "vm",
  "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
  "metric_id": "vm.cpu.cores_used",
  "source": "vm_cgroup",
  "kind": "gauge",
  "unit": "cores",
  "observed_at_ms": 1791576000000,
  "value": 1.25,
  "quality": "valid",
  "dimensions": {},
  "boot_id": "node-boot-id",
  "identity_epoch": "vmm-process-start-fence"
}
```

## API evolution

Breaking unit/identity/schema changes require v2 IDs or a new schema version; no silent interpretation change. The supported metric IDs and sources are discoverable via `/v1/monitoring/catalog`. Clients reject unknown typed states safely. Tests use golden JSON fixtures, exact counter integer roundtrips, negative/NaN rejection, counter reset, and provenance mismatch cases.
