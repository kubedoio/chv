# G4 part 1 — guest collectors, checks and plugin constraints on a real VM

**Campaign:** native monitoring implementation (#602)
**Gate:** G4 part 1 (guest inspection — prompt 04, tasks 1–9)
**PR:** PR-5 (`monitoring-g4-collectors-checks`)
**Date:** 2026-10-10 (UTC)
**Environment:** identical to the G0b/G1/G2/G3 captures — AMD EPYC 9554P,
kernel `6.8.0-142-generic`, real KVM, `cloud-hypervisor v53.0` static
binary digest-verified against `scripts/install.sh`'s pin
(`448af3d4e59b22c2…`), rust-hypervisor-fw `4a0a1e97…`, guest image
`noble-qual-patched.img` `37f7c340…`. The guest package under test is
`chv-monitor-agent_0.3.0_amd64.deb` (`76467369d129e758…`) built from
the final branch state (both gate-catch fixes and the review-round-1
fixes compiled in; byte-compared against
`target/release/chv-monitor-agent` before the run). The
manager runs in-process in the test binary — the real
`chv-controlplane-service` HTTPS/TLS/ingest stack and the real
`MonitoringStore`.

## Method

### 1. Full vertical on a real VM

The env-gated integration test
`g4_real_vm_collectors_checks_and_plugin_constraints`
(`cmd/chv-monitor-agent/tests/g4_real_vm.rs`) extends the G3 rig's
production path (bridge + tap, real `vm.create`/`vm.boot`, enriched
NoCloud seed, `dpkg -i`, one-time claim, mTLS enrollment) with the
G4 surface: the guest's `agent.toml` enables the fs/net/process
collectors, the configured + discovered systemd checks and one local
http + one deliberately-failing tcp check — **plugins stay at their
default `enabled = false`** with the allowlist populated on disk.

The scenarios (each a real guest transition driven over ssh, then
observed through the manager's own query paths — never by reading
guest state directly):

- **Filesystem fill and recovery** — 200 MiB written to `/g4fill`
  (on the ext4 root, not tmpfs): `vm.guest.fs.available_bytes` for
  `ext4:/` drops accordingly and recovers after removal.
- **Inode exhaustion on a scratch tmpfs** — a tmpfs with
  `nr_inodes=40` at `/mnt/g4inodes`, 36 files created:
  `vm.guest.fs.inodes_utilization_ratio` for the scratch mount reads
  ≥ 0.8, and the series goes stale after unmounting.
- **Service stop/start flipping the checks** — a real
  `g4-http.service` (python3 http.server) is stopped and restarted:
  `service:g4-http.service` and `http:app` flip ok → critical → ok
  while the `tcp:ssh` control stays ok.
- **not-installed ≠ stopped** — `g4-absent.service` (never
  installed) reports `unknown` with a "not installed" summary and
  **no** `vm.guest.service.up` sample.
- **Failing TCP check** — `tcp:closed` against a port with no
  listener reports `critical`.
- **Process start/exit** — a `python3` process (a configured
  selector) appears and disappears in `vm.guest.process.count`.
- **Counter advance** — the guest NIC's `vm.guest.net.rx_bytes_total`
  advances between two observations (the agent's own reporting
  traffic guarantees movement — real counters, not constants), with
  the error-counter series present.
- **Check trends** — `check.status` history accumulates points
  across the transitions above.
- **Plugin constraints** — with files present and
  `[plugins] enabled = false`: zero plugin checks; after explicit
  local enable + restart: `plugin:g4-http-health` reports ok; a
  rogue replacement executable degrades the check **without ever
  running** (a never-ran marker proves non-execution); at the end
  the allowlist directory is byte-identical and root-owned.

```sh
CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
CHV_G4_AGENT_DEB=$PWD/dist/packages/chv-monitor-agent_0.3.0_amd64.deb \
cargo test -p chv-monitor-agent --test g4_real_vm -- --nocapture
```

Result: **1 passed in 762.22s** (CI skips this test — no
KVM; the run above is the real-host record on the final branch
state; verbatim observations below).

### 2. In-process depth (identical manager/TLS code)

`cmd/chv-monitor-agent/tests/e2e.rs`
(`g4_families_checks_and_discovery_flow_end_to_end`, + the G3
lifecycle tests) runs the real `Agent` with the G4 config shape
against the real TLS listener and `MonitoringAgentService` on this
test host (which runs systemd): the full envelope — every collector
family, the systemd checks with **real discovery including instance
units**, and local http/tcp checks — is accepted whole, nothing
spools, all six G4 sample families are queryable through the store,
and the check inventory records the configured unit, both local
checks and the discovered units. This is the regression net for
gate-catch #1 below.

### 3. Package behavior on a real system

The deb/rpm smoke tests (PR-4) cover the package mechanics; the G4
delta in the unit/config surface is exercised by the rig itself
(dpkg install, conffile replacement with the G4 config, explicit
`systemctl restart` after local config edits).

## Observed (verbatim from the recorded run)

```text
g4 checkpoint: network up (bridge + tap)
g4 checkpoint: manager listening on https://192.168.63.1:37395
g4 checkpoint: creating vm (production adapter, seed built)
g4 checkpoint: seed enriched with the agent package, fixtures and ssh key
g4 checkpoint: guest executing (vmm cpu ticks +58)
g4 checkpoint: agent enrolled
g4 checkpoint: ssh reachable
g4 checkpoint: vm.guest.fs.available_bytes: 6 valid points
g4 checkpoint: vm.guest.fs.total_bytes: 6 valid points
g4 checkpoint: vm.guest.fs.inodes_utilization_ratio: 5 valid points
g4 checkpoint: vm.guest.fs.read_only: 7 valid points
g4 checkpoint: vm.guest.net.rx_bytes_total: 2 valid points
g4 checkpoint: vm.guest.net.tx_bytes_total: 2 valid points
g4 checkpoint: vm.guest.net.rx_errors_total: 2 valid points
g4 checkpoint: vm.guest.net.tcp_established: 2 valid points
g4 checkpoint: vm.guest.process.count: 2 valid points
g4 checkpoint: vm.guest.process.rss_bytes: 2 valid points
g4 checkpoint: vm.guest.process.cpu_utilization_ratio: 2 valid points
g4 checkpoint: root mount: 750505984/2525810688 bytes available
g4 checkpoint: guest NIC rx counters advance
g4 checkpoint: process selectors measure real processes
g4 checkpoint: service checks in inventory: 22 (3 configured + discovered)
g4 checkpoint: check.status: 25 valid points
g4 checkpoint: check.duration_seconds: 25 valid points
g4 checkpoint: plugins disabled by default: files present, zero plugin checks
g4 checkpoint: root available: 750505984 -> 540770304 after the 200 MB fill
g4 checkpoint: root available recovered: 750465024 (baseline 750505984)
g4 checkpoint: scratch tmpfs inode utilization: 0.025 -> 0.925 (36/40 inodes)
g4 checkpoint: inode exhaustion observed on a scratch mount; series went stale after unmount
g4 checkpoint: process exit observed: python3 selector measured 0
g4 checkpoint: process start observed: python3 selector recovered
g4 checkpoint: real outage flipped and recovered both checks; trend recorded
g4 checkpoint: plugin check reported ok after explicit enable
g4 checkpoint: tampered plugin degraded without ever being executed
g4 checkpoint: allowlist directory integrity intact end to end
g4 checkpoint: vm stopped and deleted
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 762.22s
```

Reading the observations:

- **Families**: every G4 sample family has valid points in the
  manager's bounded history for this VM — fs (available, total,
  inode utilization, read-only), net (rx/tx counters as same-epoch
  deltas, error counters, TCP established), process (count, RSS,
  CPU utilization per selector).
