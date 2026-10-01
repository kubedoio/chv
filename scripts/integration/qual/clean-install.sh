#!/usr/bin/env bash
# clean-install.sh — prompt-04 clean installation baseline (M4.2).
#
# Validates the candidate's .deb packages on a CLEAN Ubuntu noble system
# (debootstrap + systemd-nspawn on the qualification host — same physical
# host, honestly labeled), in two legs:
#
#   Leg A (static): install the packages into the clean container and
#   assert the packaging contract — service users, groups and memberships,
#   directories with exact modes/ownership, systemd units (with their
#   hardening keys), binaries, conffiles, migrations, tmpfiles, versions.
#
#   Leg B (boot): boot the container with systemd, enable + start the four
#   chv units with the PACKAGED configs, and record the honest outcome:
#   a bare package install has no CA/certs and a placeholder jwt_secret, so
#   the control plane is expected to fail CLOSED (typed startup validation)
#   — the packages-alone vs. install.sh boundary is documented evidence,
#   not a harness failure. Each unit's state and log tail are captured.
#
# Usage:
#   sudo ./clean-install.sh [OPTIONS]
#
# Options:
#   --packages DIR      .deb directory (default: /var/lib/chv/qual/packages)
#   --expect-version S  substring binaries must report in --version
#                       (default: fdfe9c3 — the v0.3.0-rc1 candidate)
#   --keep-root         keep the container root for post-mortem
#   --skip-boot         run Leg A only
#
# Environment:
#   CHV_QUAL_ROOT       persistent root (default: /var/lib/chv/qual)
#   SUITE               debootstrap suite (default: noble)
#   EXPECT_VERSION      same as --expect-version

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

CHV_QUAL_ROOT="${CHV_QUAL_ROOT:-/var/lib/chv/qual}"
PKGDIR="${CHV_QUAL_ROOT}/packages"
SUITE="${SUITE:-noble}"
# Binaries must report this substring in --version (candidate pin).
EXPECT_VERSION="${EXPECT_VERSION:-fdfe9c3}"
KEEP_ROOT=false
SKIP_BOOT=false

while [ $# -gt 0 ]; do
    case "$1" in
        --packages) PKGDIR="${2:?}"; shift 2 ;;
        --packages=*) PKGDIR="${1#*=}"; shift ;;
        --expect-version) EXPECT_VERSION="${2:?}"; shift 2 ;;
        --expect-version=*) EXPECT_VERSION="${1#*=}"; shift ;;
        --keep-root) KEEP_ROOT=true; shift ;;
        --skip-boot) SKIP_BOOT=true; shift ;;
        *) qual_die "unknown argument: $1" ;;
    esac
done

[ "$(id -u)" -eq 0 ] || qual_die "must run as root"
PKGDIR="$(cd "$PKGDIR" 2>/dev/null || qual_die "packages dir not found: $PKGDIR" && pwd)"
ls "$PKGDIR"/chv-controlplane_*.deb "$PKGDIR"/chv-node_*.deb "$PKGDIR"/chvctl_*.deb >/dev/null 2>&1 \
    || qual_die "expected chv-controlplane_*.deb, chv-node_*.deb, chvctl_*.deb in $PKGDIR"
qual_info "packages: $(find "$PKGDIR" -maxdepth 1 -name '*.deb' -printf '%f ')"

MACHINE="chv-qual-clean"
ROOT="${CHV_QUAL_ROOT}/nspawn-root"
BASE_TARBALL="${CHV_QUAL_ROOT}/nspawn-base-${SUITE}.tar.gz"

# ---------------------------------------------------------------------------
# 0. Host prerequisites (debootstrap + systemd-nspawn; recorded in evidence)
# ---------------------------------------------------------------------------
NEED_PKGS=""
for p in debootstrap systemd-container ubuntu-keyring; do
    dpkg -s "$p" >/dev/null 2>&1 || NEED_PKGS="${NEED_PKGS} ${p}"
done
if [ -n "$NEED_PKGS" ]; then
    qual_info "installing host prerequisites:${NEED_PKGS}"
    # shellcheck disable=SC2086
    apt-get update -qq && apt-get install -y -qq ${NEED_PKGS} >/dev/null
fi
qual_pass "host prerequisites present (debootstrap, systemd-container, ubuntu-keyring)"

