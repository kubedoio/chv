#!/bin/bash
# Derive package versions for different release channels.
#
# Pipeline role: Called by build-packages.sh and CI workflows to generate
# version strings from the VERSION file.
# Environment override: CHV_PKG_PRERELEASE
# Usage: ./scripts/version.sh [--rpm|--deb] [stable|rc N|nightly|pr N]
#
# Pre-release channels use the `~` suffix on both formats: `~` is the
# pre-release operator in Debian (dpkg) and RPM (rpmvercmp) version
# comparison alike, so nightly < rc < stable on .deb and .rpm. The --rpm
# and --deb flags are accepted for call-site compatibility; the output is
# identical for both formats.
#
# Environment:
#   CHV_PKG_PRERELEASE - if set, used as the pre-release suffix instead of deriving.
#                        Example: rc.1 produces 0.1.0~rc.1

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BASE_VERSION="$(cat "${REPO_ROOT}/VERSION")"

ARGS=()

# --rpm/--deb are accepted for call-site compatibility; both formats use the
# same `~` pre-release suffix, so the flags do not change the output.
for arg in "$@"; do
    case "$arg" in
        --rpm|--deb) ;;
        *) ARGS+=("$arg") ;;
    esac
done

CHANNEL="${ARGS[0]:-stable}"

get_git_sha() {
    if command -v git &>/dev/null && git -C "${REPO_ROOT}" rev-parse --git-dir &>/dev/null 2>&1; then
        git -C "${REPO_ROOT}" rev-parse --short HEAD 2>/dev/null || true
    fi
}

get_date() {
    date +%Y%m%d
}

# If CHV_PKG_PRERELEASE is set, use it directly as the suffix.
# `~` sorts below the stable release on both Debian and RPM, so no
# format-specific munging is needed.
if [ -n "${CHV_PKG_PRERELEASE:-}" ]; then
    echo "${BASE_VERSION}~${CHV_PKG_PRERELEASE}"
    exit 0
fi

case "$CHANNEL" in
    stable)
        echo "$BASE_VERSION"
        ;;
    rc)
        N="${ARGS[1]:-1}"
        echo "${BASE_VERSION}~rc.${N}"
        ;;
    nightly)
        DATE="$(get_date)"
        SHA="$(get_git_sha || true)"
        if [ -n "$SHA" ]; then
            SUFFIX="nightly.${DATE}.g${SHA}"
        else
            SUFFIX="nightly.${DATE}"
        fi
        echo "${BASE_VERSION}~${SUFFIX}"
        ;;
    pr)
        N="${ARGS[1]:-0}"
        DATE="$(get_date)"
        SHA="$(get_git_sha || true)"
        if [ -n "$SHA" ]; then
            SUFFIX="pr${N}.${DATE}.g${SHA}"
        else
            SUFFIX="pr${N}.${DATE}"
        fi
        echo "${BASE_VERSION}~${SUFFIX}"
        ;;
    *)
        echo "Unknown channel: $CHANNEL" >&2
        echo "Usage: $0 [--rpm|--deb] [stable|rc N|nightly|pr N]" >&2
        exit 1
        ;;
esac
