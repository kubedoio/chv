#!/bin/bash
# CHV Package Smoke Test — RPM-based distributions
# Installs .rpm packages in clean containers, verifies install/remove/reinstall.
#
# Usage:
#   ./scripts/package/smoke-rpm.sh [package-directory]
#
# Environment:
#   IMAGES — space-separated list of Docker images to test (default: rockylinux:9)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PKGDIR="${1:-${REPO_ROOT}/dist/packages}"
PKGDIR="$(cd "$PKGDIR" && pwd)"

IMAGES="${IMAGES:-rockylinux:9}"

ERRORS=0
error() { echo "[FAIL] $*" >&2; ERRORS=$((ERRORS + 1)); }
info() { echo "[INFO] $*"; }
pass() { echo "[PASS] $*"; }

info "Package directory: ${PKGDIR}"

for img in $IMAGES; do
    info "=========================================="
    info "Testing ${img}"
    info "=========================================="

    if ! docker info >/dev/null 2>&1; then
        error "Docker is not available. Install Docker to run smoke tests."
        ERRORS=$((ERRORS + 1))
        continue
    fi

    # Pull image if not present
    if ! docker image inspect "$img" >/dev/null 2>&1; then
        info "Pulling ${img}..."
        docker pull "$img" >/dev/null 2>&1 || {
            error "Failed to pull ${img}"
            ERRORS=$((ERRORS + 1))
            continue
        }
    fi

    # Run smoke test inside container
    if docker run --rm \
        -v "${PKGDIR}:/packages:ro" \
        -v "${SCRIPT_DIR}/smoke-common.sh:/smoke-common.sh:ro" \
        "$img" bash -c "
            set -euo pipefail
            source /smoke-common.sh

            info 'Installing packages...'
            # Install all CHV packages at once; rpm resolves inter-package dependencies
            rpm -ivh /packages/chv-controlplane-*.rpm /packages/chv-node-*.rpm /packages/chvctl-*.rpm

            verify_install_state rpm
            verify_version_output

            info 'Removing packages...'
            # Remove in reverse dependency order
            rpm -ev chv-node chv-controlplane chvctl || true

            verify_remove_state

            info 'Reinstalling packages...'
            rpm -ivh /packages/chv-controlplane-*.rpm /packages/chv-node-*.rpm /packages/chvctl-*.rpm

            verify_reinstall_state
            smoke_summary
        "; then
        pass "Smoke test passed for ${img}"
    else
        error "Smoke test failed for ${img}"
    fi
done

if [[ "$ERRORS" -gt 0 ]]; then
    echo ""
    echo "[RESULT] RPM smoke tests FAILED with ${ERRORS} error(s)"
    exit 1
else
    echo ""
    echo "[RESULT] All RPM smoke tests PASSED"
    exit 0
fi
