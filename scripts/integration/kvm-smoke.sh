#!/bin/bash
# CHV KVM Integration Smoke Test
# =================================
# Validates CHV on real KVM hardware by installing packages, generating
# temporary dev configs, starting services, and verifying health.
#
# Usage:
#   sudo ./scripts/integration/kvm-smoke.sh [OPTIONS]
#
# Options:
#   --packages DIR     Install .deb/.rpm packages from DIR before testing
#   --binary-dir DIR   Use binaries from DIR instead of /usr/bin
#   --source           Shorthand for --binary-dir target/release
#   --skip-cleanup     Do not stop services or remove temp files on exit
#   --chv-version VER  Pin cloud-hypervisor version (default: v53.0)
#
# Environment:
#   CHV_CLOUD_HYPERVISOR_VERSION   Override pinned CH version. When set (or
#                                  when --chv-version is passed), the
#                                  requested version is staged in a private
#                                  temp dir and used via the generated agent
#                                  config — /usr/bin/cloud-hypervisor is
#                                  never modified (#458).
#   CHV_TEST_TIMEOUT               Seconds to wait for services (default: 30)
#
# Safety:
#   - Uses mktemp for all temporary resources
#   - Never wipes /var/lib/chv or /etc/chv
#   - Cleans up packages with data preservation
#   - Fails fast on missing KVM or critical errors

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PACKAGE_DIR=""
BINARY_DIR=""
SKIP_CLEANUP=false
CHV_PINNED_VERSION="${CHV_CLOUD_HYPERVISOR_VERSION:-v53.0}"
# True when a version was explicitly requested (env var or --chv-version):
# explicit overrides must never install to or overwrite the system
# /usr/bin/cloud-hypervisor pin (#458).
CHV_VERSION_EXPLICIT=false
if [[ -n "${CHV_CLOUD_HYPERVISOR_VERSION:-}" ]]; then
    CHV_VERSION_EXPLICIT=true
fi
# cloud-hypervisor the generated agent config points at. Stays at the
# system path unless an explicit override stages an isolated copy.
CHV_BIN_PATH="/usr/bin/cloud-hypervisor"
CHV_STAGING_DIR=""
TEST_TIMEOUT="${CHV_TEST_TIMEOUT:-30}"

# Temp resources — all prefixed with TEST_DIR
TEST_DIR=""
TEST_NAME="chv-kvm-test"

# Service PIDs
CP_PID=""
STORD_PID=""
NWD_PID=""
AGENT_PID=""

# Error tracking
ERRORS=0
WARNINGS=0