cleanup() {
    local rc=$?
    machinectl terminate "$MACHINE" 2>/dev/null || true
    if [ "$KEEP_ROOT" = true ] || [ "${QUAL_ERRORS:-0}" -gt 0 ]; then
        qual_info "container root preserved: ${ROOT}"
    else
        rm -rf "$ROOT"
    fi
    return "$rc" 2>/dev/null || exit "$rc"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# 1. Clean base system (cached debootstrap tarball; unpacked fresh per run)
# ---------------------------------------------------------------------------
if [ ! -s "$BASE_TARBALL" ]; then
    qual_info "debootstrapping ${SUITE} base (one-time, cached)..."
    debootstrap --include=systemd,systemd-sysv,procps,iproute2,wireguard-tools,libssl3t64 \
        "$SUITE" "${CHV_QUAL_ROOT}/nspawn-base" http://archive.ubuntu.com/ubuntu \
        >"${CHV_QUAL_ROOT}/debootstrap.log" 2>&1 \
        || qual_die "debootstrap failed — see ${CHV_QUAL_ROOT}/debootstrap.log"
    rm -rf "${CHV_QUAL_ROOT}/nspawn-base/dev/.static/"
    tar -C "${CHV_QUAL_ROOT}/nspawn-base" -czf "$BASE_TARBALL" .
    rm -rf "${CHV_QUAL_ROOT}/nspawn-base"
    qual_pass "base system built and cached: ${BASE_TARBALL} ($(du -h "$BASE_TARBALL" | cut -f1))"
fi

rm -rf "$ROOT"
mkdir -p "$ROOT"
tar -C "$ROOT" -xzf "$BASE_TARBALL"
# Minimal machine identity/resolv so systemd boots cleanly (no network needed).
rm -f "$ROOT/etc/hostname" "$ROOT/etc/machine-id"
touch "$ROOT/etc/machine-id"
qual_pass "clean ${SUITE} root filesystem prepared (fresh unpack, no prior state)"

# ---------------------------------------------------------------------------
# 2. Leg A — package install + static packaging-contract assertions
# ---------------------------------------------------------------------------
qual_info "Leg A: installing packages into the clean container"
# NOTE: stage under /root — systemd-nspawn mounts a fresh tmpfs over /tmp.
mkdir -p "$ROOT/root/packages"
cp "$PKGDIR"/*.deb "$ROOT/root/packages/"
systemd-nspawn -q -D "$ROOT" --machine="${MACHINE}-install" \
    bash -c 'export DEBIAN_FRONTEND=noninteractive; dpkg -i /root/packages/*.deb' \
    >"${CHV_QUAL_ROOT}/nspawn-install.log" 2>&1 \
    || { tail -20 "${CHV_QUAL_ROOT}/nspawn-install.log" >&2; qual_die "dpkg -i failed in the container"; }
qual_pass "packages installed (dpkg -i, no dependency errors)"

# --- service users + groups (postinst contract) ---
assert_user() {
    local user="$1" home="$2" shell="$3"
    local entry
    entry="$(grep -E "^${user}:" "$ROOT/etc/passwd" || true)"
    if [ -n "$entry" ]; then
        qual_pass "user exists: ${user} (${entry})"
        case "$entry" in
            *":${home}:"*) qual_pass "user ${user} home is ${home}" ;;
            *) qual_error "user ${user} home mismatch (expected ${home}): ${entry}" ;;
        esac
        case "$entry" in
            *":${shell}") qual_pass "user ${user} shell is ${shell} (no login)" ;;
            *) qual_error "user ${user} shell is not ${shell}: ${entry}" ;;
        esac
    else
        qual_error "user missing: ${user}"
    fi
}
assert_group_member() {
    local group="$1" user="$2"
    if grep -E "^${group}:" "$ROOT/etc/group" | grep -q ":${user}\$"; then
        qual_pass "${user} is a member of ${group}"
    else
        qual_error "${user} is NOT a member of ${group}"
    fi
}
assert_user chv /var/lib/chv /usr/sbin/nologin
assert_user chv-stord /var/lib/chv /usr/sbin/nologin
for g in chv chv-stord; do
    grep -E "^${g}:" "$ROOT/etc/group" >/dev/null || qual_error "group missing: ${g}"
done
qual_pass "groups present: chv, chv-stord"
assert_group_member kvm chv
assert_group_member disk chv-stord
# Intended-but-broken: postinst's `id -nG chv-stord | grep -qw chv` matches
# the chv-stord group name itself (hyphen is a word boundary), so
# `usermod -aG chv chv-stord` never runs. Known finding — issue filed.
if grep -E "^chv:" "$ROOT/etc/group" | grep -q ":chv-stord$"; then
    qual_pass "chv-stord is a member of chv (postinst intent)"
else
    qual_warn "chv-stord is NOT a member of chv — postinst grep word-match bug (#325)"
fi
# kvm/disk memberships: only when the groups exist (they do not in a
# minimal container; on a KVM host they do — postinst is conditional).
if ! grep -q "^kvm:" "$ROOT/etc/group"; then qual_info "kvm group absent in minimal container (conditional postinst path not taken)"; fi
if ! grep -q "^disk:" "$ROOT/etc/group"; then qual_info "disk group absent in minimal container (conditional postinst path not taken)"; fi

# --- directories with exact mode/ownership (postinst + tmpfiles contract) ---
# Resolve service ids in the CONTAINER namespace: the host and the container
# allocate system users independently, so names must never be resolved via
# the host's /etc/passwd (e.g. container uid 999 is 'fwupd-refresh' here).
chv_uid="$(awk -F: '$1=="chv"{print $3; exit}' "$ROOT/etc/passwd")"
chv_gid="$(awk -F: '$1=="chv"{print $3; exit}' "$ROOT/etc/group")"
stord_uid="$(awk -F: '$1=="chv-stord"{print $3; exit}' "$ROOT/etc/passwd")"
stord_gid="$(awk -F: '$1=="chv-stord"{print $3; exit}' "$ROOT/etc/group")"
[ -n "$chv_uid" ] && [ -n "$chv_gid" ] && [ -n "$stord_uid" ] && [ -n "$stord_gid" ] \
    || qual_die "could not resolve container service uid/gids"
assert_dir_mode() {
    local desc="$1" path="$2" mode="$3" owner="$4"
    local stat_out
    stat_out="$(stat -c '%a %u:%g' "$ROOT/${path#/}" 2>/dev/null || true)"
    if [ "$stat_out" = "${mode} ${owner}" ]; then
        qual_pass "${desc}: /${path} ${mode} ${owner}"
    else
        qual_error "${desc}: /${path} expected ${mode} ${owner}, got '${stat_out}'"
    fi
}
assert_dir_mode "state dir" var/lib/chv 755 "${chv_uid}:${chv_gid}"
assert_dir_mode "log dir" var/log/chv 755 "${chv_uid}:${chv_gid}"
assert_dir_mode "agent runtime dir (Core 0700 contract)" var/lib/chv/agent 700 "${chv_uid}:${chv_gid}"
assert_dir_mode "cache dir" var/lib/chv/cache 700 "${chv_uid}:${chv_gid}"
# Storage dirs are owned by the chv runtime user (chv-stord runs as chv:
# 0600 API socket with chv-agent as the only client, and cloud-hypervisor
# as chv must read/write volume files) with the chv-stord group kept as
# the isolation seam — see #323.
assert_dir_mode "storage localdisk" var/lib/chv/storage/localdisk 770 "${chv_uid}:${stord_gid}"
assert_dir_mode "storage lvm" var/lib/chv/storage/lvm 770 "${chv_uid}:${stord_gid}"
# NOTE: /run is a fresh tmpfs under systemd-nspawn, so postinst-created
# /run/chv/* directories are not observable from the host side here; the
# durable /run contract (tmpfiles.d + unit RuntimeDirectory) is asserted
# from inside the booted container in Leg B.

# --- units (with hardening keys) ---
for u in chv-controlplane chv-agent chv-stord chv-nwd; do
    assert_file_exists "unit present: ${u}.service" "$ROOT/lib/systemd/system/${u}.service"
done
for key in NoNewPrivileges ProtectSystem ProtectHome; do
    grep -q "^${key}=" "$ROOT/lib/systemd/system/chv-controlplane.service" \
        && qual_pass "controlplane unit hardening: ${key}" \
        || qual_error "controlplane unit missing hardening key: ${key}"
done
assert_file_exists "tmpfiles config present" "$ROOT/usr/lib/tmpfiles.d/chv-node.conf"

# --- binaries, conffiles, migrations ---
for b in chv-controlplane chv-agent chv-stord chv-nwd chvctl; do
    assert_file_exists "binary present: /usr/bin/${b}" "$ROOT/usr/bin/${b}"
done
for c in controlplane.toml agent.toml stord.toml nwd.toml; do
    assert_file_exists "conffile present: /etc/chv/${c}" "$ROOT/etc/chv/${c}"
done
n_migrations="$(ls "$ROOT/usr/share/chv/migrations"/*.sql 2>/dev/null | wc -l | tr -d ' ')"
if [ "$n_migrations" -gt 40 ]; then
    qual_pass "migrations present: ${n_migrations} SQL files under /usr/share/chv/migrations"
else
    qual_error "unexpected migration count: ${n_migrations}"
fi

# --- versions (packaged binaries report the expected build) ---
for b in chv-controlplane chv-agent chv-stord; do
    v="$(chroot "$ROOT" "/usr/bin/${b}" --version 2>/dev/null | head -1 || true)"
    case "$v" in
        *"${EXPECT_VERSION}"*) qual_pass "${b} --version reports expected build: ${v}" ;;
        *) qual_error "${b} --version does not report the expected build '${EXPECT_VERSION}': ${v}" ;;
    esac
done

qual_summary "Leg A (static packaging contract)" || true

# ---------------------------------------------------------------------------
# 3. Leg B — boot with systemd, start units with packaged configs
# ---------------------------------------------------------------------------
if [ "$SKIP_BOOT" = true ]; then
    qual_info "Leg B skipped (--skip-boot)"
    qual_summary "clean-install (Leg A only)" || true
    [ "${QUAL_ERRORS}" -gt 0 ] && exit 1 || exit 0
fi

qual_info "Leg B: booting clean container with systemd (packaged configs, no install.sh)"

# Verification script runs INSIDE the container after the units start.
cat > "$ROOT/usr/local/bin/chv-qual-verify.sh" <<'EOS'
#!/bin/bash
# Runs inside the booted container. Records honest unit outcomes.
out=/var/chv-qual-results.txt
: > "$out"
# Let units settle: a unit that crashes and enters Restart=on-failure backoff
# must be observed AFTER at least one restart attempt.
sleep 15
for u in chv-controlplane chv-stord chv-nwd chv-agent; do
    echo "UNIT ${u}" >> "$out"
    systemctl show "$u" -p ActiveState -p SubState -p Result -p ExecMainStatus -p NRestarts --no-pager >> "$out"
    echo "--- journal tail ${u} ---" >> "$out"
    journalctl -u "$u" --no-pager -n 30 2>/dev/null | sed 's/^/  /' >> "$out"
done
echo "--- process snapshot ---" >> "$out"
ps -eo pid,ppid,user,stat,cmd | grep -E 'chv-[a-z]+ /|PID' | grep -v grep >> "$out"
echo "--- /run/netns ---" >> "$out"
ls -la /run/netns >> "$out" 2>&1
echo "--- /run/chv (durable runtime contract) ---" >> "$out"
ls -la /run/chv >> "$out" 2>&1
stat -c 'CHECK /run/chv/agent %a %U:%G' /run/chv/agent >> "$out" 2>/dev/null || echo "CHECK /run/chv/agent MISSING" >> "$out"
stat -c 'CHECK /run/chv/core %a %U:%G' /run/chv/core >> "$out" 2>/dev/null || echo "CHECK /run/chv/core MISSING" >> "$out"
echo "--- agent state dir (supervised children) ---" >> "$out"
ls -la /var/lib/chv/agent >> "$out" 2>&1
echo "--- listening sockets ---" >> "$out"
ss -xltnp 2>/dev/null >> "$out" || true
echo "--- versions ---" >> "$out"
for b in chv-controlplane chv-agent chv-stord chv-nwd chvctl; do
    "/usr/bin/${b}" --version >> "$out" 2>&1 || true
done
echo "--- redacted packaged configs ---" >> "$out"
for c in controlplane agent stord nwd; do
    echo "### /etc/chv/${c}.toml" >> "$out"
    sed -E 's/^(jwt_secret|encryption_key).*/\1 = <REDACTED>/' \
        "/etc/chv/${c}.toml" >> "$out" 2>/dev/null || true
done
systemctl poweroff
EOS
chmod +x "$ROOT/usr/local/bin/chv-qual-verify.sh"

cat > "$ROOT/etc/systemd/system/chv-qual-verify.service" <<'EOF'
[Unit]
Description=CHV qualification verification (one-shot)
After=chv-controlplane.service chv-stord.service chv-nwd.service chv-agent.service
Wants=chv-controlplane.service chv-stord.service chv-nwd.service chv-agent.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/chv-qual-verify.sh
TimeoutStartSec=120

[Install]
WantedBy=multi-user.target
EOF

systemd-nspawn -q -D "$ROOT" --machine="${MACHINE}-setup" systemctl enable \
    chv-controlplane.service chv-stord.service chv-nwd.service chv-agent.service chv-qual-verify.service \
    >/dev/null 2>&1 || qual_die "failed to enable units in container"
qual_pass "units enabled (controlplane, stord, nwd, agent, verify)"

RESULTS="$ROOT/var/chv-qual-results.txt"
rm -f "$RESULTS"
timeout 180 systemd-nspawn -b -D "$ROOT" --machine="$MACHINE" \
    >"${CHV_QUAL_ROOT}/nspawn-boot.log" 2>&1 \
    || qual_warn "container boot ended via timeout/failure (log: ${CHV_QUAL_ROOT}/nspawn-boot.log)"

if [ -s "$RESULTS" ]; then
    qual_pass "boot leg completed; results captured"
    cp "$RESULTS" "${CHV_QUAL_ROOT}/nspawn-results.txt"
    cat "$RESULTS" >&2
else
    qual_error "no results from the boot leg (boot log tail follows)"
    tail -20 "${CHV_QUAL_ROOT}/nspawn-boot.log" >&2 || true
fi

# Honest assertions on the boot outcome:
# - control plane MUST fail closed on a bare install (no CA/certs, placeholder
#   jwt) — a clean typed validation, not a panic;
# - chv-agent MUST come up and stay active (it defers control-plane reports
#   without enrollment material);
# - chv-stord / chv-nwd systemd units do NOT come up on a clean install:
#   two filed packaging defects (#323: stord unit runs as chv but postinst
#   owns the storage dirs chv-stord:chv-stord 0750; #324: nwd unit
#   ReadWritePaths references /run/netns which nothing creates). Recorded
#   as known findings. The agent supervises its own stord/nwd children as
#   a fallback (recorded in the process snapshot).
show_prop() {
    local unit="$1" prop="$2"
    awk -v u="^UNIT ${unit}\$" -v p="${prop}=" '$0~u{f=1;next} f&&/^UNIT /{f=0} f&&index($0,p)==1{sub("^"p,"");print;exit}' "$RESULTS"
}
cp_state="$(show_prop chv-controlplane ActiveState)"
cp_result="$(show_prop chv-controlplane Result)"
case "${cp_state}" in
    failed|activating)
        if [ "${cp_result}" = "exit-code" ] \
            && grep -A32 "journal tail chv-controlplane" "$RESULTS" | grep -q "failed to read CA certificate"; then
            qual_pass "control plane failed CLOSED on bare install (typed validation: 'failed to read CA certificate') — packages-alone boundary confirmed"
        else
            qual_error "control plane failed without the expected typed CA-certificate validation (state=${cp_state} result=${cp_result}) — inspect results"
        fi
        ;;
    active)
        qual_warn "control plane came up ACTIVE on a bare install (placeholder jwt, no certs) — inspect results (unexpected for a fail-closed design)"
        ;;
    *)
        qual_error "control plane state not captured ('${cp_state}') — inspect boot log"
        ;;
esac
agent_state="$(show_prop chv-agent ActiveState)"
if [ "$agent_state" = "active" ] && grep -q "CHECK /run/chv/agent 775 chv:chv" "$RESULTS" \
    && grep -q "CHECK /run/chv/core 700 chv:chv" "$RESULTS"; then
    qual_pass "chv-agent active; durable /run contract held (/run/chv/agent 0775 chv:chv, /run/chv/core 0700 chv:chv)"
else
    qual_error "chv-agent leg unexpected: state=${agent_state}; run-dir checks: $(grep '^CHECK /run/chv' "$RESULTS" | tr '\n' ';')"
fi
for u in chv-stord chv-nwd; do
    state="$(show_prop "$u" ActiveState)"
    if [ "$state" = "active" ]; then
        qual_pass "${u} active under systemd with packaged config"
    else
        qual_warn "${u} unit not active on clean install (state=${state}) — known packaging finding (#323 for chv-stord, #324 for chv-nwd)"
    fi
done


qual_summary "clean-install (Legs A + B)" || true
[ "${QUAL_ERRORS}" -gt 0 ] && exit 1 || exit 0
