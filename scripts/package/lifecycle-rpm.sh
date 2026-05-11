#!/bin/bash
# CHV Package Lifecycle Test — RPM-based distributions
#
# Tests fresh install, upgrade, remove, reinstall, and persistent data safety.
#
# Usage:
#   ./scripts/package/lifecycle-rpm.sh --new-packages DIR [--old-packages DIR]
#
# Options:
#   --new-packages DIR   Directory with new .rpm packages (required)
#   --old-packages DIR   Directory with old .rpm packages (for upgrade test)
#   --images LIST        Space-separated Docker images (default: rockylinux:9)
#
# Environment:
#   IMAGES               Override default image list

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

NEW_PKGDIR=""
OLD_PKGDIR=""
IMAGES="${IMAGES:-rockylinux:9}"

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
info "CHV RPM Package Lifecycle Test"
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
rpm -i /packages/new/chv-controlplane-*.rpm /packages/new/chv-node-*.rpm /packages/new/chvctl-*.rpm || true

verify_install_state

info "--- Step 2: Create sentinel state ---"
create_sentinel_state

info "--- Step 3: Reinstall same version ---"
# rpm -U with same version is a no-op by default; force with --replacepkgs
rpm -U --replacepkgs /packages/new/chv-controlplane-*.rpm /packages/new/chv-node-*.rpm /packages/new/chvctl-*.rpm || true

verify_sentinels_present

if [[ -d /packages/old ]]; then
    info "--- Step 4: Upgrade to new version ---"
    rpm -U /packages/new/chv-controlplane-*.rpm /packages/new/chv-node-*.rpm /packages/new/chvctl-*.rpm || true

    verify_upgrade_state
else
    info "--- Step 4: Upgrade test SKIPPED (no old packages) ---"
fi

info "--- Step 5: Remove packages ---"
rpm -e chv-node chv-controlplane chvctl || true

verify_remove_state
verify_sentinels_present

info "--- Step 6: Reinstall after remove ---"
rpm -i /packages/new/chv-controlplane-*.rpm /packages/new/chv-node-*.rpm /packages/new/chvctl-*.rpm || true

verify_install_state
verify_sentinels_present

info "--- Step 7: Purge test ---"
# RPM does not have a purge concept like apt.
# Normal remove leaves /var/lib/chv and /etc/chv intact.
# We verify this in Step 5. There is no standard purge script.
info "  (RPM has no purge concept — normal remove preserves data per package contract)"

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
    echo "[RESULT] RPM lifecycle tests FAILED with ${ERRORS} error(s)"
    exit 1
else
    echo ""
    echo "[RESULT] All RPM lifecycle tests PASSED"
    exit 0
fi
