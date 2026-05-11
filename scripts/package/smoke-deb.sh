#!/bin/bash
# CHV Package Smoke Test — Debian/Ubuntu
# Installs .deb packages in clean containers, verifies install/remove/reinstall.
#
# Usage:
#   ./scripts/package/smoke-deb.sh [package-directory]
#
# Environment:
#   IMAGES — space-separated list of Docker images to test (default: debian:12 ubuntu:24.04)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PKGDIR="${1:-${REPO_ROOT}/dist/packages}"
PKGDIR="$(cd "$PKGDIR" && pwd)"

IMAGES="${IMAGES:-debian:12 ubuntu:24.04}"

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
            export DEBIAN_FRONTEND=noninteractive
            apt-get update -qq >/dev/null 2>&1
            apt-get install -y -qq procps >/dev/null 2>&1 || true

            # Install all CHV packages at once
            dpkg -i /packages/chv-controlplane_*.deb /packages/chv-node_*.deb /packages/chvctl_*.deb || true
            apt-get install -f -y -qq >/dev/null 2>&1

            verify_install_state deb
            verify_version_output

            info 'Removing packages...'
            dpkg -r chv-node chv-controlplane chvctl || true
            apt-get autoremove -y -qq >/dev/null 2>&1 || true

            verify_remove_state

            info 'Reinstalling packages...'
            dpkg -i /packages/chv-controlplane_*.deb /packages/chv-node_*.deb /packages/chvctl_*.deb || true
            apt-get install -f -y -qq >/dev/null 2>&1

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
    echo "[RESULT] Debian smoke tests FAILED with ${ERRORS} error(s)"
    exit 1
else
    echo ""
    echo "[RESULT] All Debian smoke tests PASSED"
    exit 0
fi