- **Root mount truth**: `ext4:/` reads 750505984 of 2525810688
  bytes available and — the init-namespace fix under test —
  **writable**, on a guest cloud-init had just written a package
  install onto.
- **Fill/recovery**: the 200 MiB write drops root available by
  209735680 bytes (within noise of 200 MiB exactly); removal
  restores the baseline within 40 KiB.
- **Inode exhaustion**: the scratch tmpfs (`nr_inodes=40`) reads
  0.025 → 0.925 as 36 files land — and the series goes **stale**
  after unmount (an honest absence, never a frozen value). The
  tmpfs appearing at all proves the unit's slave mount propagation
  carries later operator mounts to the agent's statvfs step.
- **Real OS behavior**: 22 service checks in inventory (3 configured
  + discovered units, instance units among them — gate-catch #1's
  fix under live test); the real outage of `g4-http.service` flips
  `service:g4-http.service` and `http:app` and recovers both, with
  `check.status` accumulating 25 trend points across the
  transitions; the python3 selector measures 0 after exit and
  recovers after restart; the guest NIC's rx counter advances
  between observations.
- **Plugin constraints**: files present with `[plugins] enabled =
  false` → zero plugin checks; after explicit local enable +
  restart, `plugin:g4-http-health` reports ok; the rogue
  replacement executable degrades the check to unknown **without
  ever being executed** (a never-ran marker inside the guest proves
  non-execution); the allowlist directory ends the session
  byte-identical and root-owned.
