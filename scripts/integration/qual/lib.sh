#!/usr/bin/env bash
# lib.sh — shared helpers for the prompt-04 qualification harness
# (scripts/integration/qual/).
#
# Sourced by env-preflight.sh, deploy.sh and the per-milestone scenario
# scripts. Provides logging, hard/soft assertions, wait loops, a hermetic
# chvctl wrapper, and forbidden-outcome residue checks.
#
# Conventions:
# - A scenario FAILS on the first hard assertion (die); soft assertions
#   (error/pass) accumulate in ERRORS and fail the run at the end.
# - Forbidden-outcome assertions are first-class: every destructive or
#   recovery scenario must assert what must NOT exist/process/state remain,
#   not merely eventual success.
# - Nothing here uses CHV_ALLOW_INSECURE; a production build refuses it
#   anyway (prompt 03 / #233 + the agent gate).

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------
QUAL_ERRORS=0
QUAL_WARNINGS=0

qual_info() { echo "[QUAL][INFO] $*" >&2; }
qual_pass() { echo "[QUAL][PASS] $*" >&2; }
qual_warn() { echo "[QUAL][WARN] $*" >&2; QUAL_WARNINGS=$((QUAL_WARNINGS + 1)); }
qual_error() {
    echo "[QUAL][FAIL] $*" >&2
    QUAL_ERRORS=$((QUAL_ERRORS + 1))
    if [ -n "${QUAL_FAIL_FAST:-}" ]; then
        exit 1
    fi
}
qual_die() { echo "[QUAL][FATAL] $*" >&2; exit 1; }

qual_summary() {
    local label="$1"
    echo "" >&2
    echo "[QUAL] ===== $label =====" >&2
    echo "[QUAL] errors=$QUAL_ERRORS warnings=$QUAL_WARNINGS" >&2
    [ "$QUAL_ERRORS" -eq 0 ] || return 1
}

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
assert_cmd_ok() {
    local desc="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        qual_pass "$desc"
    else
        qual_error "$desc (command failed: $*)"
        return 1
    fi
}

assert_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if printf '%s' "$haystack" | grep -qF -- "$needle"; then
        qual_pass "$desc"
    else
        qual_error "$desc (missing: '$needle')"
        return 1
    fi
}

assert_not_contains() {
    local desc="$1" haystack="$2" needle="$3"
    if printf '%s' "$haystack" | grep -qF -- "$needle"; then
        qual_error "$desc (forbidden content present: '$needle')"
        return 1
    fi
    qual_pass "$desc"
}

assert_file_exists() {
    local desc="$1" path="$2"
    if [ -e "$path" ]; then
        qual_pass "$desc"
    else
        qual_error "$desc (missing: $path)"
        return 1
    fi
}

assert_file_contains() {
    local desc="$1" path="$2" needle="$3"
    if [ ! -e "$path" ]; then
        qual_error "$desc (file missing: $path)"
        return 1
    fi
    if grep -qF -- "$needle" "$path" 2>/dev/null; then
        qual_pass "$desc"
    else
        qual_error "$desc ('$needle' not in $path)"
        return 1
    fi
}

assert_process_alive() {
    local desc="$1" pid="$2"
    if kill -0 "$pid" 2>/dev/null; then
        qual_pass "$desc"
    else
        qual_error "$desc (pid $pid not alive)"
        return 1
    fi
}

# ---------------------------------------------------------------------------
# Wait loops
# ---------------------------------------------------------------------------
# wait_for DESC TIMEOUT_SECS CMD... — poll CMD until it exits 0.
wait_for() {
    local desc="$1" timeout="$2"
    shift 2
    local waited=0
    while ! "$@" >/dev/null 2>&1; do
        sleep 2
        waited=$((waited + 2))
        if [ "$waited" -ge "$timeout" ]; then
            qual_error "$desc (timed out after ${timeout}s)"
            return 1
        fi
    done
    qual_pass "$desc"
}

