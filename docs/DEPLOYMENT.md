# CHV Deployment Guide — All-in-One Host

This guide explains how to deploy CHV on a single Linux host that acts as both the **control plane** (orchestration, API, Web UI) and the **hypervisor** (VM runtime via Cloud Hypervisor).

> **Version:** 0.0.0.2  
> **Target:** Ubuntu 22.04/24.04 LTS or equivalent Linux with KVM support  
> **Database:** SQLite (no external database service required)

---

## Table of Contents

1. [Quick Start (One-Liner Install)](#quick-start-one-liner-install)
2. [What Gets Installed](#what-gets-installed)
3. [Build & Package a Release](#build--package-a-release)
4. [Manual Deployment (Step-by-Step)](#manual-deployment-step-by-step)
5. [Hosting the Installer (`get.cellhv.com`)](#hosting-the-installer-getcellhvcom)
6. [Operations & Troubleshooting](#operations--troubleshooting)

---

## Quick Start (One-Liner Install)

Run the official installer on a fresh Ubuntu server with root access:

```bash
curl -sfL https://get.cellhv.com/ | sh -
```

Or install a specific version:

```bash
curl -sfL https://get.cellhv.com/ | INSTALL_CHV_VERSION=0.0.0.2 sh -
```

**Network defaults** (override with environment variables):

| Variable | Default | Description |
|----------|---------|-------------|
| `INSTALL_CHV_BRIDGE_NAME` | `chvbr0` | Linux bridge name |
| `INSTALL_CHV_BRIDGE_CIDR` | `10.200.0.1/24` | Gateway IP / subnet |
| `INSTALL_CHV_BRIDGE_IFACE` | `ens19` | Host interface attached to bridge (NAT upstream) |

Example with custom network:

```bash
curl -sfL https://get.cellhv.com/ | \
  INSTALL_CHV_BRIDGE_IFACE=eth0 \
  INSTALL_CHV_BRIDGE_CIDR=192.168.100.1/24 \
  sh -
```

The installer will:

1. Install system dependencies (`nginx`, `qemu-kvm`, `bridge-utils`, `iptables`, etc.)
2. Download and install CHV binaries, Web UI assets, and database migrations
3. Download and install the Cloud Hypervisor VMM
4. Generate a self-signed TLS CA
5. Create and configure the `chvbr0` bridge with NAT
6. Write `controlplane.toml`, `agent.toml`, `stord.toml`, and `nwd.toml`
7. Install systemd services for all four CHV daemons
8. Create a one-time bootstrap token (saved to `/etc/chv/bootstrap.token`)
9. Configure nginx to serve the Web UI and proxy API calls
10. Start all services and wait for the local agent to enroll as a compute node

After ~60 seconds, open the printed IP address in your browser.  
Default login: **admin / admin**

---

## What Gets Installed

### Processes on the Host

```
┌─────────────────────────────────────────────────────────────┐
│                      Combined Host                          │
│  ┌─────────────────┐  ┌─────────────────────────────────┐   │
│  │  chv-controlplane│  │        chv-agent               │   │
│  │  gRPC :8443      │◄─┤  enrolls to control plane      │   │
│  │  HTTP :8080      │  │  manages chv-stord / chv-nwd   │   │
│  │  SQLite DB       │  │  launches cloud-hypervisor VMs │   │
│  └─────────────────┘  └─────────────────────────────────┘   │
│           ▲                        │                        │
│           │                        ├─► chv-stord (daemon)   │
│           │                        └─► chv-nwd   (daemon)   │
│      nginx :80 (UI + API proxy)                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  chvbr0 (10.200.0.1/24) ──NAT──► ens19 ──► internet │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Key Files & Directories

| Path | Purpose |
|------|---------|
| `/usr/local/bin/chv-*` | CHV binaries |
| `/usr/bin/cloud-hypervisor` | Cloud Hypervisor VMM |
| `/etc/chv/controlplane.toml` | Control plane config |
| `/etc/chv/agent.toml` | Agent config |
| `/etc/chv/stord.toml` | Storage daemon config |
| `/etc/chv/nwd.toml` | Network daemon config (bridge, CIDR, upstream iface) |
| `/etc/chv/certs/` | TLS CA and certificates |
| `/var/lib/chv/controlplane.db` | SQLite database |
| `/var/lib/chv/cache/` | Agent durable cache |
| `/var/lib/chv/storage/localdisk/` | Local disk storage pool |
| `/opt/chv/ui/` | Web UI static files |
| `/usr/local/share/chv/migrations/` | Database migration files |

### Default Ports

| Port | Service | Bound To |
|------|---------|----------|
| 8443 | gRPC (control plane ↔ agent) | `127.0.0.1` |
| 8080 | HTTP admin API | `127.0.0.1` |
| 80 | Web UI (nginx) | `0.0.0.0` |
| 9901 | Agent metrics (optional) | `127.0.0.1` |

### Verification

```bash
# Check services
systemctl status chv-controlplane
systemctl status chv-agent
systemctl status chv-stord
systemctl status chv-nwd
systemctl status nginx

# Health endpoint
curl http://127.0.0.1:8080/health

# List nodes (should show the local host after enrollment)
curl -s http://127.0.0.1:8080/v1/nodes -X POST \
  -H "Content-Type: application/json" -d '{}' | jq .

# Logs
journalctl -u chv-controlplane -f
journalctl -u chv-agent -f
```

---

## Build & Package a Release

If you are developing CHV or want to host your own installer, use the build script in this repository.

### Prerequisites
- Rust toolchain (`rustup`)
- Node.js 22+ and `npm`
- Ubuntu/Debian build host

### Build the Release Tarball

```bash
# From the repository root
./scripts/build-release.sh
```

This will:
1. Build all Rust binaries in release mode
2. Build the SvelteKit Web UI
3. Assemble a release directory with binaries, UI, migrations, configs, and the installer
4. Create `dist/chv-<VERSION>-linux-amd64.tar.gz`
5. Generate a SHA256 checksum

### Install from a Local Build (Dev/Test)

```bash
# Build, uninstall previous version, and reinstall in one step
sudo ./scripts/dev-install.sh

# First-time install (skip uninstall step)
sudo ./scripts/dev-install.sh --no-uninstall

# Override network defaults
sudo INSTALL_CHV_BRIDGE_IFACE=eth0 ./scripts/dev-install.sh
```

Or manually:

```bash
./scripts/build-release.sh
sudo INSTALL_CHV_TARBALL_PATH=dist/chv-0.0.0.2-linux-amd64.tar.gz ./scripts/install.sh
```

---

## Manual Deployment (Step-by-Step)

If you prefer to deploy manually or need to customize every step, follow the detailed guide below.

### 1. Prerequisites

#### Hardware
- x86_64 server with **hardware virtualization** (VT-x / AMD-V)
- Minimum 4 cores, 8 GB RAM, 50 GB disk

#### Software
```bash
sudo apt update
sudo apt install -y \
  build-essential curl git pkg-config libssl-dev \
  nginx qemu-kvm bridge-utils iproute2 iptables

# Verify KVM
ls /dev/kvm
```

#### Cloud Hypervisor
```bash
CHV_VERSION="51.1"
curl -fsSL "https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v${CHV_VERSION}/cloud-hypervisor-static" \
  -o /usr/local/bin/cloud-hypervisor
chmod +x /usr/local/bin/cloud-hypervisor
ln -sf /usr/local/bin/cloud-hypervisor /usr/bin/cloud-hypervisor
cloud-hypervisor --version
```

### 2. Build from Source

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Node.js
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
sudo apt install -y nodejs

# Build binaries
cargo build --workspace --release

# Build UI
cd ui && npm install && npm run build
```

### 3. Host Preparation

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin chv
sudo usermod -aG kvm chv

sudo mkdir -p /etc/chv/certs
sudo mkdir -p /var/lib/chv/{cache,images,storage/localdisk}
sudo mkdir -p /var/log/chv
sudo mkdir -p /run/chv/{controlplane,agent,stord,nwd}
sudo mkdir -p /opt/chv/ui
sudo mkdir -p /usr/local/share/chv/migrations

sudo chown -R chv:chv /var/lib/chv /var/log/chv /run/chv
sudo chmod 750 /var/lib/chv /var/log/chv
```

### 4. Install Binaries & Assets

```bash
sudo install -m 755 target/release/chv-controlplane /usr/local/bin/
sudo install -m 755 target/release/chv-agent        /usr/local/bin/
sudo install -m 755 target/release/chv-stord        /usr/local/bin/
sudo install -m 755 target/release/chv-nwd          /usr/local/bin/

sudo cp -r ui/build/* /opt/chv/ui/
sudo chown -R www-data:www-data /opt/chv/ui

sudo cp -r cmd/chv-controlplane/migrations/* /usr/local/share/chv/migrations/
sudo chown -R chv:chv /usr/local/share/chv/migrations
```

### 5. Bridge and NAT Network Setup

```bash
BRIDGE_NAME="chvbr0"
BRIDGE_CIDR="10.200.0.1/24"
BRIDGE_NET="10.200.0.0/24"
UPSTREAM_IFACE="ens19"

# Create bridge
ip link add name $BRIDGE_NAME type bridge
ip link set $BRIDGE_NAME up
ip addr add $BRIDGE_CIDR dev $BRIDGE_NAME

# Attach upstream interface
ip link set $UPSTREAM_IFACE master $BRIDGE_NAME
ip link set $UPSTREAM_IFACE up

# Enable IP forwarding and NAT
sysctl -w net.ipv4.ip_forward=1
echo "net.ipv4.ip_forward=1" | sudo tee /etc/sysctl.d/99-chv-forward.conf

iptables -t nat -A POSTROUTING -s $BRIDGE_NET ! -d $BRIDGE_NET -j MASQUERADE

# Persist iptables
sudo iptables-save | sudo tee /etc/iptables/rules.v4
```

### 6. TLS Setup

```bash
sudo openssl genrsa -out /etc/chv/certs/ca.key 4096
sudo openssl req -x509 -new -nodes -key /etc/chv/certs/ca.key \
  -sha256 -days 3650 -out /etc/chv/certs/ca.crt \
  -subj "/O=CHV/CN=chv-ca"

sudo chmod 640 /etc/chv/certs/ca.key
sudo chmod 644 /etc/chv/certs/ca.crt
sudo chown root:chv /etc/chv/certs/ca.key /etc/chv/certs/ca.crt
```

### 7. Configuration

**`/etc/chv/controlplane.toml`**
```toml
grpc_bind = "127.0.0.1:8443"
http_bind = "127.0.0.1:8080"
log_level = "info"
runtime_dir = "/run/chv/controlplane"
jwt_secret = "<generate with: openssl rand -base64 32>"

[database]
url = "sqlite:///var/lib/chv/controlplane.db"
migrations_dir = "/usr/local/share/chv/migrations"
max_connections = 4
min_connections = 1
acquire_timeout_secs = 5

[tls]
ca_cert_path = "/etc/chv/certs/ca.crt"
ca_key_path = "/etc/chv/certs/ca.key"
```

**`/etc/chv/agent.toml`**
```toml
socket_path = "/run/chv/agent/api.sock"
runtime_dir = "/run/chv/agent"
log_level = "info"
control_plane_addr = "http://127.0.0.1:8443"
stord_socket = "/run/chv/stord/api.sock"
nwd_socket = "/run/chv/nwd/api.sock"
chv_binary_path = "/usr/bin/cloud-hypervisor"
stord_binary_path = "/usr/local/bin/chv-stord"
nwd_binary_path = "/usr/local/bin/chv-nwd"
cache_path = "/var/lib/chv/cache/agent-cache.json"
node_id = ""
metrics_bind = "127.0.0.1:9901"
bootstrap_token_path = "/etc/chv/bootstrap.token"
tls_cert_path = "/run/chv/agent/agent.crt"
tls_key_path = "/run/chv/agent/agent.key"
ca_cert_path = "/etc/chv/certs/ca.crt"
```

**`/etc/chv/nwd.toml`**
```toml
socket_path = "/run/chv/nwd/api.sock"
runtime_dir = "/run/chv/nwd"
log_level = "info"
bridge_name = "chvbr0"
bridge_cidr = "10.200.0.1/24"
upstream_iface = "ens19"
```

**`/etc/chv/stord.toml`**
```toml
socket_path = "/run/chv/stord/api.sock"
runtime_dir = "/var/lib/chv/storage/localdisk"
log_level = "info"
```

### 8. systemd Services

```bash
sudo cp docs/examples/systemd/chv-controlplane.service /etc/systemd/system/
sudo cp docs/examples/systemd/chv-agent.service        /etc/systemd/system/
sudo cp docs/examples/systemd/chv-stord.service        /etc/systemd/system/
sudo cp docs/examples/systemd/chv-nwd.service          /etc/systemd/system/
sudo systemctl daemon-reload
```

Service startup order:
- `chv-controlplane` — starts first (runs migrations, opens SQLite DB)
- `chv-stord` and `chv-nwd` — start independently
- `chv-agent` — starts after all three above are up; enrolls with the control plane

### 9. Bootstrap Token

```bash
BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
printf '%s' "$BOOTSTRAP_TOKEN" | sudo tee /etc/chv/bootstrap.token
sudo chmod 640 /etc/chv/bootstrap.token
sudo chown root:chv /etc/chv/bootstrap.token

# Insert token hash into SQLite database (after control plane has started)
TOKEN_HASH=$(printf '%s' "$BOOTSTRAP_TOKEN" | sha256sum | awk '{print $1}')
EXPIRES=$(date -u -d "+1 hour" '+%Y-%m-%dT%H:%M:%SZ')
sqlite3 /var/lib/chv/controlplane.db \
  "INSERT OR IGNORE INTO bootstrap_tokens
   (token_hash, description, one_time_use, expires_at, created_at)
   VALUES ('${TOKEN_HASH}', 'Manual deploy', 1,
           '${EXPIRES}', strftime('%Y-%m-%dT%H:%M:%SZ','now'));"
```

### 10. Web UI (nginx)

```bash
sudo cp docs/examples/nginx/chv-ui.conf /etc/nginx/sites-available/chv
sudo ln -sf /etc/nginx/sites-available/chv /etc/nginx/sites-enabled/chv
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl restart nginx
```

### 11. Start Services

```bash
sudo systemctl enable --now chv-controlplane
sudo systemctl enable --now chv-stord
sudo systemctl enable --now chv-nwd
sudo systemctl enable --now chv-agent
```

---

## Hosting the Installer (`get.cellhv.com`)

The `curl -sfL https://get.cellhv.com/ | sh -` pattern requires a lightweight endpoint that serves `scripts/install.sh` as plain text.

### Option A: GitHub Pages
1. Create a repo `cellhv/get.cellhv.com`
2. Add a `CNAME` file with `get.cellhv.com`
3. Serve `scripts/install.sh` as the index
4. Point DNS to GitHub Pages

### Option B: Cloudflare Worker
```javascript
export default {
  async fetch(request) {
    const installScript = await fetch('https://raw.githubusercontent.com/cellhv/chv/main/scripts/install.sh');
    return new Response(installScript.body, {
      headers: { 'Content-Type': 'text/plain' }
    });
  }
};
```

### Recommended Release Workflow
1. Developer runs `./scripts/build-release.sh`
2. CI uploads `dist/chv-<VERSION>-linux-amd64.tar.gz` to GitHub Releases
3. The installer script at `get.cellhv.com` queries the GitHub API for the latest release tag
4. End user runs `curl -sfL https://get.cellhv.com/ | sh -`

---

## Operations & Troubleshooting

### Restart Services
```bash
sudo systemctl restart chv-controlplane
sudo systemctl restart chv-agent
sudo systemctl restart chv-stord
sudo systemctl restart chv-nwd
sudo systemctl restart nginx
```

### View Logs
```bash
sudo journalctl -u chv-controlplane -f
sudo journalctl -u chv-agent -f
sudo journalctl -u chv-stord -f
sudo journalctl -u chv-nwd -f
```

### Post-deploy Web UI smoke test
```bash
./scripts/smoke-webui-auth.sh http://<host-or-ip>
```

### Agent Fails to Enroll
- Ensure `/etc/chv/bootstrap.token` exists and is readable by the `chv` user
- Verify the control plane is listening: `ss -tlnp | grep 8443`
- Check that the token was inserted into `bootstrap_tokens` and has not expired:
  ```bash
  sqlite3 /var/lib/chv/controlplane.db \
    "SELECT description, expires_at, used_at FROM bootstrap_tokens;"
  ```
- Review agent logs: `journalctl -u chv-agent -n 100`

### `chv-stord` or `chv-nwd` Keep Restarting
- Check daemon logs for missing binary paths or permission errors
- Verify `/usr/local/bin/chv-stord` and `/usr/local/bin/chv-nwd` are executable

### Database Issues
The database is SQLite at `/var/lib/chv/controlplane.db`. It is created automatically by `chv-controlplane` on first start via sqlx migrations.

```bash
# Inspect the database
sqlite3 /var/lib/chv/controlplane.db .tables
sqlite3 /var/lib/chv/controlplane.db "SELECT * FROM nodes;"

# Reset to clean state (destructive — removes all data)
sudo systemctl stop chv-controlplane chv-agent
sudo rm /var/lib/chv/controlplane.db
sudo systemctl start chv-controlplane
```

### Bridge / NAT Not Working
```bash
# Verify bridge is up
ip addr show chvbr0

# Verify IP forwarding
sysctl net.ipv4.ip_forward

# Verify NAT rule
iptables -t nat -L POSTROUTING -n -v

# Verify interface is attached to bridge
bridge link show
```

### Web UI Shows JSON Parse Error
This happens when API endpoints return HTML instead of JSON.

**Fix:** Ensure nginx `proxy_pass` does not have a trailing slash:
```nginx
location /v1/ {
    proxy_pass http://127.0.0.1:8080;   # NO trailing slash after port
}
```

### Web UI Blank Page
- Verify `npm run build` succeeded and `index.html` exists in `/opt/chv/ui/`
- Check nginx `root` directive: `sudo nginx -T | grep root`

### Permission Denied on `/dev/kvm`
- Ensure the `chv` user is in the `kvm` group: `groups chv`
- If just added: `sudo systemctl restart chv-agent`

---

## Next Steps

- **Multi-node expansion:** Deploy additional hypervisor-only hosts with `chv-agent` pointing to the control plane's reachable IP.
- **External storage:** Configure `chv-stord` backends for shared storage.
- **Networking:** Define tenant bridges and network segments via the Web UI or API.
- **TLS hardening:** Replace the self-signed CA with your organization's PKI.
