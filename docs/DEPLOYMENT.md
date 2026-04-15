# CHV Deployment Guide — Combined Control Plane + Hypervisor Host

This guide explains how to deploy CHV on a single Linux host that acts as both the **control plane** (orchestration, API, Web UI) and the **hypervisor** (VM runtime via Cloud Hypervisor).

> **Version:** 0.0.0.2  
> **Target:** Ubuntu 22.04/24.04 LTS or equivalent Linux with KVM support

---

## Table of Contents

1. [Architecture on a Single Host](#architecture-on-a-single-host)
2. [Prerequisites](#prerequisites)
3. [Build from Source](#build-from-source)
4. [Host Preparation](#host-preparation)
5. [Database Setup](#database-setup)
6. [TLS / mTLS Setup](#tls--mtls-setup)
7. [Install Binaries & Directories](#install-binaries--directories)
8. [Configuration](#configuration)
9. [systemd Services](#systemd-services)
10. [Bootstrap & Enrollment](#bootstrap--enrollment)
11. [Web UI Deployment](#web-ui-deployment)
12. [Verification](#verification)
13. [Operations](#operations)
14. [Troubleshooting](#troubleshooting)

---

## Architecture on a Single Host

On a combined host, these processes run together:

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

**Key points for same-host deployment:**
- `chv-controlplane` listens on loopback (`127.0.0.1:8443` / `127.0.0.1:8080`).
- `chv-agent` connects to `https://127.0.0.1:8443`.
- `chv-agent` supervises `chv-stord` and `chv-nwd` as child processes.
- The Web UI is served by **nginx** (or any static file server) pointing at `ui/build/`.

---

## Prerequisites

### Hardware
- x86_64 server with **hardware virtualization** (VT-x / AMD-V)
- Minimum 4 cores, 8 GB RAM, 50 GB disk

### Software
```bash
# Essential packages
sudo apt update
sudo apt install -y \
  build-essential curl git pkg-config libssl-dev \
  postgresql postgresql-client nginx \
  qemu-kvm libvirt-daemon-system bridge-utils

# Verify KVM is available
ls /dev/kvm
# Should print: /dev/kvm
```

### Cloud Hypervisor Binary
CHV requires the Cloud Hypervisor VMM binary:

```bash
# Download latest release (example: v42.0)
CHV_VERSION="42.0"
curl -LO "https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v${CHV_VERSION}/cloud-hypervisor-v${CHV_VERSION}-x64"
sudo mv cloud-hypervisor-v${CHV_VERSION}-x64 /usr/local/bin/cloud-hypervisor
sudo chmod +x /usr/local/bin/cloud-hypervisor

# Verify
cloud-hypervisor --version
```

---

## Build from Source

### 1. Rust Toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Node.js (for Web UI)
```bash
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
sudo apt install -y nodejs
```

### 3. Build CHV Binaries
```bash
cd /path/to/chv/repo

# Release build of all Rust binaries
cargo build --workspace --release

# Binaries will be in:
#   target/release/chv-controlplane
#   target/release/chv-agent
#   target/release/chv-stord
#   target/release/chv-nwd
```

### 4. Build Web UI
```bash
cd ui
npm install
npm run build

# Static output is in: ui/build/
```

---

## Host Preparation

### Directory Layout
```bash
sudo mkdir -p /etc/chv
sudo mkdir -p /var/lib/chv/cache
sudo mkdir -p /var/lib/chv/images
sudo mkdir -p /var/lib/chv/storage
sudo mkdir -p /var/log/chv
sudo mkdir -p /run/chv
sudo mkdir -p /opt/chv/ui
```

### User & Permissions
Create a dedicated user for CHV services:

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin chv

# Allow chv user to use KVM
sudo usermod -aG kvm chv

# Set ownership
sudo chown -R chv:chv /var/lib/chv /var/log/chv /run/chv
sudo chmod 750 /var/lib/chv /var/log/chv
```

---

## Database Setup

The control plane requires PostgreSQL.

```bash
# Create database and user
sudo -u postgres psql <<'EOF'
CREATE USER chv WITH PASSWORD 'change-me-strong-password';
CREATE DATABASE chv_controlplane OWNER chv;
GRANT ALL PRIVILEGES ON DATABASE chv_controlplane TO chv;
EOF
```

> **Security:** Replace `change-me-strong-password` with a strong password and note it for `controlplane.toml`.

---

## TLS / mTLS Setup

CHV uses **mTLS** between the control plane and agents. The control plane acts as a CA and issues per-node certificates during enrollment.

### Generate CA Key Pair

```bash
sudo mkdir -p /etc/chv/certs
sudo chown root:chv /etc/chv/certs
sudo chmod 750 /etc/chv/certs

# Generate CA private key
sudo openssl genrsa -out /etc/chv/certs/ca.key 4096

# Generate self-signed CA certificate
sudo openssl req -x509 -new -nodes -key /etc/chv/certs/ca.key \
  -sha256 -days 3650 -out /etc/chv/certs/ca.crt \
  -subj "/O=CHV/CN=chv-ca"

# Secure permissions
sudo chmod 640 /etc/chv/certs/ca.key
sudo chmod 644 /etc/chv/certs/ca.crt
sudo chown root:chv /etc/chv/certs/ca.key /etc/chv/certs/ca.crt
```

### (Optional) TLS for gRPC Listener
If you want the control plane gRPC endpoint itself to present a TLS certificate (in addition to mTLS client verification):

```bash
# Server certificate for localhost
sudo openssl req -new -newkey rsa:4096 -nodes -keyout /etc/chv/certs/server.key -out /etc/chv/certs/server.csr -subj "/O=CHV/CN=localhost"

# Sign with CA
sudo openssl x509 -req -in /etc/chv/certs/server.csr -CA /etc/chv/certs/ca.crt -CAkey /etc/chv/certs/ca.key -CAcreateserial -out /etc/chv/certs/server.crt -days 365 -sha256

sudo chmod 640 /etc/chv/certs/server.key
sudo chown root:chv /etc/chv/certs/server.key
sudo rm /etc/chv/certs/server.csr
```

> **Note:** For same-host deployments, the control plane can run in **plaintext** on loopback for simplicity, but mTLS is still required for the enrollment CA to function.

---

## Install Binaries & Directories

```bash
# From the repo build output
sudo cp target/release/chv-controlplane /usr/local/bin/
sudo cp target/release/chv-agent /usr/local/bin/
sudo cp target/release/chv-stord /usr/local/bin/
sudo cp target/release/chv-nwd /usr/local/bin/

# Ensure cloud-hypervisor is available where agent expects it
sudo ln -sf /usr/local/bin/cloud-hypervisor /usr/bin/cloud-hypervisor

# Copy UI static assets
sudo cp -r ui/build/* /opt/chv/ui/
sudo chown -R www-data:www-data /opt/chv/ui

# Set binary permissions
sudo chmod 755 /usr/local/bin/chv-*
```

---

## Configuration

### 1. Control Plane Config
Create `/etc/chv/controlplane.toml`:

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
# For mTLS CA issuance (required)
ca_cert_path = "/etc/chv/certs/ca.crt"
ca_key_path = "/etc/chv/certs/ca.key"

# For gRPC server TLS (optional on loopback, recommended for production)
server_cert_path = "/etc/chv/certs/server.crt"
server_key_path = "/etc/chv/certs/server.key"
client_ca_path = "/etc/chv/certs/ca.crt"
```

Copy migrations so the control plane can apply them:
```bash
sudo mkdir -p /usr/local/share/chv
sudo cp -r cmd/chv-controlplane/migrations /usr/local/share/chv/
sudo chown -R chv:chv /usr/local/share/chv/migrations
```

### 2. Agent Config
Create `/etc/chv/agent.toml`:

```toml
socket_path = "/run/chv/agent/api.sock"
runtime_dir = "/run/chv/agent"
log_level = "info"
control_plane_addr = "https://127.0.0.1:8443"
stord_socket = "/run/chv/stord/api.sock"
nwd_socket = "/run/chv/nwd/api.sock"
chv_binary_path = "/usr/bin/cloud-hypervisor"
stord_binary_path = "/usr/local/bin/chv-stord"
nwd_binary_path = "/usr/local/bin/chv-nwd"
cache_path = "/var/lib/chv/cache/agent-cache.json"
node_id = ""                  # leave empty; assigned during enrollment
metrics_bind = "127.0.0.1:9901"

# Bootstrap token path (created in next step)
bootstrap_token_path = "/etc/chv/bootstrap.token"

# During first enrollment, the agent receives certs from the control plane.
# For steady-state reconnects, these paths are used automatically:
tls_cert_path = "/run/chv/agent/agent.crt"
tls_key_path = "/run/chv/agent/agent.key"
ca_cert_path = "/etc/chv/certs/ca.crt"
```

---

## systemd Services

### chv-controlplane.service
Create `/etc/systemd/system/chv-controlplane.service`:

```ini
[Unit]
Description=CHV Control Plane
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=chv
Group=chv
ExecStart=/usr/local/bin/chv-controlplane /etc/chv/controlplane.toml
Restart=on-failure
RestartSec=5
RuntimeDirectory=chv/controlplane
StateDirectory=chv
LogsDirectory=chv

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/run/chv/controlplane /var/lib/chv
ReadOnlyPaths=/etc/chv /usr/local/share/chv

[Install]
WantedBy=multi-user.target
```

### chv-agent.service
Create `/etc/systemd/system/chv-agent.service`:

```ini
[Unit]
Description=CHV Node Agent
After=network.target chv-controlplane.service
Wants=chv-controlplane.service

[Service]
Type=simple
User=chv
Group=chv
ExecStartPre=/bin/mkdir -p /run/chv/agent /run/chv/stord /run/chv/nwd
ExecStart=/usr/local/bin/chv-agent /etc/chv/agent.toml
Restart=on-failure
RestartSec=5
RuntimeDirectory=chv
StateDirectory=chv
LogsDirectory=chv

# Device access for KVM
DeviceAllow=/dev/kvm rw
SupplementaryGroups=kvm

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/run/chv /var/lib/chv
ReadOnlyPaths=/etc/chv /usr/local/bin /usr/bin

[Install]
WantedBy=multi-user.target
```

> **Note:** `chv-stord` and `chv-nwd` are started and supervised by `chv-agent` as child processes. They do **not** need separate systemd units.

Reload systemd:
```bash
sudo systemctl daemon-reload
```

---

## Bootstrap & Enrollment

### 1. Start the Control Plane
```bash
sudo systemctl enable --now chv-controlplane

# Verify it started correctly
sudo journalctl -u chv-controlplane -f
```

The control plane will automatically run database migrations on first start.

### 2. Create a Bootstrap Token
The agent needs a one-time bootstrap token to enroll. Insert it into the control plane database:

```bash
# Generate a secure token
BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
echo "$BOOTSTRAP_TOKEN" | sudo tee /etc/chv/bootstrap.token
sudo chmod 640 /etc/chv/bootstrap.token
sudo chown root:chv /etc/chv/bootstrap.token

# Insert token into the control plane DB
sudo -u postgres psql -d chv_controlplane -c "
INSERT INTO bootstrap_tokens (token_hash, description, one_time_use, expires_at, created_at)
VALUES (encode(digest('$BOOTSTRAP_TOKEN', 'sha256'), 'hex'), 'Initial deployment', true, now() + interval '1 hour', now())
ON CONFLICT DO NOTHING;
"
```

> If your PostgreSQL does not have `pgcrypto` enabled, enable it first:  
> `sudo -u postgres psql -d chv_controlplane -c "CREATE EXTENSION IF NOT EXISTS pgcrypto;"`

Alternatively, if the CHV project provides a CLI tool or admin API for token generation in the future, use that instead.

### 3. Start the Agent
```bash
sudo systemctl enable --now chv-agent

# Watch logs
sudo journalctl -u chv-agent -f
```

On first start, the agent will:
1. Read the bootstrap token from `/etc/chv/bootstrap.token`.
2. Connect to the control plane enrollment endpoint.
3. Receive a unique `node_id` and mTLS certificate.
4. Save certificates to `/run/chv/agent/`.
5. Start `chv-stord` and `chv-nwd`.
6. Transition to `TenantReady` state.

### 4. Clean Up Bootstrap Token (Optional)
Once enrollment succeeds, you may remove the token file:
```bash
sudo rm /etc/chv/bootstrap.token
```

---

## Web UI Deployment

The Web UI is a static site built from `ui/build/`. Serve it with nginx.

### nginx Config
Create `/etc/nginx/sites-available/chv`:

```nginx
server {
    listen 80;
    server_name _;  # accept any hostname

    root /opt/chv/ui;
    index index.html;

    # Static assets
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Proxy API calls to the control plane HTTP admin API
    location /api/ {
        proxy_pass http://127.0.0.1:8080/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # Optional: gzip for better performance
    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml;
}
```

Enable and reload:
```bash
sudo ln -sf /etc/nginx/sites-available/chv /etc/nginx/sites-enabled/chv
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl restart nginx
```

> **Production Note:** For production, configure HTTPS (e.g., with Let's Encrypt or an internal CA) and update the `server` block accordingly.

---

## Verification

### Check Service Status
```bash
sudo systemctl status chv-controlplane
sudo systemctl status chv-agent
```

### Check Admin API Health
```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
curl http://127.0.0.1:8080/admin/nodes
```

### Local Operator Tool (`chvctl`)
If `chvctl` is built and installed:
```bash
# Node status
chvctl node status

# VM list
chvctl vm list

# Storage sessions
chvctl stor sessions

# Network status
chvctl nw status
```

### Verify Agent Socket
```bash
ls -la /run/chv/agent/api.sock
ls -la /run/chv/stord/api.sock
ls -la /run/chv/nwd/api.sock
```

### Browser Test
Open `http://<server-ip>/` in a browser. You should see the CHV Web UI.

---

## Operations

### Restart Services
```bash
sudo systemctl restart chv-controlplane
sudo systemctl restart chv-agent
```

### View Logs
```bash
# Real-time
sudo journalctl -u chv-controlplane -f
sudo journalctl -u chv-agent -f

# Last 100 lines
sudo journalctl -u chv-controlplane -n 100
```

### Maintenance Mode
To safely perform host maintenance:
1. Use the Web UI or API to **enter maintenance mode** on the node.
2. Wait for VMs to migrate or shut down.
3. Stop services:
   ```bash
   sudo systemctl stop chv-agent
   sudo systemctl stop chv-controlplane
   ```
4. Perform maintenance.
5. Start services in reverse order.

### Backup
Back up these paths regularly:
- `/var/lib/chv/cache/` — agent local state
- PostgreSQL database `chv_controlplane`
- `/etc/chv/` — configuration and CA materials

---

## Troubleshooting

### Agent fails to enroll
- Check that the bootstrap token exists and is readable by the `chv` user.
- Verify the control plane is running and listening on `127.0.0.1:8443`.
- Check `journalctl -u chv-agent` for TLS or connection errors.
- Ensure the token was inserted into the database and has not expired.

### `chv-stord` or `chv-nwd` keep restarting
- The agent supervisor restarts them automatically if they crash.
- Check agent logs for process start errors (missing binaries, permission issues).
- Verify `/usr/local/bin/chv-stord` and `/usr/local/bin/chv-nwd` exist and are executable.

### Database migration errors
- Ensure PostgreSQL is running before `chv-controlplane` starts.
- Verify the database URL in `controlplane.toml`.
- Check that the `chv` DB user has ownership or sufficient privileges.

### Web UI shows blank page
- Ensure `npm run build` completed without errors.
- Verify nginx `root` points to `/opt/chv/ui`.
- Check that `index.html` exists in `/opt/chv/ui`.

### Permission denied on `/dev/kvm`
- Ensure the `chv` user is in the `kvm` group.
- Reboot or run `newgrp kvm` if testing interactively.

---

## Files Summary

| Path | Purpose |
|------|---------|
| `/usr/local/bin/chv-controlplane` | Control plane binary |
| `/usr/local/bin/chv-agent` | Node agent binary |
| `/usr/local/bin/chv-stord` | Storage daemon binary |
| `/usr/local/bin/chv-nwd` | Network daemon binary |
| `/usr/bin/cloud-hypervisor` | Cloud Hypervisor VMM |
| `/etc/chv/controlplane.toml` | Control plane config |
| `/etc/chv/agent.toml` | Agent config |
| `/etc/chv/certs/` | TLS certificates |
| `/var/lib/chv/cache/` | Agent durable cache |
| `/opt/chv/ui/` | Web UI static files |
| `/usr/local/share/chv/migrations/` | Database migrations |

---

## Next Steps

- **Multi-node expansion:** Deploy additional hypervisor-only hosts with `chv-agent` pointing to the control plane's reachable IP.
- **External storage:** Configure `chv-stord` backends for shared storage.
- **Networking:** Define tenant bridges and network segments via the Web UI or API.
