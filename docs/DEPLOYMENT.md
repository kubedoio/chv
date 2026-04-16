# CHV Deployment Guide — All-in-One Host

This guide explains how to deploy CHV on a single Linux host that acts as both the **control plane** (orchestration, API, Web UI) and the **hypervisor** (VM runtime via Cloud Hypervisor).

> **Version:** 0.0.0.2  
> **Target:** Ubuntu 22.04/24.04 LTS or equivalent Linux with KVM support

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

That's it. The installer will:

1. Install system dependencies (`postgresql`, `nginx`, `qemu-kvm`, etc.)
2. Download and install CHV binaries, Web UI assets, and database migrations
3. Download and install the Cloud Hypervisor VMM
4. Generate a self-signed TLS CA and server certificate
5. Create the PostgreSQL database and user
6. Write `controlplane.toml` and `agent.toml`
7. Install systemd services for `chv-controlplane` and `chv-agent`
8. Create a bootstrap token and insert it into the database
9. Configure nginx to serve the Web UI and proxy API calls
10. Start all services

After ~60 seconds, open the printed IP address in your browser.

---

## What Gets Installed

### Processes on the Host

```
┌─────────────────────────────────────────────────────────────┐
│                      Combined Host                          │
│  ┌─────────────────┐  ┌─────────────────────────────────┐   │
│  │  chv-controlplane│  │        chv-agent               │   │
│  │  gRPC :8443      │◄─┤  enrolls to control plane      │   │
│  │  HTTP :8080      │  │  supervises chv-stord/nwd      │   │
│  │  PostgreSQL      │  │  launches cloud-hypervisor     │   │
│  └─────────────────┘  └─────────────────────────────────┘   │
│           ▲                        │                        │
│           │                        ├─► chv-stord (child)    │
│           │                        ├─► chv-nwd   (child)    │
│           │                        └─► cloud-hypervisor VMs │
│      nginx (UI)                                             │
└─────────────────────────────────────────────────────────────┘
```

### Key Files & Directories

| Path | Purpose |
|------|---------|
| `/usr/local/bin/chv-*` | CHV binaries |
| `/usr/bin/cloud-hypervisor` | Cloud Hypervisor VMM |
| `/etc/chv/controlplane.toml` | Control plane config |
| `/etc/chv/agent.toml` | Agent config |
| `/etc/chv/certs/` | TLS CA and server certificates |
| `/var/lib/chv/cache/` | Agent durable cache |
| `/opt/chv/ui/` | Web UI static files |
| `/usr/local/share/chv/migrations/` | Database migrations |

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
systemctl status nginx

# Health endpoints
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
curl http://127.0.0.1:8080/admin/nodes

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
# Build + install in one step on the same machine
sudo ./scripts/dev-install.sh
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
  postgresql postgresql-client nginx \
  qemu-kvm libvirt-daemon-system bridge-utils

# Verify KVM
ls /dev/kvm
```

#### Cloud Hypervisor
```bash
CHV_VERSION="42.0"
curl -LO "https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v${CHV_VERSION}/cloud-hypervisor-v${CHV_VERSION}-x64"
sudo mv cloud-hypervisor-v${CHV_VERSION}-x64 /usr/local/bin/cloud-hypervisor
sudo chmod +x /usr/local/bin/cloud-hypervisor
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
sudo mkdir -p /var/lib/chv/{cache,images,storage}
sudo mkdir -p /var/log/chv
sudo mkdir -p /run/chv
sudo mkdir -p /opt/chv/ui
sudo mkdir -p /usr/local/share/chv/migrations

sudo chown -R chv:chv /var/lib/chv /var/log/chv /run/chv
sudo chmod 750 /var/lib/chv /var/log/chv
```

### 4. Install Binaries & Assets

```bash
sudo cp target/release/chv-controlplane /usr/local/bin/
sudo cp target/release/chv-agent /usr/local/bin/
sudo cp target/release/chv-stord /usr/local/bin/
sudo cp target/release/chv-nwd /usr/local/bin/
sudo ln -sf /usr/local/bin/cloud-hypervisor /usr/bin/cloud-hypervisor

