#!/usr/bin/env bash
# host-safety.sh — prompt-01's privileged host-safety suite, rebuilt as a
# committed, reusable script (prompt-04 M4.4, plan §M4.4 bullet 1).
#
# In prompt 01 the procedure was host-local (documented only in the test
# file's doc comment): build chv-nwd-core's test binaries, dig the
# #[ignore]d host_safety binary out of target/debug/deps, sudo-run its
# privileged tests, capture the nftables ruleset dump. This script commits
# that procedure so the gate is reproducible on any qualification host.
#
# What the suite proves (unchanged from prompt 01, all against the nwd
# firewall code built from THIS checkout):
#   - confines_policy_to_chv_owned_traffic
#     A CHV policy applied on synthetic chv-owned interfaces (bridge +
#     tap) drops CHV-bound traffic per policy while unrelated host-stack
#     INPUT/OUTPUT paths (veth pairs in foreign namespaces) stay
#     untouched — the regression shape of issue #227.
#   - exposure_survives_firewall_apply
#     Re-applying a firewall policy does not clobber exposure (NAT) rules
#     on the same table — the apply path must be atomic per concern.
#
# Usage (root; the tests skip silently without root, so this script
# refuses to run non-root rather than report a false pass):
#   sudo ./scripts/integration/qual/host-safety.sh \
#       [--dump FILE] [--sha <candidate-sha>]
#
#   --dump   evidence capture path for the nftables ruleset dump
#            (HOST_SAFETY_DUMP_PATH; default under the M4.4 artifacts dir)
#   --sha    candidate commit to prove code identity against. Default:
#            the pinned candidate SHA from the qualification bin dir
#            (/var/lib/chv/qual/bin/CANDIDATE_SHA) when present. The
#            script FAILS if crates/chv-nwd-core differs between that sha
#            and the working tree — the claim "the candidate passes the
#            host-safety gate" is only valid for the candidate's code.
#
# Runs standalone (no deploy.sh stack needed): the tests create and clean
# up their own table (chvhs-*), namespaces, and veth pairs, and gate on
# root + nft + ip themselves.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DUMP_PATH="${CHV_QUAL_ROOT:-/var/lib/chv/qual}/m4.4-artifacts/host-safety-ruleset.txt"
CANDIDATE_SHA=""

while [ $# -gt 0 ]; do
    case "$1" in
        --dump) DUMP_PATH="$2"; shift 2 ;;
        --sha) CANDIDATE_SHA="$2"; shift 2 ;;
        *) qual_die "unknown argument: $1 (usage: host-safety.sh [--dump FILE] [--sha SHA])" ;;
    esac
done

[ "$(id -u)" -eq 0 ] || qual_die "must run as root — the host-safety tests need real nftables (they self-skip otherwise, which would be a false pass)"
for tool in nft ip cargo; do
    command -v "$tool" >/dev/null 2>&1 \
        || qual_die "required tool missing: ${tool}"
done

mkdir -p "$(dirname "$DUMP_PATH")"
: > "$DUMP_PATH" || qual_die "cannot write dump path: ${DUMP_PATH}"

# ---------------------------------------------------------------------------
# 1. Candidate identity guard
# ---------------------------------------------------------------------------
# The suite must exercise the CANDIDATE's nwd firewall code. The tests are
# built from this checkout, so the claim is only valid when
# crates/chv-nwd-core is identical between the candidate sha and the
# working tree. (Verified for baa20c0e..HEAD at M4.4 time: the diff is
# empty. If a future candidate differs, build from a worktree of that sha
# instead — this guard fails loudly rather than let the claim go stale.)
if [ -z "$CANDIDATE_SHA" ] && [ -f "${CHV_QUAL_ROOT:-/var/lib/chv/qual}/bin/CANDIDATE_SHA" ]; then
    CANDIDATE_SHA="$(cat "${CHV_QUAL_ROOT:-/var/lib/chv/qual}/bin/CANDIDATE_SHA")"
