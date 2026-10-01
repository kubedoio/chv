#!/usr/bin/env bash
# patch-guest-image.sh — build the bootable qualification guest image
# (prompt-04, M4.3).
#
# The stock Ubuntu noble cloud image cannot boot through the CHV
# firmware chain: GRUB passes root=LABEL=cloudimg-rootfs, and LABEL
# resolution needs an initramfs that the shim → rust-hypervisor-fw
# chain never delivers (the shim aborts its Mok state import on the
# fw, so the initrd never reaches the kernel — instant
# "VFS: Unable to mount root fs" panic; isolated and root-caused in
# the M2.5 qualification, see
# docs/evidence/production-readiness/v0.3.0-rc1/02-single-authority-cutover/
# m2.5-kvm-qualification.md). The noble kernel has VIRTIO_BLK, EXT4 and
# VIRTIO_NET built in, so it needs no initramfs: patching grub.cfg to
# root=/dev/vda1 and dropping the initrd lines makes the stock image
# boot bare. This is qualification scaffolding, not product —
# direct-kernel boot of stock cloud images (initramfs in BootSpec) is
# recorded as a follow-up limitation.
#
# Output: ${CHV_QUAL_ROOT}/images/noble-qual-patched.img (qcow2), built
# from the stock seed downloaded by env-preflight.sh. Idempotent: skips
# unless --force or the output is missing.
#
# Operational gotcha (observed and repaired 2026-09-29, M2.5): the
# cloud image's ESP partition is labeled UEFI — the same label the
# host's fstab uses for /boot/efi — and after a partition scan systemd
# may remount /boot/efi from the loop image's ESP. This script records
# the original /boot/efi source before scanning and verifies (and if
# needed repairs) it after detaching.
#
# Usage: sudo ./patch-guest-image.sh [--force]
#
# Environment:
#   CHV_QUAL_ROOT   persistent root (default: /var/lib/chv/qual)
#   GUEST_IMAGE     stock seed filename (default:
#                   noble-server-cloudimg-amd64.img)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

CHV_QUAL_ROOT="${CHV_QUAL_ROOT:-/var/lib/chv/qual}"
GUEST_IMAGE="${GUEST_IMAGE:-noble-server-cloudimg-amd64.img}"
PATCHED_IMAGE="noble-qual-patched.img"

FORCE=false
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=true ;;
        *) qual_die "unknown argument: $arg (usage: patch-guest-image.sh [--force])" ;;
    esac
done

[ "$(id -u)" -eq 0 ] || qual_die "must run as root (loop mounts)"

IMAGES_DIR="${CHV_QUAL_ROOT}/images"
SEED="${IMAGES_DIR}/${GUEST_IMAGE}"
OUT="${IMAGES_DIR}/${PATCHED_IMAGE}"

[ -s "$SEED" ] || qual_die "stock seed image missing: ${SEED} — run env-preflight.sh first"
command -v qemu-img >/dev/null 2>&1 \
    || qual_die "qemu-img missing — install qemu-utils (apt-get install -y qemu-utils)"

if [ -s "$OUT" ] && [ "$FORCE" != true ]; then
    qual_pass "patched image already present: ${OUT} (sha256 $(sha256_of "$OUT" | cut -c1-16)…; use --force to rebuild)"
    exit 0
fi

# /boot/efi guard (see header): record the current mount source so it can
# be verified — and repaired — after the loop device is detached.
EFVI_SOURCE=""
if findmnt -n /boot/efi >/dev/null 2>&1; then
    EFVI_SOURCE="$(findmnt -n -o SOURCE /boot/efi)"
    qual_info "/boot/efi mounted from ${EFVI_SOURCE} — will verify after detach"
fi

