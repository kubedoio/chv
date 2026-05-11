#!/bin/sh
set -e

# CHV Generic Postremove
# Runs after package removal. Safe operations only.
#
# This script intentionally does NOT remove:
#   - /var/lib/chv/   (persistent state: databases, caches, VM data)
#   - /etc/chv/       (operator configuration)
#   - /var/log/chv/   (log history)
#
# Destructive cleanup requires explicit operator action.

if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload 2>/dev/null || true
fi

exit 0
