#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# M4.5 — Storage qualification on real KVM (nested)
#
# The prompt's reusable storage contract
#   validate → provision/consume → attach → guest write/read →
#   restart/interruption → recover → detach → cleanup → repeat
# for the two DECLARED storage profiles:
#
#   local file  — the VM-integrated path (the only profile an operator
#                 can reach today): BFF vm create seeds the boot volume
#                 (qcow2 → raw conversion), stord opens/attaches it,
#                 cloud-init writes a marker from inside the guest, a
#                 stop/start cycle proves the bytes survived (read-back
#                 on the next boot), a stord SIGKILL proves the running
#                 VM survives the daemon's death and the supervisor's
#                 replacement serves NEW provisioning, snapshot/clone
#                 exercise the advertised operator surface, and VM delete
#                 closes the stord session.
#
#   LVM         — the stord LAYER only (loopback PV → VG → LV): the
#                 LVMBackend's real contract (open/export, block
#                 write/read, COW snapshot, clone, resize, read-only
#                 policy, health) via the root-gated integration tests
#                 in crates/chv-stord-backends/tests/lvm_real.rs (the
#                 host-safety pattern: harness provisions the VG, runs
#                 the built test binary, asserts no residue).
#
# Layer truths this scenario records (not works around):
#   - No VM-integrated LVM path exists: the agent's volume reconcile
#     hardcodes backend_class "local", and LVMBackend::open consumes
#     pre-provisioned LVs (provisioning is the host operator's job).
#     LVM is qualified at the stord layer; the integration gap is a
#     finding for the evidence doc.
#   - VM create makes exactly ONE boot volume (no data volumes, no
#     standalone volume-create API) — the contract's "guest write/read"
#     is proven on the boot volume (vda).
#   - Snapshot/clone of a RUNNING VM's volume is copy-based with no
#     guest freeze; this scenario snapshots a STOPPED VM (deterministic
#     content). The running-VM consistency question is recorded, not
#     claimed.
#
# Non-claims (inherited): Ceph RBD and iSCSI are declared non-scope
# (declaration §3). Coexistence residue beyond this scenario's own
# artifacts is not claimed.
#
# Run via deploy.sh --exec (as root, on the qualification host):
#   sudo env "PATH=$PATH" GUEST_IMAGE=noble-qual-patched.img \
#     ./scripts/integration/qual/deploy.sh --exec \
#     ./scripts/integration/qual/m4.5-storage.sh
# ---------------------------------------------------------------------------

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

# --- deployment map (set by deploy.sh --exec) ---
for var in QUAL_TEST_DIR QUAL_NODE_ID QUAL_AGENT_DIR QUAL_LOGS_DIR \
    QUAL_BINARY_DIR QUAL_BFF_URL QUAL_CHVCTL QUAL_CHVCTL_CONFIG_DIR \
    QUAL_CP_PID QUAL_STORD_PID QUAL_NWD_PID QUAL_AGENT_PID \
    QUAL_DB QUAL_NETWORK_CIDR; do
    [ -n "${!var:-}" ] || qual_die "${var} not set — run via deploy.sh --exec"
done

VMS_DIR="${QUAL_AGENT_DIR}/vms"
STORD_DIR="${QUAL_TEST_DIR}/stord"          # deploy's stord runtime_dir
STORD_DB="${STORD_DIR}/stord.db"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DEFAULT_NET="default"                       # network REFERENCE for vm create
GUEST_IMAGE_PATH="${QUAL_GUEST_IMAGE_PATH:-/var/lib/chv/qual/images/noble-qual-patched.img}"

BOOT_TIMEOUT=420        # kernel banner after vm start (nested-virt margin)
LOGIND_TIMEOUT=180      # logind lines after the banner
STOP_TIMEOUT=240        # graceful stop
STORD_RESTART_TIMEOUT=90  # agent supervisor restart of a killed stord
DISPATCH_TIMEOUT=60     # journaled intent → observable host effect
LVM_VG="chvqual-m45"    # disposable VG for the LVM leg
LVM_BACKING_MB=256      # loopback PV size (sparse file)

# Persistent evidence artifacts (deploy.sh removes TEST_DIR on success).
EVIDENCE_DIR="${CHV_QUAL_ROOT:-/var/lib/chv/qual}/m4.5-artifacts"
mkdir -p "$EVIDENCE_DIR"

# Unique per-run marker the guest writes and reads back.
M45_MARKER="m45-$$-persist"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
pids_current() {
    cat > "${QUAL_TEST_DIR}/pids.current" <<EOF
CP_PID=${QUAL_CP_PID}
STORD_PID=${QUAL_STORD_PID}
NWD_PID=${QUAL_NWD_PID}
AGENT_PID=${QUAL_AGENT_PID}
EOF
}

# stord_pid — the CURRENT stord process of this deployment (deploy-started
# or the agent-supervisor's replacement). argv[0]-anchored and scoped to
# this TEST_DIR (the config path) so unrelated processes can never match.
stord_pid() {
    pgrep -f "(^|/)chv-stord( |$).*${QUAL_TEST_DIR}" | head -1
}

# stord_socket_live — the stord api socket accepts connections.
stord_socket_live() {
    python3 - "$STORD_DIR/api.sock" <<'PYEOF' 2>/dev/null
import socket, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
try:
    s.connect(sys.argv[1])
    raise SystemExit(0)
except Exception:
    raise SystemExit(1)
finally:
    s.close()
PYEOF
}

