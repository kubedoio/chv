#!/bin/bash
# CHV All-in-One Installer
# Usage:
#   curl -sfL https://get.cellhv.com/ | sh -
#   curl -sfL https://get.cellhv.com/ | INSTALL_CHV_VERSION=0.0.0.2 sh -
#
# Environment variables:
#   INSTALL_CHV_VERSION         - Version to install (default: latest)
#   INSTALL_CHV_TARBALL_PATH    - Path to local release tarball (dev mode)
#   INSTALL_CHV_SKIP_DEPS       - Set to "1" to skip apt dependency installation
#   INSTALL_CHV_SKIP_CLOUD_HV   - Set to "1" to skip cloud-hypervisor download

set -euo pipefail

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------
INSTALL_CHV_VERSION="${INSTALL_CHV_VERSION:-latest}"
INSTALL_CHV_TARBALL_PATH="${INSTALL_CHV_TARBALL_PATH:-}"
INSTALL_CHV_SKIP_DEPS="${INSTALL_CHV_SKIP_DEPS:-0}"
INSTALL_CHV_SKIP_CLOUD_HV="${INSTALL_CHV_SKIP_CLOUD_HV:-0}"

CHV_USER="chv"
CHV_CONFIG_DIR="/etc/chv"
CHV_DATA_DIR="/var/lib/chv"
CHV_LOG_DIR="/var/log/chv"
CHV_RUN_DIR="/run/chv"
CHV_UI_DIR="/opt/chv/ui"
CHV_MIGRATIONS_DIR="/usr/local/share/chv/migrations"

GITHUB_REPO="${GITHUB_REPO:-cellhv/chv}"  # Update when repo is published

# -----------------------------------------------------------------------------
# Helpers
# -----------------------------------------------------------------------------
info() { echo "[INFO] $*"; }
warn() { echo "[WARN] $*" >&2; }
fatal() { echo "[ERROR] $*" >&2; exit 1; }

cmd_exists() { command -v "$1" &>/dev/null; }

get_local_ip() {
    hostname -I 2>/dev/null | awk '{print $1}' || echo "127.0.0.1"
}

# -----------------------------------------------------------------------------
# Pre-flight checks
# -----------------------------------------------------------------------------
if [ "$(id -u)" -ne 0 ]; then
    fatal "This installer must be run as root. Try: sudo curl -sfL https://get.cellhv.com/ | sh -"
fi

ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
    fatal "Unsupported architecture: $ARCH. Only x86_64 is supported for now."
fi

# Detect if we are inside a release tarball
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/bin/chv-controlplane" ] && [ -z "$INSTALL_CHV_TARBALL_PATH" ]; then
    info "Detected local release tarball mode (binaries next to install.sh)"
    INSTALL_CHV_TARBALL_PATH="${SCRIPT_DIR}"
fi

# -----------------------------------------------------------------------------
# Install system dependencies
# -----------------------------------------------------------------------------
install_dependencies() {
    if [ "$INSTALL_CHV_SKIP_DEPS" = "1" ]; then
        info "Skipping dependency installation (INSTALL_CHV_SKIP_DEPS=1)"
        return
    fi

    info "Installing system dependencies..."
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq \
        postgresql postgresql-client nginx \
        qemu-kvm bridge-utils curl openssl \
        coreutils tar gzip
}

# -----------------------------------------------------------------------------
# Create user and directories
# -----------------------------------------------------------------------------
setup_user_and_dirs() {
    info "Setting up user and directories..."

    if ! id -u "$CHV_USER" &>/dev/null; then
        useradd --system --no-create-home --shell /usr/sbin/nologin "$CHV_USER"
    fi

    usermod -aG kvm "$CHV_USER" 2>/dev/null || true

    mkdir -p "$CHV_CONFIG_DIR"/certs
    mkdir -p "$CHV_DATA_DIR"/{cache,images,storage}
    mkdir -p "$CHV_LOG_DIR"
    mkdir -p "$CHV_RUN_DIR"
    mkdir -p "$CHV_UI_DIR"
    mkdir -p "$CHV_MIGRATIONS_DIR"

    chown -R "$CHV_USER:$CHV_USER" "$CHV_DATA_DIR" "$CHV_LOG_DIR" "$CHV_RUN_DIR"
    chmod 750 "$CHV_DATA_DIR" "$CHV_LOG_DIR"
}

