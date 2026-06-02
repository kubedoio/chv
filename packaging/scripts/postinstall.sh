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

# Ensure storage directories exist and are owned by chv-stord
mkdir -p /var/lib/chv/storage/localdisk /var/lib/chv/storage/lvm /run/chv/stord
chown -R chv-stord:chv-stord /var/lib/chv/storage /run/chv/stord || true
chmod 750 /var/lib/chv/storage/localdisk /var/lib/chv/storage/lvm || true

# Add chv user to the kvm group if it exists (required for VM runtime)
if getent group kvm >/dev/null 2>&1; then
    if ! id -nG chv | grep -qw kvm; then
        usermod -aG kvm chv
    fi
fi

# Add chv-stord user to the disk group for block device access
if getent group disk >/dev/null 2>&1; then
    if ! id -nG chv-stord | grep -qw disk; then
        usermod -aG disk chv-stord
    fi
fi

# Add chv-stord to chv group so it can traverse /var/lib/chv
if ! id -nG chv-stord | grep -qw chv; then
    usermod -aG chv chv-stord
fi

# Reload systemd so new service files are recognized
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload 2>/dev/null || true
fi

exit 0
