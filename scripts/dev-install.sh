#!/bin/bash
# Dev Install: build from source, uninstall any previous installation, then
# install all-in-one on the local machine with predefined dev defaults.
#
# After install, a default network and a test VM are automatically created:
#   Network: default (CIDR derived from INSTALL_CHV_BRIDGE_CIDR)
#   VM:      test-1 (1 CPU, 512 MB RAM, 10 GB disk)
#
# Usage: sudo ./scripts/dev-install.sh [--no-uninstall] [--no-seed]
#
# Predefined dev defaults (override via environment):
#   INSTALL_CHV_BRIDGE_IFACE  - default: ens19
#   INSTALL_CHV_BRIDGE_NAME   - default: chvbr0
#   INSTALL_CHV_BRIDGE_CIDR   - default: 10.200.0.1/24
#
# Options:
#   --no-uninstall   Skip the uninstall step (useful for first install)
#   --no-seed        Skip creating the default network and test-1 VM

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

NO_UNINSTALL=0
NO_SEED=0
for arg in "$@"; do
    case "$arg" in
        --no-uninstall) NO_UNINSTALL=1 ;;
        --no-seed) NO_SEED=1 ;;
        *) echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

# Dev network defaults
export INSTALL_CHV_BRIDGE_IFACE="${INSTALL_CHV_BRIDGE_IFACE:-ens19}"
export INSTALL_CHV_BRIDGE_NAME="${INSTALL_CHV_BRIDGE_NAME:-chvbr0}"
export INSTALL_CHV_BRIDGE_CIDR="${INSTALL_CHV_BRIDGE_CIDR:-10.200.0.1/24}"

echo "==============================================="
echo "CHV Local Dev Install (All-in-One)"
echo "Bridge: ${INSTALL_CHV_BRIDGE_NAME} (${INSTALL_CHV_BRIDGE_CIDR}) on ${INSTALL_CHV_BRIDGE_IFACE}"
if [ "$NO_SEED" = "0" ]; then
    echo "Seed:   default network + test-1 VM (1 CPU, 512 MB)"
else
    echo "Seed:   skipped (--no-seed)"
fi
echo "==============================================="
echo ""

if [ "$(id -u)" -ne 0 ]; then
    echo "This script must be run as root. Try:"
    echo "  sudo ./scripts/dev-install.sh"
    exit 1
fi

BUILD_USER="${SUDO_USER:-}"
if [ -z "$BUILD_USER" ] || ! id -u "$BUILD_USER" &>/dev/null; then
    BUILD_USER="root"
fi
if [ "$BUILD_USER" != "root" ]; then
    if ! sudo -u "$BUILD_USER" test -r "$PROJECT_ROOT" \
        || ! sudo -u "$BUILD_USER" test -w "$PROJECT_ROOT" \
        || ! sudo -u "$BUILD_USER" test -x "$PROJECT_ROOT"; then
        echo "  [WARN] $BUILD_USER cannot access $PROJECT_ROOT; falling back to root build."
        BUILD_USER="root"
    fi
fi

