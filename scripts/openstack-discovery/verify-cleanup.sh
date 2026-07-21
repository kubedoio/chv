#!/usr/bin/env bash
# Read-only residual-resource check for the reserved discovery prefix.

set -euo pipefail

readonly PREFIX="cellhv-osd-"
export PREFIX
residual=0

[[ $# -eq 0 || ( $# -eq 2 && "$1" == --runner-pid && "$2" =~ ^[1-9][0-9]*$ ) ]] || {
    printf '[FAIL] usage: %s [--runner-pid PID]\n' "$0" >&2
    exit 2
}
ignore_pids=""
if [[ $# -eq 2 ]]; then
    runner_pid="$2"
    [[ -r "/proc/${runner_pid}/cmdline" ]] || {
        printf '[FAIL] runner PID does not exist\n' >&2
        exit 2
    }
    runner_command="$(tr '\0' ' ' < "/proc/${runner_pid}/cmdline")"
    [[ "$runner_command" == *"scripts/openstack-discovery/run-path-a.py"* ]] || {
        printf '[FAIL] runner PID is not the Path A runner\n' >&2
        exit 2
    }
    current="$runner_pid"
    while [[ "$current" =~ ^[1-9][0-9]*$ ]]; do
        ignore_pids+="${ignore_pids:+,}${current}"
        parent="$(ps -o ppid= -p "$current" | awk '{print $1}')"
        [[ -n "$parent" && "$parent" != "$current" ]] || break
        current="$parent"
    done
fi
export CELLHV_CLEANUP_IGNORE_PIDS="$ignore_pids"

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

processes="$(ps -eo pid=,comm=,args= | awk '
    BEGIN { count=split(ENVIRON["CELLHV_CLEANUP_IGNORE_PIDS"], values, ","); for (i=1; i<=count; i++) ignored[values[i]]=1 }
    index($0, ENVIRON["PREFIX"]) && !($1 in ignored) { print $1, $2 }
')"
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
