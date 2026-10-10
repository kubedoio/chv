#!/bin/sh
set -e

# chv-monitor-agent preremove (guest package, ADR-026).
# Stops the service on removal; preserves all persistent data
# (credential, spool, sequence counters) — deleting them is an
# explicit operator decision, never a package side effect.
#
# Package manager conventions:
#   Debian: $1 = remove | purge | upgrade | failed-upgrade | ...
#   RPM:    $1 = 0 (uninstall) | 1 (upgrade)

ACTION=""
if [ "$1" = "remove" ] || [ "$1" = "purge" ] || [ "$1" = "0" ]; then
    ACTION="remove"
fi
if [ "$1" = "upgrade" ] || [ "$1" = "1" ]; then
    ACTION="upgrade"
fi

if [ "$ACTION" = "remove" ]; then
    if command -v systemctl >/dev/null 2>&1; then
        systemctl stop chv-monitor-agent.service 2>/dev/null || true
        systemctl daemon-reload 2>/dev/null || true
    fi
fi

exit 0