# -----------------------------------------------------------------------------
# Resolve version and download release
# -----------------------------------------------------------------------------
resolve_version() {
    if [ "$INSTALL_CHV_VERSION" = "latest" ]; then
        if cmd_exists curl; then
            # Query GitHub releases API for latest tag name
            local latest
            latest=$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -n1 | sed -E 's/.*"v([^"]+)".*/\1/') || true
            if [ -n "$latest" ]; then
                INSTALL_CHV_VERSION="$latest"
            else
                warn "Could not determine latest version from GitHub API, falling back to 0.0.0.2"
                INSTALL_CHV_VERSION="0.0.0.2"
            fi
        else
            INSTALL_CHV_VERSION="0.0.0.2"
        fi
    fi
    info "Installing CHV version: $INSTALL_CHV_VERSION"
}

download_release() {
    if [ -n "$INSTALL_CHV_TARBALL_PATH" ]; then
        if [ -d "$INSTALL_CHV_TARBALL_PATH" ]; then
            info "Using local release directory: $INSTALL_CHV_TARBALL_PATH"
            EXTRACT_DIR="$INSTALL_CHV_TARBALL_PATH"
        elif [ -f "$INSTALL_CHV_TARBALL_PATH" ]; then
            info "Using local release tarball: $INSTALL_CHV_TARBALL_PATH"
            local tmpdir
            tmpdir=$(mktemp -d)
            tar -xzf "$INSTALL_CHV_TARBALL_PATH" -C "$tmpdir"
            EXTRACT_DIR="$tmpdir/chv-${INSTALL_CHV_VERSION}-linux-amd64"
        else
            fatal "Local tarball path not found: $INSTALL_CHV_TARBALL_PATH"
        fi
        return
    fi

    local tarball_url="https://github.com/${GITHUB_REPO}/releases/download/v${INSTALL_CHV_VERSION}/chv-${INSTALL_CHV_VERSION}-linux-amd64.tar.gz"
    local tmpdir
    tmpdir=$(mktemp -d)
    local tarball="$tmpdir/chv-release.tar.gz"

    info "Downloading release tarball..."
    curl -fsSL "$tarball_url" -o "$tarball" || fatal "Failed to download release from $tarball_url"

    info "Extracting release tarball..."
    tar -xzf "$tarball" -C "$tmpdir"

    EXTRACT_DIR="$tmpdir/chv-${INSTALL_CHV_VERSION}-linux-amd64"
    CLEANUP_TMPDIR="$tmpdir"
}

# -----------------------------------------------------------------------------
# Install binaries and assets
# -----------------------------------------------------------------------------
install_binaries_and_assets() {
    info "Installing CHV binaries..."

    cp "${EXTRACT_DIR}/bin/chv-controlplane" /usr/local/bin/
    cp "${EXTRACT_DIR}/bin/chv-agent" /usr/local/bin/
    cp "${EXTRACT_DIR}/bin/chv-stord" /usr/local/bin/
    cp "${EXTRACT_DIR}/bin/chv-nwd" /usr/local/bin/
    chmod 755 /usr/local/bin/chv-*

    info "Installing Web UI assets..."
    cp -r "${EXTRACT_DIR}/ui/"* "$CHV_UI_DIR/"
    chown -R www-data:www-data "$CHV_UI_DIR"

    info "Installing database migrations..."
    cp -r "${EXTRACT_DIR}/migrations/"* "$CHV_MIGRATIONS_DIR/"
    chown -R "$CHV_USER:$CHV_USER" "$CHV_MIGRATIONS_DIR"
}

