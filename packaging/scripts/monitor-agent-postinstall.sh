#!/bin/sh
set -e

# chv-monitor-agent postinstall (guest package, ADR-026).
#
# This package runs INSIDE VMs, not on CHV nodes: it deliberately
# shares nothing with the chv-controlplane/chv-node packages — no
# `chv` user, no node directories, no dependency (installing it is an
# operator's per-VM decision, never a node-package side effect).
#
# Safe operations only:
#   - Create the dedicated chv-monitor system user/group
#   - Create the state directory (0700: it holds the credential)
#   - Reload systemd
#
# This script intentionally does NOT:
#   - Enable or start the service (it needs a configured server_url,
#     a manager CA, and a claim before it can do anything useful)
#   - Create or modify any claim/config content

if ! getent group chv-monitor >/dev/null 2>&1; then
    groupadd -r chv-monitor
fi

if ! getent passwd chv-monitor >/dev/null 2>&1; then
    useradd -r -g chv-monitor -d /var/lib/chv-monitor -s /usr/sbin/nologin chv-monitor
fi

install -d -m 0700 -o chv-monitor -g chv-monitor /var/lib/chv-monitor /var/lib/chv-monitor/spool

if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload 2>/dev/null || true
fi

echo "chv-monitor-agent: installed (disabled by default)." >&2
echo "chv-monitor-agent: set server_url and manager_ca_path in /etc/chv-monitor/agent.toml," >&2
echo "chv-monitor-agent: place a one-time claim at /var/lib/chv-monitor/claim (owner chv-monitor, 0600), then:" >&2
echo "chv-monitor-agent:   systemctl enable --now chv-monitor-agent" >&2

exit 0
