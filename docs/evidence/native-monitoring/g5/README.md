# G5 — optional vsock transport and lifecycle qualification on a real VM

**Campaign:** native monitoring implementation (#602)
**Gate:** G5 (prompt-06, tasks 1–8 — PR-7)
**Contract:** `docs/specs/contracts/chv-monitoring-vsock-v1.md`
**Date:** 2026-10-11 (UTC)
**Environment:** identical to the G0b/G1/G2/G3/G4 captures — AMD EPYC
9554P, kernel `6.8.0-142-generic`, real KVM, `cloud-hypervisor v53.0`
static binary digest-verified against `scripts/install.sh`'s pin,
rust-hypervisor-fw, guest image `noble-qual-patched.img` (qcow2). The
guest package under test is `chv-monitor-agent_0.3.0_amd64.deb`
(`66c73932cf2a55000e724d7953c13c0b5a1f347c05f74c936384d7f88210812e`,
branch tip `f13fc634`)
built from the final branch state with every gate-catch fix and every
review fix (rounds 1–2) compiled in, byte-compared against
`target/release/chv-monitor-agent` before the run. The manager runs in-process in the test binary — the real
`chv-controlplane-service` HTTPS/TLS/ingest stack and the real
`MonitoringStore`; the VM is driven through the production
`ProcessCloudHypervisorAdapter` (real `vm.create`/`vm.boot`/`vm.reboot`/
`vm.pause`/`vm.resume`/shutdown/delete over the CH API), with the real
node-side relay (`VsockRelayManager`) wired exactly as `chv-agent`
wires it.

## Method

The env-gated integration test `g5_real_vm_vsock_transport_and_lifecycle_matrix`
(`cmd/chv-monitor-agent/tests/g5_real_vm.rs`, gated on
`CHV_G5_AGENT_DEB` + the G1 environment pins) builds the same bridge +
tap + NoCloud-seed production path as the G3/G4 rigs, then adds the
vsock legs. Discriminators that make the pipe PROVABLE rather than
assumed:

- **The manager's connection PEER.** In vsock mode the guest's TLS
  bytes arrive at the manager from the relay's local dial
  (source `192.168.64.1`); in tcp mode they arrive from the guest
  itself (`192.168.64.50`). The rig asserts the peer is relay-side
  while on vsock and guest-side while on tcp.
- **The relay's own admit log** (`vsock relay connection admitted`,
  debug) plus the persisted `vm-config.json` payload, the node's
  vsock CID cursor file, and the device/relay socket paths on disk.
- **Identity and replay continuity** (`agent_id`, ingest sequence
  advancing past the pre-switch value) across every transport switch
  and lifecycle event.
- **New-boot proof, not count proof.** The manager persists batches
  with a delay, so a bare "point count rose" check can false-pass on
  a pre-event envelope still in flight at the snapshot (run 9 caught
  exactly this on the stop/start leg). The reboot and stop/start
  waits therefore require the agent's reported **boot id to change**;
  the switch and adoption waits require the **ingest sequence to pass
  snapshot+1** (the agent sends serially, so at most one stale
  envelope can be in flight).

The guest's `server_url` uses a DNS name (`g5-manager.chv.internal`)
with a DNS-only SAN in the manager certificate, because IP-literal
URLs bypass the resolver override that redirects the dial onto the
loopback forwarder (hyper behavior, verified on this rig in an earlier
iteration: the client dialed the URL port on 127.0.0.1 and nothing
else moved). The agent now refuses to start in vsock mode with an
IP-literal `server_url` (IPv4 and bracketed IPv6), so this
requirement is enforced at boot rather than left to documentation.

## Results (final run, EXIT=0, 378 s)

```
matrix: boot=PASS pipe-enroll=PASS pipe-telemetry=PASS switch-tcp=PASS
switch-vsock=PASS reboot=PASS pause-resume=PASS stop-start=PASS
agent-restart=PASS node-restart-adopt=PASS delete=PASS
cid-uniqueness=PASS migration=BLOCKED detach=N/A(v1 create-time only)
multi-node=N/A(separate gate)
```

Legs, as observed in the run log:

1. **Create with the opt-in** — `vm.create` carries
   `vsock: {cid: 1000, socket: <vm_dir>/vsock.sock}` in the persisted
   payload; the node relay binds `<vm_dir>/vsock.sock_1024` before the
   guest can possibly connect; the CID cursor commits 1000. A control
   VM created without the opt-in carries no vsock key at all.