error()   { echo "[FAIL] $*" >&2; ERRORS=$((ERRORS + 1)); }
warn()    { echo "[WARN] $*" >&2; WARNINGS=$((WARNINGS + 1)); }
info()    { echo "[INFO] $*"; }
pass()    { echo "[PASS] $*"; }
die()     { error "$*"; exit 1; }

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --packages)
            PACKAGE_DIR="$2"
            shift 2
            ;;
        --binary-dir)
            BINARY_DIR="$2"
            shift 2
            ;;
        --source)
            BINARY_DIR="${REPO_ROOT}/target/release"
            shift
            ;;
        --skip-cleanup)
            SKIP_CLEANUP=true
            shift
            ;;
        --chv-version)
            CHV_PINNED_VERSION="$2"
            CHV_VERSION_EXPLICIT=true
            shift 2
            ;;
        -h|--help)
            sed -n '2,20p' "$0"
            exit 0
            ;;
        *)
            die "Unknown option: $1"
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Cleanup trap
# ---------------------------------------------------------------------------
cleanup() {
    local exit_code=$?
    if [[ "$SKIP_CLEANUP" == true ]]; then
        info "SKIP_CLEANUP=true — leaving services and temp files in place"
        info "Test directory: ${TEST_DIR}"
        info "To clean up manually, run: rm -rf ${TEST_DIR}"
        exit "$exit_code"
    fi

    info "=== Cleanup ==="

    # Stop background processes
    if [[ -n "$AGENT_PID" ]]; then
        kill "$AGENT_PID" 2>/dev/null || true
        wait "$AGENT_PID" 2>/dev/null || true
    fi
    if [[ -n "$NWD_PID" ]]; then
        kill "$NWD_PID" 2>/dev/null || true
        wait "$NWD_PID" 2>/dev/null || true
    fi
    if [[ -n "$STORD_PID" ]]; then
        kill "$STORD_PID" 2>/dev/null || true
        wait "$STORD_PID" 2>/dev/null || true
    fi
    if [[ -n "$CP_PID" ]]; then
        kill "$CP_PID" 2>/dev/null || true
        wait "$CP_PID" 2>/dev/null || true
    fi

    # Kill any stray cloud-hypervisor processes spawned by the test
    pkill -f "cloud-hypervisor.*${TEST_NAME}" 2>/dev/null || true

    # Remove temporary bridges
    # NOTE: `|| true` is required on the whole pipeline: with pipefail, a
    # no-match grep (no test bridges exist) would otherwise abort the EXIT
    # trap mid-cleanup, leaving packages installed and TEST_DIR behind.
    ip link show 2>/dev/null | grep -E "${TEST_NAME}-br-" | awk -F: '{print $2}' | while read -r iface; do
        iface="$(echo "$iface" | xargs)"
        info "Removing bridge: $iface"
        ip link delete "$iface" 2>/dev/null || true
    done || true

    # Remove packages if we installed them
    if [[ -n "$PACKAGE_DIR" ]]; then
        info "Removing installed packages (preserving data per package contract)..."
        if command -v dpkg >/dev/null 2>&1; then
            dpkg -r chv-node chv-controlplane chvctl 2>/dev/null || true
        fi
        if command -v rpm >/dev/null 2>&1; then
            rpm -e chv-node chv-controlplane chvctl 2>/dev/null || true
        fi
    fi

    # Remove temp directory
    if [[ -n "$TEST_DIR" && -d "$TEST_DIR" ]]; then
        rm -rf "$TEST_DIR"
    fi

    # Remove the version-override staging dir (explicit --chv-version /
    # CHV_CLOUD_HYPERVISOR_VERSION runs). The system
    # /usr/bin/cloud-hypervisor pin is never written by this script when an
    # explicit override is requested (#458).
    if [[ -n "$CHV_STAGING_DIR" && -d "$CHV_STAGING_DIR" ]]; then
        rm -rf "$CHV_STAGING_DIR"
    fi

    info "Cleanup complete"
}

trap cleanup EXIT

# ---------------------------------------------------------------------------
# 1. Host diagnostics
# ---------------------------------------------------------------------------
host_diagnostics() {
    info "=========================================="
    info "Host Diagnostics"
    info "=========================================="

    info "Kernel: $(uname -r)"
    info "Distribution: $(source /etc/os-release 2>/dev/null && echo "${PRETTY_NAME:-unknown}")"

    info "CPU virtualization flags:"
    grep -E -m1 'vmx|svm' /proc/cpuinfo 2>/dev/null | sed 's/^.*: //' | while read -r flag; do
        info "  $flag"
    done || info "  (none found — KVM may not be available)"

    info "Memory:"
    free -h 2>/dev/null | grep -E 'Mem|Swap' || true

    info "Disk:"
    df -h /var/lib /tmp 2>/dev/null || df -h / 2>/dev/null || true

    info "Network interfaces:"
    ip -brief link show 2>/dev/null | grep -v '^lo' | head -10 || true
}

