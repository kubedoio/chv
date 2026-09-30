#!/usr/bin/env bash
# deploy.sh — candidate-path deployment of the full CHV stack for the
# prompt-04 qualification (M4.1).
#
# This is the M2.5-proven deployment shape, committed as code:
#   - self-signed CA (2-day validity) + server cert (SAN localhost/127.0.0.1);
#   - REAL mTLS: a pre-placed client cert is used ONLY for the EnrollNode
#     handshake; afterwards the control-plane-issued node certificate
#     (CN = node id) is used;
#   - one-time bootstrap token seeded directly into the control-plane DB
#     (the production install path creates it via the loopback-only internal
#     endpoint; the harness seeds it before the agent starts — the agent
#     enrolls at startup and the token is consumed exactly once);
#   - admin user seeded with a fresh bcrypt hash (install.sh's job in
#     production; must_change_password=0 for the qualification run);
#   - authority_mode = "core-managed" (single durable CellHV Core authority);
#   - agent runtime dir mode 0700 (Core validate_paths contract);
#   - stord path_allowlist: agent runtime dir + images dir (operator-shaped);
#   - NO CHV_ALLOW_INSECURE anywhere.
#
# Usage:
#   sudo ./deploy.sh [OPTIONS]
#
# Options:
#   --binary-dir DIR   staged binaries (default: /var/lib/chv/qual/bin —
#                      produced by env-preflight.sh --stage-binaries)
#   --test-dir DIR     deployment dir (default: mktemp /tmp/chv-qual-XXXXXX)
#   --exec CMD...      run CMD with the deployment map exported
#                      (QUAL_MAP + individual variables) after verification;
#                      teardown runs afterwards with CMD's exit status
#   --hold             keep the stack up until SIGTERM/SIGINT, then tear down
#   --skip-cleanup     leave services running and files in place on exit
#
# Default (no --exec/--hold): full deployment smoke — bring the stack up,
# verify, tear down, assert no residue. Scenario scripts (M4.3+) use --exec.
#
# On success prints the deployment map (test dir, PIDs, ports) for the
# per-milestone scenario scripts to consume. Exit code non-zero on any
# hard failure; teardown asserts forbidden residue (no cloud-hypervisor
# processes, no new links, no new nft tables) unless --skip-cleanup.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

CHV_QUAL_ROOT="${CHV_QUAL_ROOT:-/var/lib/chv/qual}"
BINARY_DIR="${CHV_QUAL_ROOT}/bin"
TEST_DIR=""
SKIP_CLEANUP=false
EXEC_CMD=""
HOLD=false
NODE_ID="qual-node-1"
NETWORK_CIDR="${QUAL_NETWORK_CIDR:-10.200.0.0/24}"
GUEST_IMAGE="${GUEST_IMAGE:-noble-server-cloudimg-amd64.img}"

while [ $# -gt 0 ]; do
    case "$1" in
        --binary-dir) BINARY_DIR="${2:?}"; shift 2 ;;
        --binary-dir=*) BINARY_DIR="${1#*=}"; shift ;;
        --test-dir) TEST_DIR="${2:?}"; shift 2 ;;
        --test-dir=*) TEST_DIR="${1#*=}"; shift ;;
        --exec) shift; EXEC_CMD="$*"; break ;;
        --hold) HOLD=true; shift ;;
        --skip-cleanup) SKIP_CLEANUP=true; shift ;;
        *) qual_die "unknown argument: $1" ;;
    esac
done

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
[ "$(id -u)" -eq 0 ] || qual_die "must run as root"
[ -e /dev/kvm ] || qual_die "/dev/kvm missing — run env-preflight.sh on a KVM host"
[ -x /usr/bin/cloud-hypervisor ] || qual_die "/usr/bin/cloud-hypervisor missing — run env-preflight.sh"
[ -s "${CHV_QUAL_ROOT}/hypervisor-fw" ] || qual_die "firmware missing — run env-preflight.sh"
[ -s "${CHV_QUAL_ROOT}/images/${GUEST_IMAGE}" ] || qual_die "guest image missing — run env-preflight.sh"
for b in chv-controlplane chv-agent chv-stord chv-nwd chvctl; do
    [ -x "${BINARY_DIR}/${b}" ] || qual_die "binary missing: ${BINARY_DIR}/${b} — run env-preflight.sh --stage-binaries"