WORK="$(mktemp -d /tmp/chv-patch-img-XXXXXX)"
trap '{
    umount "${WORK}/mnt" 2>/dev/null || true
    [ -n "${LOOPDEV:-}" ] && losetup -d "${LOOPDEV}" 2>/dev/null || true
    # ESP-label remount guard: if systemd remounted /boot/efi from the
    # ESP of the loop image (label UEFI), restore the original source.
    if [ -n "$EFVI_SOURCE" ]; then
        CURRENT_EFVI="$(findmnt -n -o SOURCE /boot/efi 2>/dev/null || true)"
        if [ "$CURRENT_EFVI" != "$EFVI_SOURCE" ]; then
            qual_warn "/boot/efi was remounted from ${CURRENT_EFVI:-nothing} — repairing to ${EFVI_SOURCE}"
            umount /boot/efi 2>/dev/null || true
            mount "$EFVI_SOURCE" /boot/efi 2>/dev/null || qual_error "failed to restore /boot/efi from ${EFVI_SOURCE}"
        fi
    fi
    rm -rf "$WORK"
}' EXIT
mkdir -p "${WORK}/mnt"

qual_info "converting seed to raw (${SEED})"
qemu-img convert -O raw "$SEED" "${WORK}/seed.raw"

LOOPDEV="$(losetup -Pf --show "${WORK}/seed.raw")"
qual_info "loop device: ${LOOPDEV} (partitions: $(ls "${LOOPDEV}"*p* 2>/dev/null | tr '\n' ' '))"
# Give udev a moment to create the partition nodes.
for _ in $(seq 1 10); do
    [ -b "${LOOPDEV}p1" ] && break
    sleep 0.5
done
[ -b "${LOOPDEV}p1" ] || qual_die "partition 1 did not appear on ${LOOPDEV}"

# The noble cloud image keeps the kernel and GRUB on a separate BOOT
# partition (ext4, label BOOT — p16 on the current image; p1 is the
# rootfs, p15 the ESP). Mount it explicitly by device and fstype; never
# by label (the ESP-label collision in the header is exactly this class
# of accident).
BOOT_PARTITION=""
for part in "${LOOPDEV}p1" "${LOOPDEV}p14" "${LOOPDEV}p15" "${LOOPDEV}p16"; do
    [ -b "$part" ] || continue
    if [ "$(blkid -o value -s TYPE "$part" 2>/dev/null)" = "ext4" ] \
        && blkid -o value -s LABEL "$part" 2>/dev/null | grep -qx BOOT; then
        BOOT_PARTITION="$part"
        break
    fi
done
[ -n "$BOOT_PARTITION" ] || qual_die "no ext4 BOOT-label partition found on ${LOOPDEV} (unexpected image layout)"
mount -t ext4 "$BOOT_PARTITION" "${WORK}/mnt"
GRUB_CFG="${WORK}/mnt/grub/grub.cfg"
[ -f "$GRUB_CFG" ] || qual_die "grub.cfg not found at ${GRUB_CFG} (unexpected image layout)"

BEFORE="$(grep -cE '^(\s*)linux' "$GRUB_CFG" || true)"
# Patch every boot entry: root by device (no initramfs to resolve LABELs)
# and no initrd line at all (the firmware chain never delivers one).
sed -i 's/root=LABEL=cloudimg-rootfs/root=\/dev\/vda1/g' "$GRUB_CFG"
sed -i '/^\s*initrd\b/d' "$GRUB_CFG"
AFTER_PATCHED="$(grep -c 'root=/dev/vda1' "$GRUB_CFG" || true)"
[ "$AFTER_PATCHED" -ge 1 ] || qual_die "no linux line patched — unexpected grub.cfg content (searched root=LABEL=cloudimg-rootfs)"
if grep -qE '^\s*initrd\b' "$GRUB_CFG"; then
    qual_die "initrd lines remain in grub.cfg after patch"
fi
qual_pass "grub.cfg patched (${BEFORE} linux entries → root=/dev/vda1, initrd lines removed)"

umount "${WORK}/mnt"
losetup -d "${LOOPDEV}"
LOOPDEV=""

qemu-img convert -O qcow2 "${WORK}/seed.raw" "$OUT"
rm -f "${WORK}/seed.raw"
qual_pass "patched image written: ${OUT} ($(du -h "$OUT" | cut -f1), sha256 $(sha256_of "$OUT" | cut -c1-16)…)"
qual_info "use it via: GUEST_IMAGE=${PATCHED_IMAGE} ./deploy.sh --exec ./m4.3-lifecycle.sh"
