# Runbook: Volume Snapshot Restore

**Severity:** SEV-2 (data unavailability) / SEV-3 (planned rollback)  
**Automation Level:** Automated for `local` and `ceph` backends; manual for `lvm` and `iscsi`  
**Estimated Time:** 2–10 minutes (local/ceph); 30–60 minutes (lvm/iscsi manual)  
**Prerequisites:**
- Volume exists and is registered in CHV
- At least one snapshot exists for the volume
- For Ceph: `rbd` CLI available on the stord host
- For LVM/iSCSI: manual disk replacement procedure (see below)

---

## 1. Identify Volume and Storage Backend

```bash
chvctl volume show <VOLUME_ID>
```

Note the `storage_backend` field. This determines the restore path.

### Via API

```bash
curl -s https://controlplane.example.com/v1/volumes/<VOLUME_ID> \
  -H "Authorization: Bearer $TOKEN" | jq '{volume_id, storage_backend, node_id, status}'
```

---

## 2. Path A: Automated Restore (local / ceph)

### 2a. List Volume Snapshots

```bash
curl -s "https://controlplane.example.com/v1/volumes/<VOLUME_ID>/snapshots" \
  -H "Authorization: Bearer $TOKEN" | jq '.snapshots[] | {snapshot_id, created_at}'
```

### 2b. Execute Restore

```bash
curl -X POST https://controlplane.example.com/v1/volumes/restore-snapshot \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "volume_id": "<VOLUME_ID>",
    "snapshot_id": "<SNAPSHOT_ID>"
  }' | jq
```

### 2c. Monitor Operation

Volume restore is dispatched as a `RestoreVolume` operation:

```bash
curl -s "https://controlplane.example.com/v1/operations?resource_id=<VOLUME_ID>" \
  -H "Authorization: Bearer $TOKEN" | jq '.operations[] | select(.operation_type == "RestoreVolume")'
```

Check stord logs:

```bash
sudo journalctl -u chv-stord -f | grep -i "restore\|snapshot"
```

### 2d. Backend-Specific Verification

**Local backend:**

```bash
# On the node hosting the volume
ls -la /var/lib/chv/stord/volumes/<VOLUME_ID>/
# Should show the snapshot rolled back to the active image
```

**Ceph backend:**

```bash
# On the stord host
rbd snap ls <pool>/<image>
# The target snapshot should show as the active state after rollback
```

---

## 3. Path B: Manual Restore (lvm / iscsi)

> **Warning:** LVM and iSCSI snapshot restore are **not implemented** in CHV stord. You must perform manual disk replacement. This procedure requires downtime for any VM attached to the volume.

### 3a. Identify Affected VMs

```bash
# Find VMs using this volume
curl -s "https://controlplane.example.com/v1/vms" \
  -H "Authorization: Bearer $TOKEN" | \
  jq --arg vol "<VOLUME_ID>" '.vms[] | select(.volume_ids[] == $vol) | {vm_id, name, status}'
```

### 3b. Stop Affected VMs

```bash
chvctl vm stop <VM_ID_1>
chvctl vm stop <VM_ID_2>
# ... for all attached VMs
```

### 3c. Locate the Volume on Disk

**LVM:**

```bash
sudo lvs | grep <VOLUME_ID>
# Note the VG and LV names
```

**iSCSI:**

```bash
sudo lsblk | grep -i iscsi
# Note the device path (e.g., /dev/sdX)
cat /etc/iscsi/initiatorname.iscsi
```

### 3d. Prepare Replacement Data

You need a point-in-time copy of the volume data. Options:

1. **From a CHV backup job** (if the volume was backed up):
   - Locate the backup artifact in S3/NFS (see [Backup Artifact Restore](backup-artifact-restore.md))
   - Download and decompress to a temporary location

2. **From an external backup** (rsync, ZFS send, etc.)

3. **From a manual LVM/iSCSI snapshot** taken outside CHV:
   - LVM: `sudo lvconvert --merge /dev/vg/snapshot-lv`
   - iSCSI: vendor-specific rollback procedure

### 3e. Replace the Volume Data

**LVM procedure:**

```bash
# Unmount if mounted
sudo umount /mnt/chv-volumes/<VOLUME_ID> 2>/dev/null || true

# Deactivate the LV
sudo lvchange -an /dev/<VG>/<LV>

# Write replacement data (example: from a qcow2 image)
sudo qemu-img convert -O raw /tmp/restore-image.qcow2 /dev/<VG>/<LV>

# Reactivate
sudo lvchange -ay /dev/<VG>/<LV>
```

**iSCSI procedure:**

```bash
# Log out of the target
sudo iscsiadm -m node -T <TARGET_IQN> -p <PORTAL> --logout

# On the iSCSI target host (or vendor management console):
# Replace the LUN contents with the restore image
# This step is vendor-specific — consult your storage array documentation

# Re-log in
sudo iscsiadm -m node -T <TARGET_IQN> -p <PORTAL> --login
```

### 3f. Update CHV State

```bash
# Verify the volume is accessible
sudo blockdev --getsize64 /dev/<DEVICE>

# Restart VMs
chvctl vm start <VM_ID_1>
```

### 3g. Record the Manual Intervention

Update the volume's metadata in CHV to reflect the manual restore (optional but recommended for audit trails):

```bash
# Note the restore time for operational records
sqlite3 /var/lib/chv/controlplane.db \
  "INSERT INTO volume_events (volume_id, event_type, created_at) VALUES ('<VOLUME_ID>', 'manual_restore', datetime('now'));"
```

---

## Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| `RestoreVolume operation failed` (local/ceph) | Check stord logs | `journalctl -u chv-stord -n 500` |
| Ceph rollback hangs | Check `rbd status` for watchers | Ensure no VM has the RBD image open |
| LVM `lvchange` fails with "open" error | `lsof /dev/<VG>/<LV>` | Stop all VMs using the volume first |
| iSCSI re-login fails | `iscsiadm -m session` | Verify target is up; check network/firewall |
| Data corruption after restore | Checksum mismatch | Re-download backup artifact; verify with `sha256sum` |

## What Happens Under the Hood (local/ceph)

### Local Backend
1. BFF validates request
2. `LifecycleService` creates `RestoreVolume` operation
3. `Orchestrator` dispatches to stord on the target node
4. Stord performs atomic rename: snapshot → active image using a `.restore-tmp` staging file
5. Operation completes

### Ceph Backend
1. Same dispatch path through orchestrator
2. Stord calls `rbd snap rollback <pool>/<image>@<snapshot_id>`
3. RBD reverts the image to the snapshot state
4. Operation completes

## Related Runbooks

- [VM Snapshot Restore](vm-snapshot-restore.md) — For VM-level (not volume-level) recovery
- [Backup Artifact Restore](backup-artifact-restore.md) — When no local snapshot exists
