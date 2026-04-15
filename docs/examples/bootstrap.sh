#!/bin/bash
# CHV Combined Host Bootstrap Script
# Run as root on the target deployment server

set -euo pipefail

CHV_USER="chv"
CHV_CONFIG_DIR="/etc/chv"
CHV_DATA_DIR="/var/lib/chv"
CHV_RUN_DIR="/run/chv"
CHV_LOG_DIR="/var/log/chv"
CHV_UI_DIR="/opt/chv/ui"
CHV_MIGRATIONS_DIR="/usr/local/share/chv/migrations"

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
mkdir -p "$CHV_DATA_DIR"/{cache,images,storage}
mkdir -p "$CHV_LOG_DIR"
mkdir -p "$CHV_RUN_DIR"
mkdir -p "$CHV_UI_DIR"
mkdir -p "$CHV_MIGRATIONS_DIR"

chown -R "$CHV_USER:$CHV_USER" "$CHV_DATA_DIR" "$CHV_LOG_DIR" "$CHV_RUN_DIR"
chmod 750 "$CHV_DATA_DIR" "$CHV_LOG_DIR"

# -----------------------------------------------------------------------------
# 2. Install binaries
# -----------------------------------------------------------------------------
echo "[2/8] Installing binaries..."

cp "$REPO_DIR/target/release/chv-controlplane" /usr/local/bin/
cp "$REPO_DIR/target/release/chv-agent" /usr/local/bin/
cp "$REPO_DIR/target/release/chv-stord" /usr/local/bin/
cp "$REPO_DIR/target/release/chv-nwd" /usr/local/bin/
chmod 755 /usr/local/bin/chv-*

# Link cloud-hypervisor if present locally
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
# 4. Generate TLS CA and server certificate
# -----------------------------------------------------------------------------
echo "[4/8] Generating TLS certificates..."

chown root:"$CHV_USER" "$CHV_CONFIG_DIR"/certs
chmod 750 "$CHV_CONFIG_DIR"/certs

if [[ ! -f "$CHV_CONFIG_DIR/certs/ca.key" ]]; then
    openssl genrsa -out "$CHV_CONFIG_DIR/certs/ca.key" 4096
    openssl req -x509 -new -nodes -key "$CHV_CONFIG_DIR/certs/ca.key" \
        -sha256 -days 3650 -out "$CHV_CONFIG_DIR/certs/ca.crt" \
        -subj "/O=CHV/CN=chv-ca"
    chmod 640 "$CHV_CONFIG_DIR/certs/ca.key"
    chmod 644 "$CHV_CONFIG_DIR/certs/ca.crt"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/certs/ca.key" "$CHV_CONFIG_DIR/certs/ca.crt"
fi

if [[ ! -f "$CHV_CONFIG_DIR/certs/server.key" ]]; then
    openssl req -new -newkey rsa:4096 -nodes -keyout "$CHV_CONFIG_DIR/certs/server.key" \
        -out "$CHV_CONFIG_DIR/certs/server.csr" -subj "/O=CHV/CN=localhost"
    openssl x509 -req -in "$CHV_CONFIG_DIR/certs/server.csr" \
        -CA "$CHV_CONFIG_DIR/certs/ca.crt" -CAkey "$CHV_CONFIG_DIR/certs/ca.key" \
        -CAcreateserial -out "$CHV_CONFIG_DIR/certs/server.crt" -days 365 -sha256
    rm -f "$CHV_CONFIG_DIR/certs/server.csr"
    chmod 640 "$CHV_CONFIG_DIR/certs/server.key"
    chown root:"$CHV_USER" "$CHV_CONFIG_DIR/certs/server.key"
fi

# -----------------------------------------------------------------------------
# 5. Install configuration files
# -----------------------------------------------------------------------------
echo "[5/8] Installing configuration files..."

cp "$REPO_DIR/docs/examples/controlplane.toml" "$CHV_CONFIG_DIR/controlplane.toml"
cp "$REPO_DIR/docs/examples/agent.toml" "$CHV_CONFIG_DIR/agent.toml"

# Replace placeholder DB password with a random one
DB_PASSWORD=$(openssl rand -base64 32 | tr -d '=+/')
sed -i "s/change-me-strong-password/${DB_PASSWORD}/g" "$CHV_CONFIG_DIR/controlplane.toml"

# -----------------------------------------------------------------------------
# 6. Install systemd services
# -----------------------------------------------------------------------------
echo "[6/8] Installing systemd services..."

cp "$REPO_DIR/docs/examples/systemd/chv-controlplane.service" /etc/systemd/system/
cp "$REPO_DIR/docs/examples/systemd/chv-agent.service" /etc/systemd/system/
systemctl daemon-reload

# -----------------------------------------------------------------------------
# 7. Create bootstrap token
# -----------------------------------------------------------------------------
echo "[7/8] Creating bootstrap token..."

BOOTSTRAP_TOKEN=$(openssl rand -hex 32)
echo "$BOOTSTRAP_TOKEN" > "$CHV_CONFIG_DIR/bootstrap.token"
chmod 640 "$CHV_CONFIG_DIR/bootstrap.token"
chown root:"$CHV_USER" "$CHV_CONFIG_DIR/bootstrap.token"

echo ""
echo "Bootstrap token: $BOOTSTRAP_TOKEN"
echo "Token saved to:  $CHV_CONFIG_DIR/bootstrap.token"
echo ""

# -----------------------------------------------------------------------------
# 8. Summary
# -----------------------------------------------------------------------------
echo "[8/8] Bootstrap complete!"
echo ""
echo "Next steps:"
echo "  1. Ensure PostgreSQL is installed and running."
echo "     Create the database with:"
echo "       sudo -u postgres psql -c \"CREATE USER chv WITH PASSWORD '${DB_PASSWORD}';\""
echo "       sudo -u postgres psql -c \"CREATE DATABASE chv_controlplane OWNER chv;\""
echo ""
echo "  2. Insert the bootstrap token into the database:"
echo "       sudo -u postgres psql -d chv_controlplane -c \"CREATE EXTENSION IF NOT EXISTS pgcrypto;\""
echo "       sudo -u postgres psql -d chv_controlplane -c \"INSERT INTO bootstrap_tokens (token_hash, description, one_time_use, expires_at, created_at) VALUES (encode(digest('${BOOTSTRAP_TOKEN}', 'sha256'), 'hex'), 'Initial deployment', true, now() + interval '1 hour', now());\""
echo ""
echo "  3. Install nginx config and restart nginx:"
echo "       cp docs/examples/nginx/chv-ui.conf /etc/nginx/sites-available/chv"
echo "       ln -sf /etc/nginx/sites-available/chv /etc/nginx/sites-enabled/chv"
echo "       systemctl restart nginx"
echo ""
echo "  4. Start CHV services:"
echo "       systemctl enable --now chv-controlplane"
echo "       systemctl enable --now chv-agent"
echo ""
echo "  5. Verify deployment:"
echo "       curl http://127.0.0.1:8080/health"
echo "       curl http://127.0.0.1:8080/admin/nodes"
echo ""
