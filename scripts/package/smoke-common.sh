#!/bin/bash
# CHV Package Smoke Test — Common Verification Functions
# Sourced by smoke-deb.sh and smoke-rpm.sh (inside containers).

set -euo pipefail

ERRORS=0
error() { echo "[FAIL] $*" >&2; ERRORS=$((ERRORS + 1)); }
info() { echo "[INFO] $*"; }
pass() { echo "[PASS] $*"; }

# ---------------------------------------------------------------------------
# Verify state after package installation
# ---------------------------------------------------------------------------
verify_install_state() {
    local pkg_type="${1:-}"

    info "Checking installed binaries..."
    for bin in chvctl chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -x "/usr/bin/${bin}" ]]; then
            info "  Found: /usr/bin/${bin}"
        else
            error "Missing or not executable: /usr/bin/${bin}"
        fi
    done

    info "Checking config files..."
    for cfg in controlplane.toml agent.toml stord.toml nwd.toml; do
        if [[ -f "/etc/chv/${cfg}" ]]; then
            info "  Found: /etc/chv/${cfg}"
        else
            error "Missing config: /etc/chv/${cfg}"
        fi
    done

    # chv.yaml is installed as a reference config by chv-node
    if [[ -f "/etc/chv/chv.yaml" ]]; then
        info "  Found: /etc/chv/chv.yaml"
    fi

    info "Checking systemd units..."
    for svc in chv-controlplane chv-agent chv-stord chv-nwd; do
        local unit_file="/lib/systemd/system/${svc}.service"
        if [[ -f "$unit_file" ]]; then
            info "  Found: ${svc}.service"
            # Verify unit references correct binary path
            if grep -q '/usr/local/bin' "$unit_file"; then
                error "${svc}.service references /usr/local/bin"
            fi
        else
            error "Missing unit: ${svc}.service"
        fi
    done

    info "Checking runtime directories..."
    if [[ -d "/var/lib/chv" ]]; then
        info "  Found: /var/lib/chv"
    else
        error "Missing: /var/lib/chv"
    fi

    if [[ -d "/var/log/chv" ]]; then
        info "  Found: /var/log/chv"
    else
        error "Missing: /var/log/chv"
    fi

    info "Checking chv user and group..."
    if getent passwd chv >/dev/null 2>&1; then
        info "  User 'chv' exists"
    else
        error "User 'chv' does not exist"
    fi
    if getent group chv >/dev/null 2>&1; then
        info "  Group 'chv' exists"
    else
        error "Group 'chv' does not exist"
    fi

    # Node-specific: kvm group membership
    if id -nG chv 2>/dev/null | grep -qw kvm; then
        info "  User 'chv' is in 'kvm' group"
    else
        info "  User 'chv' is NOT in 'kvm' group (may be expected in minimal containers)"
    fi
}

# ---------------------------------------------------------------------------
# Verify version output from binaries
# ---------------------------------------------------------------------------
verify_version_output() {
    info "Checking binary version outputs..."

    # Capture stdout+stderr AND exit code without letting `set -e` abort.
    # Bash gotcha: when this function is called plainly (no `&&` / `||`
    # guard) under `set -euo pipefail`, the two-statement form
    #     local var
    #     var="$(failing_cmd)"
    # propagates the substitution's non-zero exit and kills the shell
    # *before* any error message is printed — exactly the silent-abort
    # symptom we hit on debian:12 when a binary fails to load. Using
    # `var="$(cmd)" || rc=$?` masks the substitution exit so we can
    # capture and report it explicitly.
    local version_output rc

    rc=0
    version_output="$(/usr/bin/chvctl --version 2>&1)" || rc=$?
    if [[ "$rc" -eq 0 ]] && echo "$version_output" | grep -q "chvctl"; then
        info "  chvctl --version: ${version_output}"
    else
        error "chvctl --version failed (exit ${rc}): ${version_output}"
    fi

    for bin in chv-agent chv-controlplane chv-nwd chv-stord; do
        rc=0
        version_output="$("/usr/bin/${bin}" --version 2>&1)" || rc=$?
        if [[ "$rc" -eq 0 ]] && echo "$version_output" | grep -q "${bin}"; then
            info "  ${bin} --version: ${version_output}"
        else
            error "${bin} --version failed (exit ${rc}): ${version_output}"
        fi
    done
}

# ---------------------------------------------------------------------------
# Verify state after package removal
# ---------------------------------------------------------------------------
verify_remove_state() {
    info "Checking post-removal state..."

    info "Checking binaries were removed..."
    for bin in chvctl chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -e "/usr/bin/${bin}" ]]; then
            error "Binary still present after remove: /usr/bin/${bin}"
        else
            info "  Removed: /usr/bin/${bin}"
        fi
    done

    info "Checking systemd units were removed..."
    for svc in chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -e "/lib/systemd/system/${svc}.service" ]]; then
            error "Unit still present after remove: ${svc}.service"
        else
            info "  Removed: ${svc}.service"
        fi
    done

    info "Checking persistent data was preserved..."
    if [[ -d "/var/lib/chv" ]]; then
        info "  Preserved: /var/lib/chv"
    else
        error "Persistent data removed: /var/lib/chv"
    fi

    if [[ -d "/etc/chv" ]]; then
        info "  Preserved: /etc/chv"
    else
        error "Config directory removed: /etc/chv"
    fi

    if [[ -d "/var/log/chv" ]]; then
        info "  Preserved: /var/log/chv"
    else
        # Some package managers may remove empty log dirs; this is a warning, not a hard error
        info "  Note: /var/log/chv may have been removed (empty directory)"
    fi

    # chv user should NOT be removed automatically
    if getent passwd chv >/dev/null 2>&1; then
        info "  User 'chv' preserved (expected)"
    else
        error "User 'chv' was removed unexpectedly"
    fi
}

# ---------------------------------------------------------------------------
# Verify state after reinstall
# ---------------------------------------------------------------------------
verify_reinstall_state() {
    info "Checking reinstall state..."
    verify_install_state
    verify_version_output
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
smoke_summary() {
    if [[ "$ERRORS" -gt 0 ]]; then
        echo ""
        echo "[RESULT] Smoke test FAILED with ${ERRORS} error(s)"
        return 1
    else
        echo ""
        echo "[RESULT] Smoke test PASSED"
        return 0
    fi
}
