#!/bin/bash
# CHV Package Lifecycle Test — Common Functions
# Sourced by lifecycle-deb.sh and lifecycle-rpm.sh (inside containers).

set -euo pipefail

ERRORS=0
WARNINGS=0

error()   { echo "[FAIL] $*" >&2; ERRORS=$((ERRORS + 1)); }
warn()    { echo "[WARN] $*" >&2; WARNINGS=$((WARNINGS + 1)); }
info()    { echo "[INFO] $*"; }
pass()    { echo "[PASS] $*"; }

# ---------------------------------------------------------------------------
# Sentinel files
# ---------------------------------------------------------------------------
PERSISTENT_SENTINEL="/var/lib/chv/test-persistent-state-sentinel"
CONFIG_SENTINEL="/etc/chv/test-config-sentinel"
CONFIG_MARKER="# CHV-LIFECYCLE-TEST-MARKER"

# ---------------------------------------------------------------------------
# Create sentinel state
# ---------------------------------------------------------------------------
create_sentinel_state() {
    info "Creating sentinel state files..."

    echo "persistent-data-sentinel" > "$PERSISTENT_SENTINEL"
    echo "config-sentinel" > "$CONFIG_SENTINEL"

    # Modify a managed config file to test config|noreplace behavior
    if [[ -f /etc/chv/controlplane.toml ]]; then
        echo "$CONFIG_MARKER" >> /etc/chv/controlplane.toml
        info "  Added marker to /etc/chv/controlplane.toml"
    else
        warn "  /etc/chv/controlplane.toml not found — cannot test config preservation"
    fi

    pass "Sentinel state created"
}

# ---------------------------------------------------------------------------
# Verify sentinels exist
# ---------------------------------------------------------------------------
verify_sentinels_present() {
    info "Checking sentinel files are present..."

    if [[ -f "$PERSISTENT_SENTINEL" ]]; then
        pass "Persistent sentinel exists: $PERSISTENT_SENTINEL"
    else
        error "Persistent sentinel missing: $PERSISTENT_SENTINEL"
    fi

    if [[ -f "$CONFIG_SENTINEL" ]]; then
        pass "Config sentinel exists: $CONFIG_SENTINEL"
    else
        error "Config sentinel missing: $CONFIG_SENTINEL"
    fi

    if [[ -f /etc/chv/controlplane.toml ]]; then
        if grep -q "$CONFIG_MARKER" /etc/chv/controlplane.toml; then
            pass "Config marker preserved in /etc/chv/controlplane.toml"
        else
            error "Config marker lost from /etc/chv/controlplane.toml"
        fi
    else
        warn "  /etc/chv/controlplane.toml not found"
    fi
}

# ---------------------------------------------------------------------------
# Verify sentinels absent (for purge tests)
# ---------------------------------------------------------------------------
verify_sentinels_absent() {
    info "Checking sentinel files are absent (purge test)..."

    if [[ -f "$PERSISTENT_SENTINEL" ]]; then
        error "Persistent sentinel still present after purge: $PERSISTENT_SENTINEL"
    else
        pass "Persistent sentinel removed: $PERSISTENT_SENTINEL"
    fi

    if [[ -f "$CONFIG_SENTINEL" ]]; then
        error "Config sentinel still present after purge: $CONFIG_SENTINEL"
    else
        pass "Config sentinel removed: $CONFIG_SENTINEL"
    fi
}

# ---------------------------------------------------------------------------
# Verify install state
# ---------------------------------------------------------------------------
verify_install_state() {
    info "Checking install state..."

    for bin in chvctl chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -x "/usr/bin/${bin}" ]]; then
            pass "Binary exists: /usr/bin/${bin}"
        else
            error "Missing binary: /usr/bin/${bin}"
        fi
    done

    for unit in chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -f "/lib/systemd/system/${unit}.service" ]]; then
            pass "Unit exists: ${unit}.service"
        else
            warn "Unit not found: ${unit}.service (may be in /usr/lib/systemd/system)"
        fi
    done

    if [[ -d /var/lib/chv ]]; then
        pass "Directory exists: /var/lib/chv"
    else
        error "Missing directory: /var/lib/chv"
    fi
}

# ---------------------------------------------------------------------------
# Verify remove state (binaries gone, data preserved)
# ---------------------------------------------------------------------------
verify_remove_state() {
    info "Checking remove state..."

    for bin in chvctl chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -e "/usr/bin/${bin}" ]]; then
            error "Binary still present: /usr/bin/${bin}"
        else
            pass "Binary removed: /usr/bin/${bin}"
        fi
    done

    for unit in chv-controlplane chv-agent chv-stord chv-nwd; do
        if [[ -e "/lib/systemd/system/${unit}.service" ]]; then
            error "Unit still present: ${unit}.service"
        else
            pass "Unit removed: ${unit}.service"
        fi
    done

    # Persistent data MUST be preserved
    if [[ -d /var/lib/chv ]]; then
        pass "Persistent data preserved: /var/lib/chv"
    else
        error "Persistent data removed: /var/lib/chv"
    fi

    # Config directory MUST be preserved
    if [[ -d /etc/chv ]]; then
        pass "Config directory preserved: /etc/chv"
    else
        error "Config directory removed: /etc/chv"
    fi
}

# ---------------------------------------------------------------------------
# Verify systemd unit reload after upgrade
# ---------------------------------------------------------------------------
verify_systemd_reload() {
    info "Checking systemd daemon-reload behavior..."

    # In containers, systemd may not be running PID 1.
    # We can only verify that the postinstall script attempted daemon-reload.
    # A more robust check would require a systemd-enabled container.
    if command -v systemctl >/dev/null 2>&1; then
        info "  systemctl is available (daemon-reload would have run in postinstall)"
    else
        warn "  systemctl not available in container — skipping reload verification"
    fi
}

# ---------------------------------------------------------------------------
# Verify upgrade state (new binaries, preserved data)
# ---------------------------------------------------------------------------
verify_upgrade_state() {
    info "Checking upgrade state..."

    verify_install_state
    verify_sentinels_present
    verify_systemd_reload
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
lifecycle_summary() {
    echo ""
    if [[ "$ERRORS" -gt 0 ]]; then
        echo "[RESULT] Lifecycle test FAILED with ${ERRORS} error(s), ${WARNINGS} warning(s)"
        return 1
    elif [[ "$WARNINGS" -gt 0 ]]; then
        echo "[RESULT] Lifecycle test PASSED with ${WARNINGS} warning(s)"
        return 0
    else
        echo "[RESULT] Lifecycle test PASSED"
        return 0
    fi
}
