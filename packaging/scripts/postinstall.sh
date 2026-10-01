#!/bin/sh
set -e

# CHV Generic Postinstall
# Safe operations only:
#   - Create system user and group
#   - Create runtime/state directories
#   - Add user to kvm group (if available)
#   - Reload systemd
#
# This script intentionally does NOT:
#   - Create network bridges
#   - Initialize storage pools
#   - Start VMs
#   - Modify firewall rules
#   - Wipe or format disks

# Create the 'chv' system user and group if they don't exist
if ! getent group chv >/dev/null 2>&1; then
    groupadd -r chv
fi

if ! getent passwd chv >/dev/null 2>&1; then
    useradd -r -g chv -d /var/lib/chv -s /usr/sbin/nologin chv
fi

# Create the 'chv-stord' system user and group if they don't exist
if ! getent group chv-stord >/dev/null 2>&1; then
    groupadd -r chv-stord
fi

if ! getent passwd chv-stord >/dev/null 2>&1; then
    useradd -r -g chv-stord -d /var/lib/chv -s /usr/sbin/nologin chv-stord
fi

# Ensure state and runtime directories exist and are owned by chv
mkdir -p /var/lib/chv /var/log/chv /run/chv
chown chv:chv /var/lib/chv /var/log/chv /run/chv || true
install -d -m 0700 -o chv -g chv /var/lib/chv/agent /var/lib/chv/cache /run/chv/core
install -d -m 0775 -o chv -g chv /run/chv/agent

# Storage state lives under /var/lib/chv/storage. chv-stord runs as the
# 'chv' service user: its API socket is mode 0600 with chv-agent as the
# only client, and cloud-hypervisor (spawned by chv-agent as 'chv') must
# read and write volume files. The directories therefore must be writable
# by 'chv'. Group 'chv-stord' is kept as the isolation seam for a future
# dedicated storage-user model.
install -d -m 0770 -o chv -g chv-stord /var/lib/chv/storage/localdisk /var/lib/chv/storage/lvm
# Runtime dir for the systemd unit path (RuntimeDirectory=chv/stord creates
# the same ownership); kept here so non-systemd starts also work as 'chv'.
install -d -m 0755 -o chv -g chv /run/chv/stord

# Add chv user to the kvm group if it exists (required for VM runtime)
if getent group kvm >/dev/null 2>&1; then
    # NOTE: match the group list per-entry (`grep -qx` over one group per
    # line). A plain `id -nG | grep -qw chv` would also match `chv-stord`
    # (the hyphen is a word boundary) and silently skip the usermod.
    if ! id -nG chv | tr ' ' '\n' | grep -qx kvm; then
        usermod -aG kvm chv
    fi
fi

# Add chv-stord user to the disk group for block device access
if getent group disk >/dev/null 2>&1; then
    if ! id -nG chv-stord | tr ' ' '\n' | grep -qx disk; then
        usermod -aG disk chv-stord
    fi
fi

# Add chv-stord to chv group so it can traverse /var/lib/chv
if ! id -nG chv-stord | tr ' ' '\n' | grep -qx chv; then
    usermod -aG chv chv-stord
fi

# Reload systemd so new service files are recognized
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload 2>/dev/null || true
fi

exit 0