fi
if [ -n "$CANDIDATE_SHA" ]; then
    if git -C "$REPO_ROOT" diff --quiet "$CANDIDATE_SHA" HEAD -- crates/chv-nwd-core; then
        qual_pass "candidate identity: crates/chv-nwd-core identical at ${CANDIDATE_SHA:0:8} and HEAD"
    else
        qual_die "crates/chv-nwd-core DIFFERS between candidate ${CANDIDATE_SHA:0:8} and HEAD — building from this tree would not test the candidate (use a worktree at the candidate sha)"
    fi
else
    qual_warn "no candidate sha available (no --sha, no CANDIDATE_SHA file) — identity check skipped; results apply to THIS tree only"
fi

# ---------------------------------------------------------------------------
# 2. Build the test binaries
# ---------------------------------------------------------------------------
qual_info "building chv-nwd-core test binaries (cargo test --no-run)..."
if ! (cd "$REPO_ROOT" && cargo test -p chv-nwd-core --no-run 2>&1 | tail -5); then
    qual_die "cargo test --no-run failed for chv-nwd-core"
fi

TEST_BIN="$(find "${REPO_ROOT}/target/debug/deps" -maxdepth 1 -name 'host_safety-*' -type f -executable 2>/dev/null | sort | tail -1)"
[ -n "$TEST_BIN" ] || qual_die "could not locate the host_safety test binary under target/debug/deps"
qual_info "test binary: ${TEST_BIN}"

# ---------------------------------------------------------------------------
# 3. Run the privileged tests (the prompt-01 suite)
# ---------------------------------------------------------------------------
# Baseline: host links + nft tables the tests must not disturb (they
# create their own chvhs-* table, namespaces and veth pairs and remove
# them on drop; a leaked one is a failure the diff below catches).
LINKS_BEFORE="$(ip -o link show 2>/dev/null | awk -F': ' '{print $2}' | awk '{print $1}' | sort)"
TABLES_BEFORE="$(nft list tables 2>/dev/null | sort)"

FAILED=0
for test_name in confines_policy_to_chv_owned_traffic exposure_survives_firewall_apply; do
    qual_info "running ${test_name} (ignored/root-gated integration test)..."
    if HOST_SAFETY_DUMP_PATH="$DUMP_PATH" "$TEST_BIN" --ignored --exact --nocapture "$test_name" >/dev/null 2>&1; then
        qual_pass "host-safety: ${test_name}"
    else
        qual_error "host-safety: ${test_name} FAILED"
        FAILED=1
    fi
done

# ---------------------------------------------------------------------------
# 4. Host-stack residue assertions (forbidden outcomes)
# ---------------------------------------------------------------------------
LINKS_AFTER="$(ip -o link show 2>/dev/null | awk -F': ' '{print $2}' | awk '{print $1}' | sort)"
TABLES_AFTER="$(nft list tables 2>/dev/null | sort)"

NEW_LINKS="$(comm -13 <(printf '%s\n' "$LINKS_BEFORE") <(printf '%s\n' "$LINKS_AFTER") | grep -v '^$' || true)"
if [ -z "$NEW_LINKS" ]; then
    qual_pass "host-safety: no leaked links (namespaces/veth pairs cleaned up)"
else
    qual_error "host-safety: FORBIDDEN leaked links: $(echo "$NEW_LINKS" | tr '\n' ' ')"
    FAILED=1
fi
NEW_TABLES="$(comm -13 <(printf '%s\n' "$TABLES_BEFORE") <(printf '%s\n' "$TABLES_AFTER") | grep -v '^$' || true)"
if [ -z "$NEW_TABLES" ]; then
    qual_pass "host-safety: no leaked nft tables (chvhs-* removed)"
else
    qual_error "host-safety: FORBIDDEN leaked nft tables: $(echo "$NEW_TABLES" | tr '\n' ' ')"
    FAILED=1
fi

qual_info "nftables ruleset dump: ${DUMP_PATH} ($(wc -l < "$DUMP_PATH") lines)"

if [ "$FAILED" -ne 0 ]; then
    qual_die "host-safety suite FAILED"
fi
qual_pass "host-safety suite passed (prompt-01 gate re-proven on this host)"