2. **Boot + enrollment over the pipe** — the guest installs the deb
   via cloud-init, starts in vsock mode, and enrolls through:
   guest → loopback forwarder (127.0.0.1:<server_url port>) →
   AF_VSOCK CID 2 port 1024 → VMM muxer → relay → manager mTLS
   listener. Manager peer = relay-side. The one-time claim is
   consumed exactly once.
3. **Telemetry over the pipe** — `vm.guest.*` samples land with the
   sequence advancing.
4. **Transport switch tcp → vsock** — in-guest config flip + service
   restart; same `agent_id`, same credential, peer becomes the guest;
   sequence keeps advancing.
5. **Transport switch back to vsock** — same identity, replay
   continuity (sequence 2 → 6), peer relay-side again.
6. **Reboot** (`vm.reboot`, same VMM) — the device socket is
   defensively unlinked before the API call and re-bound by the new
   device; the relay endpoint is never disturbed; the guest reboots,
   the agent restarts with the SAME credential (no re-enroll) and
   telemetry resumes through the pipe (~130 s after the reset — the
   second boot is slower than the first; see the hard-reset note
   below).
7. **Pause/resume** — 20 s pause stalls the pipe; traffic resumes
   after `vm.resume` with no reconnection logic needed beyond the
   client's normal retry.
8. **Stop/start** — graceful stop removes both the relay endpoint and
   the device socket; `start_vm` respawns the VMM, re-derives the
   relay from the persisted payload, re-creates the device socket,
   and telemetry resumes with the same identity. The wait gates on
   the guest's boot id changing, so the pass is the NEW boot's
   ingest, not a stale count.
9. **Node restart (adoption)** — a FRESH adapter + relay manager
   adopts the running VM: the endpoint is unlinked and re-bound by the
   new manager, and after an in-guest agent restart the new traffic
   flows through the ADOPTED relay (manager peer relay-side).
10. **Delete** — all artifacts gone: device socket, relay endpoint,
    vm dir, and the store's agent row is left to its retention policy.
11. **CID uniqueness** — a second VM (created, never booted) draws
    CID 1001 with the cursor committed; no reuse.
12. **Migration** — **BLOCKED**, not mocked: the platform's live
    migration is itself unqualified (recorded on #602), so the vsock
    migration leg is reported BLOCKED per the campaign's honesty
    rule.

## Gate catches (what the rig found and fixed before PASS)

Two product defects and one rig-assertion defect were caught by
earlier iterations of this rig and are part of the evidence that the
gate has teeth:

1. **The forwarder must own the `server_url` port.** reqwest's
   `.resolve()` override redirects the host to 127.0.0.1 but the dial
   keeps the URL's port; a forwarder on an ephemeral port is never
   reached (first runs showed `Connection refused` against
   127.0.0.1:<manager port> in the guest journal). Fix: the forwarder
   binds `127.0.0.1:<server_url's port>`, which also makes an
   explicit unprivileged port a documented vsock-mode requirement.
2. **Identity files must be crash-durable.** `vm.reboot` is a HARD
   reset (no ACPI shutdown, no guest page-cache flush). The rig's
   config rewrite seconds before a reset came back zero-length and
   the agent crash-looped on an empty TOML — the same hazard applied
   to the agent's own credential (written in place with truncate:
   a torn or empty credential is unrecoverable, the one-time claim
   is consumed). Fix: `fsutil::write_durable` (tmp → fsync → rename →
   dir fsync) now backs the credential, agent state and spool
   entries, and the spool drain drops unparsable entries instead of
   wedging. Recorded as the identity-file durability decision in the
   agent security contract.
3. **A count-based resume wait can false-pass.** Run 9: the
   stop/start wait passed ~2 s after `start_vm` — with zero relay
   admissions, while the new guest was still booting — because the
   manager persisted a pre-stop envelope still in flight at the
   snapshot, and the follow-on sshd wait then absorbed the whole
   (~2 min) boot and blew its budget. Not a product defect, but a
   proof-strength defect: the leg claimed "guest back" without a
   guest. Fix: the resume waits gate on the reported boot id changing
   (reboot, stop/start) or on the ingest sequence passing
   snapshot+1 (switches, adoption); an explicit sshd wait precedes
   the in-guest restart.

## Known caveats carried on #602

- **Per-IP claim rate limit is shared per node**: piped enrollments
  arrive from the relay's source address, so N vsock VMs on one node
  enrolling within the same minute share that node's claim budget
  (10/min default). Stagger or enroll over tcp.
- **Live migration is BLOCKED** (platform migration unqualified).
- **Multi-node monitoring** is a separate campaign gate (documented,
  not implemented).
- The second boot after a hard reset is slower (~2 min) than the
  first on this image; within all rig budgets, recorded as an
  observation only.