vm_state() {
    qual_chvctl --output json vm list 2>/dev/null \
        | CHV_VM_ID="$1" python3 -c '
import json, os, sys
try:
    data = json.load(sys.stdin)
    if not isinstance(data, list):
        data = data.get("items", [])
    for item in data:
        if item.get("vm_id") == os.environ["CHV_VM_ID"]:
            print(item.get("power_state", ""))
            break
except Exception:
    print("")'
}

vm_ch_gone() {
    ! pgrep -f "(^|/)cloud-hypervisor( |$).*vms/$1" >/dev/null 2>&1
}

wait_vm_stopped() {
    local vm="$1" waited=0 strikes=0 pid
    while ! vm_ch_gone "$vm"; do
        if curl -sg --max-time 2 --unix-socket "${VMS_DIR}/${vm}/vm.sock" \
            http://localhost/api/v1/vm.info >/dev/null 2>&1; then
            strikes=0
        else
            strikes=$((strikes + 1))
            if [ "$strikes" -ge 3 ]; then
                pid="$(cat "${VMS_DIR}/${vm}/ch.pid" 2>/dev/null || true)"
                qual_error "wait_vm_stopped(${vm}): #345 wedge — CH ${pid} alive with a dead API; remediating with SIGKILL"
                [ -n "$pid" ] && kill -9 "$pid" 2>/dev/null || true
            fi
        fi
        sleep 2
        waited=$((waited + 2))
        if [ "$waited" -ge "$STOP_TIMEOUT" ]; then
            qual_error "wait_vm_stopped(${vm}): guest did not stop within ${STOP_TIMEOUT}s"
            return 1
        fi
    done
    qual_pass "vm ${vm}: guest down (CH process exited)"
    return 0
}

vm_console_log() { echo "${VMS_DIR}/$1/console.log"; }

console_has() { grep -aq "$2" "$(vm_console_log "$1")" 2>/dev/null; }

console_count() {
    local f
    f="$(vm_console_log "$1")"
    if [ -f "$f" ]; then
        grep -ac "$2" "$f" || true
    else
        echo 0
    fi
}

count_boots() {
    local f
    f="$(vm_console_log "$1")"
    if [ -f "$f" ]; then
        grep -ac 'Linux version' "$f" || true
    else
        echo 0
    fi
}

boot_count_gt() {
    [ "$(count_boots "$1")" -gt "$2" ]
}

save_console_evidence() {
    cp "$(vm_console_log "$1")" "${EVIDENCE_DIR}/console-$1-$2.log" 2>/dev/null \
        || qual_warn "no console.log to save for $1/$2"
}

wait_boot() {
    local vm="$1" banner_before
    banner_before="$(count_boots "$vm")"
    wait_for "vm ${vm}: kernel banner in console.log" "$BOOT_TIMEOUT" \
        boot_count_gt "$vm" "$banner_before" \
        || return 1
    wait_for "vm ${vm}: systemd-logind lines (boot complete)" "$LOGIND_TIMEOUT" \
        console_has "$vm" "systemd-logind" \
        || return 1
}

# volume_id_of VM_ID — the boot volume bound to this VM (CP DB).
volume_id_of() {
    sqlite_query "$QUAL_DB" \
        "SELECT volume_id FROM volume_desired_state WHERE attached_vm_id='$1'" 2>/dev/null | head -1
}

# volume_field VOLUME_ID FIELD — from the CP DB's volumes table.
volume_field() {
    sqlite_query "$QUAL_DB" \
        "SELECT $2 FROM volumes WHERE volume_id='$1'" 2>/dev/null | head -1
}

# stord_sessions VOLUME_ID — open session rows for the volume in stord.db.
stord_sessions() {
    sqlite_query "$STORD_DB" \
        "SELECT COUNT(*) FROM sessions WHERE volume_id='$1'" 2>/dev/null | head -1
}

# volume_backing VM_ID — the boot volume's backing FILE on the host
# (discovered, not assumed: the agent derives the path at reconcile).
volume_backing() {
    find "${VMS_DIR}/$1" -maxdepth 1 -name '*.img' -type f 2>/dev/null | head -1
}

