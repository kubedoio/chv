# Runbook: VM Snapshot Restore

**Severity:** SEV-2 (data unavailability) / SEV-3 (planned rollback)  
**Automation Level:** Fully automated via API and web UI  
**Estimated Time:** 2–5 minutes  
**Prerequisites:**
- Target VM exists and is registered in CHV
- At least one snapshot exists for the VM
- Target VM must be **stopped** before restore (enforced by API)

---

## 1. Verify VM State

The restore API rejects requests for VMs in `Running`, `Starting`, or `Resuming` states.

### Via chvctl

```bash
chvctl vm show <VM_ID>
```

Look for `status: Stopped`. If the VM is running, stop it first:

```bash
chvctl vm stop <VM_ID>
```

### Via API

```bash
curl -s https://controlplane.example.com/v1/vms/<VM_ID> \
  -H "Authorization: Bearer $TOKEN" | jq '.status'
```

---

## 2. List Available Snapshots

### Via chvctl

```bash
chvctl vm snapshot list <VM_ID>
```

### Via API

```bash
curl -s https://controlplane.example.com/v1/vms/<VM_ID>/snapshots \
  -H "Authorization: Bearer $TOKEN" | jq '.snapshots[] | {id, created_at, name}'
```

Note the `snapshot_id` you want to restore.

---

## 3. Execute Restore

### Via Web UI

1. Navigate to **Inventory → VMs → `<VM_NAME>`**
2. Click the **Snapshots** tab
3. Find the target snapshot and click **Restore**
4. Confirm the warning: the current VM state will be replaced

### Via chvctl

> **Note:** `chvctl backup restore` is not yet implemented. Use the API directly:

```bash
curl -X POST https://controlplane.example.com/v1/vms/snapshots/restore \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"vm_id": "<VM_ID>", "snapshot_id": "<SNAPSHOT_ID>"}'
```

### Via API

```bash
curl -X POST https://controlplane.example.com/v1/vms/snapshots/restore \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "vm_id": "<VM_ID>",
    "snapshot_id": "<SNAPSHOT_ID>"
  }' | jq
```

---

## 4. Monitor Restore Progress

Restore is dispatched as a `RestoreSnapshot` operation. Monitor via:

```bash
# Poll operation status
curl -s "https://controlplane.example.com/v1/operations?vm_id=<VM_ID>" \
  -H "Authorization: Bearer $TOKEN" | jq '.operations[] | select(.operation_type == "RestoreSnapshot") | {operation_id, status, completed_at, error}'
```

Or check the agent logs:

```bash
sudo journalctl -u chv-agent -f | grep -i restore
```

---

## 5. Verify Restore

### Start the VM

```bash
chvctl vm start <VM_ID>
```

### Verify guest health

```bash
# If guest agent is enabled
chvctl vm guest-exec <VM_ID> --command "uname -a"
```

Or connect via the configured console/SSH and verify data integrity.

---

## 6. Rollback of a Restore (if needed)

If the restored snapshot is corrupt or incorrect, you can restore a different snapshot. Each restore operation overwrites the VM's current disk state with the snapshot contents. There is no automatic "undo" — ensure you have multiple snapshots before performing a restore on a production VM.

**Best practice:** Create a new snapshot immediately before any restore:

```bash
curl -X POST https://controlplane.example.com/v1/vms/snapshots \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"vm_id": "<VM_ID>", "name": "pre-restore-safety"}'
```

---

## Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| `VM must be stopped before restore` | VM is running | Stop the VM first (`chvctl vm stop`) |
| `RestoreSnapshot operation failed` | Check agent logs | `journalctl -u chv-agent -n 200` — look for CH API errors |
| `Snapshot not found` | Snapshot was deleted by retention | Check `backup_jobs` table for shipped artifact; follow [Backup Artifact Restore](backup-artifact-restore.md) |
| VM won't start after restore | CH restore may have left partial state | Stop VM, delete `disk.qcow2`, restore from backup artifact manually |

## What Happens Under the Hood

1. BFF validates VM is stopped
2. BFF dispatches `RestoreSnapshot` to the node agent hosting the VM
3. Agent calls Cloud Hypervisor: `PUT /api/v1/vm.restore` with `source_url=file:///var/lib/chv/agent/vms/{vm_id}/snapshots/{snapshot_id}`
4. CH loads the snapshot state and disk image
5. BFF marks operation `Succeeded` or `Failed`

## Related Runbooks

- [Backup Artifact Restore](backup-artifact-restore.md) — When the local snapshot has been pruned
- [Volume Snapshot Restore](volume-snapshot-restore.md) — For volume-level (not VM-level) recovery
