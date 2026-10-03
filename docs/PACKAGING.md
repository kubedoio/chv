# CHV Packaging

## Overview

CHV is distributed as three packages to allow flexible deployment:

| Package | Purpose | Binaries |
|---------|---------|----------|
| `chvctl` | CLI tool for operators | `chvctl` |
| `chv-controlplane` | Control plane (API, Web UI, scheduler) | `chv-controlplane` |
| `chv-node` | Node services (agent, storage, networking) | `chv-agent`, `chv-stord`, `chv-nwd` |

This split lets you run the control plane on dedicated management hosts while
scaling nodes independently.

`chvctl` is a separate package; it is not part of `chv-node`. The `chv-node`
package depends on `chv-controlplane`. On `.deb` it also depends on
`wireguard-tools`; on `.rpm` that dependency is a recommendation.

## File Layout

### `chvctl`

```
/usr/bin/chvctl
/usr/share/doc/chvctl/
```

### `chv-controlplane`

```
/usr/bin/chv-controlplane
/usr/share/chv/ui/           # Web UI static assets
/usr/share/chv/migrations/   # Database migrations
/etc/chv/controlplane.toml   # Default config (noreplace)
/lib/systemd/system/chv-controlplane.service
```

### `chv-node`

```
/usr/bin/chv-agent
/usr/bin/chv-stord
/usr/bin/chv-nwd
/etc/chv/agent.toml          # Default configs (noreplace)
/etc/chv/stord.toml
/etc/chv/nwd.toml
/etc/chv/chv.yaml            # Reference-only unified config; do not edit
/usr/lib/tmpfiles.d/chv-node.conf
/lib/systemd/system/chv-agent.service
/lib/systemd/system/chv-stord.service
/lib/systemd/system/chv-nwd.service
```

The postinstall script also creates `/var/lib/chv`, `/var/log/chv`, and
`/run/chv` with the ownership described below.

## Post-install behavior

Every package runs `packaging/scripts/postinstall.sh`. The script:

- Creates the `chv` system user and group with home `/var/lib/chv` and shell `/usr/sbin/nologin`.
- Creates the reserved `chv-stord` system user and group. It is a seam for a future storage-isolation model; `chv-stord` currently runs as the `chv` user.
- Adds `chv` to the `kvm` group, and `chv-stord` to the `disk` and `chv` groups.
- Creates the storage directories `/var/lib/chv/storage/localdisk` and `/var/lib/chv/storage/lvm` as `chv:chv-stord` mode `0770`.
- Mints `/etc/chv/encryption.env` with a random `CHV_ENCRYPTION_KEY` when `chv-controlplane` is installed. The key encrypts S3 credentials at rest. The file is never regenerated; losing it makes stored credentials unrecoverable.
- Reloads systemd.

`chv-node` also ships a tmpfiles entry (`/usr/lib/tmpfiles.d/chv-node.conf`).
It recreates `/run/chv/agent`, `/run/chv/core`, and `/run/netns` at boot.
`/run/netns` is `root:chv` mode `0770` and is required by the hardened
`chv-nwd` unit.

The `chv-nwd` unit runs as `chv` with `CAP_NET_ADMIN`, `CAP_NET_RAW`, and
`CAP_SYS_ADMIN` in its ambient and bounding capability sets. `CAP_SYS_ADMIN`
is required for `ip netns` mount operations.

## Installation

### Debian / Ubuntu (.deb)

```bash
# Control plane + node on the same host
sudo dpkg -i chv-controlplane_<version>_amd64.deb chv-node_<version>_amd64.deb

# CLI on any management machine
sudo dpkg -i chvctl_<version>_amd64.deb
```

If dependency warnings appear, run:
```bash
sudo apt-get install -f
```

### RHEL / Rocky / AlmaLinux (.rpm)

```bash
# Control plane + node on the same host
sudo rpm -i chv-controlplane-<version>-1.x86_64.rpm chv-node-<version>-1.x86_64.rpm

# CLI on any management machine
sudo rpm -i chvctl-<version>-1.x86_64.rpm
```

## Post-Install Steps

1. **Create a network**  
   Create networks through the CHV API or Web UI. `chv-nwd` creates each
   network's bridge (`br-<net_id>`) when the control plane applies the
   topology. The packages do not require a pre-existing bridge. The legacy
   `chvbr0` dev bridge created by the all-in-one installer is optional.

2. **Generate TLS certificates**  
   The control plane and agent use mTLS. Generate or place certificates in
   `/etc/chv/certs/` before starting services.  
   See `docs/DEPLOYMENT.md` for a full certificate guide.

3. **Edit configuration**  
   Review and adjust:
   - `/etc/chv/controlplane.toml`
   - `/etc/chv/agent.toml`
   - `/etc/chv/stord.toml`
   - `/etc/chv/nwd.toml`

   `/etc/chv/chv.yaml` is a reference only. The binaries read the per-daemon
   TOML files listed above.

4. **Start services**
   ```bash
   sudo systemctl enable --now chv-controlplane
   sudo systemctl enable --now chv-stord
   sudo systemctl enable --now chv-nwd
   sudo systemctl enable --now chv-agent
   ```

## Upgrade

### .deb
```bash
sudo dpkg -i chv-controlplane_<version>_amd64.deb chv-node_<version>_amd64.deb
sudo apt-get install -f
sudo systemctl restart chv-controlplane chv-agent chv-stord chv-nwd
```

### .rpm
```bash
sudo rpm -U chv-controlplane-<version>-1.x86_64.rpm chv-node-<version>-1.x86_64.rpm
sudo systemctl restart chv-controlplane chv-agent chv-stord chv-nwd
```

Database migrations are applied automatically by `chv-controlplane` on startup.

## Uninstall

### .deb
```bash
sudo apt remove chvctl chv-controlplane chv-node
```

### .rpm
```bash
sudo rpm -e chvctl chv-controlplane chv-node
```

> **Note:** The packages do **not** delete `/var/lib/chv/` by default.  
> If you want a complete wipe including VM images and volumes:
> ```bash
> sudo rm -rf /var/lib/chv /var/log/chv /etc/chv
> sudo userdel chv 2>/dev/null || true
> sudo userdel chv-stord 2>/dev/null || true
> ```

## Known Gaps

- **No apt / dnf repository yet.** Packages must be downloaded and installed manually.
- **Checksum signing depends on configured secrets.** CI signs `SHA256SUMS` (GPG or cosign) only when signing keys are configured. Verify checksums out-of-band until then.

## Building Packages Locally

Run the helper script:

```bash
./scripts/build-packages.sh
```

This requires:
- `nfpm` (https://nfpm.goreleaser.com/)
- Release binaries in `target/release/`
- UI build in `ui/build/`

Output packages are written to `dist/packages/`.
