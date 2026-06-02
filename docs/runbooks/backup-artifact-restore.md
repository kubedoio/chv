# Runbook: Backup Artifact Restore

**Severity:** SEV-1 (data loss) / SEV-2 (production VM unavailability)  
**Automation Level:** **Manual** — CHV does not yet have a restore worker for shipped backup artifacts  
**Estimated Time:** 10–30 minutes  
**Prerequisites:**
- Backup job completed successfully (status = `Succeeded`)
- Access to the backup destination (S3 bucket or NFS mount)
- `qemu-img` installed on the target node
- Target VM is **stopped** before disk replacement
- Sufficient disk space on the target node for the artifact + working copy

> **Important:** The `POST /v1/backups/restores` API endpoint creates a tracking record but **does not execute the restore**. This runbook describes the manual procedure until the restore worker is implemented.

---

## 1. Identify the Backup Job

### Via chvctl

```bash
chvctl backup list
```

### Via API

```bash
curl -s "https://controlplane.example.com/v1/backups/jobs?vm_id=<VM_ID>" \
  -H "Authorization: Bearer $TOKEN" | jq '.jobs[] | {job_id, status, destination, storage_backend, checksum, size_bytes, completed_at}'
```

### Via SQLite (direct DB access)

```bash
sudo sqlite3 /var/lib/chv/controlplane.db \
  "SELECT job_id, status, destination, storage_backend, checksum, size_bytes, completed_at 
   FROM backup_jobs 
   WHERE vm_id = '<VM_ID>' AND status = 'Succeeded' 
   ORDER BY completed_at DESC LIMIT 5;"
```

Note:
- `job_id` — the artifact filename will be `{job_id}.backup`
- `destination` — the remote path (S3 key prefix or NFS mount path)
- `storage_backend` — `s3`, `nfs`, or `null`
- `checksum` — SHA256 of the artifact (verify after download)

---

## 2. Locate and Download the Artifact

### 2a. S3 Destination

```bash
# Extract bucket and prefix from the schedule configuration
# (or inspect the destination field directly)

# If you have the schedule_id:
SCHEDULE_ID=$(sudo sqlite3 /var/lib/chv/controlplane.db \
  "SELECT schedule_id FROM backup_jobs WHERE job_id = '<JOB_ID>';")
S3_DEST=$(sudo sqlite3 /var/lib/chv/controlplane.db \
  "SELECT destination FROM backup_schedules WHERE schedule_id = '$SCHEDULE_ID';")

# Example: s3://my-backup-bucket/chv/vm-backups/
# The artifact key is: {prefix}/{job_id}.backup

aws s3 cp "s3://my-backup-bucket/chv/vm-backups/<JOB_ID>.backup" \
  /tmp/<JOB_ID>.backup

# Or with a custom endpoint (MinIO, etc.):
aws s3 cp "s3://bucket/prefix/<JOB_ID>.backup" /tmp/<JOB_ID>.backup \
  --endpoint-url https://s3.example.com
```

### 2b. NFS Destination

```bash
# The NFS mount should already be mounted on the control plane (used by backup worker)
# If not, mount it:
sudo mount -t nfs backup-nfs.example.com:/exports/backups /mnt/backups

# Copy the artifact
cp /mnt/backups/<JOB_ID>.backup /tmp/<JOB_ID>.backup
```

### 2c. Null Destination (dev/test)

The artifact was never copied off-host. Check the local staging directory:

```bash
ls -la /run/chv/controlplane/backups/<JOB_ID>.backup
# If present, copy it:
cp /run/chv/controlplane/backups/<JOB_ID>.backup /tmp/<JOB_ID>.backup
```

> **Note:** The `/run/chv/controlplane/backups/` directory is tmpfs and may be cleared on reboot. Do not rely on it for production recovery.

---

## 3. Verify Artifact Integrity

```bash
# Compute SHA256 of the downloaded artifact
downloaded_checksum=$(sha256sum /tmp/<JOB_ID>.backup | awk '{print $1}')

# Compare with the expected checksum from the database
expected_checksum=$(sudo sqlite3 /var/lib/chv/controlplane.db \
  "SELECT checksum FROM backup_jobs WHERE job_id = '<JOB_ID>';")

if [ "$downloaded_checksum" = "$expected_checksum" ]; then
  echo "✅ Checksum matches"
else
  echo "❌ CHECKSUM MISMATCH — do not proceed with restore"
  echo "Expected: $expected_checksum"
  echo "Got:      $downloaded_checksum"
  exit 1
fi
```

---

## 4. Inspect the Artifact Format

CHV backup artifacts are Cloud Hypervisor snapshot files. Inspect before restoring:

```bash
# Check file type
file /tmp/<JOB_ID>.backup

# If it's a qcow2 image
qemu-img info /tmp/<JOB_ID>.backup

# If it's a directory (older CH versions or certain configurations)
ls -la /tmp/<JOB_ID>.backup/
```

The artifact may be:
- A single `qcow2` disk image
- A CH snapshot directory with `memory` and disk state

---

## 5. Prepare the Target VM

### 5a. Stop the VM

```bash
chvctl vm stop <VM_ID>
```

Verify it is stopped:

```bash
chvctl vm show <VM_ID> | grep status
```