# create_vm_userdata NAME CPU MEM_MB → VM_ID on stdout (empty on failure).
# Raw BFF POST (chvctl cannot carry cloud_init_userdata): the userdata
# writes the per-run marker on first boot (runcmd, per-instance) and
# reads it back on EVERY boot (bootcmd) — the guest-visible write/read
# proof of the storage contract.
create_vm_userdata() {
    local name="$1" cpu="$2" mem="$3" payload http
    payload="$(python3 - "$name" "$cpu" "$mem" "$M45_MARKER" <<'PYEOF'
import json, sys
name, cpu, mem, marker = sys.argv[1:5]
userdata = (
    "#cloud-config\n"
    "bootcmd:\n"
    "  - [ sh, -c, 'if [ -f /var/lib/m45.marker ]; then "
    "echo \"M45-MARKER-READBACK:$(cat /var/lib/m45.marker)\" > /dev/console; fi' ]\n"
    "runcmd:\n"
    "  - [ sh, -c, "
    f"'echo \"{marker}\" > /var/lib/m45.marker; "
    "echo \"M45-MARKER-WRITTEN:$(cat /var/lib/m45.marker)\" > /dev/console' ]\n"
)
print(json.dumps({
    "name": name,
    "cpu_count": int(cpu),
    "memory_mb": int(mem),
    "image_ref": "ubuntu-noble",
    "network_id": "default",
    "cloud_init_userdata": userdata,
}))
PYEOF
)" || { qual_error "could not build vm-create payload for ${name}"; return 1; }
    http="$(curl -s -o "${EVIDENCE_DIR}/vm-create-${name}.json" -w '%{http_code}' \
        -X POST "${QUAL_BFF_URL}/v1/vms/create" \
        -H "Authorization: Bearer $(bff_token)" -H "Content-Type: application/json" \
        -d "$payload")"
    [ "$http" = "200" ] \
        || { qual_error "BFF vm create ${name} failed (HTTP ${http}): $(cat "${EVIDENCE_DIR}/vm-create-${name}.json" 2>/dev/null)"; return 1; }
    python3 -c '
import json, sys
data = json.load(open(sys.argv[1]))
print(data.get("vm_id") or data.get("id") or "")' "${EVIDENCE_DIR}/vm-create-${name}.json"
}

# create_vm NAME CPU MEM → VM_ID via chvctl (no userdata; the M4.3/M4.4
# shape, used where guest write/read is not the point).
create_vm() {
    local name="$1" cpu="$2" mem="$3" out vm_id
    out="$(qual_chvctl --output json vm create "$name" \
        --cpu "$cpu" --memory "$mem" --image "$GUEST_IMAGE_PATH" \
        --network "$DEFAULT_NET" 2>&1)" \
        || { qual_error "vm create ${name} failed: ${out}"; return 1; }
    vm_id="$(printf '%s\n' "$out" | python3 -c '
import json, sys
raw = sys.stdin.read()
start = raw.find("{")
try:
    data = json.loads(raw[start:])
    print(data.get("vm_id") or data.get("id") or "")
except Exception:
    print("")')"
    [ -n "$vm_id" ] || { qual_error "could not parse vm_id for ${name}: ${out}"; return 1; }
    echo "$vm_id"
}

# bff_post_json PATH BODY_FILE → HTTP code (response saved to stdout).
bff_post_json() {
    curl -s -o "${EVIDENCE_DIR}/bff-$(basename "$1").json" -w '%{http_code}' \
        -X POST "${QUAL_BFF_URL}$1" \
        -H "Authorization: Bearer $(bff_token)" -H "Content-Type: application/json" \
        -d @"$2"
}

# bff_token — the JWT chvctl stored (admin), for raw BFF probes.
bff_token() { cat "${QUAL_CHVCTL_CONFIG_DIR}/chvctl/credentials" 2>/dev/null; }

save_evidence() {
    # $1 = phase label; captures storage-relevant host + DB state.
    local label="$1"
    {
        echo "### m4.5 evidence snapshot: ${label} ($(date -u +%FT%TZ))"
        echo "--- vm dirs and volume backings:"
        find "$VMS_DIR" -maxdepth 2 -name '*.img' -printf '%p %s bytes\n' 2>/dev/null || true
        echo "--- stord runtime dir:"
        ls -la "$STORD_DIR" 2>/dev/null || true
        echo "--- stord sessions:"
        sqlite_query "$STORD_DB" \
            "SELECT volume_id, vm_id, runtime_status FROM sessions" 2>/dev/null || true
        echo "--- CP volumes:"
        sqlite_query "$QUAL_DB" \
            "SELECT volume_id, node_id, capacity_bytes FROM volumes" 2>/dev/null || true
        echo "--- CP volume_desired_state:"
        sqlite_query "$QUAL_DB" \
            "SELECT volume_id, attached_vm_id, snapshot_op, snapshot_name, clone_source_volume_id FROM volume_desired_state" 2>/dev/null || true
        echo
    } >> "${EVIDENCE_DIR}/host-state.txt"
}

qual_info "=== M4.5 storage qualification start (node ${QUAL_NODE_ID}) ==="
qual_info "candidate: $(cat "${QUAL_BINARY_DIR}/CANDIDATE_SHA" 2>/dev/null || echo unknown)"
pids_current
save_evidence "start"

# ---------------------------------------------------------------------------
# Leg A — local file: provision (seed → raw) → attach → boot → guest WRITE
# ---------------------------------------------------------------------------
qual_info "--- Leg A: vm create (seeded boot volume) → host materialization → boot → guest writes the marker"

VM1_ID="$(create_vm_userdata qual-stor-1 2 1024)" || qual_die "aborting"
qual_pass "vm created: qual-stor-1 (${VM1_ID}) with cloud_init_userdata (marker ${M45_MARKER})"

VOL1_ID="$(volume_id_of "$VM1_ID")"
[ -n "$VOL1_ID" ] \
    && qual_pass "boot volume bound in CP DB (${VOL1_ID})" \
    || qual_die "no volume in volume_desired_state for ${VM1_ID}"

VOL1_CAPACITY="$(volume_field "$VOL1_ID" capacity_bytes)"
[ -n "$VOL1_CAPACITY" ] && [ "$VOL1_CAPACITY" -gt 0 ] \
    && qual_pass "volume row carries capacity (${VOL1_CAPACITY} bytes)" \
    || qual_warn "volume capacity missing/zero in CP DB: '${VOL1_CAPACITY}'"