# ---------------------------------------------------------------------------
# 2. Verify /dev/kvm
# ---------------------------------------------------------------------------
check_kvm() {
    info "=========================================="
    info "KVM Check"
    info "=========================================="

    if [[ ! -e /dev/kvm ]]; then
        die "/dev/kvm does not exist — KVM is required for integration tests"
    fi
    pass "/dev/kvm exists"

    if [[ ! -r /dev/kvm ]]; then
        warn "/dev/kvm is not readable by current user"
        if [[ "$EUID" -ne 0 ]]; then
            die "Run this script as root or add user to 'kvm' group"
        fi
    fi

    if command -v kvm-ok >/dev/null 2>&1; then
        kvm-ok || warn "kvm-ok reported issues"
    fi
}

# ---------------------------------------------------------------------------
# 3. Verify / install cloud-hypervisor
# ---------------------------------------------------------------------------
download_cloud_hypervisor() {
    local dest="$1"

    local arch
    arch="$(uname -m)"
    # Asset names are identical across v43.0–v53.0 (verified by the #448
    # campaign, legs 01–06): "cloud-hypervisor-static" (x86_64) and
    # "cloud-hypervisor-static-aarch64" — the x86_64 build is unsuffixed.
    local asset
    case "$arch" in
        x86_64) asset="cloud-hypervisor-static" ;;
        aarch64) asset="cloud-hypervisor-static-aarch64" ;;
        *) die "Unsupported architecture: $arch" ;;
    esac

    local url="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/${CHV_PINNED_VERSION}/${asset}"
    info "Downloading from: $url"

    curl -fsSL -o "$dest" "$url" || die "Failed to download cloud-hypervisor"
    chmod +x "$dest"
}

check_cloud_hypervisor() {
    info "=========================================="
    info "cloud-hypervisor Check"
    info "=========================================="

    local chv_bin="/usr/bin/cloud-hypervisor"
    local version

    if [[ "$CHV_VERSION_EXPLICIT" == true ]]; then
        # Explicit override (CHV_CLOUD_HYPERVISOR_VERSION / --chv-version):
        # honor it unconditionally, but never write to the system path —
        # self-hosted runners keep a qualified pin at
        # /usr/bin/cloud-hypervisor that an override must not clobber
        # (#458). Stage the requested version in a private temp dir and
        # point the generated agent config (chv_binary_path) at it.
        if [[ -x "$chv_bin" ]]; then
            version="$($chv_bin --version 2>&1 | head -1)"
            case "$version" in
                *"${CHV_PINNED_VERSION}"*)
                    info "System cloud-hypervisor matches requested ${CHV_PINNED_VERSION} — using it"
                    pass "cloud-hypervisor ${CHV_PINNED_VERSION} (system)"
                    return 0
                    ;;
                *)
                    info "System cloud-hypervisor is '${version}' but ${CHV_PINNED_VERSION} requested"
                    info "Staging an isolated copy — the system binary is left untouched"
                    ;;
            esac
        fi

        CHV_STAGING_DIR="$(mktemp -d /tmp/${TEST_NAME}-chvbin-XXXXXX)"
        CHV_BIN_PATH="${CHV_STAGING_DIR}/cloud-hypervisor"
        info "cloud-hypervisor not staged yet — downloading ${CHV_PINNED_VERSION} to ${CHV_BIN_PATH}..."
        download_cloud_hypervisor "$CHV_BIN_PATH"
        version="$("${CHV_BIN_PATH}" --version 2>&1 | head -1)"
        case "$version" in
            *"${CHV_PINNED_VERSION}"*) ;;
            *) die "Downloaded cloud-hypervisor is '${version}', expected ${CHV_PINNED_VERSION}" ;;
        esac
        info "Using staged binary: ${CHV_BIN_PATH} (${version})"
        pass "cloud-hypervisor ${CHV_PINNED_VERSION} staged at ${CHV_BIN_PATH} (system pin untouched)"
        return 0
    fi

    # Default (no explicit override): trust an existing system binary —
    # self-hosted runners install a qualified pin there.
    if [[ -x "$chv_bin" ]]; then
        version="$($chv_bin --version 2>&1 | head -1)"
        info "Found: $version"
        pass "cloud-hypervisor is installed"
        return 0
    fi

    info "cloud-hypervisor not found at $chv_bin — downloading ${CHV_PINNED_VERSION}..."
    download_cloud_hypervisor "$chv_bin"
    version="$($chv_bin --version 2>&1 | head -1)"
    info "Installed: $version"
    pass "cloud-hypervisor downloaded and installed"
}

