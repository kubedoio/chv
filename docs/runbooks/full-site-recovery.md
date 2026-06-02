# Runbook: Full Site Recovery

**Severity:** SEV-1 (complete infrastructure loss)  
**Automation Level:** Manual  
**Estimated Time:** 1–4 hours (depending on VM count and backup destination)  
**Prerequisites:**
- Access to off-site backup artifacts (S3 or NFS)
- Replacement hardware or cloud instances for all CHV roles (control plane + hypervisors)
- CHV installation media/packages
- Network configuration (IPs, DNS, VLANs) documented or recoverable
- Database backup from the control plane
- Certificate backup (or willingness to rotate all certs)

---

## 1. Recovery Order

CHV recovery follows a strict startup order. Do not skip steps.

| Order | Component | Why It Must Come First |
|-------|-----------|------------------------|
| 1 | Control plane host | Source of truth for all metadata |
| 2 | Network/storage infrastructure | Hypervisors need connectivity to CP and storage |
| 3 | Hypervisor hosts (agents) | Host the VMs |
| 4 | Storage nodes (stord) | Volume backends |
| 5 | VMs (from backup artifacts) | Final workload restoration |

---

## 2. Rebuild the Control Plane

Follow [Control Plane DR](control-plane-dr.md) — Scenario A (host rebuild) in full.

Key additional steps for a full site recovery:

### 2a. Verify Backup Artifacts Are Accessible

Before rebuilding hypervisors, confirm you can reach the backup destination:

```bash
# S3
aws s3 ls s3://my-backup-bucket/chv/ --recursive | head -20

# NFS
showmount -e backup-nfs.example.com
```

### 2b. Document the Original Topology

If the old control plane is completely destroyed and you don't have the DB backup, you must reconstruct the topology from memory or external documentation. Minimum required:

- List of all VM IDs and their hypervisor assignments
- Volume IDs and storage backend configuration
- Network/VLAN assignments
- Backup schedule configurations

> **Prevention:** Export this data regularly:
> ```bash
> chvctl vm list > /backup/topology-vms-$(date +%Y%m%d).json
> chvctl volume list > /backup/topology-volumes-$(date +%Y%m%d).json
> chvctl node list > /backup/topology-nodes-$(date +%Y%m%d).json
> ```

---

## 3. Rebuild Hypervisor Hosts

### 3a. Install CHV Agent

```bash
# On each hypervisor host
dpkg -i chv-agent_0.1.0_amd64.deb

# Or from tarball
tar xzf chv-0.1.0-linux-amd64.tar.gz
sudo ./install.sh --component agent
```

### 3b. Configure Agent

```bash
cat > /etc/chv/agent.toml <<'EOF'
[agent]
node_id = "<ORIGINAL_NODE_ID_OR_NEW>"
control_plane_url = "https://new-cp.example.com:443"
runtime_dir = "/var/lib/chv/agent"

[agent.tls]
ca_cert = "/etc/chv/certs/ca.crt"
client_cert = "/etc/chv/certs/agent.crt"
client_key = "/etc/chv/certs/agent.key"
EOF
```

### 3c. Enroll with Control Plane

```bash
# Generate bootstrap token on control plane
TOKEN=$(openssl rand -hex 32)
echo "$TOKEN" | sudo tee /etc/chv/bootstrap.token.new

# Enroll agent
sudo chvctl agent enroll --token "$TOKEN" --server https://new-cp.example.com:443

# Start agent
sudo systemctl enable --now chv-agent
```

### 3d. Verify Agent Registration

```bash
# On control plane
chvctl node list | jq '.nodes[] | {node_id, status}'
```

---

## 4. Rebuild Storage Nodes (stord)

### 4a. Install stord

```bash
dpkg -i chv-stord_0.1.0_amd64.deb
```

### 4b. Configure Storage Backend

**Local backend:**

```bash
cat > /etc/chv/stord.toml <<'EOF'
[stord]
backend = "local"
data_dir = "/var/lib/chv/stord"
EOF
```

**Ceph backend:**

```bash
cat > /etc/chv/stord.toml <<'EOF'
[stord]
backend = "ceph"
ceph_pool = "chv-volumes"
ceph_user = "chv"
EOF
```

### 4c. Start stord

```bash
sudo systemctl enable --now chv-stord
```

---

## 5. Restore VMs from Backup Artifacts

For each VM that needs recovery:

### 5a. Re-create VM Metadata (if DB was lost)

If you restored the control plane DB, VM metadata should already exist. Skip to 5b.

If the DB was lost, recreate VMs:

```bash
# Create VM definition
curl -X POST http://localhost:8080/v1/vms \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "<VM_NAME>",
    "vcpu_count": 4,
    "memory_mb": 8192,
    "node_id": "<HYPERVISOR_NODE_ID>"
  }' | jq '.vm_id'
```

Attach volumes:

```bash
curl -X POST http://localhost:8080/v1/vms/<VM_ID>/volumes \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"volume_id": "<VOLUME_ID>"}'
```

### 5b. Restore Disk from Backup Artifact

Follow [Backup Artifact Restore](backup-artifact-restore.md) for each VM.

Bulk restore script (if many VMs):

