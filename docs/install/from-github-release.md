# Install CHV from GitHub Releases

This guide covers installing CHV by downloading artifacts directly from GitHub Releases. This method works on any Linux distribution that supports `.deb` or `.rpm` packages.

## Choose your release

Go to the [CHV Releases](https://github.com/chv-project/chv/releases) page and select a release:

| Release type | Tag example | Who should use it |
|--------------|-------------|-------------------|
| **Stable** | `v0.1.0` | Production and evaluation |
| **RC** | `v0.1.0-rc.1` | Pre-release validation |
| **Nightly** | `nightly` | Latest `main` branch, development only |

See [Channels](channels.md) for a detailed comparison.

## Download artifacts

Each release provides:
- `.deb` packages (Debian, Ubuntu)
- `.rpm` packages (RHEL, Rocky, AlmaLinux, Fedora)
- `SHA256SUMS` — checksums for verification
- `SHA256SUMS.sig` — GPG signature (when signing is configured)
- `sbom.spdx.json` — Software Bill of Materials
- `chv-<VERSION>-linux-amd64.tar.gz` — release tarball with binaries and install script

### Quick download

```bash
VERSION="0.1.0"
RELEASE_URL="https://github.com/chv-project/chv/releases/download/v${VERSION}"

# Download .deb packages
curl -sLO "${RELEASE_URL}/chv-controlplane_${VERSION}_amd64.deb"
curl -sLO "${RELEASE_URL}/chv-node_${VERSION}_amd64.deb"
curl -sLO "${RELEASE_URL}/chvctl_${VERSION}_amd64.deb"

# Download checksums
curl -sLO "${RELEASE_URL}/SHA256SUMS"
```

For `.rpm` packages, replace the filenames accordingly (see [RHEL/Rocky/Alma](rhel-rocky-alma.md)).

## Verify integrity

Always verify checksums before installing:

```bash
sha256sum -c SHA256SUMS
```

For additional verification (signatures, SBOM, attestations), see [Verify Release Artifacts](../release/verify-release-artifacts.md).

## Install

### Debian / Ubuntu

```bash
sudo dpkg -i chv-controlplane_${VERSION}_amd64.deb \
             chv-node_${VERSION}_amd64.deb \
             chvctl_${VERSION}_amd64.deb
sudo apt-get install -f
```

Full instructions: [Debian / Ubuntu](debian-ubuntu.md)

### RHEL / Rocky / AlmaLinux

```bash
sudo rpm -i chv-controlplane-${VERSION}-1.x86_64.rpm \
         chv-node-${VERSION}-1.x86_64.rpm \
         chvctl-${VERSION}-1.x86_64.rpm
```

Full instructions: [RHEL / Rocky / AlmaLinux](rhel-rocky-alma.md)

## Install from tarball (alternative)

If your distribution does not support `.deb` or `.rpm`, use the release tarball:

```bash
VERSION="0.1.0"
TARBALL="chv-${VERSION}-linux-amd64.tar.gz"
curl -sLO "https://github.com/chv-project/chv/releases/download/v${VERSION}/${TARBALL}"

# Extract
tar -xzf "${TARBALL}"
cd "chv-${VERSION}-linux-amd64"

# Run the install script
sudo ./install.sh
```

The tarball includes:
- Pre-built binaries (`bin/`)
- Web UI static assets (`ui/`)
- Database migrations (`migrations/`)
- Systemd unit files (`systemd/`)
- Example configs (`*.toml`)
- Nginx config (`nginx/`)

## Configure and start

After installation, edit configs and start services:

```bash
# Edit configs
sudo editor /etc/chv/controlplane.toml
sudo editor /etc/chv/agent.toml

# Start services
sudo systemctl daemon-reload
sudo systemctl enable --now chv-controlplane
sudo systemctl enable --now chv-agent chv-stord chv-nwd
```

> **Do not start services with default configs in production.** Set a strong `jwt_secret` and provision TLS certificates first.

## Upgrade

Download the new release and install over the existing packages. Config files and persistent data are preserved.

```bash
# Debian/Ubuntu
sudo dpkg -i chv-controlplane_${NEW_VERSION}_amd64.deb ...

# RHEL/Rocky/Alma
sudo rpm -U chv-controlplane-${NEW_VERSION}-1.x86_64.rpm ...

# Restart services
sudo systemctl restart chv-controlplane chv-agent chv-stord chv-nwd
```

## See also

- [Channels](channels.md) — choosing between stable, RC, nightly, and PR artifacts
- [Uninstall](uninstall.md) — safe removal and data cleanup
- [Verify Release Artifacts](../release/verify-release-artifacts.md) — signatures and attestations
