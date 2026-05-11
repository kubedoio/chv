#!/bin/bash
# Smoke test for version standardization.
# Validates VERSION, Cargo.toml, and version.sh outputs.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

ERRORS=0

error() {
    echo "FAIL: $1" >&2
    ERRORS=$((ERRORS + 1))
}

# ---------------------------------------------------------------------------
# 1. VERSION must be SemVer three-segment
# ---------------------------------------------------------------------------
VERSION="$(cat VERSION)"
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    error "VERSION ($VERSION) does not match ^\\d+\\.\\d+\\.\\d+$"
fi

# ---------------------------------------------------------------------------
# 2. All binary crate Cargo.toml versions must match VERSION
# ---------------------------------------------------------------------------
for crate_toml in cmd/*/Cargo.toml; do
    CRATE_VERSION="$(grep -m1 '^version = ' "$crate_toml" | sed -E 's/version = "([^"]+)".*/\1/')"
    if [ "$CRATE_VERSION" != "$VERSION" ]; then
        error "$crate_toml version ($CRATE_VERSION) does not match VERSION ($VERSION)"
    fi
done

# ---------------------------------------------------------------------------
# 3. Validate version.sh outputs
# ---------------------------------------------------------------------------

# stable
STABLE="$("${REPO_ROOT}/scripts/version.sh" stable)"
if [ "$STABLE" != "$VERSION" ]; then
    error "version.sh stable returned '$STABLE', expected '$VERSION'"
fi

# rc 1
RC1="$("${REPO_ROOT}/scripts/version.sh" rc 1)"
EXPECTED_RC1="${VERSION}~rc.1"
if [ "$RC1" != "$EXPECTED_RC1" ]; then
    error "version.sh rc 1 returned '$RC1', expected '$EXPECTED_RC1'"
fi

# nightly
NIGHTLY="$("${REPO_ROOT}/scripts/version.sh" nightly)"
if ! [[ "$NIGHTLY" =~ ^${VERSION}~nightly\.[0-9]{8}(\.g[0-9a-f]+)?$ ]]; then
    error "version.sh nightly returned '$NIGHTLY', expected '${VERSION}~nightly.YYYYMMDD[.gSHORTSHA]'"
fi

# pr 42
PR42="$("${REPO_ROOT}/scripts/version.sh" pr 42)"
if ! [[ "$PR42" =~ ^${VERSION}~pr42\.[0-9]{8}(\.g[0-9a-f]+)?$ ]]; then
    error "version.sh pr 42 returned '$PR42', expected '${VERSION}~pr42.YYYYMMDD[.gSHORTSHA]'"
fi

# rpm stable
RPM_STABLE="$("${REPO_ROOT}/scripts/version.sh" --rpm stable)"
if [ "$RPM_STABLE" != "$VERSION" ]; then
    error "version.sh --rpm stable returned '$RPM_STABLE', expected '$VERSION'"
fi

# rpm rc 1
RPM_RC1="$("${REPO_ROOT}/scripts/version.sh" --rpm rc 1)"
EXPECTED_RPM_RC1="${VERSION}-0.1.rc1"
if [ "$RPM_RC1" != "$EXPECTED_RPM_RC1" ]; then
    error "version.sh --rpm rc 1 returned '$RPM_RC1', expected '$EXPECTED_RPM_RC1'"
fi

# rpm nightly
RPM_NIGHTLY="$("${REPO_ROOT}/scripts/version.sh" --rpm nightly)"
if ! [[ "$RPM_NIGHTLY" =~ ^${VERSION}\^[a-z0-9]+\.[0-9]{8}(\.g[0-9a-f]+)?$ ]]; then
    error "version.sh --rpm nightly returned '$RPM_NIGHTLY', expected '${VERSION}^nightly.YYYYMMDD[.gSHORTSHA]'"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
if [ "$ERRORS" -gt 0 ]; then
    echo "Smoke version test failed with $ERRORS error(s)." >&2
    exit 1
fi

echo "Smoke version test passed."
