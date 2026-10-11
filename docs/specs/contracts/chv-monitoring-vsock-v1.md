# CHV monitoring vsock transport contract v1

**Status:** Proposed
**Authority:** [ADR-026](../adr/026-optional-monitor-agent-and-guest-identity.md) (identity invariants)
**Scope:** the optional VM vsock device and the guest↔control-plane monitoring
transport that rides it (campaign #602, gate G5 / PR-7).

## What this contract is

The monitoring agent's traffic can optionally ride a virtio-vsock device
instead of the management network. The design rule is **one TLS/HTTP stack,
two possible wires**: the guest agent runs the exact same TLS, HTTP,
enrollment, ingestion and rotation code on both transports. The transport
selection happens strictly below the HTTP stack; nothing above it — URL
semantics, SNI, certificate validation, credentials, replay state —
changes with the transport.

## The opt-in

- A VM gets a vsock device **only** when the deployment opts the VM in at
  create time (`vsock_enabled` through the BFF/orchestrator path). No VM
  is silently attached a device, and existing VM definitions keep their
  exact meaning (the field defaults to absent).
- The opt-in is **create-time only** in v1: there is no hot-add, hot-remove
  or hot-detach of the device. Changing it requires a delete + re-create.
- The opt-in marker travels in the VM definition (`VmDefinition.vsock`,
  mirrored in the persisted `vm-config.json` payload as the concrete
  `vsock: { cid, socket_path }`). The node authority owns the addressing;
  callers above it only carry the boolean intent.

## CID allocation

- The host kernel registers nothing for vsock CIDs under Cloud Hypervisor
  v53: **CID uniqueness is a CHV convention**, enforced by a durable,
  node-scoped, monotonically increasing cursor
  (`<node_runtime_dir>/vsock-cid-cursor`, written under an exclusive
  `flock`, commit order write-then-`set_len`).
- First allocation on a fresh node is CID 1000 (above the reserved 0–2
  and well-known low CIDs); allocation fails loudly beyond 4 000 000 000
  rather than wrapping into reuse.
- A non-empty malformed cursor file fails loudly (never re-issued from
  scratch — that would risk CID reuse against live VMs).
- A VMM respawn (stop/start) keeps the CID from the persisted payload;
  only a delete frees the VM's identity, and CIDs are never reused.

## The device and its sockets

- Cloud Hypervisor binds and listens on the configured `socket` path
  (the host→guest entry direction — unused by this transport, which is
  guest-initiated) and, for each guest→host connection, connects out to
  `"<socket>_<destination_port>"`. The port therefore selects the
  host-side listener.
- **The monitoring port is a fixed contract constant: 1024**
  (`VSOCK_MONITOR_PORT`). It is mirrored in the guest package
  (`chv-monitor-agent`, which cannot depend on the host's hypervisor API
  crate); the G5 rig asserts both sides agree at runtime, and this
  document pins the value.
- Device socket layout per VM: `<vm_dir>/vsock.sock` (the VMM's listener)
  and `<vm_dir>/vsock.sock_1024` (the node relay's listener). Guest
  packets to CIDs other than the host's are dropped by the VMM muxer;
  the muxer caps at 1023 concurrent connections per port.
- The VMM unlinks its socket only on a clean process exit; pause
  preserves connections; snapshot/restore resets them. The node side
  never assumes unlink-on-teardown: create, respawn and reboot all
  clear a stale device-socket path defensively before it is re-bound.

## The node relay (chv-agent)

- The relay is a **transparent TLS pipe**: it accepts the VMM's connect
  on `<vm_dir>/vsock.sock_1024` and splices raw bytes to the control
  plane's existing mTLS TLS listener (dialed from the node's
  `control_plane_addr`).
- The relay never parses, terminates, or authenticates guest payload.
  The end-to-end TLS session is guest↔control-plane. The vsock CID and
  every socket path are transport plumbing, **never authentication**
  (ADR-026): the one-time claim and the client certificate remain the
  only credentials.
- Gate: the relay exists for a VM exactly when the VM's persisted
  `vm-config.json` carries `vsock.socket` (read from the payload, never
  re-derived). Both lifecycle authority paths (Core runtime and the
  legacy reconciler/direct-RPC path) go through the same adapter
  choke point, so the hook set is: create (start), start/respawn
  (re-derive), reboot (keep — same VMM), pause/resume (keep — the
  pipe stalls and resumes), stop and delete (stop and remove the
  endpoint), and agent restart (`adopt_running_vms` re-binds the
  endpoint for every live vsock VM).
- Bounds: at most **2 concurrent connections per VM** (over-bound
  connections are closed immediately) and a **15-minute idle timeout**
  per connection. A relay that cannot parse the control-plane address
  fails soft (no relay, loud warning, VMs unaffected); an opted-in VM
  whose relay is missing reports no data but is never broken by it.
- Stale relay socket paths are unlinked before bind.

## The guest transport (chv-monitor-agent)

- Opt-in via `[transport] type = "vsock"` in `agent.toml` (additive
  config; the default `"tcp"` behaves exactly like pre-G5 builds;
  unknown kinds and fields are rejected).
- The transport is a loopback forwarder: a TCP listener on `127.0.0.1`
  at the `server_url`'s port whose connections are spliced
  byte-for-byte into AF_VSOCK streams to the host (CID 2, port 1024).
  The HTTP client is redirected onto it with a resolver override for
  the `server_url` host, so only the dial moves — TLS SNI and
  certificate validation keep using the URL's host.
- Because the pipe **is** the network path, vsock mode never uses a
  proxy. A failed host dial closes the loopback connection immediately;
  the client retries with its normal backoff.
- `server_url` must use a **DNS name, not an IP literal**: IP-literal
  URLs bypass the resolver override in the HTTP client, and vsock mode
  would silently not take effect. (Deployments that must use an IP
  literal should register a name for it or stay on tcp.)
- `server_url` must also carry an **explicit unprivileged port** in
  vsock mode: the resolver override redirects the host to 127.0.0.1
  but the dial keeps the URL's port, so the forwarder must bind
  exactly that port — and a portless https URL means 443, which the
  non-root agent service cannot bind. The agent fails loudly at
  startup when the forwarder cannot bind (nothing else in the guest
  may hold that port either).
- The agent fails loudly at startup when vsock is configured but the
  forwarder cannot start (wrong-network silent fallback is worse than
  an honest failure). The loopback listener is unauthenticated by
  design: it is reachable only inside the guest, and the TLS at the
  control plane is the boundary — the same trust model as the relay.
- The packaged systemd unit grants `AF_VSOCK` (the vsock socket needs
  no device node; `PrivateDevices=` stays on).

## Identity, replay and switching

- Guest identity and replay state are transport-agnostic: the same
  credential, `agent_id`, `install_id` and ingest sequence survive
  switching the guest between tcp and vsock (and back), including
  across guest reboots and agent restarts. No re-enrollment happens on
  a transport switch.
- `vm.reboot` is a HARD reset from the guest's perspective (the VMM
  tears the machine down and re-creates it without an ACPI shutdown,
  so the guest kernel cannot flush its page cache). The agent's
  identity files are therefore written crash-durable (fsync before
  rename — see the security contract's identity-file durability
  decision); anything else the guest wrote shortly before a reset
  (e.g. an operator editing `agent.toml` seconds before a reboot) can
  come back empty. That is a property of hard resets, not of the
  transport.
- Enrollment, ingestion and rotation ride the pipe unchanged.

## Known caveats and limits (v1)

- **Per-IP claim rate limit is shared per node.** Piped enrollments
  arrive from the relay's source address, so N vsock VMs on one node
  enrolling within the same minute share that node's claim-rate budget
  (10/min by default). Large batch enrollments on one node should
  either be staggered or performed over tcp.
- **Live migration is BLOCKED**: the platform's live migration is
  itself unqualified, so the vsock migration leg is reported BLOCKED
  rather than mocked. Snapshot/restore of a vsock VM resets its
  connections (guest reconnects; the relay needs no reconfiguration).
- **Multi-node monitoring is out of scope** for this contract (a
  separate campaign gate); the transport is single-node.
- **Device detach does not exist** in v1 (create-time only, see above).

## Qualification

Real-host evidence for this contract lives in
`docs/evidence/native-monitoring/g5/README.md` (lifecycle matrix:
boot, enroll-over-pipe, transport switches, reboot, pause/resume,
stop/start, node restart via adoption, delete, CID uniqueness; the
migration leg is recorded BLOCKED). No performance, safety or
availability claim is made beyond that record.
