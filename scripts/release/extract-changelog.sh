#!/bin/bash
# Extract the changelog section for a specific version from CHANGELOG.md.
#
# Usage:
#   ./scripts/release/extract-changelog.sh VERSION
#
# Example:
#   ./scripts/release/extract-changelog.sh 0.1.0
#   ./scripts/release/extract-changelog.sh 0.1.0-rc.1
#
# Exit codes:
#   0 - section found and printed to stdout
#   1 - section not found

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

CHANGELOG="${REPO_ROOT}/CHANGELOG.md"
VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 VERSION" >&2
    echo "Example: $0 0.1.0" >&2
    exit 1
fi

if [[ ! -f "$CHANGELOG" ]]; then
    echo "Error: CHANGELOG.md not found at $CHANGELOG" >&2
    exit 1
fi

# Strip leading 'v' if present
VERSION="${VERSION#v}"

TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT

# Extract the section for this version.
# Match headers like: ## [0.1.0] - 2026-05-10
# or: ## [0.1.0-rc.1] - 2026-05-10
awk -v ver="$VERSION" '
    BEGIN { found=0 }
    /^## \[/ {
        # Extract version from header: ## [X.Y.Z] ...
        match($0, /^## \[([^]]+)\]/, arr)
        header_ver = arr[1]
        if (found && header_ver != ver) {
            exit
        }
        if (header_ver == ver) {
            found = 1
        }
    }
    found { print }
' "$CHANGELOG" > "$TMPFILE"

if [[ ! -s "$TMPFILE" ]]; then
    echo "Error: No changelog entry found for version ${VERSION}" >&2
    echo "Expected a section like '## [${VERSION}] - YYYY-MM-DD' in CHANGELOG.md" >&2
    exit 1
fi

cat "$TMPFILE"
