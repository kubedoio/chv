#!/usr/bin/env bash
#
# check-no-println.sh — Enforce ADR-009 logging rule.
#
# ADR-009 (docs/specs/adr/009-logging-and-observability.md) mandates:
#   "Library crates must use tracing::info!, tracing::warn!, tracing::error!,
#    tracing::debug!"
#   "println! and eprintln! are forbidden in library and service code"
#   "CLI tools may use println! for user-facing output only"
#
# This script enforces the ban on println!/eprintln!/print!/eprint! in
# library crates under crates/. Binary crates under cmd/ are intentionally
# excluded:
#   - cmd/chvctl is a CLI tool (allowed by ADR-009).
#   - cmd/<daemon>/src/main.rs uses println! exclusively for --version output,
#     which is conventional CLI surface.
#   - cmd/*/build.rs uses println! to emit cargo:rustc-env directives, which
#     is the documented Cargo build-script API and not application logging.
#
# Test code (#[cfg(test)] blocks, tests/ trees) is also out of scope: tracing
# is for production diagnostics; tests can use stdout freely. We exclude
# `*/tests/*` paths and `*_test.rs` files.
#
# Exit codes:
#   0 — no violations
#   1 — violations found (printed to stderr)
#   2 — script invocation error

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Library crates — ADR-009 hard ban applies here.
SEARCH_ROOTS=("crates")

PATTERN='\b(println|eprintln|print|eprint)!'

# grep -E: extended regex; -r: recursive; -n: line numbers; --include: only .rs.
# We deliberately do NOT exclude tests inside crates/ at this stage because
# library tests should also use tracing where they assert on logs; if a
# legitimate test-only println! appears, annotate the call site with
# `#[allow]` and document the reason.
violations="$(grep -rEn \
  --include='*.rs' \
  --exclude-dir='target' \
  "$PATTERN" \
  "${SEARCH_ROOTS[@]}" 2>/dev/null || true)"

if [[ -n "$violations" ]]; then
  echo "ADR-009 violation: println!/eprintln! found in library crates." >&2
  echo "" >&2
  echo "$violations" >&2
  echo "" >&2
  echo "Fix: replace with tracing::info!, tracing::warn!, tracing::error!," >&2
  echo "or tracing::debug! per docs/specs/adr/009-logging-and-observability.md." >&2
  exit 1
fi

echo "ADR-009 check passed: no println!/eprintln! in library crates."
exit 0
