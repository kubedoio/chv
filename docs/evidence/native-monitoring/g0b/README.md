# G0b — pinned-v53.0 counter fixture baseline (native monitoring campaign)

**Campaign:** native monitoring implementation (#602, plan
`docs/plans/2026-10-09-native-monitoring-implementation.md`)
**Gate:** G0b (empirical half of G0; G0a satisfied by the design merge, #599 → `13c00553`)
**Verdict:** **PASS** — with two design-relevant empirical facts folded into the
proposed documents (see §5) and one pre-existing production defect recorded as
the PR-1 baseline (see §4). No contract contradiction found; no design
amendment beyond the one-line source clarification in the native spec.

## 1. Environment and artifacts

| Item | Value |
|---|---|
| Date | 2026-10-09 (UTC), capture timestamps in `fixtures/capture-timestamps.txt` |
| Host | AMD EPYC 9554P 64-Core (16 vCPU visible), 31 GiB RAM, kernel `6.8.0-142-generic` |
| KVM | `/dev/kvm` present and functional (VMs booted and ran for the full capture) |
| VMM (qualified pin) | `cloud-hypervisor v53.0` static build, sha256 `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` — **digest-verified** against `scripts/install.sh`'s pin before use |
| VMM (comparison) | `cloud-hypervisor v43.0.0` (previous pin, `/usr/bin/cloud-hypervisor`, sha256 `a250a934…`) — captured for schema-history evidence only |
| Firmware | rust-hypervisor-fw 0.5.0 at `/var/lib/chv/qual/hypervisor-fw`, sha256 `4a0a1e97…` (same artifact as every v53.0 requalification leg) |
| Guest image | `/var/lib/chv/qual/images/noble-qual-patched.img`, sha256 `37f7c340…` (requalification artifact), attached **`readonly=on`** — the shared image is never written |
| VM shape | 2 vCPU, 512 MiB, single disk, one tap NIC, `--serial null`, per-run api-socket paths (matches the m2.5 e-series / requalification leg-02 shape, with a NIC added for net-counter capture) |
| Transport | HTTP over per-VM unix domain socket (`curl --unix-socket`), same transport the agent uses (`ch_api.rs`) |

Full digests in `fixtures/artifact-digests.txt`.

## 2. Method

1. Digest-verify the v53.0 static binary against the install-script pin; boot
   one VM per version in the qualification shape.
2. Capture `GET /api/v1/vmm.ping`, `GET /api/v1/vm.info`,
   `GET /api/v1/vm.counters` (t0), wait ~10 s, capture `vm.counters` (t1).
3. Hard-kill the v53.0 VM (SIGTERM; the test firmware ignores ACPI shutdown),
   boot a fresh v53.0 VM, capture `vm.counters` ~4 s after boot —
   counter-reset evidence.
4. Cross-check every captured shape against the pinned v53.0 OpenAPI
   (`vmm/src/api/openapi/cloud-hypervisor.yaml` @ `v53.0`, excerpt in
   `fixtures/openapi-vmcounters-v53.yaml`).
5. A broadcast-ping attempt against the VM's tap produced no net-counter
   movement (`_net1` stays zero — the firmware has no network stack), so the
   net fixtures show the schema with zero values; the schema is the deliverable.

## 3. Findings (all fixture-backed)

### 3.1 `vm.counters` is a flat device-keyed map — no `cpus`, `net`, or `block` objects

v53.0 response (verbatim shape, from `fixtures/vm.counters.t0.json`):

```json
{
  "_disk0": { "read_bytes": 147753984, "read_ops": 64306, "write_bytes": 0,
              "write_ops": 0, "read_latency_min": 3, "read_latency_max": 65932,
              "read_latency_avg": 19, "write_latency_min": 18446744073709551615,
              "write_latency_max": 18446744073709551615,
              "write_latency_avg": 1844674407370955 },
  "_net1":  { "rx_bytes": 0, "rx_frames": 0, "tx_bytes": 0, "tx_frames": 0 }
}
```

Confirmed by the pinned OpenAPI: `VmCounters = map<string, map<string, int64>>`
with device ids (`_disk0`, `_net1`) as keys — the same ids `vm.info` reports
under `disks[].id` / `nets[].id`. v43.0 (previous pin) returns the **same flat
shape** (`fixtures/vm.counters.v43.json`) — this is not a v43→v53 change.

### 3.2 No CPU-usage counter exists on the pinned API

Neither v53.0 nor v43.0 `vm.counters` contains a `cpus` section, and the
pinned v53.0 OpenAPI defines no other endpoint exposing per-VM CPU time.
Consequence for the design: the `vmm` source cannot produce
`vm.cpu.cores_used` on the qualified pin; the `vm_cgroup` source (already an
allowed source in `chv-monitoring-metrics-v1.md`) is the primary. Folded into
the native spec (§5).

### 3.3 No-data sentinels are u64::MAX-shaped

`write_latency_min`/`write_latency_max` report `18446744073709551615`
(u64::MAX) and `write_latency_avg` reports `1844674407370955` when no write
has occurred. Naive latency averaging or min/max passthrough fabricates
giant values; naive "unwrap_or(0)" conflates no-data with zero. The metrics
contract's `quality` marker and "never a numeric placeholder" rules are
load-bearing here, and the sampler must treat u64::MAX-shaped sentinels as
absence, not data.

### 3.4 Counters reset to a new per-process base on restart

VM 1 t1: `read_ops = 65367`. After a hard kill and fresh boot, ~4 s in:
`read_ops = 61646`, `read_bytes = 31562752` — the same base the v43.0 run
showed at the same point (the firmware's early reads are deterministic).
Counters do not continue across processes and do not start at zero; they start
wherever the new process's device activity lands. Rate computation must be
boot-epoch-scoped (`boot_id`), and non-monotonic deltas within an epoch are
resets, not spikes — exactly ADR-025's counter semantics.

### 3.5 No memory counters; provisioned memory is on `vm.info`

`vm.counters` has no memory section (both versions). `vm.info` carries
`config.memory.size` (536870912 in the fixture) and `config.cpus.boot_vcpus`
— the configuration-derived inputs for `vm.memory.provisioned_bytes` and
`vm.cpu.assigned_vcpus`. Guest-internal memory usage is not observable from
the VMM, confirming the design's split between host-accounted memory
(`vm_cgroup`) and guest-reported memory (`guest_agent`).

### 3.6 Operational notes from the capture

- CH warns `Non-raw image type detected. In the future it will be necessary
  to specify image_type for non-raw files` for qcow2 disks — the agent already
  pins `image_type` explicitly (requalification follow-up); future pins must
  keep doing so.
- `vm.shutdown` (graceful) does not terminate a firmware-only guest that
  ignores ACPI; lifecycle tests that need deterministic teardown should use
  the process-supervisor path, as production does.

## 4. Pre-existing production defect (recorded; fixed by PR-1, not here)

The current `vm_counters` implementation
(`crates/chv-agent-runtime-ch/src/process.rs`, `vm_counters`) parses
`/cpus/usage/cpu_seconds` and top-level `net`/`block` objects — a schema that
matches neither the qualified v53.0 pin nor the previous v43.0 pin. Every
lookup misses and every `unwrap_or` silently yields zero, so VM resource
metrics have been zero end-to-end:

- adapter: `cpu_percent = 0.0`, `memory_bytes_*` (the `/memory/available`
  pointer also matches nothing) `= 0`, disk/net `= 0`;
- `cmd/chv-agent/src/main.rs` fills `VmStateReport` with those zeros (and
  pre-zeroes them for non-running VMs);
- `crates/chv-controlplane-service/src/telemetry.rs` suppresses the insert
  entirely when all values are zero ("Store runtime counters if any are
  present (non-zero)") — so the `vm_metrics` table receives no rows at all
  from this path today;
- the BFF `POST /v1/metrics` "top consumers" query and the UI render the
  resulting zeros/absence, and `NodeHealthDashboard.svelte` fabricates graph
  history with `Math.random()` (lines 56–61).

PR-0 changes no code: this is the G0b baseline record. The defect is the
concrete before-state for gate G1 — PR-1's sampler replaces this path with
the contract's typed, quality-carrying sources.

## 5. Reconciliation against the proposed documents

| Document | Check | Result |
|---|---|---|
| `chv-monitoring-metrics-v1.md` | registry sources vs pinned API | consistent — `vm.cpu.cores_used` already allows `vm_cgroup`; `vm.block.*`/`vm.net.*` from `vmm` match the captured per-device counters; `vm.memory.provisioned_bytes` (derived) matches `vm.info` |
| `chv-native-monitoring-spec.md` | source table | one clarification folded: the v53.0 `vm.counters` CPU arm is unavailable, `vm_cgroup` is the primary for VM CPU (§3.2) |
| ADR-025 counter semantics | vs captured reset/sentinel behavior | confirmed load-bearing (§3.3, §3.4) |
| `chv-monitoring-ingestion-v1.md`, ADR-026/027 | not exercised by G0b | untouched |
| Contracts vs current code | prompt-00 reconciliation | the only contradiction is the §4 defect (code wrong, contracts right); no contract change needed |

## 6. Fixture manifest

| File | Content |
|---|---|
| `fixtures/vmm.ping.json` / `vmm.ping.fresh.json` | v53.0 build/version/features + pid (two processes) |
| `fixtures/vm.info.json` | full v53.0 VM config (cpus, memory.size, disks[].id, nets[].id, payload) |
| `fixtures/vm.counters.t0.json` / `t1.json` | v53.0 counters ~10 s apart — monotonic device deltas |
| `fixtures/vm.counters.fresh-boot.json` | v53.0 counters ~4 s after a fresh process boot — per-process reset base |
| `fixtures/vm.counters.v43.json` | v43.0 counters, same shape — schema-history control |
| `fixtures/openapi-vmcounters-v53.yaml` | verbatim `/vm.counters` path + `VmCounters` schema from the pinned OpenAPI |
| `fixtures/artifact-digests.txt`, `fixtures/capture-timestamps.txt` | binary/firmware/image digests and capture time |

Fixtures are captured artifacts — never hand-edited; refresh only by re-running
the capture on a real host against the pinned binary (digest-verified).