# NOTE: wait_for evaluates its args ONCE — the backing discovery must
# re-run per poll, hence the function (the m4.4-documented trap). The
# wait gates on CONVERSION COMPLETION, not file existence: the local
# backend materializes the backing as an intermediate qcow2 copy first
# (smaller than capacity) and only reaches full size after the
# qcow2→raw conversion + set_len (run-2 finding).
vm_volume_ready() {
    local p
    p="$(volume_backing "$1")"
    [ -n "$p" ] && [ "$(stat -c %s "$p" 2>/dev/null || echo 0)" -ge "$2" ]
}
wait_for "volume backing materialized + seed conversion complete (full size)" \
    "$DISPATCH_TIMEOUT" \
    vm_volume_ready "$VM1_ID" "${VOL1_CAPACITY:-1}" \
    || qual_die "no volume backing file under ${VMS_DIR}/${VM1_ID}"
VOL1_PATH="$(volume_backing "$VM1_ID")"
qual_pass "volume backing materialized on host: ${VOL1_PATH}"

# The seed conversion contract: the backing must be RAW (the qcow2 seed
# was converted — a lingering qcow2 would still boot via CH but breaks
# the local backend's snapshot/clone, which reject qcow2 explicitly).
if qemu-img info "$VOL1_PATH" 2>/dev/null | grep -q 'file format: raw'; then
    qual_pass "seeded volume is RAW (qcow2 → raw conversion ran)"
else
    qual_warn "volume backing is not raw per qemu-img: $(qemu-img info "$VOL1_PATH" 2>/dev/null | grep 'file format' || echo 'qemu-img unavailable')"
fi

VOL1_SIZE="$(stat -c %s "$VOL1_PATH" 2>/dev/null || echo 0)"
[ "$VOL1_SIZE" -ge "${VOL1_CAPACITY:-0}" ] \
    && qual_pass "backing size ${VOL1_SIZE} ≥ requested capacity ${VOL1_CAPACITY}" \
    || qual_warn "backing size ${VOL1_SIZE} < requested capacity ${VOL1_CAPACITY}"

# stord opened a session for the volume (the agent↔stord attach path).
stord_session_open() { [ "$(stord_sessions "$VOL1_ID")" -ge 1 ]; }
wait_for "stord session open for the volume (${STORD_DB})" "$DISPATCH_TIMEOUT" \
    stord_session_open \
    || qual_error "no stord session row for ${VOL1_ID}"

save_evidence "leg-a provisioned"

qual_chvctl vm start "$VM1_ID" >/dev/null || qual_die "vm start failed for ${VM1_ID}"
wait_boot "$VM1_ID" || qual_die "guest did not boot (Leg A)"

# THE guest write: cloud-init runcmd wrote the marker to the boot volume.
wait_for "guest wrote the marker to the boot volume (console)" "$LOGIND_TIMEOUT" \
    console_has "$VM1_ID" "M45-MARKER-WRITTEN:${M45_MARKER}" \
    || qual_die "marker write never appeared in the guest console (Leg A)"
qual_pass "guest WROTE the marker (vda): ${M45_MARKER}"
save_console_evidence "$VM1_ID" leg-a
save_evidence "leg-a booted"

# ---------------------------------------------------------------------------
# Leg B — stop/start → the SAME bytes read back (persistence across the
# attach/detach cycle of the boot volume)
# ---------------------------------------------------------------------------
qual_info "--- Leg B: stop → start → guest reads the SAME marker back"

qual_chvctl vm stop "$VM1_ID" >/dev/null || qual_error "vm stop failed for ${VM1_ID}"
wait_vm_stopped "$VM1_ID" || qual_die "vm did not stop (Leg B)"

# At rest: the backing persists, unchanged (data survives the stop).
[ -f "$VOL1_PATH" ] && [ "$(stat -c %s "$VOL1_PATH" 2>/dev/null)" = "$VOL1_SIZE" ] \
    && qual_pass "volume backing persists at stop (size unchanged, ${VOL1_SIZE})" \
    || qual_error "volume backing changed/disappeared at stop: ${VOL1_PATH}"

# The stord session survives the stop (the volume stays open agent-side).
[ "$(stord_sessions "$VOL1_ID")" -ge 1 ] \
    && qual_pass "stord session survives the stop (volume stays open)" \
    || qual_warn "stord session for ${VOL1_ID} gone at stop (re-open on start would be the truth to record)"

qual_chvctl vm start "$VM1_ID" >/dev/null || qual_die "vm re-start failed for ${VM1_ID}"

# Boot detection on the SECOND boot cannot count kernel banners: the
# graceful stop TRUNCATES console.log (agent: "truncated console.log on
# graceful stop") and the agent re-spawns CH quickly, so the fresh log
# may already contain boot 2's banner when a snapshot is taken (run-4
# finding: banner_before=1 with only one more boot coming → false
# timeout). Instead wait for the boot-2 evidence directly: logind lines
# in the truncated log (boot 1's are gone), then THE contract — the
# readback, which only a boot with the persisted marker file can print.
wait_for "vm ${VM1_ID}: re-booted (logind lines in the fresh console)" \
    "$LOGIND_TIMEOUT" \
    console_has "$VM1_ID" "systemd-logind" \
    || qual_die "guest did not re-boot (Leg B)"