done
CANDIDATE_SHA="$(cat "${BINARY_DIR}/CANDIDATE_SHA" 2>/dev/null || echo unknown)"
qual_info "candidate SHA: ${CANDIDATE_SHA}"

# Service PIDs + deployment state (consumed by scenario scripts via the
# deployment map file).
CP_PID="" STORD_PID="" NWD_PID="" AGENT_PID=""

if [ -z "$TEST_DIR" ]; then
    TEST_DIR="$(mktemp -d /tmp/chv-qual-XXXXXX)"
else
    mkdir -p "$TEST_DIR"
fi
chmod 0700 "$TEST_DIR"
MAP_FILE="${TEST_DIR}/deployment.map"
qual_info "test directory: $TEST_DIR"

# Baseline snapshot for teardown residue assertions: host links + nft tables
# that exist BEFORE the deployment must all exist AFTER teardown, and no new
# ones may remain.
ip -o link show 2>/dev/null | awk -F': ' '{print $2}' | awk '{print $1}' \
    | sort > "${TEST_DIR}/links.before"
nft list tables 2>/dev/null | sort > "${TEST_DIR}/nft-tables.before" || true

cleanup() {
    local rc=$?
    if [ "$SKIP_CLEANUP" = true ]; then
        qual_info "skip-cleanup: services left running, test dir preserved: $TEST_DIR"
        return "$rc" 2>/dev/null || exit "$rc"
    fi
    qual_info "teardown starting"
    # Stop in reverse dependency order; SIGTERM then SIGKILL.
    for pid in "${AGENT_PID:-}" "${NWD_PID:-}" "${STORD_PID:-}" "${CP_PID:-}"; do
        [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
    done
    for _ in $(seq 1 50); do
        local alive=0
        for pid in "${AGENT_PID:-}" "${NWD_PID:-}" "${STORD_PID:-}" "${CP_PID:-}"; do
            [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && alive=1
        done
        [ "$alive" -eq 0 ] && break
        sleep 0.2
    done
    for pid in "${AGENT_PID:-}" "${NWD_PID:-}" "${STORD_PID:-}" "${CP_PID:-}"; do
        [ -n "$pid" ] && kill -9 "$pid" 2>/dev/null || true
    done
    # Kill any cloud-hypervisor the stack left behind (forbidden residue).
    pkill -x cloud-hypervisor 2>/dev/null || true
    sleep 1

    # --- Forbidden-outcome residue assertions ---
    assert_no_ch_residue "teardown"
    ip -o link show 2>/dev/null | awk -F': ' '{print $2}' | awk '{print $1}' \
        | sort > "${TEST_DIR}/links.after" || true
    local new_links
    new_links="$(comm -13 "${TEST_DIR}/links.before" "${TEST_DIR}/links.after" | grep -v '^$' || true)"
    if [ -z "$new_links" ]; then
        qual_pass "teardown: no new host links remain"
    else
        qual_error "teardown: FORBIDDEN new host links remain: $(echo "$new_links" | tr '\n' ' ')"
    fi
    nft list tables 2>/dev/null | sort > "${TEST_DIR}/nft-tables.after" || true
    local new_tables
    new_tables="$(comm -13 "${TEST_DIR}/nft-tables.before" "${TEST_DIR}/nft-tables.after" | grep -v '^$' || true)"
    if [ -z "$new_tables" ]; then
        qual_pass "teardown: no new nft tables remain"
    else
        qual_error "teardown: FORBIDDEN new nft tables remain: $(echo "$new_tables" | tr '\n' ' ')"
    fi

    rm -rf "$TEST_DIR"
    qual_info "teardown complete (test dir removed)"
    return "$rc" 2>/dev/null || exit "$rc"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# 1. Certificates, token, admin credential, configs
# ---------------------------------------------------------------------------
qual_info "generating certificates, bootstrap token, admin credential, configs"

certs_dir="${TEST_DIR}/certs"; logs_dir="${TEST_DIR}/logs"
cp_dir="${TEST_DIR}/controlplane"; agent_dir="${TEST_DIR}/agent"
stord_dir="${TEST_DIR}/stord"; nwd_dir="${TEST_DIR}/nwd"
images_dir="${CHV_QUAL_ROOT}/images"
mkdir -p "$certs_dir" "$logs_dir" "$cp_dir" "$stord_dir" "$nwd_dir" "${agent_dir}/storage"

# CA (2-day validity — long enough for a qualification run, short enough to
# be obviously throwaway).
openssl genrsa -out "$certs_dir/ca.key" 4096 2>/dev/null
openssl req -x509 -new -nodes -key "$certs_dir/ca.key" -sha256 -days 2 \
    -out "$certs_dir/ca.crt" \
    -subj "/O=CHV Qualification/CN=chv-qual-ca" 2>/dev/null

# Server cert with SAN localhost + 127.0.0.1 (control-plane gRPC/HTTP).
openssl genrsa -out "$certs_dir/server.key" 2048 2>/dev/null
openssl req -new -key "$certs_dir/server.key" -out "$certs_dir/server.csr" \
    -subj "/O=CHV Qualification/CN=localhost" 2>/dev/null
cat > "$certs_dir/server.ext" <<EOF
subjectAltName = DNS:localhost, IP:127.0.0.1
EOF
openssl x509 -req -in "$certs_dir/server.csr" \
    -CA "$certs_dir/ca.crt" -CAkey "$certs_dir/ca.key" \
    -CAcreateserial -out "$certs_dir/server.crt" \
    -days 2 -sha256 -extfile "$certs_dir/server.ext" 2>/dev/null
rm -f "$certs_dir/server.csr" "$certs_dir/server.ext"

# Pre-placed client cert used ONLY for the EnrollNode handshake.
openssl genrsa -out "$certs_dir/enroll-client.key" 2048 2>/dev/null
openssl req -new -key "$certs_dir/enroll-client.key" -out "$certs_dir/enroll-client.csr" \
    -subj "/O=CHV Qualification/CN=qual-enroll" 2>/dev/null
openssl x509 -req -in "$certs_dir/enroll-client.csr" \
    -CA "$certs_dir/ca.crt" -CAkey "$certs_dir/ca.key" \
    -CAcreateserial -out "$certs_dir/enroll-client.crt" \
    -days 2 -sha256 2>/dev/null
rm -f "$certs_dir/enroll-client.csr"

chmod 644 "$certs_dir"/*.crt
chmod 600 "$certs_dir"/*.key

# One-time bootstrap token (value only in this file, mode 0600; the DB gets
# the sha256 hash — same as the production validate_and_consume path).
BOOTSTRAP_TOKEN="$(openssl rand -hex 24)"
install -m 0600 /dev/null "${agent_dir}/bootstrap-token"
printf '%s\n' "$BOOTSTRAP_TOKEN" > "${agent_dir}/bootstrap-token"

# Admin credential (fresh per run; must_change_password=0).
ADMIN_PASSWORD="$(openssl rand -hex 12)"

# Migrations directory: repo checkout shape (package installs use
# /usr/share/chv/migrations — same files).
MIGRATIONS_DIR="${REPO_ROOT_OVERRIDE:-$(cd "${SCRIPT_DIR}/../../.." && pwd)}/cmd/chv-controlplane/migrations"
[ -d "$MIGRATIONS_DIR" ] || qual_die "migrations dir not found: $MIGRATIONS_DIR"

# --- controlplane.toml ---
cat > "${TEST_DIR}/controlplane.toml" <<EOF
grpc_bind = "127.0.0.1:8443"
http_bind = "127.0.0.1:8080"
log_level = "info"
runtime_dir = "${cp_dir}"
jwt_secret = "qual-$(openssl rand -hex 16)-jwt-secret"

[database]
url = "sqlite://${cp_dir}/controlplane.db"
migrations_dir = "${MIGRATIONS_DIR}"
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

# --- agent.toml (core-managed authority) ---
cat > "${TEST_DIR}/agent.toml" <<EOF
socket_path = "${agent_dir}/api.sock"
runtime_dir = "${agent_dir}"
log_level = "info"
authority_mode = "core-managed"
core_api_socket_path = "${agent_dir}/core.sock"
control_plane_addr = "https://127.0.0.1:8443"
stord_socket = "${stord_dir}/api.sock"
nwd_socket = "${nwd_dir}/api.sock"
chv_binary_path = "/usr/bin/cloud-hypervisor"
stord_binary_path = "${BINARY_DIR}/chv-stord"
nwd_binary_path = "${BINARY_DIR}/chv-nwd"
cache_path = "${agent_dir}/agent-cache.json"
node_id = "${NODE_ID}"
metrics_bind = "127.0.0.1:9100"
storage_base_dir = "${agent_dir}/storage"
console_bind = "127.0.0.1:8444"
bootstrap_token_path = "${agent_dir}/bootstrap-token"

tls_cert_path = "${certs_dir}/enroll-client.crt"
tls_key_path = "${certs_dir}/enroll-client.key"
ca_cert_path = "${certs_dir}/ca.crt"
EOF

# --- stord.toml (operator-shaped path allowlist) ---
cat > "${TEST_DIR}/stord.toml" <<EOF
socket_path = "${stord_dir}/api.sock"
runtime_dir = "${stord_dir}"
log_level = "info"
path_allowlist = ["${agent_dir}", "${stord_dir}", "${images_dir}"]
EOF

# --- nwd.toml ---
cat > "${TEST_DIR}/nwd.toml" <<EOF
socket_path = "${nwd_dir}/api.sock"
runtime_dir = "${nwd_dir}"
log_level = "info"
EOF

# Agent runtime dir MUST be 0700 for the Core-authority startup validation
# (cellhv-core-startup::validate_paths). Done last, after token file writes.
chmod 0700 "${agent_dir}"

qual_pass "configs generated (core-managed authority, real mTLS, 0700 runtime dir)"

# ---------------------------------------------------------------------------
# 2. Start control-plane, wait for migrations, seed admin + bootstrap token
# ---------------------------------------------------------------------------
qual_info "starting chv-controlplane"
"${BINARY_DIR}/chv-controlplane" "${TEST_DIR}/controlplane.toml" \
    > "${logs_dir}/controlplane.log" 2>&1 &
CP_PID=$!

DB="${cp_dir}/controlplane.db"
# Poll helper: the sqlite query must be re-evaluated on every attempt (a
# command substitution in the wait_for argument list would be expanded once).
cp_users_table_ready() {
    [ -s "$DB" ] || return 1
    [ -n "$(sqlite_query "$DB" "SELECT name FROM sqlite_master WHERE type='table' AND name='users'" 2>/dev/null)" ]
}
wait_for "control-plane up (migrations applied, users table exists)" 30 cp_users_table_ready \
    || qual_die "control-plane did not become ready — log: $(tail -20 "${logs_dir}/controlplane.log" 2>/dev/null)"

# Seed admin user (production: install.sh post-migration with a random
# password; here: fresh bcrypt hash, must_change_password=0).
ADMIN_HASH="$(bcrypt_hash "$ADMIN_PASSWORD")"
[ -n "$ADMIN_HASH" ] || qual_die "bcrypt hash generation failed (htpasswd)"
sqlite_exec "$DB" "INSERT OR REPLACE INTO users (user_id, username, password_hash, role, must_change_password) VALUES ('qual-admin', 'admin', '${ADMIN_HASH}', 'admin', 0)"
qual_pass "admin user seeded (bcrypt, fresh random password)"

# Seed the one-time bootstrap token (sha256 hex — same as validate_and_consume).
TOKEN_HASH="$(printf '%s' "$BOOTSTRAP_TOKEN" | python3 -c 'import hashlib,sys; print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest())')"
sqlite_exec "$DB" "INSERT INTO bootstrap_tokens (token_hash, description, one_time_use) VALUES ('${TOKEN_HASH}', 'qual harness one-time token', 1)"
qual_pass "one-time bootstrap token seeded"

# ---------------------------------------------------------------------------
# 3. Start stord + nwd, then the agent (enrolls at startup)
# ---------------------------------------------------------------------------
qual_info "starting chv-stord"
"${BINARY_DIR}/chv-stord" "${TEST_DIR}/stord.toml" \
    > "${logs_dir}/stord.log" 2>&1 &
STORD_PID=$!
wait_for "stord socket up" 20 test -S "${stord_dir}/api.sock" \
    || qual_die "stord did not come up — log: $(tail -20 "${logs_dir}/stord.log" 2>/dev/null)"

qual_info "starting chv-nwd"
"${BINARY_DIR}/chv-nwd" "${TEST_DIR}/nwd.toml" \
    > "${logs_dir}/nwd.log" 2>&1 &
NWD_PID=$!
wait_for "nwd socket up" 20 test -S "${nwd_dir}/api.sock" \
    || qual_die "nwd did not come up — log: $(tail -20 "${logs_dir}/nwd.log" 2>/dev/null)"

qual_info "starting chv-agent (authority_mode=core-managed; enrolls at startup)"
"${BINARY_DIR}/chv-agent" "${TEST_DIR}/agent.toml" \
    > "${logs_dir}/agent.log" 2>&1 &
AGENT_PID=$!

# Enrollment evidence: the control-plane-issued node cert (NOT the pre-placed
# enrollment client cert) must appear in the runtime dir.
wait_for "agent enrolled (CP-issued node cert present)" 60 \
    test -s "${agent_dir}/agent.crt" -a -s "${agent_dir}/agent.key" \
    || { qual_error "agent enrollment did not complete — log tail:"; tail -30 "${logs_dir}/agent.log" >&2; qual_die "aborting"; }
ISSUED_CN="$(openssl x509 -in "${agent_dir}/agent.crt" -noout -subject 2>/dev/null)"
assert_contains "issued node cert CN is the node id" "$ISSUED_CN" "CN = ${NODE_ID}"
assert_file_exists "core API socket up" "${agent_dir}/core.sock"
wait_for "agent gRPC socket up" 30 test -S "${agent_dir}/api.sock" \
    || qual_error "agent api.sock did not appear"

# The one-time token must be consumed exactly once.
TOKEN_STATE="$(sqlite_query "$DB" "SELECT used_at IS NOT NULL FROM bootstrap_tokens WHERE token_hash='${TOKEN_HASH}'")"
assert_contains "one-time bootstrap token consumed" "$TOKEN_STATE" "1"

# No insecure-mode escape hatch anywhere in the environment.
if env | grep -q '^CHV_ALLOW_INSECURE='; then
    qual_error "FORBIDDEN: CHV_ALLOW_INSECURE is set in the environment"
else
    qual_pass "CHV_ALLOW_INSECURE not set"
fi

# ---------------------------------------------------------------------------
# 4. Management-plane verification via chvctl (BFF → control-plane)
# ---------------------------------------------------------------------------
QUAL_CHVCTL="${BINARY_DIR}/chvctl"
QUAL_BFF_URL="http://127.0.0.1:8080"
QUAL_CHVCTL_CONFIG_DIR="${TEST_DIR}/chvctl-config"
mkdir -p "${QUAL_CHVCTL_CONFIG_DIR}/chvctl"

qual_chvctl login --username admin --password "$ADMIN_PASSWORD" >/dev/null \
    && qual_pass "chvctl login (bcrypt admin over BFF)" \
    || qual_error "chvctl login failed"

NODE_LIST="$(qual_chvctl node list --output json 2>/dev/null || true)"
assert_contains "node enrolled and visible via BFF" "$NODE_LIST" "${NODE_ID}"
assert_contains "node state TenantReady" "$NODE_LIST" "TenantReady"

# Create the qualification network (M2.5 shape: 'default', bridge-local).
qual_chvctl network create default --cidr "$NETWORK_CIDR" >/dev/null \
    && qual_pass "network 'default' created (${NETWORK_CIDR})" \
    || qual_error "network creation failed"

# Import the guest seed image via file:// source URL.
qual_chvctl image import ubuntu-noble --url "file://${images_dir}/${GUEST_IMAGE}" --format qcow2 >/dev/null \
    && qual_pass "image 'ubuntu-noble' imported (file://${images_dir}/${GUEST_IMAGE})" \
    || qual_error "image import failed"

# Health check.
qual_chvctl health overview >/dev/null 2>&1 \
    && qual_pass "chvctl health overview OK" \
    || qual_warn "chvctl health overview returned non-zero"

# ---------------------------------------------------------------------------
# 5. Deployment map (for scenario scripts) + summary
# ---------------------------------------------------------------------------
cat > "$MAP_FILE" <<EOF
TEST_DIR=${TEST_DIR}
CANDIDATE_SHA=${CANDIDATE_SHA}
NODE_ID=${NODE_ID}
NETWORK_CIDR=${NETWORK_CIDR}
CP_PID=${CP_PID}
STORD_PID=${STORD_PID}
NWD_PID=${NWD_PID}
AGENT_PID=${AGENT_PID}
DB=${DB}
AGENT_DIR=${agent_dir}
CERTS_DIR=${certs_dir}
LOGS_DIR=${logs_dir}
BINARY_DIR=${BINARY_DIR}
ADMIN_PASSWORD=${ADMIN_PASSWORD}
BFF_URL=${QUAL_BFF_URL}
CHVCTL_CONFIG_DIR=${QUAL_CHVCTL_CONFIG_DIR}
EOF
chmod 0600 "$MAP_FILE"

qual_info "deployment map: ${MAP_FILE}"
qual_info "stack running: CP=${CP_PID} STORD=${STORD_PID} NWD=${NWD_PID} AGENT=${AGENT_PID}"

# ---------------------------------------------------------------------------
# 6. Scenario hand-off (--exec / --hold) or deployment smoke (default)
# ---------------------------------------------------------------------------
qual_summary "deploy (stack up, verified)"

SCENARIO_RC=0
if [ -n "$EXEC_CMD" ]; then
    qual_info "running scenario: ${EXEC_CMD}"
    # shellcheck disable=SC2086
    if set -- $EXEC_CMD; env QUAL_MAP="$MAP_FILE" QUAL_TEST_DIR="$TEST_DIR" \
        QUAL_NODE_ID="$NODE_ID" QUAL_DB="$DB" QUAL_AGENT_DIR="$agent_dir" \
        QUAL_CERTS_DIR="$certs_dir" QUAL_LOGS_DIR="$logs_dir" \
        QUAL_BINARY_DIR="$BINARY_DIR" QUAL_BFF_URL="$QUAL_BFF_URL" \
        QUAL_ADMIN_PASSWORD="$ADMIN_PASSWORD" \
        QUAL_CHVCTL_CONFIG_DIR="$QUAL_CHVCTL_CONFIG_DIR" \
        QUAL_CHVCTL="$QUAL_CHVCTL" QUAL_CP_PID="$CP_PID" \
        QUAL_STORD_PID="$STORD_PID" QUAL_NWD_PID="$NWD_PID" \
        QUAL_AGENT_PID="$AGENT_PID" QUAL_NETWORK_CIDR="$NETWORK_CIDR" \
        "$@"; then
        qual_pass "scenario exited 0"
    else
        SCENARIO_RC=$?
        qual_error "scenario exited non-zero (${SCENARIO_RC})"
    fi
elif [ "$HOLD" = true ]; then
    qual_info "holding stack until SIGTERM/SIGINT ..."
    trap 'qual_info "signal received — tearing down"; exit 0' TERM INT
    while :; do sleep 5; done
else
    qual_info "deployment smoke complete — tearing down (use --exec/--hold to keep the stack for scenarios)"
fi

qual_summary "deploy + scenario (rc=${SCENARIO_RC})"
exit "$SCENARIO_RC"
