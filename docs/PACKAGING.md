# CHV Packaging

## Overview

CHV is distributed as three packages to allow flexible deployment:

| Package | Purpose | Binaries |
|---------|---------|----------|
| `chvctl` | CLI tool for operators | `chvctl` |
| `chv-controlplane` | Management plane (API, Web UI, scheduler) | `chv-controlplane` |
| `chv-node` | Hypervisor node (agent, storage, networking) | `chv-agent`, `chv-stord`, `chv-nwd` |

This split lets you run the control plane on dedicated management hosts while
scaling hypervisor nodes independently.

## File Layout

### `chvctl`

```
/usr/bin/chvctl
/usr/share/doc/chvctl/
```

### `chv-controlplane`

```
/usr/bin/chv-controlplane
/usr/share/chv/ui/     # Web UI static assets
/usr/share/chv/migrations/   # Database migrations
/etc/chv/             # Config directory (created, not overwritten)
/var/lib/chv/         # Data directory (created)
/var/log/chv/         # Log directory (created)
/lib/systemd/system/chv-controlplane.service
```

### `chv-node`

```
/usr/bin/chv-agent
/usr/bin/chv-stord
/usr/bin/chv-nwd
/etc/chv/             # Config directory (created, not overwritten)
/var/lib/chv/         # Data directory (created)
/var/log/chv/         # Log directory (created)
/lib/systemd/system/chv-agent.service
/lib/systemd/system/chv-stord.service
/lib/systemd/system/chv-nwd.service
```

All packages create the `chv` system user (`--system --no-create-home --shell /usr/sbin/nologin`) on first install.

## Installation

### Debian / Ubuntu (.deb)

```bash
# Control plane + node on the same host
sudo dpkg -i chv-controlplane_0.1.0_amd64.deb chv-node_0.1.0_amd64.deb

# CLI on any management machine
sudo dpkg -i chvctl_0.1.0_amd64.deb
```

If dependency warnings appear, run:
```bash
sudo apt-get install -f
```

### RHEL / CentOS / Fedora / openSUSE (.rpm)

```bash
# Control plane + node on the same host
sudo rpm -i chv-controlplane-0.1.0-1.x86_64.rpm chv-node-0.1.0-1.x86_64.rpm

# CLI on any management machine
sudo rpm -i chvctl-0.1.0-1.x86_64.rpm
```

## Post-Install Steps

1. **Create the bridge (if needed)**  
   The network daemon expects a bridge (default `chvbr0`). The package does not
   create it automatically because the upstream interface varies per host.  
   Example:
   ```bash
   sudo ip link add name chvbr0 type bridge
   sudo ip link set chvbr0 up
   sudo ip addr add 10.200.0.1/24 dev chvbr0
   ```

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
sudo dpkg -i chv-controlplane_0.1.X_amd64.deb chv-node_0.1.X_amd64.deb
sudo apt-get install -f
sudo systemctl restart chv-controlplane chv-agent chv-stord chv-nwd
```

### .rpm
```bash
sudo rpm -U chv-controlplane-0.1.X-1.x86_64.rpm chv-node-0.1.X-1.x86_64.rpm
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
> ```

## Known Gaps

- **No apt / dnf repository yet.** Packages must be downloaded and installed manually.
- **No package signing yet.** Verify checksums out-of-band until repositories and signing are in place.
- **No automated bridge creation.** Admins must create the bridge and configure NAT/routing as needed.

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