# THE guest read: bootcmd (every boot) cats the marker the FIRST boot
# wrote — proving the bytes survived the stop/start cycle on the same
# volume. This cannot false-positive: boot 1's bootcmd ran before the
# marker existed, and boot 1's console was truncated at the stop — a
# READBACK line in the current log can only come from boot 2 reading
# the persisted file. It also proves start did NOT re-seed the volume
# (a re-seed would delete the marker file).
wait_for "guest read the marker back (console)" "$LOGIND_TIMEOUT" \
    console_has "$VM1_ID" "M45-MARKER-READBACK:${M45_MARKER}" \
    || qual_die "marker read-back never appeared in the guest console (Leg B)"
qual_pass "guest READ BACK the same marker after restart: ${M45_MARKER}"

# runcmd is per-instance: boot 2 must NOT have re-written the marker
# (its console is fresh post-truncation, so zero WRITTEN lines is the
# expected shape; any WRITTEN line means runcmd re-ran or the console
# was not truncated — either way worth recording).
WRITTEN_ON_BOOT2="$(console_count "$VM1_ID" "M45-MARKER-WRITTEN")"
if [ "$WRITTEN_ON_BOOT2" -eq 0 ]; then
    qual_pass "runcmd did not re-run on the second boot (per-instance; no re-seed)"
else
    qual_warn "M45-MARKER-WRITTEN appeared ${WRITTEN_ON_BOOT2}x in the second boot's console — runcmd re-ran (per-instance violation?) or the console was not truncated"
fi
save_console_evidence "$VM1_ID" leg-b
save_evidence "leg-b restarted"

# ---------------------------------------------------------------------------
# Leg C — interruption: stord SIGKILL under a running VM → data plane
# unaffected → supervisor recovery → NEW provisioning works (repeat)
# ---------------------------------------------------------------------------
qual_info "--- Leg C: stord SIGKILL (VM running) → recovery → fresh provisioning"

STORD_PID_BEFORE="$(stord_pid)"
[ -n "$STORD_PID_BEFORE" ] || qual_die "no stord process found (Leg C)"
kill -9 "$STORD_PID_BEFORE"
qual_info "stord (pid ${STORD_PID_BEFORE}) SIGKILLed"

# The running VM is unaffected: CH holds the disk fd; the data plane is
# kernel state (the M4.4-proven shape, now for storage).
sleep 3
[ "$(vm_state "$VM1_ID")" = "Running" ] && ! vm_ch_gone "$VM1_ID" \
    && qual_pass "running VM unaffected by stord death (CH alive, power_state Running)" \
    || qual_error "VM ${VM1_ID} disturbed by stord SIGKILL (state: $(vm_state "$VM1_ID"))"

# The agent supervisor restarts stord (like nwd — the same supervisor).
wait_for "agent supervisor restarted stord" "$STORD_RESTART_TIMEOUT" \
    bash -c "[ \"\$(pgrep -f \"(^|/)chv-stord( |$).*${QUAL_TEST_DIR}\" | head -1)\" != \"${STORD_PID_BEFORE}\" ]" \
    || qual_die "stord was not restarted by the supervisor (Leg C)"
STORD_PID_AFTER="$(stord_pid)"
qual_pass "stord restarted by the agent supervisor (pid ${STORD_PID_BEFORE} → ${STORD_PID_AFTER})"
wait_for "restarted stord socket live" 20 stord_socket_live \
    || qual_error "restarted stord socket not accepting"

# Recovery must serve NEW provisioning (the repeat of the contract): a
# fresh VM create goes through the restarted stord's open/attach.
VM2_ID="$(create_vm qual-stor-2 2 1024)" || qual_die "fresh provisioning failed after stord restart (Leg C)"
qual_pass "fresh vm create accepted after stord restart (qual-stor-2: ${VM2_ID})"
VOL2_ID="$(volume_id_of "$VM2_ID")"
VM2_session_open() { [ "$(stord_sessions "$VOL2_ID")" -ge 1 ]; }
[ -n "$VOL2_ID" ] && wait_for "restarted stord opened the new volume (session row present)" \
    "$DISPATCH_TIMEOUT" VM2_session_open \
    && qual_pass "restarted stord opened the new volume (session row present)" \
    || qual_error "no stord session for the new volume ${VOL2_ID}"
qual_chvctl vm start "$VM2_ID" >/dev/null || qual_die "vm start failed for ${VM2_ID}"
wait_boot "$VM2_ID" || qual_die "guest did not boot (Leg C repeat)"
save_console_evidence "$VM2_ID" leg-c

# VM-1 is still healthy through all of this.
[ "$(vm_state "$VM1_ID")" = "Running" ] \
    && qual_pass "vm-1 still Running across the whole interruption" \
    || qual_error "vm-1 state drifted during Leg C: $(vm_state "$VM1_ID")"