# ---------------------------------------------------------------------------
# Forbidden-outcome residue checks (used by every teardown)
# ---------------------------------------------------------------------------
# count_cloud_hypervisor_processes — number of running cloud-hypervisor
# processes on the host (excluding grep itself). pgrep exits 1 when none
# match (the desired state) — must not trip pipefail.
count_cloud_hypervisor_processes() {
    { pgrep -x cloud-hypervisor 2>/dev/null || true; } | wc -l | tr -d ' '
}

# assert_no_ch_residue DESC — no cloud-hypervisor processes remain.
assert_no_ch_residue() {
    local desc="$1"
    local n
    n="$(count_cloud_hypervisor_processes)"
    if [ "$n" -eq 0 ]; then
        qual_pass "$desc (no cloud-hypervisor processes)"
    else
        qual_error "$desc (FORBIDDEN: $n cloud-hypervisor process(es) remain)"
        pgrep -ax cloud-hypervisor >&2 || true
        return 1
    fi
}

# assert_no_tap_residue DESC PREFIX — no tap interfaces with PREFIX remain.
assert_no_tap_residue() {
    local desc="$1" prefix="$2"
    local taps
    taps="$(ip -o link show 2>/dev/null | awk -F': ' '{print $2}' | grep -E "^${prefix}" || true)"
    if [ -z "$taps" ]; then
        qual_pass "$desc (no tap residue)"
    else
        qual_error "$desc (FORBIDDEN: tap interfaces remain: $(echo "$taps" | tr '\n' ' '))"
        return 1
    fi
}

# ---------------------------------------------------------------------------
# Hermetic chvctl wrapper
# ---------------------------------------------------------------------------
# qual_chvctl — runs chvctl with the deployment's server URL and isolated
# credential storage. Requires QUAL_CHVCTL, QUAL_BFF_URL and
# QUAL_CHVCTL_CONFIG_DIR to be set by the sourcing script.
qual_chvctl() {
    [ -n "${QUAL_CHVCTL:-}" ] || qual_die "QUAL_CHVCTL not set (path to chvctl binary)"
    [ -n "${QUAL_BFF_URL:-}" ] || qual_die "QUAL_BFF_URL not set"
    [ -n "${QUAL_CHVCTL_CONFIG_DIR:-}" ] || qual_die "QUAL_CHVCTL_CONFIG_DIR not set"
    XDG_CONFIG_HOME="$QUAL_CHVCTL_CONFIG_DIR" \
        "$QUAL_CHVCTL" --server "$QUAL_BFF_URL" "$@"
}

# ---------------------------------------------------------------------------
# Misc helpers
# ---------------------------------------------------------------------------
sha256_of() { sha256sum "$1" | awk '{print $1}'; }

# sqlite_exec DB SQL — execute SQL against a sqlite DB (python3 stdlib; the
# sqlite3 CLI is not guaranteed on qualification hosts).
sqlite_exec() {
    local db="$1" sql="$2"
    python3 - "$db" "$sql" <<'PYEOF'
import sqlite3, sys
db, sql = sys.argv[1], sys.argv[2]
conn = sqlite3.connect(db)
conn.execute(sql)
conn.commit()
conn.close()
PYEOF
}

# sqlite_query DB SQL — print rows (pipe-separated) from a sqlite DB.
# CRITICAL: live reads of a database the control-plane has open MUST use a
# read-only URI — a read-write python connection that closes while the CP's
# pool connections are idle acquires the exclusive lock, checkpoints, and
# UNLINKS the -wal/-shm sidecars; the CP's open connections then write new
# commits into the unlinked WAL inode (silent data loss). Verified
# empirically with strace (M4.1 evidence §4).
sqlite_query() {
    local db="$1" sql="$2"
    python3 - "$db" "$sql" <<'PYEOF'
import sqlite3, sys
db, sql = sys.argv[1], sys.argv[2]
conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
for row in conn.execute(sql):
    print("|".join(str(c) if c is not None else "" for c in row))
conn.close()
PYEOF
}

# bcrypt_hash PASSWORD — print a bcrypt hash (htpasswd from apache2-utils;
# installed by env-preflight.sh).
bcrypt_hash() {
    htpasswd -bnBC 10 "" "$1" 2>/dev/null | tr -d ':\n' | sed 's/^\$2y\$/\$2b\$/'
}