```bash
#!/bin/bash
# restore-all-vms.sh — run on control plane

API_TOKEN="$TOKEN"
BACKEND="s3"
BUCKET="my-backup-bucket"
PREFIX="chv/vm-backups"

# Get all successful backup jobs
jobs=$(curl -s "http://localhost:8080/v1/backups/jobs?status=Succeeded" \
  -H "Authorization: Bearer $API_TOKEN" | jq -r '.jobs[] | [.job_id, .vm_id, .destination] | @tsv')

while IFS=$'\t' read -r job_id vm_id destination; do
  echo "Restoring VM $vm_id from job $job_id"
  
  # Download artifact
  aws s3 cp "s3://$BUCKET/$PREFIX/$job_id.backup" /tmp/$job_id.backup
  
  # Verify checksum
  expected=$(sqlite3 /var/lib/chv/controlplane.db \
    "SELECT checksum FROM backup_jobs WHERE job_id = '$job_id';")
  actual=$(sha256sum /tmp/$job_id.backup | awk '{print $1}')
  
  if [ "$expected" != "$actual" ]; then
    echo "  ❌ CHECKSUM MISMATCH for $job_id — skipping"
    continue
  fi
  
  # Stop VM
  chvctl vm stop "$vm_id" 2>/dev/null || true
  sleep 5
  
  # Copy disk to hypervisor (requires SSH access)
  node_ip=$(chvctl node get $(chvctl vm get "$vm_id" | jq -r '.node_id') | jq -r '.management_ip')
  scp /tmp/$job_id.backup "root@$node_ip:/var/lib/chv/agent/vms/$vm_id/disk.qcow2"
  
  # Fix ownership
  ssh "root@$node_ip" "chown -R chv:chv /var/lib/chv/agent/vms/$vm_id/"
  
  # Start VM
  chvctl vm start "$vm_id"
  
  echo "  ✅ Restored VM $vm_id"
done <<< "$jobs"
```

---

## 6. Restore Volumes

### 6a. Local Volumes

Local volumes were stored on stord hosts. If the stord host was lost, the volumes are lost unless backed up externally.

### 6b. Ceph Volumes

Ceph volumes survive stord host loss (data is in the Ceph cluster). Simply:

1. Reinstall stord
2. Connect to the same Ceph pool
3. Volumes are automatically available

### 6c. Recreate Lost Volumes

If volumes were lost:

```bash
# Create new volume
curl -X POST http://localhost:8080/v1/volumes \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "<VOLUME_NAME>",
    "size_gb": 100,
    "storage_backend": "local",
    "node_id": "<STOR_NODE_ID>"
  }' | jq '.volume_id'
```

If you have a volume backup artifact, restore it manually to the stord data directory.

---

## 7. Reconfigure Backup Schedules

If the DB was restored, schedules should persist. Verify:

```bash
chvctl backup list
```

If schedules are missing, recreate them:

```bash
curl -X POST http://localhost:8080/v1/backups/schedules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "vm_id": "<VM_ID>",
    "cron_expression": "0 2 * * *",
    "retention_count": 7,
    "retention_days": 30,
    "destination": "s3://my-backup-bucket/chv/",
    "s3_access_key": "<ACCESS_KEY>",
    "s3_secret_key": "<SECRET_KEY>"
  }'
```

> **Note:** S3 credentials are encrypted at rest in the database using AES-256-GCM.

---

## 8. Post-Recovery Validation

### 8a. Infrastructure Health

```bash
# All services running
sudo systemctl is-active chv-controlplane chv-agent chv-stord chv-nwd

# All nodes registered
chvctl node list | jq '.nodes | length'

# All VMs running or stopped as expected
chvctl vm list | jq '.vms[] | {vm_id, name, status, node_id}'
```

### 8b. VM Health Checks

Verify each recovered VM is running and reachable:

```bash
# For each critical VM
for vm in vm-1 vm-2 vm-3; do
  echo "Checking $vm..."
  chvctl vm get "$vm" | jq '{vm_id, status, node_id}'
  # Or verify via SSH:
  # ssh user@$vm_ip "uptime"
done
```

### 8c. Backup Worker Verification

```bash
# Trigger a test backup
curl -X POST http://localhost:8080/v1/backups/run \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"vm_id": "<TEST_VM_ID>", "label": "post-dr-test"}' | jq '.job_id'

# Wait and verify it succeeds
curl -s "http://localhost:8080/v1/backups/jobs/<JOB_ID>" \
  -H "Authorization: Bearer $TOKEN" | jq '{status, completed_at, checksum}'
```

### 8d. Run a Full Backup Cycle

```bash
# Force all schedules to run (or wait for next cron tick)
# Verify artifacts appear in S3/NFS
aws s3 ls s3://my-backup-bucket/chv/ --recursive | grep $(date +%Y%m%d)
```

---

## 9. Post-Incident Actions

1. **Document the root cause** in your incident tracker
2. **Update network diagrams** if they changed during rebuild
3. **Verify off-site backups are still running** — the DR event may have disrupted schedules
4. **Run a backup restore drill** within 48 hours to ensure the recovered system can actually restore
5. **Update this runbook** if any steps didn't work as expected
6. **Consider implementing automated restore** — the manual steps in this runbook are a significant operational risk

---

## Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| Agent enrollment fails with `invalid token` | Token expired or wrong CA | Generate fresh token; verify CA cert matches |
| VMs won't start after restore | Disk images incompatible with new CH version | Convert with `qemu-img convert`; check CH release notes |
| Ceph volumes not visible | Wrong pool or user | Verify `stord.toml` matches old configuration |
| Backup worker not creating jobs | Schedules not enabled | `sqlite3 /var/lib/chv/controlplane.db "SELECT schedule_id, enabled FROM backup_schedules;"` |
| S3 upload fails after recovery | Credentials rotated or bucket policy changed | Update schedule with new S3 credentials |
| High VM boot time after restore | Disk images need `virtio` drivers | Ensure VM config matches original (same disk bus) |

## Related Runbooks

- [Control Plane DR](control-plane-dr.md) — For control-plane-only failures
- [Backup Artifact Restore](backup-artifact-restore.md) — For single-VM recovery from shipped backups
- [VM Snapshot Restore](vm-snapshot-restore.md) — For restoring from local snapshots