# Detach/close on delete (VM-2's teardown): the stord session row must
# be gone after VM delete (close_volume deletes the session row).
qual_chvctl vm delete "$VM2_ID" >/dev/null || qual_error "vm delete failed for ${VM2_ID}"
wait_vm_stopped "$VM2_ID" || qual_error "vm ${VM2_ID} did not stop for delete"
wait_for "vm ${VM2_ID}: CH gone" 30 vm_ch_gone "$VM2_ID" || true
wait_for "stord session for ${VOL2_ID} closed on delete" "$DISPATCH_TIMEOUT" \
    bash -c "[ \"\$(sqlite_query '${STORD_DB}' \"SELECT COUNT(*) FROM sessions WHERE volume_id='${VOL2_ID}'\" 2>/dev/null | head -1)\" = \"0\" ]" \
    && qual_pass "stord session closed on VM delete (detach/close contract)" \
    || qual_error "stord session for ${VOL2_ID} NOT closed on VM delete"
save_evidence "leg-c recovered"

# ---------------------------------------------------------------------------
# Leg D — snapshot + clone of the (stopped) boot volume: the advertised
# operator surface, end to end through the journaled dispatch
# ---------------------------------------------------------------------------
qual_info "--- Leg D: snapshot + clone (chvctl → BFF → CP journal → agent → stord copy)"

qual_chvctl vm stop "$VM1_ID" >/dev/null || qual_error "vm stop failed for ${VM1_ID}"
wait_vm_stopped "$VM1_ID" || qual_die "vm did not stop (Leg D)"

SNAP_NAME="m45snap"
qual_chvctl volume snapshot "$VOL1_ID" --name "$SNAP_NAME" >/dev/null 2>&1 \
    && qual_pass "chvctl volume snapshot accepted (${VOL1_ID} → ${SNAP_NAME})" \
    || qual_die "chvctl volume snapshot failed (was the #372 CLI fix deployed?)"

# The journaled intent dispatches: the snapshot copy appears in stord's
# runtime dir as {volume_id}-{snapshot_name}.img.
SNAP_FILE="${STORD_DIR}/${VOL1_ID}-${SNAP_NAME}.img"
wait_for "snapshot copy materialized (${SNAP_FILE})" "$DISPATCH_TIMEOUT" \
    test -s "$SNAP_FILE" \
    && qual_pass "snapshot materialized on the host ($(stat -c %s "$SNAP_FILE") bytes)" \
    || qual_error "snapshot file never appeared: ${SNAP_FILE}"

[ -f "$SNAP_FILE" ] && [ "$(stat -c %s "$SNAP_FILE" 2>/dev/null)" = "$VOL1_SIZE" ] \
    && qual_pass "snapshot is a full point-in-time copy (size == volume)" \
    || qual_warn "snapshot size differs from volume ($(stat -c %s "$SNAP_FILE" 2>/dev/null) vs ${VOL1_SIZE})"

# Clone: the target is a NEW volume id (the #372 contract).
CLONE_ID="vol-m45clone$$"
qual_chvctl volume clone "$VOL1_ID" --name "$CLONE_ID" >/dev/null 2>&1 \
    && qual_pass "chvctl volume clone accepted (${VOL1_ID} → ${CLONE_ID})" \
    || qual_error "chvctl volume clone failed"

# The clone dispatches: a copy keyed by source and target appears.
# (Per-poll function — wait_for evaluates args once.)
clone_file_present() {
    [ -n "$(find "$STORD_DIR" -maxdepth 1 -name "*${CLONE_ID}*.img" -type f 2>/dev/null | head -1)" ]
}
wait_for "clone copy materialized in stord runtime dir" "$DISPATCH_TIMEOUT" \
    clone_file_present \
    && qual_pass "clone materialized on the host" \
    || qual_error "clone file never appeared for target ${CLONE_ID} (dispatch gap?)"
CLONE_FILE="$(find "$STORD_DIR" -maxdepth 1 -name "*${CLONE_ID}*.img" -type f 2>/dev/null | head -1)"
qual_info "clone file: ${CLONE_FILE:-none}"
[ -n "$CLONE_FILE" ] && [ "$(stat -c %s "$CLONE_FILE" 2>/dev/null)" = "$VOL1_SIZE" ] \
    && qual_pass "clone is a full copy (size == volume)" \
    || qual_warn "clone size differs ($(stat -c "$CLONE_FILE" 2>/dev/null))"

# The CP recorded the clone's desired state (the journal trail).
CLONE_DS="$(sqlite_query "$QUAL_DB" \
    "SELECT clone_source_volume_id FROM volume_desired_state WHERE volume_id='${CLONE_ID}'" 2>/dev/null | head -1)"
[ "$CLONE_DS" = "$VOL1_ID" ] \
    && qual_pass "CP DB records the clone intent (${CLONE_ID} ← ${VOL1_ID})" \
    || qual_warn "no clone intent row for ${CLONE_ID} in volume_desired_state (got: '${CLONE_DS}')"

# Delete the snapshot through the BFF (chvctl has no delete-snapshot
# command — recorded; the route exists).
DELETE_BODY="$(mktemp "${QUAL_TEST_DIR}/m45-del-snap.json")"
echo "{\"volume_id\":\"${VOL1_ID}\",\"snapshot_name\":\"${SNAP_NAME}\"}" > "$DELETE_BODY"
HTTP_CODE="$(bff_post_json "/v1/volumes/delete-snapshot" "$DELETE_BODY")"
[ "$HTTP_CODE" = "200" ] \
    && qual_pass "BFF delete-snapshot accepted (HTTP ${HTTP_CODE})" \
    || qual_error "BFF delete-snapshot failed (HTTP ${HTTP_CODE}): $(cat "${EVIDENCE_DIR}/bff-delete-snapshot.json" 2>/dev/null)"
