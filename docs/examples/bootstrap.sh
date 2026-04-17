#!/bin/bash
# CHV Combined Host Bootstrap Script
# Manual bootstrap helper for building from source on the target host.
# For automated all-in-one deployment, use scripts/install.sh instead.
#
# Run as root on the target deployment server:
#   sudo ./docs/examples/bootstrap.sh <path-to-chv-repo>

set -euo pipefail

CHV_USER="chv"
CHV_CONFIG_DIR="/etc/chv"
CHV_DATA_DIR="/var/lib/chv"
CHV_RUN_DIR="/run/chv"
CHV_LOG_DIR="/var/log/chv"
CHV_UI_DIR="/opt/chv/ui"
CHV_MIGRATIONS_DIR="/usr/local/share/chv/migrations"
CHV_DB_PATH="${CHV_DATA_DIR}/controlplane.db"

# Bridge/network defaults (match installer defaults)
BRIDGE_NAME="${INSTALL_CHV_BRIDGE_NAME:-chvbr0}"
BRIDGE_CIDR="${INSTALL_CHV_BRIDGE_CIDR:-10.200.0.1/24}"
BRIDGE_IFACE="${INSTALL_CHV_BRIDGE_IFACE:-ens19}"

REPO_DIR="${1:-}"

if [[ -z "$REPO_DIR" ]]; then
    echo "Usage: $0 <path-to-chv-repo>"
    exit 1
fi

echo "=== CHV Combined Host Bootstrap ==="

# -----------------------------------------------------------------------------
# 1. Create user and directories
# -----------------------------------------------------------------------------
echo "[1/8] Creating user and directories..."

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

# -----------------------------------------------------------------------------
# 2. Install binaries
# -----------------------------------------------------------------------------
echo "[2/8] Installing binaries..."

install -m 755 "$REPO_DIR/target/release/chv-controlplane" /usr/local/bin/
install -m 755 "$REPO_DIR/target/release/chv-agent"        /usr/local/bin/
install -m 755 "$REPO_DIR/target/release/chv-stord"        /usr/local/bin/
install -m 755 "$REPO_DIR/target/release/chv-nwd"          /usr/local/bin/

if [[ -f /usr/local/bin/cloud-hypervisor ]] && [[ ! -f /usr/bin/cloud-hypervisor ]]; then
    ln -sf /usr/local/bin/cloud-hypervisor /usr/bin/cloud-hypervisor
fi

# -----------------------------------------------------------------------------
# 3. Copy UI assets and migrations
# -----------------------------------------------------------------------------
echo "[3/8] Copying UI assets and migrations..."

cp -r "$REPO_DIR/ui/build/"* "$CHV_UI_DIR/"
chown -R www-data:www-data "$CHV_UI_DIR"

cp -r "$REPO_DIR/cmd/chv-controlplane/migrations/"* "$CHV_MIGRATIONS_DIR/"
chown -R "$CHV_USER:$CHV_USER" "$CHV_MIGRATIONS_DIR"

# -----------------------------------------------------------------------------
# 4. Generate TLS CA
# -----------------------------------------------------------------------------
echo "[4/8] Generating TLS certificates..."

chown root:"$CHV_USER" "$CHV_CONFIG_DIR"/certs
chmod 750 "$CHV_CONFIG_DIR"/certs

if [[ ! -f "$CHV_CONFIG_DIR/certs/ca.key" ]]; then
    openssl genrsa -out "$CHV_CONFIG_DIR/certs/ca.key" 4096 2>/dev/null
    openssl req -x509 -new -nodes -key "$CHV_CONFIG_DIR/certs/ca.key" \
        -sha256 -days 3650 -out "$CHV_CONFIG_DIR/certs/ca.crt" \
        -subj "/O=CHV/CN=chv-ca" 2>/dev/null
    chmod 640 "$CHV_CONFIG_DIR/certs/ca.key"
    chmod 644 "$CHV_CONFIG_DIR/certs/ca.crt"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/certs/ca.key" "$CHV_CONFIG_DIR/certs/ca.crt"
fi

# -----------------------------------------------------------------------------
# 5. Install configuration files
# -----------------------------------------------------------------------------
echo "[5/8] Installing configuration files..."

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

cp "$REPO_DIR/docs/examples/agent.toml"  "$CHV_CONFIG_DIR/agent.toml"
cp "$REPO_DIR/docs/examples/stord.toml"  "$CHV_CONFIG_DIR/stord.toml"

cat > "$CHV_CONFIG_DIR/nwd.toml" <<EOF
socket_path = "/run/chv/nwd/api.sock"
runtime_dir = "/run/chv/nwd"
log_level = "info"
bridge_name = "${BRIDGE_NAME}"
bridge_cidr = "${BRIDGE_CIDR}"
upstream_iface = "${BRIDGE_IFACE}"
EOF

chmod 640 "$CHV_CONFIG_DIR/agent.toml" "$CHV_CONFIG_DIR/stord.toml" "$CHV_CONFIG_DIR/nwd.toml"
chown root:"$CHV_USER" "$CHV_CONFIG_DIR/agent.toml" "$CHV_CONFIG_DIR/stord.toml" "$CHV_CONFIG_DIR/nwd.toml"

# -----------------------------------------------------------------------------
# 6. Install systemd services
# -----------------------------------------------------------------------------
echo "[6/8] Installing systemd services..."

cp "$REPO_DIR/docs/examples/systemd/chv-controlplane.service" /etc/systemd/system/
cp "$REPO_DIR/docs/examples/systemd/chv-agent.service"        /etc/systemd/system/
cp "$REPO_DIR/docs/examples/systemd/chv-stord.service"        /etc/systemd/system/
cp "$REPO_DIR/docs/examples/systemd/chv-nwd.service"          /etc/systemd/system/
systemctl daemon-reload

# -----------------------------------------------------------------------------
# 7. Create bootstrap token
# -----------------------------------------------------------------------------
echo "[7/8] Creating bootstrap token..."

BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
printf '%s' "$BOOTSTRAP_TOKEN" > "$CHV_CONFIG_DIR/bootstrap.token"
chmod 640 "$CHV_CONFIG_DIR/bootstrap.token"
chown root:"$CHV_USER" "$CHV_CONFIG_DIR/bootstrap.token"

echo ""
echo "Bootstrap token: $BOOTSTRAP_TOKEN"
echo "Token saved to:  $CHV_CONFIG_DIR/bootstrap.token"
echo "(The agent reads this file automatically at startup.)"
echo ""

# -----------------------------------------------------------------------------
# 8. Summary
# -----------------------------------------------------------------------------
echo "[8/8] Bootstrap complete!"
echo ""
echo "Database: SQLite at ${CHV_DB_PATH}"
echo "  (created automatically by chv-controlplane on first start)"
echo ""
echo "Next steps:"
echo "  1. Start services:"
echo "       systemctl enable --now chv-controlplane chv-stord chv-nwd"
echo "       systemctl enable --now chv-agent"
echo ""
echo "  2. Install nginx and configure proxy:"
echo "       cp docs/examples/nginx/chv-ui.conf /etc/nginx/sites-available/chv"
echo "       ln -sf /etc/nginx/sites-available/chv /etc/nginx/sites-enabled/chv"
echo "       systemctl restart nginx"
echo ""
echo "  3. Verify deployment:"
echo "       curl http://127.0.0.1:8080/health"
echo ""
echo "  For a fully automated install, run: sudo ./scripts/install.sh"
echo ""