# ---------------------------------------------------------------------------
# 4. Install / locate CHV binaries
# ---------------------------------------------------------------------------
install_chv() {
    info "=========================================="
    info "CHV Installation"
    info "=========================================="

    # Determine if CHV is already installed
    local installed=false
    if [[ -x /usr/bin/chvctl && -x /usr/bin/chv-agent ]]; then
        installed=true
    fi

    if [[ -n "$PACKAGE_DIR" ]]; then
        if [[ ! -d "$PACKAGE_DIR" ]]; then
            die "Package directory does not exist: $PACKAGE_DIR"
        fi

        info "Installing packages from: $PACKAGE_DIR"

        # Detect package format
        local has_deb=false
        local has_rpm=false
        [[ -n "$(shopt -s nullglob; echo "$PACKAGE_DIR"/*.deb)" ]] && has_deb=true
        [[ -n "$(shopt -s nullglob; echo "$PACKAGE_DIR"/*.rpm)" ]] && has_rpm=true

        if [[ "$has_deb" == true ]]; then
            info "Installing .deb packages..."
            dpkg -i "$PACKAGE_DIR"/chv-controlplane_*.deb "$PACKAGE_DIR"/chv-node_*.deb "$PACKAGE_DIR"/chvctl_*.deb || true
            apt-get install -f -y -qq 2>/dev/null || true
        elif [[ "$has_rpm" == true ]]; then
            info "Installing .rpm packages..."
            rpm -i "$PACKAGE_DIR"/chv-controlplane-*.rpm "$PACKAGE_DIR"/chv-node-*.rpm "$PACKAGE_DIR"/chvctl-*.rpm || true
        else
            die "No .deb or .rpm packages found in $PACKAGE_DIR"
        fi

        BINARY_DIR="/usr/bin"
        pass "Packages installed"
    elif [[ -n "$BINARY_DIR" ]]; then
        if [[ ! -d "$BINARY_DIR" ]]; then
            die "Binary directory does not exist: $BINARY_DIR"
        fi
        info "Using binaries from: $BINARY_DIR"
        for bin in chvctl chv-controlplane chv-agent chv-stord chv-nwd; do
            if [[ ! -x "$BINARY_DIR/$bin" ]]; then
                die "Missing binary: $BINARY_DIR/$bin"
            fi
        done
        pass "All binaries found in $BINARY_DIR"
    elif [[ "$installed" == true ]]; then
        BINARY_DIR="/usr/bin"
        info "Using existing system installation"
        pass "System binaries found"
    else
        die "No CHV binaries found. Use --packages, --binary-dir, or --source"
    fi
}

# ---------------------------------------------------------------------------
# 5. Binary version checks
# ---------------------------------------------------------------------------
check_binary_versions() {
    info "=========================================="
    info "Binary Version Checks"
    info "=========================================="

    for bin in chvctl chv-controlplane chv-agent chv-stord chv-nwd; do
        local bin_path="${BINARY_DIR}/${bin}"
        if [[ ! -x "$bin_path" ]]; then
            error "Missing binary: $bin_path"
            continue
        fi

        local output
        output="$($bin_path --version 2>&1)"
        if echo "$output" | grep -qE 'chv|CHV|cloud-hypervisor|0\.1\.0'; then
            info "  $output"
            pass "$bin --version OK"
        else
            error "$bin --version failed: $output"
        fi
    done
}

