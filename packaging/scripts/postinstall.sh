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

# Credential encryption key (#335, report finding H-7): the control plane
# derives an AES-256-GCM key from CHV_ENCRYPTION_KEY to encrypt S3
# credentials at rest (crates/chv-controlplane-store/src/
# credential_crypto.rs); without it they are stored in plaintext.
# chv-controlplane.service loads this file via EnvironmentFile. Mint once
# and NEVER regenerate (dpkg configure on upgrade runs this again): existing
# encrypted credentials would become unrecoverable. Key material comes from
# /dev/urandom (no openssl dependency in the postinst environment).
if [ -x /usr/bin/chv-controlplane ] && [ ! -f /etc/chv/encryption.env ]; then
    mkdir -p /etc/chv
    _chv_key="$(head -c 32 /dev/urandom 2>/dev/null | od -An -tx1 2>/dev/null | tr -d ' \n')"
    # Only write a well-formed key: an empty file would disable encryption
    # and be preserved forever by the create-if-absent guard above. (If we
    # skip entirely the variable stays unset, so the control plane still
    # logs its plaintext warning at startup — loud, not silent.)
    if printf '%s' "$_chv_key" | grep -qE '^[0-9a-f]{64}$'; then
        # Subshell: umask 077 must not leak into later directory creation
        # in this script (state/log dirs are 0755 by contract).
        (
            umask 077
            printf 'CHV_ENCRYPTION_KEY=%s\n' "$_chv_key" > /etc/chv/encryption.env
        )
    fi
elif [ -x /usr/bin/chv-controlplane ] && [ -f /etc/chv/encryption.env ]; then
    # Preserved, not regenerated — but warn loudly if it is unusable.
    if ! grep -q '^CHV_ENCRYPTION_KEY=[0-9a-f]\{64\}$' /etc/chv/encryption.env; then
        echo "chv-controlplane: WARNING: /etc/chv/encryption.env exists but is empty or malformed;" >&2
        echo "chv-controlplane: S3 credentials will be stored in plaintext until it is fixed." >&2
    fi
fi
if [ -f /etc/chv/encryption.env ]; then
    chmod 0600 /etc/chv/encryption.env
fi

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
