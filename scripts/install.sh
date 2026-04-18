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
#   INSTALL_CHV_BRIDGE_IFACE    - Host interface to attach to bridge (default: ens19)
#   INSTALL_CHV_BRIDGE_NAME     - Bridge name (default: chvbr0)
#   INSTALL_CHV_BRIDGE_CIDR     - Bridge CIDR (default: 10.200.0.1/24)

set -euo pipefail

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------
INSTALL_CHV_VERSION="${INSTALL_CHV_VERSION:-latest}"
INSTALL_CHV_TARBALL_PATH="${INSTALL_CHV_TARBALL_PATH:-}"
INSTALL_CHV_SKIP_DEPS="${INSTALL_CHV_SKIP_DEPS:-0}"
INSTALL_CHV_SKIP_CLOUD_HV="${INSTALL_CHV_SKIP_CLOUD_HV:-0}"

# Network defaults
INSTALL_CHV_BRIDGE_IFACE="${INSTALL_CHV_BRIDGE_IFACE:-ens19}"
INSTALL_CHV_BRIDGE_NAME="${INSTALL_CHV_BRIDGE_NAME:-chvbr0}"
INSTALL_CHV_BRIDGE_CIDR="${INSTALL_CHV_BRIDGE_CIDR:-10.200.0.1/24}"

CHV_USER="chv"
CHV_CONFIG_DIR="/etc/chv"
CHV_DATA_DIR="/var/lib/chv"
CHV_LOG_DIR="/var/log/chv"
CHV_RUN_DIR="/run/chv"
CHV_UI_DIR="/opt/chv/ui"
CHV_MIGRATIONS_DIR="/usr/local/share/chv/migrations"
CHV_DB_PATH="${CHV_DATA_DIR}/controlplane.db"

GITHUB_REPO="${GITHUB_REPO:-cellhv/chv}"

# Populated later
EXTRACT_DIR=""
CLEANUP_TMPDIR=""
JWT_SECRET=""
BOOTSTRAP_TOKEN=""

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
    fatal "This installer must be run as root. Try: sudo ./scripts/install.sh"
fi

ARCH=$(uname -m)
case "$ARCH" in
    x86_64)
        CHV_ARCH="amd64"
        ;;
    aarch64|arm64)
        CHV_ARCH="arm64"
        ;;
    *)
        fatal "Unsupported architecture: $ARCH. Only x86_64 and arm64 are supported."
        ;;
esac
info "Detected architecture: $ARCH (image suffix: $CHV_ARCH)"

# Detect if we are running from inside a release tarball directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/bin/chv-controlplane" ] && [ -z "$INSTALL_CHV_TARBALL_PATH" ]; then
    info "Detected local release directory mode (binaries next to install.sh)"
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
        nginx \
        qemu-kvm bridge-utils iproute2 iptables curl openssl \
        coreutils tar gzip sqlite3
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
    mkdir -p "$CHV_DATA_DIR"/{cache,images,storage/localdisk}
    mkdir -p "$CHV_LOG_DIR"
    mkdir -p "$CHV_RUN_DIR"/{controlplane,agent,stord,nwd}
    mkdir -p "$CHV_UI_DIR"
    mkdir -p "$CHV_MIGRATIONS_DIR"

    chown -R "$CHV_USER:$CHV_USER" "$CHV_DATA_DIR" "$CHV_LOG_DIR" "$CHV_RUN_DIR"
    chmod 750 "$CHV_DATA_DIR" "$CHV_LOG_DIR"
}

