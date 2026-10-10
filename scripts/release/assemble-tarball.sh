#!/bin/bash
# Assemble the CHV release tarball from prebuilt artifacts.
#
# Single source of truth for the tarball's layout and naming — used by both
# scripts/build-release.sh (local builds) and .github/workflows/release.yml
# (stable/RC releases). The workflow previously inlined its own assembly,
# which drifted from build-release.sh (missing tmpfiles/, no checksum) and
# would have published a tarball that install.sh rejects (#440).
#
# Usage: ./scripts/release/assemble-tarball.sh <version>
#   <version>  version WITHOUT the leading 'v' (e.g. 0.2.0, 0.2.0-rc.1)
#
# Requires (must already be built):
#   target/release/{chv-controlplane,chv-agent,chv-stord,chv-nwd,chvctl}
#   ui/build/
#
# Output:
#   dist/chv-<version>-linux-amd64.tar.gz
#   dist/chv-<version>-linux-amd64.tar.gz.sha256
#
# The tarball name matches the URL install.sh constructs:
#   releases/download/v<version>/chv-<version>-linux-amd64.tar.gz

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_ROOT"

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <version-without-leading-v>" >&2
    exit 1
fi

VERSION="$1"
ARCH="linux-amd64"
RELEASE_NAME="chv-${VERSION}-${ARCH}"
RELEASE_DIR="dist/${RELEASE_NAME}"
TARBALL="dist/${RELEASE_NAME}.tar.gz"

# Fail closed on missing build inputs — a tarball assembled from stale or
# partial artifacts must never reach a release.
for bin in chv-controlplane chv-agent chv-stord chv-nwd chvctl; do
    if [ ! -f "target/release/${bin}" ]; then
        echo "ERROR: target/release/${bin} not found — run 'cargo build --workspace --release' first" >&2
        exit 1
    fi
done
if [ ! -d ui/build ]; then
    echo "ERROR: ui/build/ not found — build the Web UI first (cd ui && npm install && npm run build)" >&2
    exit 1
fi

# Idempotent: remove only this version's previous output, not all of dist/
# (the release workflow's package job also writes into dist/).
rm -rf "${RELEASE_DIR}" "${TARBALL}" "${TARBALL}.sha256"

echo "Assembling ${TARBALL}..."

mkdir -p "${RELEASE_DIR}/bin"
mkdir -p "${RELEASE_DIR}/ui"
mkdir -p "${RELEASE_DIR}/migrations"
mkdir -p "${RELEASE_DIR}/systemd"
mkdir -p "${RELEASE_DIR}/nginx"

cp target/release/chv-controlplane "${RELEASE_DIR}/bin/"
cp target/release/chv-agent       "${RELEASE_DIR}/bin/"
cp target/release/chv-stord       "${RELEASE_DIR}/bin/"
cp target/release/chv-nwd         "${RELEASE_DIR}/bin/"
cp target/release/chvctl          "${RELEASE_DIR}/bin/"

cp -r ui/build/* "${RELEASE_DIR}/ui/"
cp -r cmd/chv-controlplane/migrations/* "${RELEASE_DIR}/migrations/"
# The monitoring store's own migrations (ADR-027, #602): the separate
# disposable monitoring.db schema, applied to /usr/local/share/chv/
# monitoring-migrations by install.sh. The control-plane binary also
# embeds this set as a fallback, but the packaged tree stays the
# operator-inspectable source of truth.
mkdir -p "${RELEASE_DIR}/monitoring-migrations"
cp -r cmd/chv-controlplane/monitoring-migrations/* "${RELEASE_DIR}/monitoring-migrations/"

cp docs/examples/systemd/chv-controlplane.service "${RELEASE_DIR}/systemd/"
cp docs/examples/systemd/chv-agent.service        "${RELEASE_DIR}/systemd/"
cp docs/examples/systemd/chv-stord.service        "${RELEASE_DIR}/systemd/"
cp docs/examples/systemd/chv-nwd.service          "${RELEASE_DIR}/systemd/"
# Same tmpfiles entry the .deb ships (packaging/nfpm/chv-node.yaml); the
# tarball's install.sh installs it so the unit-boot path gets /run/netns.
# install.sh fails closed without it — do not ship a tarball missing this.
mkdir -p "${RELEASE_DIR}/tmpfiles"
cp packaging/tmpfiles/chv-node.conf               "${RELEASE_DIR}/tmpfiles/"
cp docs/examples/nginx/chv-ui.conf               "${RELEASE_DIR}/nginx/"

cp docs/examples/controlplane.toml "${RELEASE_DIR}/controlplane.toml.example"
cp docs/examples/agent.toml        "${RELEASE_DIR}/agent.toml.example"
cp docs/examples/stord.toml        "${RELEASE_DIR}/stord.toml.example"
cp docs/examples/nwd.toml          "${RELEASE_DIR}/nwd.toml.example"
cp docs/examples/compat.toml       "${RELEASE_DIR}/compat.toml"
cp scripts/install.sh              "${RELEASE_DIR}/install.sh"

tar -czf "${TARBALL}" -C dist "${RELEASE_NAME}"

(
    cd dist
    sha256sum "${RELEASE_NAME}.tar.gz" > "${RELEASE_NAME}.tar.gz.sha256"
)

echo "Tarball:  ${TARBALL}"
echo "Size:     $(du -h "${TARBALL}" | cut -f1)"
echo "Checksum: ${TARBALL}.sha256"