rm -f "$DELETE_BODY"
wait_for "snapshot file removed by delete-snapshot" "$DISPATCH_TIMEOUT" \
    bash -c "[ ! -f '${SNAP_FILE}' ]" \
    && qual_pass "snapshot file removed from the host" \
    || qual_error "snapshot file still present after delete-snapshot: ${SNAP_FILE}"
save_evidence "leg-d snapshotted"

# ---------------------------------------------------------------------------
# Leg E — cleanup: VM delete → stord session closed → residue assertions
# ---------------------------------------------------------------------------
qual_info "--- Leg E: vm delete → session closed → no residue"

qual_chvctl vm delete "$VM1_ID" >/dev/null || qual_error "vm delete failed for ${VM1_ID}"
wait_vm_stopped "$VM1_ID" || qual_error "vm ${VM1_ID} did not stop for delete"
wait_for "vm ${VM1_ID}: CH gone" 30 vm_ch_gone "$VM1_ID" || true

wait_for "stord session for ${VOL1_ID} closed on delete" "$DISPATCH_TIMEOUT" \
    bash -c "[ \"\$(sqlite_query '${STORD_DB}' \"SELECT COUNT(*) FROM sessions WHERE volume_id='${VOL1_ID}'\" 2>/dev/null | head -1)\" = \"0\" ]" \
    && qual_pass "stord session closed on VM delete" \
    || qual_error "stord session for ${VOL1_ID} NOT closed on VM delete"

# The volume row: cascade or retention — record the truth.
VOL1_ROW="$(sqlite_query "$QUAL_DB" \
    "SELECT COUNT(*) FROM volumes WHERE volume_id='${VOL1_ID}'" 2>/dev/null | head -1)"
[ "$VOL1_ROW" = "0" ] \
    && qual_pass "volume row removed with the VM (cascade)" \
    || qual_warn "volume row retained after VM delete (${VOL1_ROW} row) — M2.5 residual class"

# The volume backing: the M2.5 residual was retention. Record the truth.
if [ -f "$VOL1_PATH" ]; then
    qual_warn "volume backing RETAINED on VM delete (known M2.5 residual): ${VOL1_PATH}"
elif [ -d "${VMS_DIR}/${VM1_ID}" ]; then
    qual_pass "volume backing removed on VM delete (vm dir retained: $(ls "${VMS_DIR}/${VM1_ID}" 2>/dev/null | tr '\n' ' '))"
else
    qual_pass "vm dir fully removed on delete"
fi

# Forbidden outcomes: no CH processes, no leaked conversion processes,
# no stray stord sessions at all.
[ -z "$(pgrep -f "(^|/)cloud-hypervisor( |$).*vms/" 2>/dev/null)" ] \
    && qual_pass "no CH processes remain" \
    || qual_error "CH processes remain: $(pgrep -af cloud-hypervisor | head -3)"
[ -z "$(pgrep -x qemu-img 2>/dev/null)" ] \
    && qual_pass "no leaked qemu-img conversion processes" \
    || qual_error "qemu-img still running: $(pgrep -a qemu-img)"
STORD_SESSIONS_LEFT="$(sqlite_query "$STORD_DB" \
    "SELECT COUNT(*) FROM sessions" 2>/dev/null | head -1)"
[ "${STORD_SESSIONS_LEFT:-0}" = "0" ] \
    && qual_pass "no stord sessions remain" \
    || qual_warn "stord sessions remain (${STORD_SESSIONS_LEFT}): $(sqlite_query "$STORD_DB" 'SELECT volume_id, runtime_status FROM sessions' 2>/dev/null | head -3)"
save_evidence "leg-e cleaned"

# ---------------------------------------------------------------------------
# Leg F — LVM profile (stord layer): real-LVM contract via the root-gated
# integration tests, on a harness-provisioned loopback VG
# ---------------------------------------------------------------------------
qual_info "--- Leg F: LVM (stord layer) — loopback PV → VG → LV, real-LVM backend contract"

for tool in losetup pvcreate vgcreate lvcreate blockdev lvs cargo; do
    command -v "$tool" >/dev/null 2>&1 \
        || qual_die "LVM leg needs ${tool} on the qualification host"
done

# Identity guard (host-safety pattern): building the tests from THIS tree
# only tests the deployed build if chv-stord-backends is identical.
CANDIDATE_SHA="$(cat "${QUAL_BINARY_DIR}/CANDIDATE_SHA" 2>/dev/null || true)"
if [ -n "$CANDIDATE_SHA" ]; then
    if git -C "$REPO_ROOT" diff --quiet "$CANDIDATE_SHA" HEAD -- crates/chv-stord-backends; then
        qual_pass "candidate identity: crates/chv-stord-backends identical at ${CANDIDATE_SHA:0:8} and HEAD"
    else
        qual_die "crates/chv-stord-backends DIFFERS between candidate ${CANDIDATE_SHA:0:8} and HEAD — building from this tree would not test the candidate"
    fi
else
    qual_warn "no candidate sha available — identity check skipped; results apply to THIS tree only"
fi

# Baseline for the residue assertions.
LOOPS_BEFORE="$(losetup -a 2>/dev/null | cut -d: -f1 | sort)"