# -----------------------------------------------------------------------------
# Resolve version and download/locate release
# -----------------------------------------------------------------------------
resolve_version() {
    if [ "$INSTALL_CHV_VERSION" = "latest" ]; then
        if cmd_exists curl; then
            local latest
            latest=$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" 2>/dev/null \
                | grep '"tag_name":' | head -n1 | sed -E 's/.*"v([^"]+)".*/\1/') || true
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
            CLEANUP_TMPDIR="$tmpdir"
        else
            fatal "Local tarball path not found: $INSTALL_CHV_TARBALL_PATH"
        fi
        return
    fi

    local tarball_url="https://github.com/${GITHUB_REPO}/releases/download/v${INSTALL_CHV_VERSION}/chv-${INSTALL_CHV_VERSION}-linux-amd64.tar.gz"
    local tmpdir
    tmpdir=$(mktemp -d)
    local tarball="$tmpdir/chv-release.tar.gz"
    CLEANUP_TMPDIR="$tmpdir"

    info "Downloading release tarball..."
    curl -fsSL "$tarball_url" -o "$tarball" || fatal "Failed to download release from $tarball_url"

    info "Extracting release tarball..."
    tar -xzf "$tarball" -C "$tmpdir"
    EXTRACT_DIR="$tmpdir/chv-${INSTALL_CHV_VERSION}-linux-amd64"
}

# -----------------------------------------------------------------------------
# Install binaries and assets
# -----------------------------------------------------------------------------
install_binaries_and_assets() {
    info "Installing CHV binaries..."

    install -m 755 "${EXTRACT_DIR}/bin/chv-controlplane" /usr/local/bin/
    install -m 755 "${EXTRACT_DIR}/bin/chv-agent"        /usr/local/bin/
    install -m 755 "${EXTRACT_DIR}/bin/chv-stord"        /usr/local/bin/
    install -m 755 "${EXTRACT_DIR}/bin/chv-nwd"          /usr/local/bin/

    info "Installing Web UI assets..."
    rm -rf "$CHV_UI_DIR"/*
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
# Base OS Image Download
# -----------------------------------------------------------------------------
BASE_IMAGE_URL="https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-${CHV_ARCH}.img"
BASE_IMAGE_PATH="${CHV_DATA_DIR}/images/ubuntu-noble-${CHV_ARCH}.img"

download_base_image() {
    info "Checking for base OS image..."

    mkdir -p "${CHV_DATA_DIR}/images"
    chown "${CHV_USER}:${CHV_USER}" "${CHV_DATA_DIR}/images"

    if [ -f "$BASE_IMAGE_PATH" ]; then
        info "Base image already exists at ${BASE_IMAGE_PATH}, skipping download."
        return
    fi

    info "Downloading Ubuntu Noble base image for ${CHV_ARCH}..."
    info "URL: ${BASE_IMAGE_URL}"

    if curl -fsSL --retry 3 --retry-delay 5 "$BASE_IMAGE_URL" -o "$BASE_IMAGE_PATH"; then
        chown "${CHV_USER}:${CHV_USER}" "$BASE_IMAGE_PATH"
        chmod 644 "$BASE_IMAGE_PATH"
        local img_size
        img_size=$(du -h "$BASE_IMAGE_PATH" | cut -f1)
        info "Base image downloaded (${img_size}) -> ${BASE_IMAGE_PATH}"
    else
        warn "Failed to download base image from ${BASE_IMAGE_URL}"
        warn "You can manually download and import it later via the Web UI."
    fi
}

# -----------------------------------------------------------------------------
# Import Base Image into Control Plane
# -----------------------------------------------------------------------------
import_base_image() {
    if [ ! -f "$BASE_IMAGE_PATH" ]; then
        info "No base image to import."
        return
    fi

    info "Importing base image into control plane..."

    # Wait for CP to be ready (should already be up from start_services)
    local attempt=1
    while [ $attempt -le 30 ]; do
        if curl -sf "http://127.0.0.1:8080/health" &>/dev/null; then
            break
        fi
        sleep 1
        ((attempt++))
    done

    if [ $attempt -gt 30 ]; then
        warn "Control plane not available for image import."
        return
    fi

    # Check if image already imported
    local existing
    existing=$(sqlite3 "${CHV_DB_PATH}" \
        "SELECT COUNT(*) FROM images WHERE display_name='ubuntu-noble' OR source_url='${BASE_IMAGE_URL}';" 2>/dev/null || echo "0")

    if [ "${existing}" -gt 0 ] 2>/dev/null; then
        info "Base image already imported, skipping."
        return
    fi

    local image_id
    image_id=$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || openssl rand -hex 16)
    local size_bytes
    size_bytes=$(stat -c%s "$BASE_IMAGE_PATH" 2>/dev/null || stat -f%z "$BASE_IMAGE_PATH" 2>/dev/null || echo "0")

    sqlite3 "${CHV_DB_PATH}" \
        "INSERT INTO images
         (image_id, display_name, image_type, format, size_bytes, checksum, source_url, os, version, status, node_id, created_at, updated_at)
         VALUES ('${image_id}', 'ubuntu-noble', 'disk', 'raw', ${size_bytes}, NULL, '${BASE_IMAGE_PATH}', 'ubuntu', '24.04', 'available', NULL,
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'));" 2>/dev/null

    # Verify import
    local imported
    imported=$(sqlite3 "${CHV_DB_PATH}" \
        "SELECT COUNT(*) FROM images WHERE image_id='${image_id}';" 2>/dev/null || echo "0")

    if [ "${imported}" -gt 0 ] 2>/dev/null; then
        info "Base image imported successfully (image_id: ${image_id})."
    else
        warn "Failed to import base image into database."
        warn "You can import it manually via: sqlite3 ${CHV_DB_PATH}"
    fi
}

# -----------------------------------------------------------------------------
# TLS Certificate Generation
# -----------------------------------------------------------------------------
generate_certs() {
    info "Generating TLS certificates..."

    chown root:"$CHV_USER" "$CHV_CONFIG_DIR"/certs
    chmod 750 "$CHV_CONFIG_DIR"/certs

    if [ ! -f "$CHV_CONFIG_DIR/certs/ca.key" ]; then
        openssl genrsa -out "$CHV_CONFIG_DIR/certs/ca.key" 4096 2>/dev/null
        openssl req -x509 -new -nodes -key "$CHV_CONFIG_DIR/certs/ca.key" \
            -sha256 -days 3650 -out "$CHV_CONFIG_DIR/certs/ca.crt" \
            -subj "/O=CHV/CN=chv-ca" 2>/dev/null
        chmod 640 "$CHV_CONFIG_DIR/certs/ca.key"
        chmod 644 "$CHV_CONFIG_DIR/certs/ca.crt"
        chown root:"$CHV_USER" "$CHV_CONFIG_DIR/certs/ca.key" "$CHV_CONFIG_DIR/certs/ca.crt"
    fi
}

# -----------------------------------------------------------------------------
# Bridge and NAT network setup
# -----------------------------------------------------------------------------
setup_network() {
    info "Setting up bridge network ${INSTALL_CHV_BRIDGE_NAME} (${INSTALL_CHV_BRIDGE_CIDR}) on ${INSTALL_CHV_BRIDGE_IFACE}..."

    # Create bridge if absent
    if ! ip link show "${INSTALL_CHV_BRIDGE_NAME}" &>/dev/null; then
        ip link add name "${INSTALL_CHV_BRIDGE_NAME}" type bridge
    fi
    ip link set "${INSTALL_CHV_BRIDGE_NAME}" up

    # Assign gateway IP to the bridge (idempotent)
    local bridge_ip="${INSTALL_CHV_BRIDGE_CIDR%/*}"
    local bridge_prefix="${INSTALL_CHV_BRIDGE_CIDR#*/}"
    if ! ip addr show "${INSTALL_CHV_BRIDGE_NAME}" | grep -q "${bridge_ip}"; then
        ip addr add "${INSTALL_CHV_BRIDGE_CIDR}" dev "${INSTALL_CHV_BRIDGE_NAME}"
    fi

    # Attach host interface to the bridge if it exists and is not already a member
    if ip link show "${INSTALL_CHV_BRIDGE_IFACE}" &>/dev/null; then
        local current_master
        current_master=$(ip link show "${INSTALL_CHV_BRIDGE_IFACE}" | grep -o "master [^ ]*" | awk '{print $2}' || true)
        if [ "$current_master" != "${INSTALL_CHV_BRIDGE_NAME}" ]; then
            ip link set "${INSTALL_CHV_BRIDGE_IFACE}" master "${INSTALL_CHV_BRIDGE_NAME}"
            ip link set "${INSTALL_CHV_BRIDGE_IFACE}" up
        fi
    else
        warn "Interface ${INSTALL_CHV_BRIDGE_IFACE} not found — bridge created without upstream port."
        warn "Set INSTALL_CHV_BRIDGE_IFACE to an existing interface if you need external connectivity."
    fi

    # Enable IP forwarding
    sysctl -qw net.ipv4.ip_forward=1
    echo "net.ipv4.ip_forward=1" > /etc/sysctl.d/99-chv-forward.conf

    # NAT outbound traffic from bridge subnet
    local bridge_net="${bridge_ip%.*}.0/${bridge_prefix}"
    if ! iptables -t nat -C POSTROUTING -s "${bridge_net}" ! -d "${bridge_net}" -j MASQUERADE 2>/dev/null; then
        iptables -t nat -A POSTROUTING -s "${bridge_net}" ! -d "${bridge_net}" -j MASQUERADE
    fi

    # Persist iptables rules (iptables-persistent / netfilter-persistent)
    if cmd_exists netfilter-persistent; then
        netfilter-persistent save 2>/dev/null || true
    elif cmd_exists iptables-save; then
        mkdir -p /etc/iptables
        iptables-save > /etc/iptables/rules.v4
    fi

    # Persist bridge via systemd-networkd or a simple netplan/ifupdown snippet
    # We use a drop-in under /etc/network/interfaces.d (ifupdown) if available,
    # otherwise write a systemd.network unit (networkd).
    if [ -d /etc/network/interfaces.d ]; then
        cat > "/etc/network/interfaces.d/chvbr0" <<EOF
auto ${INSTALL_CHV_BRIDGE_NAME}
iface ${INSTALL_CHV_BRIDGE_NAME} inet static
    address ${INSTALL_CHV_BRIDGE_CIDR}
    bridge_ports ${INSTALL_CHV_BRIDGE_IFACE}
    bridge_stp off
    bridge_fd 0
EOF
    else
        mkdir -p /etc/systemd/network
        cat > "/etc/systemd/network/10-${INSTALL_CHV_BRIDGE_NAME}.netdev" <<EOF
[NetDev]
Name=${INSTALL_CHV_BRIDGE_NAME}
Kind=bridge
EOF
        cat > "/etc/systemd/network/11-${INSTALL_CHV_BRIDGE_NAME}.network" <<EOF
[Match]
Name=${INSTALL_CHV_BRIDGE_NAME}

[Network]
Address=${INSTALL_CHV_BRIDGE_CIDR}
IPForward=yes
EOF
        cat > "/etc/systemd/network/12-${INSTALL_CHV_BRIDGE_IFACE}.network" <<EOF
[Match]
Name=${INSTALL_CHV_BRIDGE_IFACE}

[Network]
Bridge=${INSTALL_CHV_BRIDGE_NAME}
EOF
    fi

    info "Bridge ${INSTALL_CHV_BRIDGE_NAME} ready (${INSTALL_CHV_BRIDGE_CIDR}), NAT via ${INSTALL_CHV_BRIDGE_IFACE}."
}

# -----------------------------------------------------------------------------
# Configuration Files
# -----------------------------------------------------------------------------
install_configs() {
    info "Installing configuration files..."

    JWT_SECRET=$(openssl rand -base64 32 | tr -d '=+/')

    cat > "$CHV_CONFIG_DIR/controlplane.toml" <<EOF
grpc_bind = "127.0.0.1:8443"
http_bind = "127.0.0.1:8080"
log_level = "info"
runtime_dir = "/run/chv/controlplane"
jwt_secret = "${JWT_SECRET}"

[database]
url = "sqlite://${CHV_DB_PATH}"
migrations_dir = "${CHV_MIGRATIONS_DIR}"
max_connections = 4
min_connections = 1
acquire_timeout_secs = 5

[tls]
ca_cert_path = "${CHV_CONFIG_DIR}/certs/ca.crt"
ca_key_path = "${CHV_CONFIG_DIR}/certs/ca.key"
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
storage_base_dir = "${CHV_DATA_DIR}/storage"
bootstrap_token_path = "${CHV_CONFIG_DIR}/bootstrap.token"
tls_cert_path = "/run/chv/agent/agent.crt"
tls_key_path = "/run/chv/agent/agent.key"
ca_cert_path = "${CHV_CONFIG_DIR}/certs/ca.crt"
EOF
    chmod 640 "$CHV_CONFIG_DIR/agent.toml"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/agent.toml"

    cat > "$CHV_CONFIG_DIR/stord.toml" <<EOF
socket_path = "/run/chv/stord/api.sock"
runtime_dir = "${CHV_DATA_DIR}/storage/localdisk"
log_level = "info"
EOF
    chmod 640 "$CHV_CONFIG_DIR/stord.toml"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/stord.toml"

    cat > "$CHV_CONFIG_DIR/nwd.toml" <<EOF
socket_path = "/run/chv/nwd/api.sock"
runtime_dir = "/run/chv/nwd"
log_level = "info"
bridge_name = "${INSTALL_CHV_BRIDGE_NAME}"
bridge_cidr = "${INSTALL_CHV_BRIDGE_CIDR}"
upstream_iface = "${INSTALL_CHV_BRIDGE_IFACE}"
EOF
    chmod 640 "$CHV_CONFIG_DIR/nwd.toml"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/nwd.toml"
}

# -----------------------------------------------------------------------------
# Bootstrap Token (written to file; control plane reads it on startup)
# -----------------------------------------------------------------------------
create_bootstrap_token() {
    info "Creating bootstrap token..."

    BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
    printf '%s' "$BOOTSTRAP_TOKEN" > "$CHV_CONFIG_DIR/bootstrap.token"
    chmod 640 "$CHV_CONFIG_DIR/bootstrap.token"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/bootstrap.token"
}

# -----------------------------------------------------------------------------
# systemd Services
# -----------------------------------------------------------------------------
install_systemd_services() {
    info "Installing systemd services..."

    cat > /etc/systemd/system/chv-controlplane.service <<'EOF'
[Unit]
Description=CHV Control Plane
After=network.target

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

    cat > /etc/systemd/system/chv-stord.service <<'EOF'
[Unit]
Description=CHV Storage Daemon
After=network.target

[Service]
Type=simple
User=chv
Group=chv
ExecStartPre=/bin/mkdir -p /run/chv/stord
ExecStart=/usr/local/bin/chv-stord /etc/chv/stord.toml
Restart=on-failure
RestartSec=5
RuntimeDirectory=chv/stord
StateDirectory=chv
LogsDirectory=chv
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/run/chv/stord /var/lib/chv/storage

[Install]
WantedBy=multi-user.target
EOF

    cat > /etc/systemd/system/chv-nwd.service <<'EOF'
[Unit]
Description=CHV Network Daemon
After=network.target

[Service]
Type=simple
User=chv
Group=chv
ExecStartPre=/bin/mkdir -p /run/chv/nwd
ExecStart=/usr/local/bin/chv-nwd /etc/chv/nwd.toml
Restart=on-failure
RestartSec=5
RuntimeDirectory=chv/nwd
StateDirectory=chv
LogsDirectory=chv
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/run/chv/nwd

[Install]
WantedBy=multi-user.target
EOF

    cat > /etc/systemd/system/chv-agent.service <<'EOF'
[Unit]
Description=CHV Node Agent
After=network.target chv-controlplane.service chv-stord.service chv-nwd.service
Wants=chv-controlplane.service chv-stord.service chv-nwd.service

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
# nginx Setup (serves UI + proxies API)
# -----------------------------------------------------------------------------
install_nginx() {
    info "Configuring nginx..."

    cat > /etc/nginx/sites-available/chv <<'EOF'
server {
    listen 80;
    server_name _;

    root /opt/chv/ui;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location = /index.html {
        add_header Cache-Control "no-cache, no-store, must-revalidate";
    }

    location /_app/immutable/ {
        add_header Cache-Control "public, max-age=31536000, immutable";
    }

    location /api/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /v1/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml+rss text/javascript;
}
EOF

    rm -f /etc/nginx/sites-enabled/default
    ln -sf /etc/nginx/sites-available/chv /etc/nginx/sites-enabled/chv

    nginx -t || fatal "nginx configuration test failed"
    systemctl restart nginx
}

# -----------------------------------------------------------------------------
# Start Services and Wait for Enrollment
# -----------------------------------------------------------------------------
start_services() {
    info "Enabling and starting CHV services..."

    systemctl enable --now chv-controlplane
    systemctl enable --now chv-stord
    systemctl enable --now chv-nwd

    info "Waiting for control plane to apply database migrations (up to 30s)..."
    local attempt=1
    while [ $attempt -le 30 ]; do
        if [ -f "${CHV_DB_PATH}" ] && \
           ( /usr/local/bin/chv-controlplane --check-db "${CHV_DB_PATH}" 2>/dev/null \
             || ( cmd_exists sqlite3 && sqlite3 "${CHV_DB_PATH}" "SELECT 1 FROM bootstrap_tokens LIMIT 1;" &>/dev/null 2>&1 ) ); then
            info "Database ready."
            break
        fi
        sleep 1
        ((attempt++))
    done
    # Fallback: wait for HTTP health endpoint (doesn't require sqlite3 on host)
    attempt=1
    while [ $attempt -le 30 ]; do
        if curl -sf "http://127.0.0.1:8080/health" &>/dev/null; then
            info "Control plane API is up."
            break
        fi
        sleep 1
        ((attempt++))
    done
    if [ $attempt -gt 30 ]; then
        fatal "Control plane did not become healthy within 30s. Check: journalctl -u chv-controlplane -n 50"
    fi

    # Insert bootstrap token directly into the SQLite database
    info "Inserting bootstrap token into database..."
    local token_hash
    token_hash=$(printf '%s' "$BOOTSTRAP_TOKEN" | sha256sum | awk '{print $1}')
    local expires
    expires=$(date -u -d "+1 hour" '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null \
              || date -u -v+1H '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null \
              || echo "")
    if ! cmd_exists sqlite3; then
        fatal "sqlite3 is required to seed bootstrap tokens. Install sqlite3 or rerun without INSTALL_CHV_SKIP_DEPS=1."
    fi
    local expires_sql="NULL"
    if [ -n "$expires" ]; then
        expires_sql="'${expires}'"
    fi
    sqlite3 "${CHV_DB_PATH}" \
        "INSERT OR REPLACE INTO bootstrap_tokens
         (token_hash, description, one_time_use, used_at, expires_at, created_at, updated_at)
         VALUES ('${token_hash}', 'All-in-one installer', 1, NULL,
                 ${expires_sql},
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'));"

    local seeded_tokens
    seeded_tokens=$(sqlite3 "${CHV_DB_PATH}" \
        "SELECT COUNT(*) FROM bootstrap_tokens WHERE token_hash='${token_hash}';" 2>/dev/null || echo "0")
    if [ "${seeded_tokens}" -lt 1 ] 2>/dev/null; then
        fatal "Failed to seed bootstrap token in ${CHV_DB_PATH}."
    fi

    # Ensure agent cache is cleared so reinstall triggers fresh enrollment
    rm -f "${CHV_DATA_DIR}/cache/agent-cache.json"

    systemctl enable --now chv-agent

    info "Waiting for agent enrollment (up to 60s)..."
    attempt=1
    while [ $attempt -le 60 ]; do
        local node_count
        node_count=0
        if cmd_exists sqlite3; then
            node_count=$(sqlite3 "${CHV_DB_PATH}" \
                "SELECT COUNT(*) FROM nodes;" 2>/dev/null || echo "0")
        else
            # Fall back to HTTP API poll
            node_count=$(curl -sf "http://127.0.0.1:8080/v1/nodes" \
                -X POST -H "Content-Type: application/json" -d '{}' 2>/dev/null \
                | grep -o '"total_items":[0-9]*' | grep -o '[0-9]*' || echo "0")
        fi
        if [ "${node_count}" -gt 0 ] 2>/dev/null; then
            info "Node enrolled successfully."
            break
        fi
        sleep 1
        ((attempt++))
    done
    if [ $attempt -gt 60 ]; then
        warn "Node enrollment did not complete within 60s."
        warn "Check enrollment status: journalctl -u chv-agent -n 50"
    fi
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
download_base_image
generate_certs
setup_network
install_configs
create_bootstrap_token
install_systemd_services
install_nginx
start_services
import_base_image

LOCAL_IP=$(get_local_ip)

cat <<EOF

===============================================
  CHV All-in-One Installation Complete!
===============================================

Version:        ${INSTALL_CHV_VERSION}
Web UI:         http://${LOCAL_IP}/
API:            http://127.0.0.1:8080/
Database:       ${CHV_DB_PATH}
Bridge:         ${INSTALL_CHV_BRIDGE_NAME} (${INSTALL_CHV_BRIDGE_CIDR})
Upstream iface: ${INSTALL_CHV_BRIDGE_IFACE} (NAT enabled)

Services:
  systemctl status chv-controlplane
  systemctl status chv-agent
  systemctl status chv-stord
  systemctl status chv-nwd
  systemctl status nginx

Logs:
  journalctl -u chv-controlplane -f
  journalctl -u chv-agent -f

Defaults:
  Admin login:   admin / admin
  Bootstrap token valid for 1 hour from installation.

EOF
