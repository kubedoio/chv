#!/bin/bash
# CHV Packaging Smoke Test
# Checks that release artifacts, packaging configs, and generated packages
# contain the expected files.
#
# Usage:
#   ./scripts/smoke-packages.sh
#
# Returns:
#   0 on success, 1 on failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

RELEASE_DIR="${PROJECT_ROOT}/target/release"
UI_BUILD_DIR="${PROJECT_ROOT}/ui/build"
PACKAGING_DIR="${PROJECT_ROOT}/packaging"
NFPM_CONFIG="${PACKAGING_DIR}/nfpm.yaml"

ERRORS=0

error() {
    echo "[FAIL] $*" >&2
    ERRORS=$((ERRORS + 1))
}

info() {
    echo "[INFO] $*"
}

# ---------------------------------------------------------------------------
# 1. Check release binaries
# ---------------------------------------------------------------------------
info "Checking release binaries in ${RELEASE_DIR}..."

for binary in chvctl chv-agent chv-controlplane chv-nwd chv-stord; do
    if [[ -x "${RELEASE_DIR}/${binary}" ]]; then
        info "  Found: ${binary}"
    else
        error "Missing or not executable: ${RELEASE_DIR}/${binary}"
    fi
done

# ---------------------------------------------------------------------------
# 2. Check UI build
# ---------------------------------------------------------------------------
info "Checking UI build in ${UI_BUILD_DIR}..."

if [[ -d "${UI_BUILD_DIR}" ]] && [[ -f "${UI_BUILD_DIR}/index.html" ]]; then
    info "  UI build present"
else
    error "UI build missing or incomplete (expected ${UI_BUILD_DIR}/index.html)"
fi

# ---------------------------------------------------------------------------
# 3. Check packaging configs
# ---------------------------------------------------------------------------
info "Checking packaging configs in ${PACKAGING_DIR}..."

nfpm_count=$(find "${PACKAGING_DIR}/nfpm" -maxdepth 1 -name '*.yaml' | wc -l)
if [[ "${nfpm_count}" -gt 0 ]]; then
    info "  Found: ${nfpm_count} nfpm package definition(s)"
else
    error "No nfpm .yaml files found in packaging/nfpm/"
fi

if [[ -d "${PACKAGING_DIR}/systemd" ]]; then
    svc_count=$(find "${PACKAGING_DIR}/systemd" -maxdepth 1 -name '*.service' | wc -l)
    if [[ "${svc_count}" -gt 0 ]]; then
        info "  Found: ${svc_count} systemd unit(s)"
    else
        error "No systemd .service files found in packaging/systemd/"
    fi
else
    error "Missing directory: packaging/systemd/"
fi

if [[ -d "${PACKAGING_DIR}/scripts" ]]; then
    script_count=$(find "${PACKAGING_DIR}/scripts" -maxdepth 1 -type f | wc -l)
    if [[ "${script_count}" -gt 0 ]]; then
        info "  Found: ${script_count} packaging script(s)"
    else
        error "No scripts found in packaging/scripts/"
    fi
else
    error "Missing directory: packaging/scripts/"
fi

if [[ -x "${PROJECT_ROOT}/scripts/build-packages.sh" ]]; then
    info "  Found: scripts/build-packages.sh"
else
    error "Missing or not executable: scripts/build-packages.sh"
fi

# ---------------------------------------------------------------------------
# 4. Optional nfpm build and package inspection
# ---------------------------------------------------------------------------
if command -v nfpm &>/dev/null; then
    info "nfpm detected — running package builds..."

    rm -rf "${PROJECT_ROOT}/dist/packages"

    # Build packages using the official build script (skip build since we checked binaries above)
    if "${PROJECT_ROOT}/scripts/build-packages.sh" --skip-build; then
        info "  Package builds completed"
    else
        error "Package build failed"
    fi

    # Verify expected packages were created
    PKG_VERSION="$(cat "${PROJECT_ROOT}/VERSION")"
    for pkg in chvctl chv-controlplane chv-node; do
        deb_file="${PROJECT_ROOT}/dist/packages/${pkg}_${PKG_VERSION}_amd64.deb"
        rpm_file="${PROJECT_ROOT}/dist/packages/${pkg}-${PKG_VERSION}-1.x86_64.rpm"

        if [[ -f "${deb_file}" ]]; then
            info "  Found: $(basename "${deb_file}")"
        else
            error "Missing Debian package: ${deb_file}"
        fi

        if [[ -f "${rpm_file}" ]]; then
            info "  Found: $(basename "${rpm_file}")"
        else
            error "Missing RPM package: ${rpm_file}"
        fi
    done

    # -----------------------------------------------------------------------
    # 5. Inspect generated packages
    # -----------------------------------------------------------------------
    info "Inspecting generated packages..."

    # Debian metadata inspection
    for deb in "${PROJECT_ROOT}/dist/packages/"/*.deb; do
        [[ -e "${deb}" ]] || continue
        pkg_name=$(basename "${deb}")
        info "  Inspecting ${pkg_name}..."

        if command -v dpkg-deb &>/dev/null; then
            if dpkg-deb -I "${deb}" &>/dev/null; then
                info "    dpkg-deb -I OK"
            else
                error "    dpkg-deb -I failed for ${pkg_name}"
            fi

            contents=$(dpkg-deb -c "${deb}" || true)
            if [[ -z "${contents}" ]]; then
                error "    ${pkg_name} has no contents"
            else
                # Verify key binaries are present
                if echo "${contents}" | grep -q 'usr/bin/chv'; then
                    info "    Contains expected binaries"
                else
                    error "    ${pkg_name} missing expected binaries in usr/bin/"
                fi
            fi
        else
            info "    dpkg-deb not available, skipping Debian metadata inspection"
        fi
    done

    # RPM metadata inspection
    for rpm in "${PROJECT_ROOT}/dist/packages/"/*.rpm; do
        [[ -e "${rpm}" ]] || continue
        pkg_name=$(basename "${rpm}")
        info "  Inspecting ${pkg_name}..."

        if command -v rpm &>/dev/null; then
            if rpm -qip "${rpm}" &>/dev/null; then
                info "    rpm -qip OK"
            else
                error "    rpm -qip failed for ${pkg_name}"
            fi

            contents=$(rpm -qlp "${rpm}" 2>/dev/null || true)
            if [[ -z "${contents}" ]]; then
                error "    ${pkg_name} has no contents"
            else
                if echo "${contents}" | grep -q 'usr/bin/chv'; then
                    info "    Contains expected binaries"
                else
                    error "    ${pkg_name} missing expected binaries in usr/bin/"
                fi
            fi
        else
            info "    rpm not available, skipping RPM metadata inspection"
        fi
    done
else
    info "nfpm not installed — skipping package build and inspection"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
if [[ "${ERRORS}" -gt 0 ]]; then
    echo ""
    echo "[RESULT] Smoke test FAILED with ${ERRORS} error(s)"
    exit 1
else
    echo ""
    echo "[RESULT] Smoke test PASSED"
    exit 0
fi