# Build the test binary (host-safety pattern).
qual_info "building chv-stord-backends test binaries (cargo test --no-run)..."
if ! (cd "$REPO_ROOT" && cargo test -p chv-stord-backends --no-run 2>&1 | tail -5); then
    qual_die "cargo test --no-run failed for chv-stord-backends"
fi
TEST_BIN="$(find "${REPO_ROOT}/target/debug/deps" -maxdepth 1 -name 'lvm_real-*' -type f -executable 2>/dev/null | sort | tail -1)"
[ -n "$TEST_BIN" ] || qual_die "could not locate the lvm_real test binary under target/debug/deps"
qual_info "test binary: ${TEST_BIN}"

# Provision the disposable VG (the operator model: stord consumes
# pre-provisioned volumes; provisioning is out-of-band by design).
LVM_BACKING="${QUAL_TEST_DIR}/m45-lvm-backing.img"
truncate -s "${LVM_BACKING_MB}M" "$LVM_BACKING"
LVM_LOOP="$(losetup -f --show "$LVM_BACKING")"
[ -n "$LVM_LOOP" ] || qual_die "no free loop device"
pvcreate -f "$LVM_LOOP" >/dev/null 2>&1 || qual_die "pvcreate failed"
vgcreate "$LVM_VG" "$LVM_LOOP" >/dev/null 2>&1 || qual_die "vgcreate failed"
qual_pass "loopback VG provisioned: ${LVM_LOOP} → ${LVM_VG} ($(vgs "$LVM_VG" --noheadings -o vg_size 2>/dev/null | tr -d ' '))"

# Run the real-LVM contract tests serially (snapshots claim 100%FREE).
LVM_TESTS="lvm_real_open_export_and_block_roundtrip lvm_real_open_rejects_wrong_class \
lvm_real_snapshot_is_copy_on_write lvm_real_clone_copies_data lvm_real_resize_grows_volume \
lvm_real_read_only_policy_blocks_writes lvm_real_health_reflects_existence"
LVM_FAILED=0
for test_name in $LVM_TESTS; do
    qual_info "running ${test_name} (ignored/root-gated real-LVM test)..."
    if CHV_LVM_TEST_VG="$LVM_VG" "$TEST_BIN" --ignored --exact --nocapture "$test_name" \
        > "${EVIDENCE_DIR}/lvm-${test_name}.log" 2>&1; then
        qual_pass "lvm-real: ${test_name}"
    else
        qual_error "lvm-real: ${test_name} FAILED (see ${EVIDENCE_DIR}/lvm-${test_name}.log)"
        LVM_FAILED=1
    fi
done

# Teardown + residue assertions (forbidden outcomes).
vgremove -f "$LVM_VG" >/dev/null 2>&1 || qual_error "vgremove failed"
pvremove "$LVM_LOOP" >/dev/null 2>&1 || qual_error "pvremove failed"
losetup -d "$LVM_LOOP" 2>/dev/null || qual_error "losetup -d failed"
rm -f "$LVM_BACKING"

[ -z "$(vgs --noheadings -o vg_name 2>/dev/null | grep -x "$LVM_VG")" ] \
    && qual_pass "no VG residue (${LVM_VG} removed)" \
    || qual_error "VG ${LVM_VG} still present"
[ -z "$(lvs --noheadings -o lv_name,vg_name 2>/dev/null | grep "$LVM_VG")" ] \
    && qual_pass "no LV residue in ${LVM_VG}" \
    || qual_error "LVs remain in ${LVM_VG}: $(lvs "$LVM_VG" --noheadings 2>/dev/null)"
losetup "$LVM_LOOP" >/dev/null 2>&1 \
    && qual_error "loop device ${LVM_LOOP} still attached" \
    || qual_pass "loop device ${LVM_LOOP} detached"
LOOPS_AFTER="$(losetup -a 2>/dev/null | cut -d: -f1 | sort)"
NEW_LOOPS="$(comm -13 <(printf '%s\n' "$LOOPS_BEFORE") <(printf '%s\n' "$LOOPS_AFTER") | grep -v '^$' || true)"
[ -z "$NEW_LOOPS" ] \
    && qual_pass "no new loop devices remain" \
    || qual_error "new loop devices remain: ${NEW_LOOPS}"
[ ! -f "$LVM_BACKING" ] \
    && qual_pass "loopback backing file removed" \
    || qual_error "backing file remains: ${LVM_BACKING}"

[ "$LVM_FAILED" = "0" ] \
    && qual_pass "LVM backend contract proven on real LVM (7 root-gated tests)" \
    || qual_error "LVM backend contract has failures"

save_evidence "leg-f lvm done"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
pids_current
if [ "$QUAL_ERRORS" -gt 0 ]; then
    qual_error "M4.5 scenario finished with ${QUAL_ERRORS} error(s)"
    if [ -n "${QUAL_PRESERVE_DIR:-}" ]; then
        qual_error "test dir preserved for post-mortem: ${QUAL_PRESERVE_DIR}"
    else
        qual_info "test dir: ${QUAL_TEST_DIR} (deploy keeps it on failure)"
    fi
    exit 1
fi
qual_pass "M4.5 storage scenario complete: local file (VM-integrated) + LVM (stord layer) qualified"
exit 0