sudo cp -r ui/build/* /opt/chv/ui/
sudo chown -R www-data:www-data /opt/chv/ui

sudo cp -r cmd/chv-controlplane/migrations/* /usr/local/share/chv/migrations/
sudo chown -R chv:chv /usr/local/share/chv/migrations
```

### 5. Database Setup

```bash
sudo -u postgres psql <<'EOF'
CREATE USER chv WITH PASSWORD 'change-me-strong-password';
CREATE DATABASE chv_controlplane OWNER chv;
GRANT ALL PRIVILEGES ON DATABASE chv_controlplane TO chv;
EOF
```

### 6. TLS / mTLS Setup

```bash
sudo openssl genrsa -out /etc/chv/certs/ca.key 4096
sudo openssl req -x509 -new -nodes -key /etc/chv/certs/ca.key \
  -sha256 -days 3650 -out /etc/chv/certs/ca.crt \
  -subj "/O=CHV/CN=chv-ca"

sudo openssl req -new -newkey rsa:4096 -nodes -keyout /etc/chv/certs/server.key \
  -out /etc/chv/certs/server.csr -subj "/O=CHV/CN=localhost"
sudo openssl x509 -req -in /etc/chv/certs/server.csr \
  -CA /etc/chv/certs/ca.crt -CAkey /etc/chv/certs/ca.key \
  -CAcreateserial -out /etc/chv/certs/server.crt -days 365 -sha256
sudo rm -f /etc/chv/certs/server.csr

sudo chmod 640 /etc/chv/certs/ca.key /etc/chv/certs/server.key
sudo chmod 644 /etc/chv/certs/ca.crt /etc/chv/certs/server.crt
sudo chown root:chv /etc/chv/certs/*.key /etc/chv/certs/*.crt
```

### 7. Configuration

**`/etc/chv/controlplane.toml`**
```toml
grpc_bind = "127.0.0.1:8443"
http_bind = "127.0.0.1:8080"
log_level = "info"
runtime_dir = "/run/chv/controlplane"

[database]
url = "postgres://chv:change-me-strong-password@127.0.0.1:5432/chv_controlplane"
migrations_dir = "/usr/local/share/chv/migrations"
max_connections = 16
min_connections = 1

[tls]
ca_cert_path = "/etc/chv/certs/ca.crt"
ca_key_path = "/etc/chv/certs/ca.key"
# gRPC server TLS is optional; disabled by default for all-in-one loopback deployments
# server_cert_path = "/etc/chv/certs/server.crt"
# server_key_path = "/etc/chv/certs/server.key"
# client_ca_path = "/etc/chv/certs/ca.crt"
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

### 8. systemd Services

See example units in `docs/examples/systemd/` or the installer script output.

Key rules:
- `chv-controlplane` starts **after** `postgresql`
- `chv-agent` starts **after** `chv-controlplane`
- `chv-agent` supervises `chv-stord` and `chv-nwd` as child processes (no separate systemd units needed)

### 9. Bootstrap Token

```bash
BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
echo "$BOOTSTRAP_TOKEN" | sudo tee /etc/chv/bootstrap.token
sudo chmod 640 /etc/chv/bootstrap.token
sudo chown root:chv /etc/chv/bootstrap.token

sudo -u postgres psql -d chv_controlplane -c "CREATE EXTENSION IF NOT EXISTS pgcrypto;"
sudo -u postgres psql -d chv_controlplane -c "
INSERT INTO bootstrap_tokens (token_hash, description, one_time_use, expires_at, created_at)
VALUES (encode(digest('$BOOTSTRAP_TOKEN', 'sha256'), 'hex'), 'Manual deploy', true, now() + interval '1 hour', now())
ON CONFLICT DO NOTHING;
"
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
sudo systemctl daemon-reload
sudo systemctl enable --now chv-controlplane
sudo systemctl enable --now chv-agent
```

---

## Hosting the Installer (`get.cellhv.com`)

The `curl -sfL https://get.cellhv.com/ | sh -` pattern requires a lightweight endpoint that serves `scripts/install.sh` as plain text.

### Option A: GitHub Pages
1. Create a repo `cellhv/get.cellhv.com`
2. Add a `CNAME` file with `get.cellhv.com`
3. Upload `scripts/install.sh` as the `index.html` (or serve it via a simple redirect page)
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

### Option C: S3 + CloudFront
- Upload `install.sh` to an S3 bucket
- Configure CloudFront distribution with the bucket as origin
- Set the object `Content-Type` to `text/plain`
- Point `get.cellhv.com` CNAME to the CloudFront distribution

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
sudo systemctl restart nginx
```

### View Logs
```bash
sudo journalctl -u chv-controlplane -f
sudo journalctl -u chv-agent -f
```

### Post-deploy Web UI smoke test
After restarting nginx/control plane, verify login and legacy route compatibility:

```bash
./scripts/smoke-webui-auth.sh http://<host-or-ip>
```

### Agent Fails to Enroll
- Ensure the bootstrap token file exists and is readable by the `chv` user
- Verify the control plane is listening on `127.0.0.1:8443`
- Check that the token was inserted into the `bootstrap_tokens` table and has not expired
- Review agent logs: `journalctl -u chv-agent -n 100`

### `chv-stord` or `chv-nwd` Keep Restarting
- The agent supervisor automatically restarts crashed child daemons
- Check agent logs for missing binary paths or permission errors
- Verify `/usr/local/bin/chv-stord` and `/usr/local/bin/chv-nwd` are executable

### Database Migration Errors
- Ensure PostgreSQL is running before `chv-controlplane` starts
- Verify the database URL in `controlplane.toml`
- Confirm the `chv` DB user has sufficient privileges

### Web UI Shows JSON Parse Error (`<!doctype html>` is not valid JSON)
This happens when the Web UI calls `/api/v1/...` endpoints that the current Rust control plane does not yet implement.

**Root cause:** The SvelteKit frontend expects a full REST API (e.g., `/api/v1/nodes`, `/api/v1/vms`), but the active Rust backend currently only exposes `/health`, `/ready`, `/metrics`, `/admin/nodes`, and `/admin/operations`.

**Immediate fixes:**
1. **Fix nginx proxy path stripping:** Ensure `proxy_pass` does NOT have a trailing slash:
   ```nginx
   location /api/ {
       proxy_pass http://127.0.0.1:8080;   # NO trailing slash
       proxy_intercept_errors off;
   }
   ```
2. **Rebuild and restart the control plane** so that unmatched API routes return a clean JSON 404 instead of an empty body or HTML fallback.

**Long-term fix:** Implement the missing `/api/v1/*` REST routes in `crates/chv-controlplane-service/src/api/` (or bridge them to the gRPC services).

### Web UI Blank Page
- Verify `npm run build` succeeded and `index.html` exists in `/opt/chv/ui/`
- Check nginx `root` directive points to `/opt/chv/ui/`
- Ensure the `/api/` proxy location forwards to `http://127.0.0.1:8080` (no trailing slash)

### Permission Denied on `/dev/kvm`
- Ensure the `chv` user is in the `kvm` group
- Reboot or run `newgrp kvm`

---

## Next Steps

- **Multi-node expansion:** Deploy additional hypervisor-only hosts with `chv-agent` pointing to the control plane's reachable IP.
- **External storage:** Configure `chv-stord` backends for shared storage.
- **Networking:** Define tenant bridges and network segments via the Web UI or API.
- **TLS hardening:** Replace the self-signed CA with your organization's PKI.
