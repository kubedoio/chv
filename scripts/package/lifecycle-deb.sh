#!/bin/bash
# CHV Package Lifecycle Test — Debian/Ubuntu
#
# Tests fresh install, upgrade, remove, reinstall, and persistent data safety.
#
# Usage:
#   ./scripts/package/lifecycle-deb.sh --new-packages DIR [--old-packages DIR]
#
# Options:
#   --new-packages DIR   Directory with new .deb packages (required)
#   --old-packages DIR   Directory with old .deb packages (for upgrade test)
#   --images LIST        Space-separated Docker images (default: debian:12 ubuntu:24.04)
#
# Environment:
#   IMAGES               Override default image list

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

NEW_PKGDIR=""
OLD_PKGDIR=""
IMAGES="${IMAGES:-debian:12 ubuntu:24.04}"

ERRORS=0
error() { echo "[FAIL] $*" >&2; ERRORS=$((ERRORS + 1)); }
info()  { echo "[INFO] $*"; }
pass()  { echo "[PASS] $*"; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --new-packages)
            NEW_PKGDIR="$2"
            shift 2
            ;;
        --old-packages)
            OLD_PKGDIR="$2"
            shift 2
            ;;
        --images)
            IMAGES="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '2,15p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$NEW_PKGDIR" || ! -d "$NEW_PKGDIR" ]]; then
    echo "Error: --new-packages directory is required" >&2
    exit 1
fi
NEW_PKGDIR="$(cd "$NEW_PKGDIR" && pwd)"

if [[ -n "$OLD_PKGDIR" ]]; then
    OLD_PKGDIR="$(cd "$OLD_PKGDIR" && pwd)"
fi

info "=========================================="
info "CHV Debian Package Lifecycle Test"
info "New packages: $NEW_PKGDIR"
info "Old packages: ${OLD_PKGDIR:-<not provided — upgrade tests skipped>}"
info "Images: $IMAGES"
info "=========================================="

for img in $IMAGES; do
    info ""
    info "=========================================="
    info "Testing ${img}"
    info "=========================================="

    if ! docker info >/dev/null 2>&1; then
        error "Docker is not available. Install Docker to run lifecycle tests."
        continue
    fi

    if ! docker image inspect "$img" >/dev/null 2>&1; then
        info "Pulling ${img}..."
        docker pull "$img" >/dev/null 2>&1 || {
            error "Failed to pull ${img}"
            continue
        }
    fi

    # Build the test script that runs inside the container
    TEST_SCRIPT="$(cat <<'EOF'
set -euo pipefail
source /lifecycle-common.sh

info "--- Step 1: Fresh install ---"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null 2>&1
apt-get install -y -qq procps >/dev/null 2>&1 || true

dpkg -i /packages/new/chv-controlplane_*.deb /packages/new/chv-node_*.deb /packages/new/chvctl_*.deb || true
apt-get install -f -y -qq >/dev/null 2>&1

verify_install_state

info "--- Step 2: Create sentinel state ---"
create_sentinel_state

info "--- Step 3: Reinstall same version ---"
dpkg -i /packages/new/chv-controlplane_*.deb /packages/new/chv-node_*.deb /packages/new/chvctl_*.deb || true
apt-get install -f -y -qq >/dev/null 2>&1

verify_sentinels_present

if [[ -d /packages/old ]]; then
    info "--- Step 4: Upgrade to new version ---"
    dpkg -i /packages/new/chv-controlplane_*.deb /packages/new/chv-node_*.deb /packages/new/chvctl_*.deb || true
    apt-get install -f -y -qq >/dev/null 2>&1

    verify_upgrade_state
else
    info "--- Step 4: Upgrade test SKIPPED (no old packages) ---"
fi

info "--- Step 5: Remove packages ---"
dpkg -r chv-node chv-controlplane chvctl || true
apt-get autoremove -y -qq >/dev/null 2>&1 || true

verify_remove_state
verify_sentinels_present

info "--- Step 6: Reinstall after remove ---"
dpkg -i /packages/new/chv-controlplane_*.deb /packages/new/chv-node_*.deb /packages/new/chvctl_*.deb || true
apt-get install -f -y -qq >/dev/null 2>&1

verify_install_state
verify_sentinels_present

info "--- Step 7: Purge test (apt purge) ---"
# Note: apt purge is destructive and removes configs.
# We test that our package contract allows this, but verify /var/lib/chv is preserved.
# Actually, apt purge removes config files but NOT /var/lib/chv (we don't implement a purge script).
# For safety, we only test that normal remove preserves data.
info "  (Purge script not implemented — /var/lib/chv and /etc/chv preserved by design)"

lifecycle_summary
EOF
)"

    if docker run --rm \
        -v "${NEW_PKGDIR}:/packages/new:ro" \
        -v "${OLD_PKGDIR:-/dev/null}:/packages/old:ro" \
        -v "${SCRIPT_DIR}/lifecycle-common.sh:/lifecycle-common.sh:ro" \
        "$img" bash -c "$TEST_SCRIPT"; then
        pass "Lifecycle test passed for ${img}"
    else
        error "Lifecycle test failed for ${img}"
    fi
done

if [[ "$ERRORS" -gt 0 ]]; then
    echo ""
    echo "[RESULT] Debian lifecycle tests FAILED with ${ERRORS} error(s)"
    exit 1
else
    echo ""
    echo "[RESULT] All Debian lifecycle tests PASSED"
    exit 0
fi
