#!/bin/bash
# Verify CHV packaging safety:
#   1. Systemd units are included.
#   2. Config files are marked properly.
#   3. Maintainer scripts exist and are executable.
#   4. Scripts contain no destructive commands.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PACKAGING_DIR="${REPO_ROOT}/packaging"
SCRIPTS_DIR="${PACKAGING_DIR}/scripts"
SYSTEMD_DIR="${PACKAGING_DIR}/systemd"
NFPM_DIR="${PACKAGING_DIR}/nfpm"

ERRORS=0
error() { echo "[FAIL] $*" >&2; ERRORS=$((ERRORS + 1)); }
info() { echo "[INFO] $*"; }

echo "==============================================="
echo "CHV Packaging Safety Check"
echo "==============================================="

# ---------------------------------------------------------------------------
# 1. Systemd unit files are included for existing binaries
# ---------------------------------------------------------------------------
info "Checking systemd units..."

for binary in chv-agent chv-stord chv-nwd chv-controlplane; do
    svc_file="${SYSTEMD_DIR}/${binary}.service"
    if [[ -f "$svc_file" ]]; then
        info "  Found: ${binary}.service"
    else
        error "Missing systemd unit: ${binary}.service"
    fi
done

# Verify units reference /usr/bin (not /usr/local/bin)
for svc_file in "${SYSTEMD_DIR}"/*.service; do
    [[ -e "$svc_file" ]] || continue
    if grep -q '/usr/local/bin' "$svc_file"; then
        error "$(basename "$svc_file") references /usr/local/bin"
    fi
done
info "  No units reference /usr/local/bin"

# ---------------------------------------------------------------------------
# 2. Config files are included and marked properly
# ---------------------------------------------------------------------------
info "Checking nFPM config markings..."

for yaml in "${NFPM_DIR}"/*.yaml; do
    [[ -e "$yaml" ]] || continue
    pkg_name="$(basename "$yaml" .yaml)"

    if grep -q 'type: config|noreplace' "$yaml"; then
        info "  ${pkg_name} has config|noreplace files"
    elif grep -q 'dst: /etc/chv/' "$yaml"; then
        # Config files under /etc/chv should be marked
        error "${pkg_name} has /etc/chv files not marked as config|noreplace"
    fi
done

# ---------------------------------------------------------------------------
# 3. Maintainer scripts exist and are executable
# ---------------------------------------------------------------------------
info "Checking maintainer scripts..."

for script in postinstall.sh preremove.sh postremove.sh; do
    script_path="${SCRIPTS_DIR}/${script}"
    if [[ -f "$script_path" ]]; then
        if [[ -x "$script_path" ]]; then
            info "  ${script} exists and is executable"
        else
            error "${script} exists but is not executable"
        fi
    else
        error "Missing maintainer script: ${script}"
    fi
done

# ---------------------------------------------------------------------------
# 4. Scripts contain no destructive commands
# ---------------------------------------------------------------------------
info "Checking scripts for destructive commands..."

DESTRUCTIVE_PATTERNS=(
    'rm -rf /var/lib/chv'
    'rm -rf /etc/chv'
    'mkfs\.'
    'fdisk'
    'parted'
    'dd if='
    'brctl addbr'
    'ip link add.*type bridge'
    'iptables -F'
    'nft flush'
)

for script in "${SCRIPTS_DIR}"/*.sh; do
    [[ -e "$script" ]] || continue
    script_name="$(basename "$script")"
    for pattern in "${DESTRUCTIVE_PATTERNS[@]}"; do
        if grep -qE "$pattern" "$script"; then
            error "${script_name} contains destructive pattern: ${pattern}"
        fi
    done
done
info "  No destructive patterns found"

# ---------------------------------------------------------------------------
# 5. Scripts preserve persistent data on remove
# ---------------------------------------------------------------------------
info "Checking data preservation in remove scripts..."

for script in preremove.sh postremove.sh; do
    script_path="${SCRIPTS_DIR}/${script}"
    [[ -e "$script_path" ]] || continue

    if grep -q 'rm.*\-rf.*\(/var/lib/chv\|/etc/chv\)' "$script_path"; then
        error "${script} contains rm -rf against persistent directories"
    fi
    if grep -q 'userdel.*chv' "$script_path"; then
        error "${script} removes the chv user"
    fi
    if grep -q 'groupdel.*chv' "$script_path"; then
        error "${script} removes the chv group"
    fi
done
info "  Persistent data and user preservation confirmed"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
if [[ "$ERRORS" -gt 0 ]]; then
    echo ""
    echo "[RESULT] Safety check FAILED with ${ERRORS} error(s)"
    exit 1
else
    echo ""
    echo "[RESULT] Safety check PASSED"
    exit 0
fi