# -----------------------------------------------------------------------------
# Install Cloud Hypervisor
# -----------------------------------------------------------------------------
install_cloud_hypervisor() {
    if [ "$INSTALL_CHV_SKIP_CLOUD_HV" = "1" ]; then
        info "Skipping Cloud Hypervisor installation (INSTALL_CHV_SKIP_CLOUD_HV=1)"
        return
    fi

    if cmd_exists cloud-hypervisor; then
        info "Cloud Hypervisor already installed: $(cloud-hypervisor --version 2>/dev/null || true)"
        if [ ! -f /usr/bin/cloud-hypervisor ] && [ -f /usr/local/bin/cloud-hypervisor ]; then
            ln -sf /usr/local/bin/cloud-hypervisor /usr/bin/cloud-hypervisor
        fi
        return
    fi

    local chv_version="51.1"
    info "Downloading Cloud Hypervisor v${chv_version}..."
    curl -fsSL "https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v${chv_version}/cloud-hypervisor-static" \
        -o /usr/local/bin/cloud-hypervisor
    chmod +x /usr/local/bin/cloud-hypervisor
    ln -sf /usr/local/bin/cloud-hypervisor /usr/bin/cloud-hypervisor
    info "Cloud Hypervisor installed: $(cloud-hypervisor --version)"
}

# -----------------------------------------------------------------------------
# TLS Certificate Generation
# -----------------------------------------------------------------------------
generate_certs() {
    info "Generating TLS certificates..."

    chown root:"$CHV_USER" "$CHV_CONFIG_DIR"/certs
    chmod 750 "$CHV_CONFIG_DIR"/certs

    if [ ! -f "$CHV_CONFIG_DIR/certs/ca.key" ]; then
        openssl genrsa -out "$CHV_CONFIG_DIR/certs/ca.key" 4096
        openssl req -x509 -new -nodes -key "$CHV_CONFIG_DIR/certs/ca.key" \
            -sha256 -days 3650 -out "$CHV_CONFIG_DIR/certs/ca.crt" \
            -subj "/O=CHV/CN=chv-ca"
        chmod 640 "$CHV_CONFIG_DIR/certs/ca.key"
        chmod 644 "$CHV_CONFIG_DIR/certs/ca.crt"
        chown root:"$CHV_USER" "$CHV_CONFIG_DIR/certs/ca.key" "$CHV_CONFIG_DIR/certs/ca.crt"
    fi

    if [ ! -f "$CHV_CONFIG_DIR/certs/server.key" ]; then
        openssl req -new -newkey rsa:4096 -nodes -keyout "$CHV_CONFIG_DIR/certs/server.key" \
            -out "$CHV_CONFIG_DIR/certs/server.csr" -subj "/O=CHV/CN=localhost"
        openssl x509 -req -in "$CHV_CONFIG_DIR/certs/server.csr" \
            -CA "$CHV_CONFIG_DIR/certs/ca.crt" -CAkey "$CHV_CONFIG_DIR/certs/ca.key" \
            -CAcreateserial -out "$CHV_CONFIG_DIR/certs/server.crt" -days 365 -sha256
        rm -f "$CHV_CONFIG_DIR/certs/server.csr"
        chmod 640 "$CHV_CONFIG_DIR/certs/server.key"
        chown root:"$CHV_USER" "$CHV_CONFIG_DIR/certs/server.key"
    fi
}

# -----------------------------------------------------------------------------
# Database Setup
# -----------------------------------------------------------------------------
setup_database() {
    info "Setting up PostgreSQL database..."

    # Ensure PostgreSQL is running
    if ! pg_isready -q 2>/dev/null; then
        systemctl start postgresql || true
    fi

    DB_PASSWORD=$(openssl rand -base64 32 | tr -d '=+/')
    JWT_SECRET=$(openssl rand -base64 32 | tr -d '=+/')

    # Create user and database idempotently
    sudo -u postgres psql -c "CREATE USER $CHV_USER WITH PASSWORD '$DB_PASSWORD';" >/dev/null 2>&1 || true
    sudo -u postgres psql -c "CREATE DATABASE chv_controlplane OWNER $CHV_USER;" >/dev/null 2>&1 || true
}

