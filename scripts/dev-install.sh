#!/bin/bash
# Dev Install: build from source and install ALL-IN-ONE on the local machine
# Usage: sudo ./scripts/dev-install.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==============================================="
echo "CHV Local Dev Install (All-in-One)"
echo "==============================================="
echo ""

if [ "$(id -u)" -ne 0 ]; then
    echo "This script must be run as root. Try:"
    echo "  sudo ./scripts/dev-install.sh"
    exit 1
fi

cd "$PROJECT_ROOT"

# Step 1: build release tarball
echo "[1/2] Building release tarball..."
./scripts/build-release.sh

VERSION=$(cat VERSION | tr -d '[:space:]')
TARBALL="dist/chv-${VERSION}-linux-amd64.tar.gz"

# Step 2: run installer with local tarball
echo "[2/2] Running installer with local tarball..."
export INSTALL_CHV_TARBALL_PATH="$TARBALL"
export INSTALL_CHV_VERSION="$VERSION"
"$PROJECT_ROOT/scripts/install.sh"

echo ""
echo "Dev install complete!"
