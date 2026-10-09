#!/bin/bash
# CHV NetBox Qualification — real-NetBox lane driver (issue #586,
# ADR-024 decision 4, lane 3).
#
# Boots the pinned, disposable NetBox compose stack
# (deploy/netbox-qualification), waits for it to publish the
# qualification environment, runs the five `qualification_*`
# wrappers of the composed projection suite against the live
# instance, optionally re-records the golden netbox4 fixtures from
# it (--record), and tears the stack down again.
#
# This is the tripwire that keeps the in-process simulator honest:
# the same scenario code that runs against the simulator on every
# `cargo test` runs here against the real thing, so wire-shape drift
# fails loudly instead of silently accumulating.
#
# Usage:
#   ./scripts/netbox-qualify.sh [OPTIONS]
#   make netbox-qualify                       # same thing
#
# Options:
#   --record      Also re-record the golden fixtures under
#                 crates/chv-netbox-sim/tests/fixtures/netbox4/
#                 from the live instance (the recorder test), then
#                 run the fixture fidelity tests against the freshly
#                 recorded set before tearing down.
#   --keep        Keep the compose stack up on exit (re-poke it with
#                 docker compose -f deploy/netbox-qualification/
#                 docker-compose.yml ps; NetBox UI: chv-qualification
#                 / chv-qualification-password on 127.0.0.1:18780)
#   --print-env   Boot (or reuse) the stack, wait until the
#                 qualification environment is published, print the
#                 two variables, and exit with the stack still up
#                 (implies --keep) — for driving the lane manually:
#                   eval "$(./scripts/netbox-qualify.sh --print-env)"
#   --help        This help
#
# Environment:
#   NETBOX_QUALIFY_BOOT_TIMEOUT  Seconds to wait for the stack to
#                                become ready (default: 900 — the
#                                first run pulls the images and runs
#                                NetBox's migrations)
#
# Prerequisites: docker with the compose plugin, curl, and the Rust
# toolchain (the scenario suite is cargo-run). The suite's TestDb is
# in-memory SQLite; the only database the lane needs is the one in
# the compose stack itself.
#
# Logs: /tmp/chv-netbox-qualify-logs/ (run.log, compose.log) — the
# CI workflow uploads this directory on failure.

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers (repo script conventions)
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

info()    { echo "[netbox-qualify] [INFO] $*"; }
warn()    { echo "[netbox-qualify] [WARN] $*" >&2; }
fatal()   { echo "[netbox-qualify] [FATAL] $*" >&2; exit 1; }

usage() {
    # Print the header comment (everything from line 2 up to the first
    # line that is not part of it) as the help text.
    awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' \
        "${BASH_SOURCE[0]}"
    exit 0
}

# ---------------------------------------------------------------------------
# Options
# ---------------------------------------------------------------------------
RECORD=false
KEEP=false
PRINT_ENV=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --record)     RECORD=true; shift ;;
        --keep)       KEEP=true; shift ;;
        --print-env)  PRINT_ENV=true; KEEP=true; shift ;;
        --help|-h)    usage ;;
        *)            fatal "unknown option: $1 (try --help)" ;;
    esac
done

BOOT_TIMEOUT="${NETBOX_QUALIFY_BOOT_TIMEOUT:-900}"

COMPOSE_FILE="${REPO_ROOT}/deploy/netbox-qualification/docker-compose.yml"
COMPOSE=(docker compose -f "$COMPOSE_FILE")

LOG_DIR="/tmp/chv-netbox-qualify-logs"
RUN_LOG="${LOG_DIR}/run.log"
COMPOSE_LOG="${LOG_DIR}/compose.log"
ENV_STAGING="${LOG_DIR}/qualification.env"

command -v docker >/dev/null 2>&1 || fatal "docker not found"
docker compose version >/dev/null 2>&1 || fatal "docker compose plugin not found"
command -v curl  >/dev/null 2>&1 || fatal "curl not found"
command -v cargo >/dev/null 2>&1 || fatal "cargo not found (rust toolchain required)"

cd "$REPO_ROOT"
mkdir -p "$LOG_DIR"
: > "$RUN_LOG"

# ---------------------------------------------------------------------------
# Teardown on any exit path
# ---------------------------------------------------------------------------
STACK_UP=false