# -----------------------------------------------------------------------------
# Configuration Files
# -----------------------------------------------------------------------------
install_configs() {
    info "Installing configuration files..."

    cat > "$CHV_CONFIG_DIR/controlplane.toml" <<EOF
grpc_bind = "127.0.0.1:8443"
http_bind = "127.0.0.1:8080"
log_level = "info"
runtime_dir = "/run/chv/controlplane"
jwt_secret = "${JWT_SECRET}"

[database]
url = "postgres://${CHV_USER}:${DB_PASSWORD}@127.0.0.1:5432/chv_controlplane"
migrations_dir = "${CHV_MIGRATIONS_DIR}"
max_connections = 16
min_connections = 1
acquire_timeout_secs = 5
idle_timeout_secs = 300
max_lifetime_secs = 1800

[tls]
ca_cert_path = "${CHV_CONFIG_DIR}/certs/ca.crt"
ca_key_path = "${CHV_CONFIG_DIR}/certs/ca.key"
# gRPC server TLS is optional; disabled by default for all-in-one loopback deployments
# server_cert_path = "${CHV_CONFIG_DIR}/certs/server.crt"
# server_key_path = "${CHV_CONFIG_DIR}/certs/server.key"
# client_ca_path = "${CHV_CONFIG_DIR}/certs/ca.crt"
EOF
    chmod 640 "$CHV_CONFIG_DIR/controlplane.toml"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/controlplane.toml"

    cat > "$CHV_CONFIG_DIR/agent.toml" <<EOF
socket_path = "/run/chv/agent/api.sock"
runtime_dir = "/run/chv/agent"
log_level = "info"
control_plane_addr = "http://127.0.0.1:8443"
stord_socket = "/run/chv/stord/api.sock"
nwd_socket = "/run/chv/nwd/api.sock"
chv_binary_path = "/usr/bin/cloud-hypervisor"
stord_binary_path = "/usr/local/bin/chv-stord"
nwd_binary_path = "/usr/local/bin/chv-nwd"
cache_path = "${CHV_DATA_DIR}/cache/agent-cache.json"
node_id = ""
metrics_bind = "127.0.0.1:9901"
bootstrap_token_path = "${CHV_CONFIG_DIR}/bootstrap.token"
tls_cert_path = "/run/chv/agent/agent.crt"
tls_key_path = "/run/chv/agent/agent.key"
ca_cert_path = "${CHV_CONFIG_DIR}/certs/ca.crt"
EOF
    chmod 640 "$CHV_CONFIG_DIR/agent.toml"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/agent.toml"
}

# -----------------------------------------------------------------------------
# systemd Services
# -----------------------------------------------------------------------------
install_systemd_services() {
    info "Installing systemd services..."

    cat > /etc/systemd/system/chv-controlplane.service <<'EOF'
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
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/run/chv/controlplane /var/lib/chv
ReadOnlyPaths=/etc/chv /usr/local/share/chv

[Install]
WantedBy=multi-user.target
EOF

    cat > /etc/systemd/system/chv-agent.service <<'EOF'
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
DeviceAllow=/dev/kvm rw
SupplementaryGroups=kvm
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/run/chv /var/lib/chv
ReadOnlyPaths=/etc/chv /usr/local/bin /usr/bin

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
}

