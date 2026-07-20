#!/usr/bin/env bash
# Read-only residual-resource check for the reserved discovery prefix.

set -euo pipefail

readonly PREFIX="cellhv-osd-"
export PREFIX
residual=0

report_matches() {
    local label="$1"
    local output="$2"
    if [[ -n "$output" ]]; then
        printf '[FAIL] residual %s:\n%s\n' "$label" "$output" >&2
        residual=1
    else
        printf '[PASS] no reserved-prefix %s found\n' "$label"
    fi
}

processes="$(ps -eo pid=,comm=,args= | awk 'index($0, ENVIRON["PREFIX"]) { print $1, $2 }')"
report_matches "processes" "$processes"

if command -v ip >/dev/null 2>&1; then
    if ! link_inventory="$(ip -brief link show 2>/dev/null)"; then
        printf '[WARN] network link inventory was denied or failed\n' >&2
        residual=1
        link_inventory=""
    fi
    if ! namespace_inventory="$(ip netns list 2>/dev/null)"; then
        printf '[WARN] network namespace inventory was denied or failed\n' >&2
        residual=1
        namespace_inventory=""
    fi
    links="$(awk -v prefix="$PREFIX" 'index($1, prefix) == 1 { print }' <<< "$link_inventory")"
    namespaces="$(awk -v prefix="$PREFIX" 'index($1, prefix) == 1 { print }' <<< "$namespace_inventory")"
    report_matches "network links" "$links"
    report_matches "network namespaces" "$namespaces"
else
    printf '[WARN] ip is unavailable; network cleanup cannot be verified\n' >&2
    residual=1
fi

if [[ "$residual" -ne 0 ]]; then
    printf '[RESULT] cleanup verification failed\n' >&2
    exit 1
fi
printf '[RESULT] cleanup verification passed\n'