cleanup() {
    trap - EXIT INT TERM
    local status=$?
    if [[ "$STACK_UP" == true ]]; then
        "${COMPOSE[@]}" logs --no-color > "$COMPOSE_LOG" 2>/dev/null || true
        if [[ "$KEEP" == true ]]; then
            info "stack kept up: ${COMPOSE[*]} ps (tear down later with: ${COMPOSE[*]} down -v)"
        else
            info "tearing the compose stack down (down -v)..."
            "${COMPOSE[@]}" down -v > /dev/null 2>&1 || \
                warn "docker compose down -v failed — clean up manually: ${COMPOSE[*]} down -v"
        fi
    fi
    info "logs: ${RUN_LOG} (compose: ${COMPOSE_LOG})"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# ---------------------------------------------------------------------------
# Boot the stack and wait for the qualification environment
# ---------------------------------------------------------------------------
info "booting the NetBox qualification stack (${COMPOSE_FILE})..."
"${COMPOSE[@]}" up -d > /dev/null
STACK_UP=true

# The one-shot qualification-init service publishes netbox.env onto
# the shared volume once NetBox is healthy and the token is minted;
# waiting for the file is waiting for the whole boot chain.
info "waiting for the qualification environment (first run pulls images + runs migrations; timeout ${BOOT_TIMEOUT}s)..."
deadline=$((SECONDS + BOOT_TIMEOUT))
while :; do
    if "${COMPOSE[@]}" exec -T netbox cat /run/qualification/netbox.env \
            > "$ENV_STAGING" 2>/dev/null && [[ -s "$ENV_STAGING" ]]; then
        break
    fi
    if (( SECONDS >= deadline )); then
        fatal "the qualification stack did not become ready within ${BOOT_TIMEOUT}s —

--- docker compose ps ---
$("${COMPOSE[@]}" ps 2>/dev/null || true)
--- netbox log (tail) ---
$("${COMPOSE[@]}" logs --no-color --tail 40 netbox 2>/dev/null || true)

Full logs land in ${COMPOSE_LOG}; the stack has been torn down."
    fi
    sleep 5
done

# shellcheck disable=SC1090
source "$ENV_STAGING"
[[ -n "${NETBOX_QUALIFICATION_URL:-}" ]]   || fatal "netbox.env carries no NETBOX_QUALIFICATION_URL"
[[ -n "${NETBOX_QUALIFICATION_TOKEN:-}" ]] || fatal "netbox.env carries no NETBOX_QUALIFICATION_TOKEN"

# Belt and braces: the API must actually answer (authenticated) before
# the suite runs — a published env with a dead API would only move the
# failure into the test harness.
info "waiting for the NetBox API at ${NETBOX_QUALIFICATION_URL}..."
deadline=$((SECONDS + 120))
while :; do
    if curl -sf -o /dev/null \
            -H "Authorization: Token ${NETBOX_QUALIFICATION_TOKEN}" \
            "${NETBOX_QUALIFICATION_URL}/api/status/"; then
        break
    fi
    if (( SECONDS >= deadline )); then
        fatal "the NetBox API did not answer /api/status/ within 120s"
    fi
    sleep 2
done
info "NetBox is up and the qualification token authenticates."

export NETBOX_QUALIFICATION_URL NETBOX_QUALIFICATION_TOKEN

# ---------------------------------------------------------------------------
# --print-env: hand the environment to a shell and leave the stack up
# ---------------------------------------------------------------------------
if [[ "$PRINT_ENV" == true ]]; then
    echo "NETBOX_QUALIFICATION_URL=${NETBOX_QUALIFICATION_URL}"
    echo "NETBOX_QUALIFICATION_TOKEN=${NETBOX_QUALIFICATION_TOKEN}"
    info "--print-env: stack left up (implies --keep); run the suite manually with:"
    info "  NETBOX_QUALIFICATION_URL=... NETBOX_QUALIFICATION_TOKEN=... \\"
    info "    cargo test -p chv-controlplane-service --lib qualification -- --ignored --test-threads=1 --nocapture"
    exit 0
fi

# ---------------------------------------------------------------------------
# The qualification suite (the five qualification_* wrappers)
# ---------------------------------------------------------------------------
FAILED=0

info "running the real-NetBox qualification suite (5 scenarios)..."
if ! cargo test -p chv-controlplane-service --lib qualification \
        -- --ignored --test-threads=1 --nocapture 2>&1 | tee -a "$RUN_LOG"; then
    FAILED=1
    warn "the qualification suite FAILED against the real NetBox — this is the \
drift tripwire firing: compare the failing assertions with the simulator arm's \
behavior before touching either side."
fi

# ---------------------------------------------------------------------------
# --record: refresh the golden fixtures from the live instance
# ---------------------------------------------------------------------------
if [[ "$RECORD" == true ]]; then
    info "--record: re-recording the golden fixtures from the live instance..."
    # The stable fixture base matches the fixtures' canonical
    # http://netbox.example.com URLs (see the fixtures' README).
    if ! NETBOX_RECORD_BASE_URL=http://netbox.example.com \
            cargo test -p chv-netbox-sim --test record_fixtures \
            -- --ignored --nocapture 2>&1 | tee -a "$RUN_LOG"; then
        FAILED=1
        warn "fixture re-recording FAILED"
    else
        # The freshly recorded set must satisfy the fidelity suite
        # before the stack goes away: a recording the simulator cannot
        # reproduce is a broken recording, not a better fixture.
        info "running the fixture fidelity tests against the freshly recorded set..."
        if ! cargo test -p chv-netbox-sim --test fixture_tests 2>&1 | tee -a "$RUN_LOG"; then
            FAILED=1
            warn "the fixture fidelity tests FAILED against the freshly recorded fixtures"
        fi
    fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo
info "==================== qualification summary ===================="
grep "test result:" "$RUN_LOG" | sed 's/^/  /' || true
if [[ "$FAILED" == 0 ]]; then
    info "RESULT: PASS — the real NetBox behaved as the simulator models it."
else
    warn "RESULT: FAIL — see ${RUN_LOG} and ${COMPOSE_LOG}."
fi
info "================================================================"

exit "$FAILED"