# -----------------------------------------------------------------------------
# Bootstrap Token
# -----------------------------------------------------------------------------
create_bootstrap_token() {
    info "Creating bootstrap token for agent enrollment..."

    BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
    printf '%s' "$BOOTSTRAP_TOKEN" > "$CHV_CONFIG_DIR/bootstrap.token"
    chmod 640 "$CHV_CONFIG_DIR/bootstrap.token"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/bootstrap.token"

    sudo -u postgres psql -d chv_controlplane -c "CREATE EXTENSION IF NOT EXISTS pgcrypto;" >/dev/null
    sudo -u postgres psql -d chv_controlplane -c \
        "INSERT INTO bootstrap_tokens (token_hash, description, one_time_use, expires_at, created_at) VALUES (encode(digest('${BOOTSTRAP_TOKEN}', 'sha256'), 'hex'), 'All-in-one installer', true, now() + interval '1 hour', now()) ON CONFLICT DO NOTHING;" >/dev/null
}

# -----------------------------------------------------------------------------
# nginx Setup
# -----------------------------------------------------------------------------
install_nginx() {
    info "Configuring nginx for CHV Web UI..."

    cat > /etc/nginx/sites-available/chv <<'EOF'
server {
    listen 80;
    server_name _;

    root /opt/chv/ui;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location /api/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_intercept_errors off;
    }

    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml+rss text/javascript;
}
EOF

    if [ -f /etc/nginx/sites-enabled/default ]; then
        rm -f /etc/nginx/sites-enabled/default
    fi

    if [ ! -f /etc/nginx/sites-enabled/chv ]; then
        ln -sf /etc/nginx/sites-available/chv /etc/nginx/sites-enabled/chv
    fi

    nginx -t || fatal "nginx configuration test failed"
    systemctl restart nginx
}

# -----------------------------------------------------------------------------
# Start Services
# -----------------------------------------------------------------------------
start_services() {
    info "Starting CHV services..."

    systemctl enable --now chv-controlplane
    
    info "Waiting for control plane to run database migrations..."
    
    # Poll the database for up to 30 seconds until the table appears
    local max_attempts=30
    local attempt=1
    while [ $attempt -le $max_attempts ]; do
        # Check if the table exists in PostgreSQL
        if sudo -u postgres psql -d chv_controlplane -tAc "SELECT 1 FROM information_schema.tables WHERE table_name='bootstrap_tokens';" | grep -q 1; then
            info "Database migrations confirmed."
            break
        fi
        sleep 1
        ((attempt++))
    done

    if [ $attempt -gt $max_attempts ]; then
        fatal "Control plane failed to create database tables within 30 seconds. Check logs with: journalctl -u chv-controlplane --no-pager -n 50"
    fi
    
    # Now it is safe to insert the token
    create_bootstrap_token
    
    systemctl enable --now chv-agent
}

# -----------------------------------------------------------------------------
# Cleanup
# -----------------------------------------------------------------------------
cleanup() {
    if [ -n "${CLEANUP_TMPDIR:-}" ] && [ -d "$CLEANUP_TMPDIR" ]; then
        rm -rf "$CLEANUP_TMPDIR"
    fi
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------
trap cleanup EXIT

install_dependencies
setup_user_and_dirs
resolve_version
download_release
install_binaries_and_assets
install_cloud_hypervisor
generate_certs
setup_database
install_configs
install_systemd_services
install_nginx
start_services

LOCAL_IP=$(get_local_ip)

cat <<EOF

===============================================
  CHV All-in-One Installation Complete!
===============================================

Version:        ${INSTALL_CHV_VERSION}
Web UI:         http://${LOCAL_IP}/
Admin API:      http://127.0.0.1:8080/
Health:         curl http://127.0.0.1:8080/health
Nodes:          curl http://127.0.0.1:8080/admin/nodes

Services:
  systemctl status chv-controlplane
  systemctl status chv-agent
  systemctl status nginx

Logs:
  journalctl -u chv-controlplane -f
  journalctl -u chv-agent -f

Next steps:
  1. Open http://${LOCAL_IP}/ in your browser.
  2. The local agent will auto-enroll using the
     bootstrap token (valid for 1 hour).
  3. Create VMs through the Web UI or gRPC API.

EOF
