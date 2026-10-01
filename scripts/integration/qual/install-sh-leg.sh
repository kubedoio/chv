#!/usr/bin/env bash
# install-sh-leg.sh — prompt-04 qualification leg for the install.sh path
# (scripts/integration/qual/).
#
# The M4.2 clean-install harness (clean-install.sh) covers the .deb path
# (Legs A+B). The release-tarball install.sh path had no automated coverage,
# which is exactly how it drifted: embedded systemd units and a storage
# chown in start_services() carried the stale pre-#323/#328 model for two
# fix rounds while the packaged surfaces were corrected (review finding,
# 2026-10-01). This leg closes that gap.
#
# What it proves, on a clean debootstrap+nspawn container:
#   1. install.sh completes end-to-end (rc=0) from a release tarball.
#   2. The units it writes to /etc/systemd/system/ are byte-identical to the
#      canonical units shipped in the tarball's systemd/ (packaging/systemd/
#      is the source of truth) — no stale embedded copies.
#   3. The tmpfiles entry is installed and applied (/run/netns root:chv 0770).
#   4. Storage/agent directory ownership matches the deb postinst contract
#      (#323): /var/lib/chv/storage* chv:chv-stord 0770, agent/vms.
#   5. All four CHV units reach active/running with zero restarts under the
#      canonical units (same boot semantics as clean-install Leg B).
#
# Container-environment adjustments (documented, not product behavior):
#   - 'universe' is enabled in apt sources: the debootstrap base carries only
#     'main' (dnsmasq lives in universe), while stock Ubuntu server images
#     enable universe by default.
#   - The default nginx.conf [::]:80 listen is commented out: the container
#     shares the host network namespace, which may have no IPv6. Real hosts
#     are unaffected.
#   - INSTALL_CHV_NO_SEED=1 and INSTALL_CHV_NO_BRIDGE=1: dev-resource seeding
#     would drive network creation through nwd inside the restricted shared
#     netns, and the legacy bridge bootstrap needs CAP_NET_ADMIN that nspawn
#     withholds when not using --private-network. Both are environment
#     constraints, not install.sh defects; the deb-path legs cover the
#     daemon-side behavior.
#   - Pre-flight: the host must not have anything bound on port 80 (the
#     container shares the host netns, and nginx binds 0.0.0.0:80).
#
# Usage:
#   sudo ./install-sh-leg.sh --tarball dist/chv-0.2.0-linux-amd64.tar.gz
#
# Environment:
#   CHV_QUAL_ROOT  qualification state dir (default /var/lib/chv/qual)
#   CHV_QUAL_FAIL_FAST  set to abort on the first failed assertion

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/integration/qual/lib.sh
source "${SCRIPT_DIR}/lib.sh"

TARBALL=""
SUITE="${SUITE:-noble}"

usage() {
    echo "usage: sudo $0 --tarball <release-tarball.tar.gz>" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --tarball) TARBALL="${2:?}"; shift 2 ;;
        --tarball=*) TARBALL="${1#*=}"; shift ;;
        *) usage ;;
    esac
done
[ -n "$TARBALL" ] || usage
[ -f "$TARBALL" ] || qual_die "tarball not found: ${TARBALL}"
TARBALL="$(realpath "$TARBALL")"
TARBALL_BASE="$(basename "$TARBALL")"
TARBALL_NAME="${TARBALL_BASE%.tar.gz}"

CHV_QUAL_ROOT="${CHV_QUAL_ROOT:-/var/lib/chv/qual}"
BASE_TARBALL="${CHV_QUAL_ROOT}/nspawn-base-${SUITE}.tar.gz"
ROOT="${CHV_QUAL_ROOT}/installsh-root"
MACHINE="chv-installsh-qual"

[ "$(id -u)" -eq 0 ] || qual_die "must run as root (sudo)"
[ -s "$BASE_TARBALL" ] || qual_die "base tarball missing: ${BASE_TARBALL} (run clean-install.sh once to build it)"
[ -x "$(command -v systemd-nspawn)" ] || qual_die "systemd-nspawn not available"
command -v bootctl >/dev/null 2>&1 || true

# Pre-flight: the container shares the host network namespace, so nginx's
# 0.0.0.0:80 bind collides with anything already on host port 80.
if ss -xltne 2>/dev/null | grep -q ':80 '; then
    holder="$(ss -xltnp 2>/dev/null | grep ':80 ' | head -1 | sed 's/.*users:(("//' | cut -d'"' -f1)"
    qual_die "host port 80 is occupied (by '${holder:-unknown}'); the container shares the host netns and install.sh starts nginx on :80. Stop the host service first (e.g. 'systemctl stop nginx')."
fi
qual_pass "host port 80 free for the container leg"

# ---------------------------------------------------------------------------
# Fresh container from the cached base
# ---------------------------------------------------------------------------
qual_info "preparing clean container from ${BASE_TARBALL}"
rm -rf "$ROOT"
mkdir -p "$ROOT"
tar -C "$ROOT" -xzf "$BASE_TARBALL"
rm -rf "${ROOT}/dev/.static/" 2>/dev/null || true
cp "$TARBALL" "${ROOT}/root/"

