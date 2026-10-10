# Install the guest monitoring agent (optional)

`chv-monitor-agent` is the OPTIONAL in-guest monitoring agent
([ADR-026](../specs/adr/026-optional-monitor-agent-and-guest-identity.md)).
It installs **inside a Linux VM**, never on a CHV host, and it is not
part of the `chv-node` package — installing it is an explicit per-VM
operator decision.

**A VM without this agent is fully usable.** The agent only adds
read-only guest telemetry (CPU, load, memory, uptime, OS identity) to
the VM's monitoring tab. It cannot mutate VM state, run commands, or
open inbound ports; it makes outbound HTTPS calls to the manager and
nothing else.

## Supported guests

| Requirement | Detail |
|---|---|
| OS | Linux with `/proc`, `/etc/os-release` |
| Init | systemd (the package ships a hardened unit) |
| Architecture | `amd64` — same as every published CHV artifact |

There is no Windows, BSD, or non-systemd build, and no snap/flatpak:
only the `.deb`/`.rpm` below. Gaps are documented, not papered over
with nonworking packages.

## 1. Enable guest ingestion on the manager

Guest ingestion is off by default and needs HTTPS. On the manager
host, set in `/etc/chv/controlplane.toml`:

```toml
[http_tls]
server_cert_path = "/etc/chv/certs/http-server.crt"
server_key_path = "/etc/chv/certs/http-server.key"

[monitoring.guest_ingestion]
enabled = true
public_base_url = "https://manager.example.internal:8443"
agent_ca_cert_path = "/etc/chv/certs/monitor-agent-ca.crt"
agent_ca_key_path = "/etc/chv/certs/monitor-agent-ca.key"
```

The agent CA must be **separate** from the node-enrollment CA. The
manager refuses to boot with guest ingestion enabled but TLS or the
agent CA missing. See
[`docs/examples/controlplane.toml`](../examples/controlplane.toml)
for every field and its default.

## 2. Install the package in the guest

Grab the `chv-monitor-agent` `.deb` (Debian/Ubuntu) or `.rpm`
(RHEL-family) from the same release as your control plane and install
it inside the VM:

```bash
# Debian / Ubuntu guest
sudo dpkg -i chv-monitor-agent_<version>_amd64.deb

# RHEL / Rocky / Alma guest
sudo rpm -i chv-monitor-agent-<version>.x86_64.rpm
```

The package creates a dedicated `chv-monitor` system user, a `0700`
state directory at `/var/lib/chv-monitor`, and a hardened, **disabled**
systemd unit. Nothing runs yet.

## 3. Configure and enroll

1. Point the agent at your manager:

   ```bash
   sudo editor /etc/chv-monitor/agent.toml
   # server_url = "https://manager.example.internal:8443"   <- set
   # manager_ca_path = "/etc/chv-monitor/manager-ca.pem"     <- set
   ```

   `server_url` must be `https://`. Copy the manager's TLS server
   certificate (or its CA) to `/etc/chv-monitor/manager-ca.pem` —
   the agent never speaks monitoring over plain HTTP and never trusts
   a manager it cannot authenticate.

2. Issue a one-time claim for this VM in the WebUI (VM detail →
   metrics tab → **Guest monitoring agent** → *Enroll agent*). The
   claim token is shown once.

3. Place the claim in the guest as root:

   ```bash
   sudo install -o chv-monitor -g chv-monitor -m 0600 <claim-file> \
       /var/lib/chv-monitor/claim
   ```

4. Start the service:

   ```bash
   sudo systemctl enable --now chv-monitor-agent
   ```

The agent enrolls on its next tick (15 s by default), the claim file
is deleted, and the VM's metrics tab starts showing guest telemetry
alongside the host-side series.

## Security notes — read before automating

- **Claim distribution.** A claim is single-use and short-lived, but
  until it is redeemed anyone holding it can enroll an agent for that
  VM. Deliver it over a trusted channel (your config management, not
  a shared wiki). Treat it like a password reset link.
- **Never bake claims into VM images.** A golden image containing a
  claim or a credential produces clones that share (or fight over)
  one agent identity. The manager flags a second install reusing a
  credential as an **identity conflict** (a cloned-image signal) and
  blocks reporting until an operator resets it. Give every VM its own
  claim at first boot.
- **Credential trust, precisely.** An enrolled agent proves only that
  it redeemed an operator-issued claim on that install. It does NOT
  prove the VM image, kernel, or hardware — CHV makes no hardware
  attestation claim. A compromised guest can read its own telemetry
  (and lie about it); it cannot reach other VMs' telemetry or any VM
  lifecycle operation.
- **Removal ≠ revocation.** Uninstalling the package in the guest
  leaves the manager-side registration active until an operator
  revokes it in the WebUI. Revoke first when decommissioning a VM.

## Rotation, outages, and removal

- **Rotation** is automatic: the manager forces a rotation when the
  credential approaches expiry; the agent renews over the same TLS
  channel. No operator action.
- **Manager outages** are expected: the agent spools batches to disk
  (bounded, age-pruned) and replays them oldest-first when the
  manager returns. The ingestion contract keeps history *live*: the
  manager accepts samples up to 5 minutes old, so an outage longer
  than that recovers roughly the last 5 minutes of data — older
  spooled batches are discarded as stale, never silently re-stamped.
- **Removal**: `sudo apt remove chv-monitor-agent` (or `dnf remove`)
  stops the service and preserves `/var/lib/chv-monitor` (credential,
  spool, counters). Deleting that directory re-randomizes the install
  identity — the manager treats the next enrollment as a new install,
  not a continuation.

## Uninstall checklist

```bash
sudo systemctl disable --now chv-monitor-agent   # if still enabled
sudo apt purge chv-monitor-agent                 # removes config too
sudo rm -rf /var/lib/chv-monitor                 # explicit: state is kept otherwise
```

Then revoke the agent in the WebUI (VM detail → metrics tab → Guest
monitoring agent → *Revoke*) so the manager-side record and any
credential material are retired.
