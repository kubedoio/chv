#!/bin/bash
# Build a CHV release tarball for linux-amd64
# Usage: ./scripts/build-release.sh [VERSION]
# Output: dist/chv-<VERSION>-linux-amd64.tar.gz

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Read current version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)".*/\1/')

# Optional version bump
if [ $# -ge 1 ]; then
    NEW_VERSION="$1"
    echo "Bumping version: ${VERSION} -> ${NEW_VERSION}"
    sed -i "s/^version = \"${VERSION}\"/version = \"${NEW_VERSION}\"/" Cargo.toml
    cargo check --workspace
    git add Cargo.toml Cargo.lock
    git commit -m "release: bump version to ${NEW_VERSION}"
    VERSION="${NEW_VERSION}"
fi
ARCH="linux-amd64"
RELEASE_NAME="chv-${VERSION}-${ARCH}"
RELEASE_DIR="dist/${RELEASE_NAME}"
TARBALL="dist/${RELEASE_NAME}.tar.gz"

echo "==============================================="
echo "Building CHV Release"
echo "Version: ${VERSION}"
echo "Architecture: ${ARCH}"
echo "==============================================="

# -----------------------------------------------------------------------------
# 1. Clean previous build artifacts
# -----------------------------------------------------------------------------
echo "[1/6] Cleaning previous release artifacts..."
rm -rf "dist/"

# -----------------------------------------------------------------------------
# 2. Build Rust workspace
# -----------------------------------------------------------------------------
echo "[2/6] Building Rust binaries (release)..."
cargo build --workspace --release

# -----------------------------------------------------------------------------
# 3. Build Web UI
# -----------------------------------------------------------------------------
echo "[3/6] Building Web UI..."

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
# 4. Assemble release directory
# -----------------------------------------------------------------------------
echo "[4/6] Assembling release directory..."
mkdir -p "${RELEASE_DIR}/bin"
mkdir -p "${RELEASE_DIR}/ui"
mkdir -p "${RELEASE_DIR}/migrations"
mkdir -p "${RELEASE_DIR}/systemd"
mkdir -p "${RELEASE_DIR}/nginx"

cp target/release/chv-controlplane "${RELEASE_DIR}/bin/"
cp target/release/chv-agent       "${RELEASE_DIR}/bin/"
cp target/release/chv-stord       "${RELEASE_DIR}/bin/"
cp target/release/chv-nwd         "${RELEASE_DIR}/bin/"

cp -r ui/build/* "${RELEASE_DIR}/ui/"
cp -r cmd/chv-controlplane/migrations/* "${RELEASE_DIR}/migrations/"

cp docs/examples/systemd/chv-controlplane.service "${RELEASE_DIR}/systemd/"
cp docs/examples/systemd/chv-agent.service        "${RELEASE_DIR}/systemd/"
cp docs/examples/systemd/chv-stord.service        "${RELEASE_DIR}/systemd/"
cp docs/examples/systemd/chv-nwd.service          "${RELEASE_DIR}/systemd/"
cp docs/examples/nginx/chv-ui.conf               "${RELEASE_DIR}/nginx/"

cp docs/examples/controlplane.toml "${RELEASE_DIR}/controlplane.toml.example"
cp docs/examples/agent.toml        "${RELEASE_DIR}/agent.toml.example"
cp docs/examples/stord.toml        "${RELEASE_DIR}/stord.toml.example"
cp docs/examples/nwd.toml          "${RELEASE_DIR}/nwd.toml.example"
cp scripts/install.sh              "${RELEASE_DIR}/install.sh"

# -----------------------------------------------------------------------------
# 5. Create tarball
# -----------------------------------------------------------------------------
echo "[5/6] Creating tarball..."
tar -czf "${TARBALL}" -C dist "${RELEASE_NAME}"

# -----------------------------------------------------------------------------
# 6. Checksum
# -----------------------------------------------------------------------------
echo "[6/6] Generating checksum..."
cd dist
sha256sum "${RELEASE_NAME}.tar.gz" > "${RELEASE_NAME}.tar.gz.sha256"
cd "$PROJECT_ROOT"

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