# ---------------------------------------------------------------------------
# One-shot service: run install.sh inside the booted container, then record
# the contract evidence.
# ---------------------------------------------------------------------------
cat > "${ROOT}/usr/local/bin/chv-installsh-run.sh" <<EOS
#!/bin/bash
out=/var/installsh-results.txt
: > "\$out"
cd /root
# Environment parity with a stock server image (see header note).
echo "deb http://archive.ubuntu.com/ubuntu ${SUITE} main universe" > /etc/apt/sources.list
sed -i 's/^\s*listen \[::\]:80/#&/' /etc/nginx/nginx.conf 2>/dev/null || true
tar -xzf "${TARBALL_BASE}"
echo "=== install.sh run ===" >> "\$out"
INSTALL_CHV_TARBALL_PATH="/root/${TARBALL_NAME}" \\
INSTALL_CHV_NO_SEED=1 INSTALL_CHV_NO_BRIDGE=1 \\
  "/root/${TARBALL_NAME}/install.sh" > /var/installsh.log 2>&1
echo "INSTALL_RC=\$?" >> "\$out"
sleep 15
echo "=== units ===" >> "\$out"
for u in chv-controlplane chv-agent chv-stord chv-nwd; do
  if diff -q "/etc/systemd/system/\${u}.service" "/root/${TARBALL_NAME}/systemd/\${u}.service" >/dev/null 2>&1; then
    echo "UNIT-IDENTICAL \${u}" >> "\$out"
  else
    echo "UNIT-DIFFERS \${u}" >> "\$out"
  fi
  systemctl show "\$u" -p ActiveState -p SubState -p Result -p NRestarts --no-pager | sed "s/^/\${u} /" >> "\$out"
done
echo "=== tmpfiles ===" >> "\$out"
if diff -q /usr/lib/tmpfiles.d/chv-node.conf "/root/${TARBALL_NAME}/tmpfiles/chv-node.conf" >/dev/null 2>&1; then
  echo "TMPFILES-IDENTICAL" >> "\$out"
else
  echo "TMPFILES-DIFFERS-OR-MISSING" >> "\$out"
fi
echo "=== runtime contract ===" >> "\$out"
stat -c 'CHECK /run/netns %a %U:%G' /run/netns >> "\$out" 2>&1 || echo "CHECK /run/netns MISSING" >> "\$out"
for d in /var/lib/chv/storage /var/lib/chv/storage/localdisk /var/lib/chv/storage/lvm /var/lib/chv/agent/vms; do
  stat -c "CHECK \${d} %a %U:%G" "\$d" >> "\$out" 2>&1 || echo "CHECK \${d} MISSING" >> "\$out"
done
echo "=== install log tail ===" >> "\$out"
tail -15 /var/installsh.log >> "\$out"
systemctl poweroff
EOS
chmod +x "${ROOT}/usr/local/bin/chv-installsh-run.sh"

cat > "${ROOT}/etc/systemd/system/chv-installsh-run.service" <<'EOF'
[Unit]
Description=CHV install.sh qualification (one-shot)
After=multi-user.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/chv-installsh-run.sh
TimeoutStartSec=900

[Install]
WantedBy=multi-user.target
EOF

systemd-nspawn -q -D "$ROOT" --machine="${MACHINE}-setup" systemctl enable chv-installsh-run.service \
    >/dev/null 2>&1 || qual_die "failed to enable the one-shot service in the container"
qual_pass "one-shot install service enabled"

# ---------------------------------------------------------------------------
# Boot and collect
# ---------------------------------------------------------------------------
qual_info "booting container (install.sh runs inside; several minutes)"
RESULTS="${ROOT}/var/installsh-results.txt"
rm -f "$RESULTS"
timeout 900 systemd-nspawn -b -D "$ROOT" --machine="$MACHINE" \
    >"${CHV_QUAL_ROOT}/installsh-boot.log" 2>&1 \
    || qual_warn "container boot ended via timeout/failure (log: ${CHV_QUAL_ROOT}/installsh-boot.log)"

[ -s "$RESULTS" ] || qual_die "no results collected (see ${CHV_QUAL_ROOT}/installsh-boot.log)"
RESULTS_TEXT="$(cat "$RESULTS")"
qual_pass "results collected"

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
assert_contains "install.sh completed with rc=0" "$RESULTS_TEXT" "INSTALL_RC=0"

for u in chv-controlplane chv-agent chv-stord chv-nwd; do
    assert_contains "unit ${u} byte-identical to the tarball's canonical unit" \
        "$RESULTS_TEXT" "UNIT-IDENTICAL ${u}"
    assert_contains "unit ${u} active" "$RESULTS_TEXT" "${u} ActiveState=active"
    assert_contains "unit ${u} running (not crash-looping)" "$RESULTS_TEXT" "${u} SubState=running"
    assert_contains "unit ${u} zero restarts" "$RESULTS_TEXT" "${u} NRestarts=0"
done

assert_contains "tmpfiles entry installed and identical to packaging" \
    "$RESULTS_TEXT" "TMPFILES-IDENTICAL"
assert_contains "/run/netns 770 root:chv (tmpfiles applied)" \
    "$RESULTS_TEXT" "CHECK /run/netns 770 root:chv"

assert_contains "storage dir chv:chv-stord 0770 (#323 contract)" \
    "$RESULTS_TEXT" "CHECK /var/lib/chv/storage 770 chv:chv-stord"
assert_contains "localdisk chv:chv-stord 0770" \
    "$RESULTS_TEXT" "CHECK /var/lib/chv/storage/localdisk 770 chv:chv-stord"
assert_contains "lvm chv:chv-stord 0770" \
    "$RESULTS_TEXT" "CHECK /var/lib/chv/storage/lvm 770 chv:chv-stord"
assert_contains "agent/vms 0775" \
    "$RESULTS_TEXT" "CHECK /var/lib/chv/agent/vms 775"

assert_not_contains "no unit diverged from the canonical copies" \
    "$RESULTS_TEXT" "UNIT-DIFFERS"

qual_info "full results transcript: ${RESULTS}"
qual_summary "install-sh-leg" || true
exit $?