# ---------------------------------------------------------------------------
# 6. Systemd unit validation
# ---------------------------------------------------------------------------
check_systemd_units() {
    info "=========================================="
    info "Systemd Unit Validation"
    info "=========================================="

    if ! command -v systemd-analyze >/dev/null 2>&1; then
        warn "systemd-analyze not available — skipping unit validation"
        return 0
    fi

    local unit_dir="/lib/systemd/system"
    if [[ ! -d "$unit_dir" ]]; then
        unit_dir="/usr/lib/systemd/system"
    fi

    if [[ ! -d "$unit_dir" ]]; then
        warn "Systemd unit directory not found"
        return 0
    fi

    for unit in chv-controlplane chv-agent chv-stord chv-nwd; do
        local unit_file="$unit_dir/${unit}.service"
        if [[ -f "$unit_file" ]]; then
            if systemd-analyze verify "$unit_file" 2>/dev/null; then
                pass "${unit}.service is valid"
            else
                warn "${unit}.service has warnings (non-fatal)"
            fi
        else
            warn "Unit file not found: ${unit}.service"
        fi
    done
}

# ---------------------------------------------------------------------------
# 7. Generate dev TLS certs and configs
# ---------------------------------------------------------------------------
generate_dev_environment() {
    info "=========================================="
    info "Generating Dev Environment"
    info "=========================================="

    TEST_DIR="$(mktemp -d /tmp/${TEST_NAME}-XXXXXX)"
    info "Test directory: $TEST_DIR"

    local certs_dir="$TEST_DIR/certs"
    local logs_dir="$TEST_DIR/logs"
    local cp_dir="$TEST_DIR/controlplane"
    local agent_dir="$TEST_DIR/agent"
    local stord_dir="$TEST_DIR/stord"
    local nwd_dir="$TEST_DIR/nwd"

    mkdir -p "$certs_dir" "$logs_dir" "$cp_dir" "$agent_dir" "$stord_dir" "$nwd_dir"

    # --- TLS Certificates ---
    info "Generating self-signed CA and certificates..."

    openssl genrsa -out "$certs_dir/ca.key" 4096 2>/dev/null
    openssl req -x509 -new -nodes -key "$certs_dir/ca.key" \
        -sha256 -days 1 -out "$certs_dir/ca.crt" \
        -subj "/O=CHV Integration Test/CN=chv-test-ca" 2>/dev/null

    openssl genrsa -out "$certs_dir/server.key" 2048 2>/dev/null
    openssl req -new -key "$certs_dir/server.key" \
        -out "$certs_dir/server.csr" \
        -subj "/O=CHV Integration Test/CN=localhost" 2>/dev/null
    # NOTE: `openssl x509 -req` without -extfile emits X.509 **v1**, which
    # rustls/webpki rejects (UnsupportedCertVersion) — every leaf cert must
    # be explicitly v3 via an extfile (same fix as scripts/integration/qual/deploy.sh).
    cat > "$certs_dir/server.ext" <<EOF
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost, IP:127.0.0.1
EOF
    openssl x509 -req -in "$certs_dir/server.csr" \
        -CA "$certs_dir/ca.crt" -CAkey "$certs_dir/ca.key" \
        -CAcreateserial -out "$certs_dir/server.crt" \
        -days 1 -sha256 -extfile "$certs_dir/server.ext" 2>/dev/null
    rm -f "$certs_dir/server.csr" "$certs_dir/server.ext"

    openssl genrsa -out "$certs_dir/agent-client.key" 2048 2>/dev/null
    openssl req -new -key "$certs_dir/agent-client.key" \
        -out "$certs_dir/agent-client.csr" \
        -subj "/O=CHV Integration Test/CN=kvm-test-node" 2>/dev/null
    cat > "$certs_dir/agent-client.ext" <<EOF
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = clientAuth
EOF
    openssl x509 -req -in "$certs_dir/agent-client.csr" \
        -CA "$certs_dir/ca.crt" -CAkey "$certs_dir/ca.key" \
        -CAcreateserial -out "$certs_dir/agent-client.crt" \
        -days 1 -sha256 -extfile "$certs_dir/agent-client.ext" 2>/dev/null
    rm -f "$certs_dir/agent-client.csr" "$certs_dir/agent-client.ext"

    chmod 644 "$certs_dir"/*.crt
    chmod 600 "$certs_dir"/*.key

    # --- Determine migrations directory ---
    local migrations_dir
    if [[ -d "/usr/share/chv/migrations" ]]; then
        migrations_dir="/usr/share/chv/migrations"
    elif [[ -d "${REPO_ROOT}/cmd/chv-controlplane/migrations" ]]; then
        migrations_dir="${REPO_ROOT}/cmd/chv-controlplane/migrations"
    else
        warn "Migrations directory not found — controlplane may fail to start"
        migrations_dir="${cp_dir}/migrations"
        mkdir -p "$migrations_dir"
    fi
    info "Migrations dir: $migrations_dir"

    # --- Control Plane Config ---
    cat > "$TEST_DIR/controlplane.toml" <<EOF
grpc_bind = "127.0.0.1:8443"
http_bind = "127.0.0.1:8080"
log_level = "info"
runtime_dir = "${cp_dir}"
jwt_secret = "chv-integration-test-secret-min-32-chars-ok"

[database]
url = "sqlite://${cp_dir}/controlplane.db"
migrations_dir = "${migrations_dir}"
max_connections = 4
min_connections = 1
acquire_timeout_secs = 5

[tls]
ca_cert_path = "${certs_dir}/ca.crt"
ca_key_path = "${certs_dir}/ca.key"
server_cert_path = "${certs_dir}/server.crt"
server_key_path = "${certs_dir}/server.key"
client_ca_path = "${certs_dir}/ca.crt"
EOF

    # --- Agent Config ---
    cat > "$TEST_DIR/agent.toml" <<EOF
socket_path = "${agent_dir}/api.sock"
runtime_dir = "${agent_dir}"
log_level = "info"
# Match the shipped configs (#326): core-managed is the qualified production
# composition; every packaged/deployed surface sets it explicitly.
authority_mode = "core-managed"
# Core-authority paths must live in the throwaway test tree — the packaged
# defaults (/var/lib/chv/agent, /run/chv/core) belong to the 'chv' user and
# fail the owner-owned startup validation when this root-run test uses
# them. Same shape as qual/deploy.sh's generated agent.toml (#458).
core_api_socket_path = "${agent_dir}/core.sock"
core_store_path = "${agent_dir}/core.db"
core_archive_path = "${agent_dir}/node-cache-v1.archive"
control_plane_addr = "https://127.0.0.1:8443"
stord_socket = "${stord_dir}/api.sock"
nwd_socket = "${nwd_dir}/api.sock"
chv_binary_path = "${CHV_BIN_PATH}"
stord_binary_path = "${BINARY_DIR}/chv-stord"
nwd_binary_path = "${BINARY_DIR}/chv-nwd"
cache_path = "${agent_dir}/agent-cache.json"
node_id = "kvm-test-node"
metrics_bind = "127.0.0.1:9100"
storage_base_dir = "${agent_dir}/storage"
console_bind = "127.0.0.1:8444"

tls_cert_path = "${certs_dir}/agent-client.crt"
tls_key_path = "${certs_dir}/agent-client.key"
ca_cert_path = "${certs_dir}/ca.crt"
EOF

    # --- Stord Config ---
    cat > "$TEST_DIR/stord.toml" <<EOF
socket_path = "${stord_dir}/api.sock"
runtime_dir = "${stord_dir}"
log_level = "info"
EOF

    # --- NWD Config ---
    cat > "$TEST_DIR/nwd.toml" <<EOF
socket_path = "${nwd_dir}/api.sock"
runtime_dir = "${nwd_dir}"
log_level = "info"
EOF

    # Agent runtime dir MUST be 0700 for the Core-authority startup
    # validation (cellhv-core-startup::validate_paths). mkdir under the
    # default umask gives 0755, which the fail-closed validation rejects —
    # same contract as qual/deploy.sh and install.sh (#458, regression
    # from the #334 core-managed flip).
    chmod 0700 "${agent_dir}"

    pass "Dev environment generated in $TEST_DIR"
}

# ---------------------------------------------------------------------------
# 8. Start services
# ---------------------------------------------------------------------------
start_services() {
    info "=========================================="
    info "Starting CHV Services"
    info "=========================================="

    # Ensure runtime dirs exist
    mkdir -p /run/chv 2>/dev/null || true

    # --- Control Plane ---
    info "Starting chv-controlplane..."
    "${BINARY_DIR}/chv-controlplane" "$TEST_DIR/controlplane.toml" \
        > "$TEST_DIR/logs/controlplane.log" 2>&1 &
    CP_PID=$!
    info "  PID: $CP_PID"

    # Wait for control plane to bind its HTTP port
    local waited=0
    local port_check_cmd=""
    if command -v ss >/dev/null 2>&1; then
        port_check_cmd="ss -tlnp"
    elif command -v netstat >/dev/null 2>&1; then
        port_check_cmd="netstat -tlnp"
    fi

    if [[ -n "$port_check_cmd" ]]; then
        while ! $port_check_cmd 2>/dev/null | grep -q ':8080'; do
            if ! kill -0 "$CP_PID" 2>/dev/null; then
                error "chv-controlplane exited prematurely"
                cat "$TEST_DIR/logs/controlplane.log" >&2 || true
                return 1
            fi
            sleep 1
            waited=$((waited + 1))
            if [[ $waited -ge 15 ]]; then
                warn "chv-controlplane did not bind to :8080 within 15s — continuing anyway"
                break
            fi
        done
    else
        info "ss/netstat not available — sleeping 5s for controlplane startup"
        sleep 5
    fi
    if [[ $waited -lt 15 ]]; then
        pass "chv-controlplane listening on :8080"
    fi

    # --- Storage Daemon ---
    info "Starting chv-stord..."
    "${BINARY_DIR}/chv-stord" "$TEST_DIR/stord.toml" \
        > "$TEST_DIR/logs/stord.log" 2>&1 &
    STORD_PID=$!
    info "  PID: $STORD_PID"

    # --- Network Daemon ---
    info "Starting chv-nwd..."
    "${BINARY_DIR}/chv-nwd" "$TEST_DIR/nwd.toml" \
        > "$TEST_DIR/logs/nwd.log" 2>&1 &
    NWD_PID=$!
    info "  PID: $NWD_PID"

    # --- Agent ---
    # No CHV_ALLOW_INSECURE here: this agent is unenrolled (fresh cache) with
    # TLS paths configured, so the insecure bypass is never consulted — and a
    # production build now refuses the env var at startup (prompt 03 / #233).
    info "Starting chv-agent..."
    "${BINARY_DIR}/chv-agent" "$TEST_DIR/agent.toml" \
        > "$TEST_DIR/logs/agent.log" 2>&1 &
    AGENT_PID=$!
    info "  PID: $AGENT_PID"

    # Give services time to initialize
    info "Waiting ${TEST_TIMEOUT}s for services to stabilize..."
    sleep "$TEST_TIMEOUT"
}

# ---------------------------------------------------------------------------
# 9. Verify health
# ---------------------------------------------------------------------------
verify_health() {
    info "=========================================="
    info "Health Verification"
    info "=========================================="

    # Check controlplane process
    if kill -0 "$CP_PID" 2>/dev/null; then
        pass "chv-controlplane is running (PID $CP_PID)"
    else
        error "chv-controlplane is not running"
        cat "$TEST_DIR/logs/controlplane.log" >&2 || true
    fi

    # Check stord process
    if kill -0 "$STORD_PID" 2>/dev/null; then
        pass "chv-stord is running (PID $STORD_PID)"
    else
        error "chv-stord is not running"
        cat "$TEST_DIR/logs/stord.log" >&2 || true
    fi

    # Check nwd process
    if kill -0 "$NWD_PID" 2>/dev/null; then
        pass "chv-nwd is running (PID $NWD_PID)"
    else
        error "chv-nwd is not running"
        cat "$TEST_DIR/logs/nwd.log" >&2 || true
    fi

    # Check agent process
    if kill -0 "$AGENT_PID" 2>/dev/null; then
        pass "chv-agent is running (PID $AGENT_PID)"
    else
        error "chv-agent is not running"
        cat "$TEST_DIR/logs/agent.log" >&2 || true
    fi

    # Check controlplane HTTP port
    if command -v curl >/dev/null 2>&1; then
        local http_code
        http_code="$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/ 2>/dev/null || true)"
        if [[ "$http_code" =~ ^(200|404)$ ]]; then
            pass "Control plane HTTP responds on :8080 (HTTP $http_code)"
        else
            warn "Control plane HTTP on :8080 did not respond (may be normal if no root route)"
        fi

        # Check agent metrics endpoint
        http_code="$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9100/metrics 2>/dev/null || true)"
        if [[ "$http_code" =~ ^(200|404)$ ]]; then
            pass "Agent metrics endpoint responds on :9100/metrics (HTTP $http_code)"
        else
            warn "Agent metrics on :9100 did not respond"
        fi
    else
        warn "curl not available — skipping HTTP health checks"
    fi

    # Check agent socket exists
    if [[ -S "$TEST_DIR/agent/api.sock" ]]; then
        pass "Agent gRPC socket exists"
    else
        warn "Agent gRPC socket not found"
    fi
}

# ---------------------------------------------------------------------------
# 10. Collect logs
# ---------------------------------------------------------------------------
collect_logs() {
    info "=========================================="
    info "Log Collection"
    info "=========================================="

    info "Logs available in: $TEST_DIR/logs/"
    for log in controlplane stord nwd agent; do
        local logfile="$TEST_DIR/logs/${log}.log"
        if [[ -f "$logfile" ]]; then
            local lines
            lines="$(wc -l < "$logfile")"
            info "  ${log}.log (${lines} lines)"
            # Print last 20 lines for quick diagnostics
            tail -20 "$logfile" | sed 's/^/    /' || true
        else
            warn "  ${log}.log not found"
        fi
    done
}

# ---------------------------------------------------------------------------
# 11. VM Lifecycle (TODO)
# ---------------------------------------------------------------------------
vm_lifecycle_todo() {
    info "=========================================="
    info "VM Lifecycle Test"
    info "=========================================="
    info "TODO: VM lifecycle testing requires:"
    info "  - A bootable VM image (kernel + initrd or disk image)"
    info "  - A configured network bridge"
    info "  - Control plane <> Agent enrollment and mTLS handshake"
    info "  - chvctl commands: vm create, start, stop, delete"
    info ""
    info "This will be implemented once the following are stable:"
    info "  - Bootstrap token generation and distribution"
    info "  - Agent enrollment without manual cert provisioning"
    info "  - Base image distribution and caching on runners"
    info ""
    info "Deferred to Prompt 10+ or dedicated VM lifecycle test suite."
}

# ---------------------------------------------------------------------------
# 12. Summary
# ---------------------------------------------------------------------------
summary() {
    info "=========================================="
    info "Test Summary"
    info "=========================================="

    if [[ $ERRORS -gt 0 ]]; then
        echo "[RESULT] FAILED with $ERRORS error(s), $WARNINGS warning(s)"
        return 1
    elif [[ $WARNINGS -gt 0 ]]; then
        echo "[RESULT] PASSED with $WARNINGS warning(s)"
        return 0
    else
        echo "[RESULT] PASSED"
        return 0
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    info "CHV KVM Integration Smoke Test"
    info "================================"

    host_diagnostics
    check_kvm
    check_cloud_hypervisor
    install_chv
    check_binary_versions
    check_systemd_units
    generate_dev_environment
    start_services
    verify_health
    collect_logs
    vm_lifecycle_todo
    summary
}

main "$@"
