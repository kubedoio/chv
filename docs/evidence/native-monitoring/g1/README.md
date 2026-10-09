# G1 — native sampler evidence (real-host, qualified pin)

**Campaign:** native monitoring implementation (#602)
**Gate:** G1 (native measurement and source correctness, prompt 01)
**Date:** 2026-10-09 (UTC)
**Environment:** identical to the G0b capture — AMD EPYC 9554P, kernel
`6.8.0-142-generic`, real KVM, `cloud-hypervisor v53.0` static binary
digest-verified against `scripts/install.sh`'s pin
(`448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc`),
rust-hypervisor-fw `4a0a1e97…`, guest image `noble-qual-patched.img`
`37f7c340…` attached **`readonly=on`** (never written).

## Method

The env-gated integration test `g1_real_vmm_counters_measure_real_load`
(`crates/chv-agent-runtime-ch/src/process.rs`) boots the qualified VMM in
the qualification shape (2 vCPU, 512 MiB, one read-only disk,
`--serial null`), inserts the process as an adopted VMM exactly as the
runtime does, and drives the production `vm_counters` path:

```sh
CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
cargo test -p chv-agent-runtime-ch --lib g1_real_vmm -- --nocapture
```

Result: **1 passed in 15.27s** (CI skips this test — no KVM; the run
above is the real-host record).

## Observed values (verbatim from the run)

```text
g1 first:  VmCounters { cpu_percent: 0.0, memory_bytes_used: 75350016,
            memory_bytes_total: 0, disk_bytes_read: 31562752,
            disk_bytes_written: 0, net_bytes_rx: 0, net_bytes_tx: 0 }
g1 second: VmCounters { cpu_percent: 155.44063230023787,
            memory_bytes_used: 302358528, memory_bytes_total: 0,
            disk_bytes_read: 131572736, disk_bytes_written: 0,
            net_bytes_rx: 0, net_bytes_tx: 0 }
g1 after restart: VmCounters { cpu_percent: 0.0,
            memory_bytes_used: 111140864, memory_bytes_total: 0,
            disk_bytes_read: 31562752, disk_bytes_written: 0,
            net_bytes_rx: 0, net_bytes_tx: 0 }
```

(first = 5 s after boot; second = 6 s later, during the noble guest's
boot I/O; after-restart = fresh VMM process for the same VM entry.)

## Metric-by-metric proof

| Metric path | Source (truthful) | Evidence |
|---|---|---|
| VM host CPU (`vm.cpu.cores_used` semantics; legacy field `cpu_percent`) | `vmm` — identity-fenced VMM process `/proc/<pid>/stat` utime+stime deltas, epoch-scoped to start ticks | First observation: **0.0** (no interval — `insufficient_samples`, never a guessed number). Second: **155.4%** of one core — a real booting 2-vCPU guest. After VMM restart: **0.0** — the epoch crossing emits no rate (no fabricated spike). Unit tests additionally prove a busy-loop process yields a real positive rate and a recycled pid fails the epoch |
| VM host-accounted memory (`vm.memory.host_accounted_bytes` semantics; legacy field `memory_bytes_used`) | `vmm` — VMM process `VmRSS` | 75.3 MB at 5 s → 302.4 MB while the guest touched its 512 MiB → 111.1 MB for the fresh process after restart. Real RSS readings of the real VMM |
| VM block counters (`vm.block.*_bytes_total` semantics; legacy `disk_bytes_*`) | `vmm` — flat device map `vm.counters` | 31,562,752 bytes at 5 s (the exact G0b fresh-boot base) → 131,572,736 during boot I/O → restart resets to 31,562,752 (per-process base, G0b reset semantics). Parser unit-tested against the verbatim G0b fixtures, sentinel and partial-sum rules included |
| VM net counters (`vm.net.*_bytes_total`) | `vmm` — flat device map `vm.counters` | 0 — the test firmware has no network stack (G0b finding); the shape parses, the value is a real zero, not a missing-field default |
| VM provisioned memory/CPUs | `derived` from the VM spec (`VmRecord.memory_bytes`) | filled at the state-report call site from configuration, never presented as measurement |
| Node CPU (`node.cpu.capacity_ratio`) | `node_os` — retained sysinfo collector | `metrics_server` repair: first scrape has no interval (flattens to 0 on the legacy `/metrics` surface, `insufficient_samples` on the sample path); the sampler's collector unit-tests prove interval measurement and retention across sub-interval cycles |
| Node load / memory / swap / fs / per-interface / per-device counters | `node_os` — `/proc/loadavg`, sysinfo, `/proc/net/dev`, `/proc/diskstats` | parser unit tests (real `/proc` reads on the test host) + sampler end-to-end test proving contract samples flow with epochs on counters and no value on non-valid quality |
| Sampler bounds and health | — | unit tests: hung source times out and the cycle continues; full sink drops and counts instead of blocking; closed sink exits the loop; `/metrics` carries only global labels |

## Honest absences (reported, not faked)

- **No `vm_cgroup` arm in production yet**: the runtime does not place
  VMM processes in dedicated per-VM cgroups; the cgroup v2 probe module
  (fence + `cpu.stat`/`memory.current` readers) is implemented and
  unit-tested, but production VM CPU/memory come from the `vmm`
  process-stat arm. The contract's source row records both arms.
- **stord/nwd provider sources**: `chv-stord` exposes volume *health*
  (not a v1 registry metric) and neither daemon exposes attributable
  capacity/throughput — no adapters wired, no coverage faked.
- **Guest memory** (`vm.memory.guest_available_bytes`): guest-agent
  scope (G3/G4), untouched.
- **Legacy-transport flattening**: `VmStateReport`/`vm_metrics` fields
  cannot carry quality markers; unavailable values flatten to `0` there
  (documented on `VmCounters`). The v1 sample path (PR-2 ingest) is the
  quality-carrying transport.

## Gate verdict

**G1 PASS** for the native sampler scope of PR-1: host and VM CPU,
memory, disk and network samples measure real loads with truthful
sources and no-data handling; lifecycle is untouched by collection
(read-only `/proc` + existing API polling; the vm_counters path holds
the same `vms` write lock discipline as before); unsupported metrics are
reported honestly. Deferred items are recorded above and in #602.
