#!/bin/bash
# Build a CHV release tarball for linux-amd64
# Usage: ./scripts/build-release.sh
# Output: dist/chv-<VERSION>-linux-amd64.tar.gz
#
# To bump the version before building, run:
#   ./scripts/bump-version.sh [major|minor|patch]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

if ! command -v cargo &>/dev/null; then
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    elif [ -d "/root/.cargo/bin" ]; then
        export PATH="/root/.cargo/bin:$PATH"
    fi
fi

CHANNEL="${CHV_RELEASE_CHANNEL:-stable}"

# Derive extra args for version.sh based on channel
EXTRA_ARGS=""
case "$CHANNEL" in
    rc)
        EXTRA_ARGS="${CHV_RC_NUMBER:-1}"
        ;;
    pr)
        EXTRA_ARGS="${CHV_PR_NUMBER:-0}"
        ;;
esac

VERSION=$("${PROJECT_ROOT}/scripts/version.sh" "${CHANNEL}" ${EXTRA_ARGS})
ARCH="linux-amd64"
RELEASE_NAME="chv-${VERSION}-${ARCH}"
TARBALL="dist/${RELEASE_NAME}.tar.gz"

echo "==============================================="
echo "Building CHV Release"
echo "Version: ${VERSION}"
echo "Architecture: ${ARCH}"
echo "==============================================="

# -----------------------------------------------------------------------------
# 1. Clean previous build artifacts
# -----------------------------------------------------------------------------
echo "[1/4] Cleaning previous release artifacts..."
rm -rf "dist/"

# -----------------------------------------------------------------------------
# 2. Build Rust workspace
# -----------------------------------------------------------------------------
echo "[2/4] Building Rust binaries (release)..."
cargo build --workspace --release

# -----------------------------------------------------------------------------
# 3. Build Web UI
# -----------------------------------------------------------------------------
echo "[3/4] Building Web UI..."

# Ensure npm is available (nvm installs may not be on PATH when running via sudo)
if ! command -v npm &>/dev/null && [ -d "$HOME/.nvm/versions/node" ]; then
    NODE_BIN_DIR=$(find "$HOME/.nvm/versions/node" -maxdepth 1 -type d | sort -V | tail -n 1)/bin
    export PATH="$NODE_BIN_DIR:$PATH"
fi

cd ui
npm install
npm run build
cd "$PROJECT_ROOT"

# -----------------------------------------------------------------------------
# 4. Assemble release tarball + checksum
# -----------------------------------------------------------------------------
# Shared with .github/workflows/release.yml so the workflow-published tarball
# cannot drift from the local one (single source of truth; see #440).
echo "[4/4] Assembling release tarball..."
"${PROJECT_ROOT}/scripts/release/assemble-tarball.sh" "${VERSION}"

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------
echo ""
echo "==============================================="
echo "Release build complete!"
echo "==============================================="
echo "Tarball: ${TARBALL}"
echo "Size:    $(du -h "${TARBALL}" | cut -f1)"
echo "SHA256:  $(cat "dist/${RELEASE_NAME}.tar.gz.sha256" | awk '{print $1}')"
echo ""
echo "Test locally with:"
echo "  INSTALL_CHV_TARBALL_PATH=${TARBALL} sudo ./scripts/install.sh"
echo ""