### 5b. Locate the VM's Current Disk

```bash
# On the node hosting the VM
ls -la /var/lib/chv/agent/vms/<VM_ID>/
# Look for disk.qcow2 or similar
```

### 5c. Create a Safety Backup of Current State (Optional but Recommended)

```bash
sudo cp /var/lib/chv/agent/vms/<VM_ID>/disk.qcow2 \
  /var/lib/chv/agent/vms/<VM_ID>/disk-pre-restore.qcow2
```

---

## 6. Restore the Artifact

### 6a. Single Disk Image (qcow2)

```bash
# Replace the existing disk
sudo cp /tmp/<JOB_ID>.backup /var/lib/chv/agent/vms/<VM_ID>/disk.qcow2

# Ensure correct ownership
sudo chown -R chv:chv /var/lib/chv/agent/vms/<VM_ID>/
```

### 6b. Cloud Hypervisor Snapshot Directory

If the artifact is a snapshot directory:

```bash
# Remove old snapshot state (if any)
sudo rm -rf /var/lib/chv/agent/vms/<VM_ID>/snapshots/manual-restore

# Extract/place the snapshot
sudo mkdir -p /var/lib/chv/agent/vms/<VM_ID>/snapshots/manual-restore
sudo cp -a /tmp/<JOB_ID>.backup/* /var/lib/chv/agent/vms/<VM_ID>/snapshots/manual-restore/
sudo chown -R chv:chv /var/lib/chv/agent/vms/<VM_ID>/snapshots/

# Use the VM Snapshot Restore API to load it
curl -X POST https://controlplane.example.com/v1/vms/snapshots/restore \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "vm_id": "<VM_ID>",
    "snapshot_path": "/var/lib/chv/agent/vms/<VM_ID>/snapshots/manual-restore"
  }'
```

> **Note:** The `snapshot_path` field in the restore API may not be exposed in the public BFF contract. If not, use the CH API directly via `chv-agent` or restart the VM with the restored disk.

### 6c. Direct Cloud Hypervisor API (last resort)

If CHV APIs are unavailable, use the CH API socket directly:

```bash
# Find the VM's API socket
VM_SOCK=/var/lib/chv/agent/vms/<VM_ID>/vm.sock

# Create VM config first (if VM is fully down)
# Then restore:
curl -X PUT --unix-socket "$VM_SOCK" \
  http://localhost/api/v1/vm.restore \
  -H "Content-Type: application/json" \
  -d "{\"source_url\": \"file:///var/lib/chv/agent/vms/<VM_ID>/snapshots/manual-restore\"}"
```

---

## 7. Start the VM and Verify

```bash
chvctl vm start <VM_ID>
```

Monitor boot:

```bash
# Serial console
chvctl vm console <VM_ID>

# Or logs
sudo journalctl -u chv-agent -f | grep <VM_ID>
```

Verify guest integrity:

```bash
chvctl vm guest-exec <VM_ID> --command "fsck -n /dev/vda1"
```

---

## 8. Record the Restore in CHV

Since the restore was performed manually, update CHV's tracking:

```bash
# Create a restore record via API (for audit trail)
curl -X POST https://controlplane.example.com/v1/backups/restores \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "backup_job_id": "<JOB_ID>",
    "target_vm_id": "<VM_ID>",
    "notes": "Manually restored from S3 artifact. Checksum verified."
  }' | jq '.restore_id'

# Then mark it as succeeded (since the API stub doesn't execute):
sudo sqlite3 /var/lib/chv/controlplane.db \
  "UPDATE backup_restores SET status = 'Succeeded', completed_at = datetime('now') 
   WHERE backup_job_id = '<JOB_ID>' AND target_vm_id = '<VM_ID>';"
```

---

## Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| `CHECKSUM MISMATCH` after download | Network corruption or wrong artifact | Re-download; verify bucket/path |
| `qemu-img info` shows corruption | Incomplete download or CH version mismatch | Re-download; check artifact size matches `size_bytes` in DB |
| VM fails to start after restore | Disk format mismatch | Convert with `qemu-img convert -O qcow2` |
| CH API returns `BadRequest` on restore | Snapshot format incompatible with CH version | Check CH version at backup time vs now; may need intermediate conversion |
| Permission denied on disk | Wrong ownership | `sudo chown -R chv:chv /var/lib/chv/agent/vms/<VM_ID>/` |
| Guest filesystem errors | Disk image from running VM | Always stop VM before creating snapshots; `fsck` the restored disk |

## When Will This Be Automated?

A restore worker is on the roadmap. When implemented, the procedure will be:

```bash
# Future automated API (NOT YET AVAILABLE)
curl -X POST https://controlplane.example.com/v1/backups/restores \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"backup_job_id": "<JOB_ID>", "target_vm_id": "<VM_ID>"}'

# Poll for completion
curl -s https://controlplane.example.com/v1/backups/restores/<RESTORE_ID> \
  -H "Authorization: Bearer $TOKEN" | jq '{status, progress_percent, error}'
```

Until then, follow this manual runbook.

## Related Runbooks

- [VM Snapshot Restore](vm-snapshot-restore.md) — For restoring from local snapshots (automated)
- [Control Plane DR](control-plane-dr.md) — If the control plane itself is lost
