#!/usr/bin/env bash
# Copy and redact an explicit allowlist of Phase A2 text evidence.

set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly INPUTS_PATH="${1:-}"
readonly FILE_LIST="${2:-}"
readonly DESTINATION="${3:-}"

fail() { printf '[FAIL] %s\n' "$*" >&2; exit 1; }

[[ $# -eq 3 ]] || fail "usage: $0 INPUTS.env FILES.tsv NEW_EVIDENCE_DIRECTORY"
"${SCRIPT_DIR}/preflight.sh" "$INPUTS_PATH"
exec python3 "${SCRIPT_DIR}/collect.py" "$INPUTS_PATH" "$FILE_LIST" "$DESTINATION"
