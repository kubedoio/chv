# G3 — optional guest agent on a real VM (secure enrollment, telemetry, revoke)

**Campaign:** native monitoring implementation (#602)
**Gate:** G3 (optional guest monitoring agent — prompt 03)
**PR:** PR-4 (`monitoring-g3-guest-agent`)
**Date:** 2026-10-10 (UTC)
**Environment:** identical to the G0b/G1/G2 captures — AMD EPYC 9554P,
kernel `6.8.0-142-generic`, real KVM, `cloud-hypervisor v53.0` static
binary digest-verified against `scripts/install.sh`'s pin
(`448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc`),
rust-hypervisor-fw `4a0a1e97…`, guest image `noble-qual-patched.img`
`37f7c340…`. The guest package under test is
`chv-monitor-agent_0.3.0_amd64.deb` (`45fe10782d22a8e5…`) built from
this branch by `scripts/build-packages.sh`'s exact nfpm invocation
(with the review-round-1 fixes compiled in).

## Method

### 1. Full vertical on a real VM

The env-gated integration test `g3_real_vm_guest_agent_enrolls_collects_and_revokes`
(`cmd/chv-monitor-agent/tests/g3_real_vm.rs`) drives the **production**
path end to end — nothing mocked between the guest's systemd unit and
the manager's SQLite rows:

- Test-side network: a host bridge (`br-g3ev`, 192.168.62.1/24) and
  tap, created with plain `ip` commands (production topology is nwd's
  netns/nft work — not this gate's subject). The in-process manager's
  real `serve_tls` listener (the production `chv-controlplane-service`
  HTTPS stack, rustls, client certs optional at the listener and
  *required* by the agent routes) binds the bridge IP with a server
  certificate whose SAN is that IP.
- `ProcessCloudHypervisorAdapter::create_vm` (real REST `vm.create`,
  real NoCloud seed build: user-data/meta-data/network-config from the
  production code path, including the static-IP network-config for the
  rig's NIC).
- Between `create_vm` and `start_vm` the seed is **enriched**: the
  `.deb`, the agent's `agent.toml` (5 s interval), the manager's server
  certificate PEM and the one-time claim token are added to the seed
  directory and the ISO rebuilt with the identical genisoimage
  invocation (`-volid cidata -joliet -rock`), atomically renamed into
  place. The guest installs everything from the seed — no network file
  server involved.
- `start_vm` (real `vm.boot`), then a **boot proof**: the VMM process's
  CPU time must advance (`vm.boot` 200/204 alone is not evidence the
  guest executes — see the honest finding below).
- Inside the guest, cloud-init `runcmd` runs **exactly what
  `docs/install/guest-monitor-agent.md` tells an operator to run**:
  mount the seed, `dpkg -i` the package (creating the `chv-monitor`
  user, the 0700 state dir, the hardened disabled unit), overwrite the
  conffile with the real config, place the claim with
  `install -o chv-monitor -m 0600`, `systemctl enable --now`.

```sh
CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
CHV_G3_AGENT_DEB=dist/packages/chv-monitor-agent_0.3.0_amd64.deb \
cargo test -p chv-monitor-agent --test g3_real_vm -- --nocapture
```

Result: **1 passed in 123.01s** (CI skips this test — no KVM; the run
above is the real-host record on the final branch state, re-run after
the review-round-2 fixes; verbatim observations below).

### 2. In-process lifecycle depth (identical manager/TLS code)

`cmd/chv-monitor-agent/tests/e2e.rs` (4 tests, <1 s) runs the real
`Agent` against the real TLS listener, `MonitoringAgentService` and
stores, covering the paths a second real-VM boot cannot reach without
console/SSH access to the running guest: forced rotation, revoke →
401 → fresh-claim re-enrollment, spool replay ordering, and the
poison-pill rule (a 400 `invalid_batch` discards, a 5xx retries).

### 3. Package behavior on a real system

The deb/rpm smoke tests (extended in this PR) install/remove/reinstall
the guest package alongside the host packages and verify the user
creation, 0700 state dir, unit hardening directives,
disabled-by-default, config `noreplace` handling, and state
preservation across removal.

## Observed (verbatim from the recorded run)

```text
g3 checkpoint: network up (bridge + tap)
g3 checkpoint: manager listening on https://192.168.62.1:44807
g3 checkpoint: creating vm (production adapter, seed built)
g3 checkpoint: seed enriched with the agent package and inputs
g3 checkpoint: guest executing (vmm cpu ticks +58)
g3 checkpoint: vm booted; waiting for cloud-init to install and the agent to enroll
g3 checkpoint: still waiting for enrollment (poll 30); guest console tail:
[...] cloud-init[670]: Cloud-init v. 26.1-0ubuntu1~24.04.1 running 'modules:config' at Sat, 10 Oct 2026 10:33:54 +0000. Up 14.04 seconds.
[...] cloud-init[780]: Cloud-init v. 26.1-0ubuntu1~24.04.1 running 'modules:final' at Sat, 10 Oct 2026 10:34:30 +0000. Up 49.40 seconds.
[...] cloud-init[780]: Selecting previously unselected package chv-monitor-agent.
[...] cloud-init[780]: Unpacking chv-monitor-agent (0.3.0) ...
[...] cloud-init[780]: Setting up chv-monitor-agent (0.3.0) ...
[...] cloud-init[780]: chv-monitor-agent: installed (disabled by default).
[...] cloud-init[780]: chv-monitor-agent: place a one-time claim at /var/lib/chv-monitor/claim (owner chv-monitor, 0600), then:
[...] cloud-init[780]: Created symlink /etc/systemd/system/multi-user.target.wants/chv-monitor-agent.service → /usr/lib/systemd/system/chv-monitor-agent.service.
[  OK  ] Started chv-monitor-agent.service - CHV Guest Monitoring Agent.
[...] cloud-init[780]: Cloud-init v. 26.1-0ubuntu1~24.04.1 finished at Sat, 10 Oct 2026 10:34:31 +0000. Datasource DataSourceNoCloud [seed=/dev/vdb].  Up 50.64 seconds
g3 checkpoint: agent enrolled
g3 checkpoint: vm.guest.load1: 1 valid points
g3 checkpoint: vm.guest.uptime_seconds: 1 valid points
g3 checkpoint: vm.guest.cpu.utilization_ratio: 1 valid points
g3 checkpoint: vm.memory.guest_available_bytes: 2 valid points
g3 checkpoint: manager outage begins
g3 checkpoint: manager back; waiting for the spool to drain
g3 checkpoint: spool drained: sequence 2 -> 6
g3 checkpoint: agent revoked; waiting for the block to settle
g3 checkpoint: revocation blocks reporting, liveness frozen
g3 checkpoint: vm stopped and deleted
```

(The `[...]` lines are the rig's periodic guest-console dump — the
real in-guest systemd/cloud-init log, not test narration.)

Reading the observations:

- **Enrollment**: firmware boot → kernel → cloud-init → `dpkg -i` →
  systemd start → claim redemption over mutual TLS through the bridge,
  with cloud-init finishing at Up 50.64 s and the agent enrolled right
  behind it. The registry row for the VM is
  `active`, bound to exactly this VM (`vm_id` match is asserted, not
  assumed), and the OS identity on the row (`os_name` "Ubuntu",
  `os_kernel_release` present) came from inside the guest via the
  batch envelope's privacy-allowlist fields.
- **Telemetry**: `vm.guest.load1`, `vm.guest.uptime_seconds`,
  `vm.guest.cpu.utilization_ratio` and
  `vm.memory.guest_available_bytes` (the guest-collected memory,
  separately labeled from host-accounted VM memory) all have valid
  points in the manager's bounded history for this VM.
- **Outage durability**: with the listener down ~15 s (3 s settle
  baseline + a 12 s freeze observation, spanning 2+ collection
  intervals at the rig's 5 s), the registry's sequence high-water
  froze at 2 — no ingestion while the manager is away. After the
  listener returned, the high-water advanced to 6: the spooled
  batches drained oldest-first through the same enrolled credential.
- **Revocation**: after the operator revoke, `status` is `revoked` and
  both `last_seen_at_ms` and `last_sequence` are frozen across a 30 s
  observation window straddling multiple collection ticks — the agent
  is 401-blocked, not merely quiet. (The settle-then-compare pattern
  tolerates a batch in flight at revoke time: the store's own
  `status = 'active'` guard rejects it.)
- **Teardown**: graceful `stop_vm` + `delete_vm`, bridge and tap
  removed, no leaked VMM process.

## Gate criteria vs. evidence

| G3 requirement | Evidence |
|---|---|
| Secure Linux guest telemetry on a real VM | The recorded run above: real firmware boot, real cloud-init package install, mTLS claim redemption, four guest metric families with valid points, OS identity from inside the guest |
| Separate off-by-default package; no host package pulls it in | nfpm config has no host-package relationship; smoke tests verify disabled-by-default and no cross-dependency; the rig had to `systemctl enable --now` explicitly |
| Secure enrollment (hashed single-use claims, TTL, operator-authorized, audited) | SHA-256-hashed storage, atomic single-use consume (store tests), operator-tier issuance route with audit events; the real-VM run redeemed exactly one claim and the manager consumed it |
| Scoped identity (its own VM's telemetry + own rotation only) | Enrolled cert CN=agent_id OU=chv-monitor-agent from a dedicated CA; agent routes require the client cert and bind every sample to the enrolled VM (`ForbiddenTarget` on mismatch — service tests); no VM-authority or command-execution endpoint exists in the agent router |
| Rotation and revocation | Rotation + revoke-re-enroll in-process e2e against the identical manager/TLS code; revocation's terminal block proven on the real VM above |
| No guest inbound port; outbound HTTPS with authenticated server | The guest's only connection is outbound to the manager's TLS listener; the agent config rejects plain `http://` (config test) and pins the manager CA (no WebPKI/accept-any path anywhere — review-verified) |
| Rate/size/series limits enforced server-side | Body, sample-count, per-agent rate and series caps in `MonitoringAgentService` (service tests), ceilings now validated at boot so config cannot raise them |
| Non-installed guests remain fully usable; unmodified host-only path | G2's real-VM record is exactly that path (host-side VMM metrics, no guest package); the agent adds telemetry, VM lifecycle never depends on it (G2's disk-full evidence + this rig's clean teardown) |
| Record precise credential trust limitations; no hardware attestation claims | Below |

## Credential trust — the precise claim

An enrolled agent credential proves **that its holder redeemed an
operator-issued, single-use, short-lived claim bound to this VM
identity on this install** — nothing more. It does **not** prove the
VM image, kernel, or hardware; CHV makes **no hardware attestation
claim**, and none of the evidence above should be read as one. A
stolen unexpired claim is a bearer secret until consumed; a
compromised guest can read (and falsify) its own telemetry but cannot
reach other VMs' telemetry, other endpoints, or any VM lifecycle
operation. Cloned images sharing a credential are detected as
identity conflicts and blocked until an operator reset
(`identity_conflict` — covered by store/service tests and the UI
vocabulary).

## Honest absences and findings (reported, not faked)

- **Re-enrollment on the running real guest** (revoke → fresh claim →
  re-enroll *in the same booted VM*) is covered by the in-process e2e
  (`tests/e2e.rs`) rather than the real-VM rig: delivering a second
  claim into a running guest needs console/SSH access the rig does not
  provision. The manager and TLS stack are the identical production
  code in both runs.
- **cloud-hypervisor refuses qcow2 backing files** —
  `Maximum disk nesting depth exceeded` on `vm.boot` (found while
  building this rig; the failed boot also exposed that a 204
  `vm.boot` is not proof a guest executes). The rig therefore uses a
  full sparse copy of the pinned image as the writable root disk: the
  pinned bytes themselves are only ever read, matching the G0b/G1/G2
  discipline.
- **Agent-side filesystem collectors, process/service collectors,
  plugins and checks** are G4 (prompt 04) scope — not faked here.
- **vsock transport** is prompt 06 scope; this gate's guest path is
  outbound HTTPS over the bridge, as designed for v1.
- The fixed settle windows (3 s outage baseline settle, 12 s freeze
  observation, 15 s revocation settle) are sized against the rig's
  5 s collection interval with margin; the assertions they feed
  (frozen high-water, frozen liveness) are idempotent re-reads, not
  timing races.

## Gate verdict

**G3 PASS** for the PR-4 scope: secure Linux guest telemetry on a real
VM through the production create → seed → boot → install → enroll →
ingest path, outage durability and revocation proven on the same VM,
the unmodified host-only monitoring path for guests without the
package carried by the G2 record, and the credential trust limitations
recorded precisely above.
