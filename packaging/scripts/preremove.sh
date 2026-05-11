#!/bin/sh
set -e

# CHV Generic Preremove
# Safely stops CHV services before package removal.
# Preserves all persistent data.
#
# Package manager conventions:
#   Debian: $1 = remove | purge | upgrade | failed-upgrade | ...
#   RPM:    $1 = 0 (uninstall) | 1 (upgrade)

ACTION=""
if [ "$1" = "remove" ] || [ "$1" = "purge" ] || [ "$1" = "0" ]; then
    ACTION="remove"
fi

# Skip service stop on upgrade
if [ "$1" = "upgrade" ] || [ "$1" = "1" ]; then
    ACTION="upgrade"
fi

if [ "$ACTION" = "remove" ]; then
    if command -v systemctl >/dev/null 2>&1; then
        # Stop all known CHV services gracefully.
        # Missing services are silently ignored.
        for svc in chv-controlplane chv-agent chv-stord chv-nwd; do
            systemctl stop "${svc}.service" 2>/dev/null || true
        done
        systemctl daemon-reload 2>/dev/null || true
    fi
fi

exit 0
