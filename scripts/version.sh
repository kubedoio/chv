#!/bin/bash
# Derive package versions for different release channels.
#
# Pipeline role: Called by build-packages.sh and CI workflows to generate
# Debian (~suffix) and RPM (-suffix) version strings from the VERSION file.
# Environment override: CHV_PKG_PRERELEASE
# Usage: ./scripts/version.sh [--rpm|--deb] [stable|rc N|nightly|pr N]
#
# Environment:
#   CHV_PKG_PRERELEASE - if set, used as the pre-release suffix instead of deriving.
#                        Example: rc.1 produces 0.1.0~rc.1 (deb) or 0.1.0-0.1.rc1 (rpm)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BASE_VERSION="$(cat "${REPO_ROOT}/VERSION")"

FORMAT="deb"
ARGS=()

for arg in "$@"; do
    case "$arg" in
        --rpm) FORMAT="rpm" ;;
        --deb) FORMAT="deb" ;;
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

# If CHV_PKG_PRERELEASE is set, use it directly as the suffix
if [ -n "${CHV_PKG_PRERELEASE:-}" ]; then
    SUFFIX="$CHV_PKG_PRERELEASE"
    if [ "$FORMAT" = "rpm" ]; then
        if [[ "$SUFFIX" =~ ^rc\.([0-9]+)$ ]]; then
            echo "${BASE_VERSION}-0.1.rc${BASH_REMATCH[1]}"
            exit 0
        fi
        # General fallback: replace ~ with - for RPM safety
        SUFFIX="${SUFFIX//~/-}"
        echo "${BASE_VERSION}-${SUFFIX}"
        exit 0
    else
        echo "${BASE_VERSION}~${SUFFIX}"
        exit 0
    fi
fi

case "$CHANNEL" in
    stable)
        echo "$BASE_VERSION"
        ;;
    rc)
        N="${ARGS[1]:-1}"
        if [ "$FORMAT" = "rpm" ]; then
            echo "${BASE_VERSION}-0.1.rc${N}"
        else
            echo "${BASE_VERSION}~rc.${N}"
        fi
        ;;
    nightly)
        DATE="$(get_date)"
        SHA="$(get_git_sha || true)"
        if [ -n "$SHA" ]; then
            SUFFIX="nightly.${DATE}.g${SHA}"
        else
            SUFFIX="nightly.${DATE}"
        fi
        if [ "$FORMAT" = "rpm" ]; then
            echo "${BASE_VERSION}~${SUFFIX}"
        else
            echo "${BASE_VERSION}~${SUFFIX}"
        fi
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
        if [ "$FORMAT" = "rpm" ]; then
            echo "${BASE_VERSION}~${SUFFIX}"
        else
            echo "${BASE_VERSION}~${SUFFIX}"
        fi
        ;;
    *)
        echo "Unknown channel: $CHANNEL" >&2
        echo "Usage: $0 [--rpm|--deb] [stable|rc N|nightly|pr N]" >&2
        exit 1
        ;;
esac
