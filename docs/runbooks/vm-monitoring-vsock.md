# VM Monitoring vsock Transport Runbook

**Scenario:** operate, diagnose and recover the optional vsock monitoring
transport (contract: `docs/specs/contracts/chv-monitoring-vsock-v1.md`).
**Severity when broken:** SEV-3 — monitoring telemetry loss only. A vsock
transport problem never affects VM lifecycle, the guest, or the node.
**Time to execute:** 5–15 min.

## Background

A VM opted in with `vsock_enabled` carries a virtio-vsock device. The
guest's `chv-monitor-agent` (with `[transport] type = "vsock"`) pipes its
TLS bytes through the guest kernel to the node's `chv-agent`, which splices
them to the control plane's mTLS listener. The transport is plumbing only:
the one-time claim and the client certificate are the only credentials
(ADR-026).

Data path (vsock mode):

```
guest agent → 127.0.0.1 loopback forwarder (in-guest)
            → AF_VSOCK CID 2, port 1024
            → VMM muxer → <vm_dir>/vsock.sock_1024 (node relay, chv-agent)
            → control-plane mTLS listener (from the node's control_plane_addr)
```

## Symptoms and diagnosis

### Guest agent does not enroll or report (vsock mode)

1. **Check the opt-in actually happened.** On the node:
   ```sh
   # VM runtime dir (typically /var/lib/chv-agent/vms/<VM_ID>)
   sudo cat <vm_dir>/vm-config.json | grep -A3 '"vsock"'
   ```
   - No `vsock` key → the VM was not created with `vsock_enabled`. It is
     create-time only; fixing it means delete + re-create, or run the guest
     on tcp transport.
2. **Check the relay endpoint exists** (chv-agent side):
   ```sh
   sudo ls -l <vm_dir>/vsock.sock_1024
   sudo journalctl -u chv-agent | grep -i vsock
   ```
   - Missing while the VM runs → restart `chv-agent` (adoption re-binds the
     endpoint for every live vsock VM; the guest is not disturbed):
     `sudo systemctl restart chv-agent`.
   - "vsock monitoring relay disabled for this agent process" warning →
     the node's `control_plane_addr` is wrong; fix it and restart
     `chv-agent`.
   - "vsock relay: cannot bind the guest endpoint" warning → a stale
     socket file or a permissions problem on `<vm_dir>`; the warning
     names the socket path. `systemctl restart chv-agent` re-runs the
     adoption sweep and re-binds the endpoint.
3. **Check the guest side.** In the guest:
   ```sh
   systemctl status chv-monitor-agent
   journalctl -u chv-monitor-agent -n 50
   ss -tn | grep 127.0.0.1   # the agent dials the loopback forwarder
   ```
   - `vsock transport failed to start` at boot → the kernel has no AF_VSOCK
     (very old kernel) or the unit is not the packaged one (missing
     `AF_VSOCK` in `RestrictAddressFamilies`). Install the packaged unit.
   - Connect-then-EOF retries → the relay endpoint is missing or the
     control plane is unreachable from the node; see step 2.
4. **Check `server_url` is a DNS name, not an IP literal.** IP-literal URLs
   bypass the resolver override (it only applies to hostnames), so vsock mode
   refuses to start: the agent exits at boot with "vsock transport failed to
   start … requires a DNS name". Register a name or stay on tcp.
5. **Check `server_url` carries an explicit unprivileged port.** The
   forwarder binds `127.0.0.1` at the URL's port; a portless https URL means
   443, which the non-root `chv-monitor` service cannot bind — the agent
   exits at startup with "vsock transport failed to start". Give the
   manager's URL an explicit port, or lower
   `net.ipv4.ip_unprivileged_port_start`, or stay on tcp.

### Transport switch (tcp ↔ vsock)

Switching is a guest-local action; identity, credentials and replay state
survive it (no re-enrollment):

```sh
# in the guest
sudo sed -i 's/^type = "vsock"$/type = "tcp"/' /etc/chv-monitor/agent.toml
sudo systemctl restart chv-monitor-agent
```

Reverse the substitution to switch back. The spool replays anything missed
while disconnected.

### Node restart

Nothing to do: `chv-agent` adopts running VMs and re-binds every relay
endpoint. Guest connections re-establish on the agent's next tick (up to
one interval + backoff of telemetry gap).

### Batch enrollment of many vsock VMs on one node

Piped enrollments share the node's source address, so N VMs enrolling in
the same minute share the per-IP claim rate limit (10/min by default).
Stagger creates by ~6 s per VM, or enroll over tcp first and switch.

## What is deliberately NOT supported (v1)

- **Hot-attach/hot-detach of the device** — create-time only.
- **Live migration** — platform live migration is unqualified; the vsock
  migration leg is BLOCKED (see the G5 evidence record).
- **Multi-node transport** — the relay dials the node's configured control
  plane; there is no cross-node routing.

## Verification

After any recovery step, confirm end-to-end telemetry from the manager:
`vm.guest.*` samples for the VM resume within ~2 intervals (default
15 s), and the agent shows `active` (not `offline`) in the manager's agent
list.