# -----------------------------------------------------------------------------
# Step 1: Uninstall previous installation
# -----------------------------------------------------------------------------
if [ "$NO_UNINSTALL" = "0" ]; then
    echo "[1/4] Removing previous CHV installation..."

    # Stop and disable all CHV services
    for svc in chv-agent chv-stord chv-nwd chv-controlplane; do
        if systemctl is-active --quiet "$svc" 2>/dev/null; then
            echo "  Stopping $svc..."
            systemctl stop "$svc" || true
        fi
        if systemctl is-enabled --quiet "$svc" 2>/dev/null; then
            systemctl disable "$svc" || true
        fi
    done

    # Remove systemd unit files
    rm -f /etc/systemd/system/chv-controlplane.service
    rm -f /etc/systemd/system/chv-agent.service
    rm -f /etc/systemd/system/chv-stord.service
    rm -f /etc/systemd/system/chv-nwd.service
    systemctl daemon-reload

    # Remove binaries
    rm -f /usr/local/bin/chv-controlplane
    rm -f /usr/local/bin/chv-agent
    rm -f /usr/local/bin/chv-stord
    rm -f /usr/local/bin/chv-nwd

    # Remove config (keep certs to avoid re-generating if same host)
    rm -f /etc/chv/controlplane.toml
    rm -f /etc/chv/agent.toml
    rm -f /etc/chv/stord.toml
    rm -f /etc/chv/nwd.toml
    rm -f /etc/chv/bootstrap.token

    # Remove database (clean state for new install)
    rm -f /var/lib/chv/controlplane.db

    # Remove agent cache so re-install triggers fresh enrollment
    rm -f /var/lib/chv/cache/agent-cache.json

    # Remove UI assets and migrations
    rm -rf /opt/chv/ui/*
    rm -rf /usr/local/share/chv/migrations/*

    # Remove nginx site (will be reinstalled)
    rm -f /etc/nginx/sites-enabled/chv
    rm -f /etc/nginx/sites-available/chv

    # Tear down bridge (will be recreated)
    if ip link show "${INSTALL_CHV_BRIDGE_NAME}" &>/dev/null; then
        echo "  Removing bridge ${INSTALL_CHV_BRIDGE_NAME}..."
        ip link set "${INSTALL_CHV_BRIDGE_NAME}" down || true
        ip link delete "${INSTALL_CHV_BRIDGE_NAME}" type bridge 2>/dev/null || true
    fi

    # Remove network persistence files
    rm -f "/etc/network/interfaces.d/${INSTALL_CHV_BRIDGE_NAME}"
    rm -f "/etc/systemd/network/10-${INSTALL_CHV_BRIDGE_NAME}.netdev"
    rm -f "/etc/systemd/network/11-${INSTALL_CHV_BRIDGE_NAME}.network"
    rm -f "/etc/systemd/network/12-${INSTALL_CHV_BRIDGE_IFACE}.network"

    echo "  Previous installation removed."
else
    echo "[1/4] Skipping uninstall (--no-uninstall)"
fi

# -----------------------------------------------------------------------------
# Step 2: Build release tarball from source
# -----------------------------------------------------------------------------
echo "[2/4] Building release tarball from source..."
cd "$PROJECT_ROOT"
if [ "$BUILD_USER" = "root" ]; then
    ./scripts/build-release.sh
else
    echo "  Building as user: $BUILD_USER (preserves incremental cargo/npm caches)"
    sudo -u "$BUILD_USER" -H /bin/bash -lc "cd \"$PROJECT_ROOT\" && ./scripts/build-release.sh"
fi

# -----------------------------------------------------------------------------
# Step 3: Determine version and tarball path
# -----------------------------------------------------------------------------
VERSION=$(cat VERSION | tr -d '[:space:]')
TARBALL="dist/chv-${VERSION}-linux-amd64.tar.gz"

echo "[3/4] Built tarball: $TARBALL"

# -----------------------------------------------------------------------------
# Step 4: Run installer with local tarball
# -----------------------------------------------------------------------------
echo "[4/4] Running installer..."
export INSTALL_CHV_TARBALL_PATH="$(realpath "$TARBALL")"
export INSTALL_CHV_VERSION="$VERSION"
export INSTALL_CHV_NO_SEED="$NO_SEED"

"$PROJECT_ROOT/scripts/install.sh"

echo ""
echo "Dev install complete! Version: $VERSION"
if [ "$NO_SEED" = "0" ]; then
    echo ""
    echo "Seeded resources:"
    echo "  Network: default (${INSTALL_CHV_BRIDGE_CIDR%/*} network)"
    echo "  VM:      test-1 (1 CPU, 512 MB RAM, 10 GB disk)"
fi