- **Teardown**: graceful `stop_vm` + `delete_vm`, bridge and tap
  removed, no leaked VMM process.

## The gate working as designed — two real-OS catches

The recorded run is the **fourth** attempt, and the history is part
of the evidence. The first two failures were not rig bugs; they
were the G4 gate ("check inventory reflects real OS behavior")
catching contract and collector defects that no fixture-based test
could catch. The third attempt validated both fixes and passed
every scenario, but predated the review-round-1 fixes — the fourth
(above) is the final branch state:

1. **Systemd instance units starved the store** (`a5afd752`). The
   rig's own ssh login starts `user@1000.service`; bounded discovery
   found it; the agent's unit-name charset allowed its `@` but the
   manager-side check-id charset did not — so the first batch
   carrying a discovered instance unit was rejected **whole**, the
   agent retried the poisoned batch forever, and the VM enrolled
   while reporting nothing. Symptom: every sample family read as
   absent. Fix: check identifiers accept `[A-Za-z0-9._:/@-]`
   (ingestion-contract v1 amendment); regression net: the in-process
   e2e test above.
2. **The sandboxed mount namespace lied about read-only state**
   (`a3eb2e6a`). The agent's own systemd unit hardens it with
   `ProtectSystem=strict` — which remounts `/` read-only *in the
   agent's mount namespace*. The fs collector read
   `/proc/self/mounts` and honestly reported a view that was false
   about the guest: the freshly-booted, cloud-init-written root read
   as read-only. Fix: the family reads PID 1's mounts
   (`/proc/1/mounts`) — the live, unsandboxed init-namespace view
   (world-readable; an honest absence on hidepid-hardened guests;
   later operator mounts still reach the statvfs step through the
   unit's slave mount propagation, which the scratch-tmpfs scenario
   proves).

## Gate criteria vs. evidence (G4 part 1)

| G4 requirement | Evidence |
|---|---|
| Check inventory and trend charts reflect real OS behavior | The recorded run: service stop/start flips `service:` and `http:` checks ok → critical → ok with `tcp:ssh` as the control; not-installed is `unknown` with no `service.up` sample; fs fill/recovery and inode exhaustion move the fs series; process start/exit moves selector counts; NIC counters advance; `check.status` history accumulates across the transitions — and the two real-OS catches above are themselves evidence the gate discriminates |
| Arbitrary code deployment from the manager impossible | No plugin field/route/table exists in the ingest protocol (structural); the recorded run ends with the allowlist directory byte-identical and root-owned after a full session of active manager ingestion |
| Plugin execution disabled by default and constrained when enabled | Files present + `enabled = false` → zero plugin checks; explicit local enable → pinned plugin ok; rogue executable → degraded without execution (never-ran marker); module tests pin every constraint (digest mismatch, symlink escape, ownership, kill, caps) |

## Honest absences and findings (reported, not faked)

- **Alerting** (thresholds, notification routing) is G4 part 2
  (PR-6) — not faked here.
- **vsock / multi-node transport** is prompt 06 scope (PR-7); this
  gate's guest path is outbound HTTPS over the bridge, as designed
  for v1.
- **Plugin signature verification and centrally managed plugins**
  are out of v1 — the root-owned sha256 allowlist is the v1
  mechanism; the security contract records the limitation.
- **hidepid-hardened guests**: `/proc/1/mounts` is world-readable
  by default, but a guest mounted with `hidepid=1/2` denies it to
  the unprivileged agent user and the fs family degrades to an
  honest absence (no fabricated mount state).
- **Escaped systemd unit names** (`\x20`-style escapes for
  special-character paths) are outside both the agent's unit-name
  charset and the check-id charset — such units are filtered from
  discovery and cannot be configured. Bounded and honest; recorded
  as a v1 limitation.
- The local e2e suite on a shared host can contend with parallel
  cargo test runs (CI is immune — it runs suites serially per
  crate); noted on #602 as existing debt.

## Gate verdict

**G4 part 1 PASS** for the PR-5 scope: real-OS guest inspection
through the production create → seed → boot → install → enroll →
ingest path — filesystems (fill/recovery, inode exhaustion,
staleness), network (advancing counters), processes (start/exit),
systemd services (real outage flip/recovery, discovery including
instance units, the not-installed tri-state), declarative local
checks (http ok, failing tcp critical), and the plugin constraints
(default-off, pinned-ok-when-enabled, never-executed tamper,
allowlist integrity) — all observed through the manager's own query
paths on a real VM, with the gate's two real-OS catches recorded
above as evidence the gate discriminates. Alerting (G4 part 2) is
PR-6; vsock/multi-node is PR-7.
