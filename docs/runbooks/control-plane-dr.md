# Runbook: Control Plane Disaster Recovery

**Severity:** SEV-1 (complete control plane outage)  
**Automation Level:** Partially automated (DB auto-backup) + manual steps  
**Estimated Time:** 15–30 minutes  
**Prerequisites:**
- Access to backup artifacts: SQLite DB backups, certificate backups, and VM backup artifacts
- A fresh or spare host meeting CHV control plane requirements
- Same CHV version (or newer) installed on the replacement host
- DNS or load balancer updated to point to the replacement host

---

## 1. Assess the Scope of Failure

### Scenario A: Control Plane Host Failed (VMs still running on hypervisors)
- VMs continue running but cannot be managed
- No API, no UI, no scheduling
- **Recovery target:** Rebuild control plane, re-enroll agents

### Scenario B: Control Plane Database Corrupted
- Control plane starts but API returns errors
- SQLite corruption or disk full
- **Recovery target:** Restore DB from latest backup

### Scenario C: Certificate Authority Compromised or Lost
- mTLS between control plane and agents breaks
- Agents cannot reconnect
- **Recovery target:** Rotate certificates, re-enroll agents

---

## 2. Scenario A: Rebuild Control Plane Host

### 2a. Prepare the Replacement Host

Install CHV control plane on the new host:

```bash
# Install from package (Debian/Ubuntu)
dpkg -i chv-controlplane_0.1.0_amd64.deb

# Or from release tarball
tar xzf chv-0.1.0-linux-amd64.tar.gz
sudo ./install.sh --component controlplane
```

### 2b. Restore the Database

CHV automatically backs up the DB before migrations:

```bash
# List available auto-backups on the old host (mount the old disk if needed)
ls -lt /var/lib/chv/backups/

# Copy the latest backup to the new host
scp old-host:/var/lib/chv/backups/controlplane-*.db.bak /tmp/

# Or use your external backup:
# scp backup-server:/backups/chv-YYYYMMDD-HHMMSS.db /tmp/

# Stop the control plane before restoring
sudo systemctl stop chv-controlplane

# Restore the database
sudo cp /tmp/controlplane-*.db.bak /var/lib/chv/controlplane.db
sudo chown chv:chv /var/lib/chv/controlplane.db
sudo chmod 640 /var/lib/chv/controlplane.db
```

### 2c. Restore Certificates

```bash
# Extract certificate backup
sudo tar xzf /backup/chv-certs-YYYYMMDD.tar.gz -C /

# Ensure correct ownership
sudo chown -R root:root /etc/chv/certs/
sudo chmod 600 /etc/chv/certs/*.key
```

### 2d. Verify Configuration

```bash
# Check the control plane config
cat /etc/chv/controlplane.toml

# Ensure listen addresses match the new host's IPs
# Ensure database path is correct
# Ensure JWT secret is set (or CHV_JWT_SECRET env var)
```

### 2e. Start Control Plane

```bash
sudo systemctl start chv-controlplane
sudo systemctl status chv-controlplane

# Verify API health
curl -s http://localhost:8080/health | jq
```

### 2f. Re-enroll Agents

Agents will attempt to reconnect but may fail if the control plane's gRPC listener address changed.

```bash
# On each hypervisor host:
sudo systemctl restart chv-agent

# Check agent logs for connection success
sudo journalctl -u chv-agent -n 100 | grep -i "connected\|certificate\|error"
```

If agents cannot connect due to certificate issues:

```bash
# Generate new bootstrap token on control plane
TOKEN=$(openssl rand -hex 32)
echo "$TOKEN" | sudo tee /etc/chv/bootstrap.token.new
sudo chmod 600 /etc/chv/bootstrap.token.new

# On agent host, re-enroll
sudo chvctl agent enroll --token "$TOKEN" --server https://new-cp.example.com:443
```

### 2g. Verify VM States

```bash
# List all VMs — they should show their last known state
chvctl vm list

# VMs that were Running should still be running on their hypervisors
# The control plane will reconcile state on next agent heartbeat
```

---

## 3. Scenario B: Database Corruption Recovery

### 3a. Stop Control Plane

```bash
sudo systemctl stop chv-controlplane
```

### 3b. Attempt SQLite Repair

```bash
# Backup the corrupted DB first
sudo cp /var/lib/chv/controlplane.db /var/lib/chv/controlplane.db.corrupt.$(date +%s)

# Try SQLite's built-in repair
sqlite3 /var/lib/chv/controlplane.db ".recover" | sqlite3 /var/lib/chv/controlplane.db.recovered

# If recovered file looks valid, replace
sudo mv /var/lib/chv/controlplane.db.recovered /var/lib/chv/controlplane.db
sudo chown chv:chv /var/lib/chv/controlplane.db
```

### 3c. If Repair Fails, Restore from Backup

Follow steps 2b (restore DB) and 2e (start control plane) from Scenario A.

---

## 4. Scenario C: Certificate Rotation

### 4a. Generate New CA and Certificates

```bash
# Use CHV's built-in cert generation (if available)
# Or manually with openssl

# Generate new CA
sudo openssl req -x509 -newkey rsa:4096 -keyout /etc/chv/certs/ca.key \
  -out /etc/chv/certs/ca.crt -days 3650 -nodes \
  -subj "/CN=chv-ca/O=CloudHypervisor"

# Generate new control plane cert
sudo openssl req -newkey rsa:4096 -keyout /etc/chv/certs/controlplane.key \
  -out /etc/chv/certs/controlplane.csr -nodes \
  -subj "/CN=new-cp.example.com/O=CloudHypervisor"
sudo openssl x509 -req -in /etc/chv/certs/controlplane.csr \
  -CA /etc/chv/certs/ca.crt -CAkey /etc/chv/certs/ca.key \
  -CAcreateserial -out /etc/chv/certs/controlplane.crt -days 365

# Generate new agent cert (per agent host)
# Agents will re-generate their own certs on re-enrollment
```

### 4b. Distribute New CA

```bash
# Copy new CA to all agent hosts
for host in agent-1 agent-2 agent-3; do
  scp /etc/chv/certs/ca.crt "$host:/tmp/chv-ca.crt"
  ssh "$host" "sudo cp /tmp/chv-ca.crt /etc/chv/certs/ca.crt && sudo systemctl restart chv-agent"
done
```

### 4c. Re-enroll All Agents

Follow step 2f from Scenario A for each agent.

---

## 5. Post-Recovery Validation Checklist

```bash
# 1. API health
curl -s http://localhost:8080/health | jq

# 2. All agents connected
chvctl node list | jq '.nodes[] | {node_id, status, last_seen_at}'

# 3. All VMs visible
chvctl vm list | wc -l

# 4. Backup schedules still configured
chvctl backup list

# 5. Test a non-destructive operation
chvctl vm show <TEST_VM_ID>

# 6. Verify backup worker is running
sudo systemctl status chv-controlplane | grep -i backup

# 7. Check for any failed operations after recovery
curl -s "http://localhost:8080/v1/operations?status=Failed" \
  -H "Authorization: Bearer $TOKEN" | jq '.operations | length'
```

---

## Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| Control plane won't start after DB restore | Migration version mismatch | `rm /var/lib/chv/controlplane.db` and re-initialize (data loss) or apply pending migrations manually |
| Agents show `certificate verify failed` | CA mismatch | Re-distribute CA cert and restart agents |
| VMs show `Unknown` state | Agent hasn't heartbeated yet | Wait 30s; check agent connectivity |
| Backup schedules missing | Restored from old DB | Recreate schedules via API or UI |
| `chvctl` returns `connection refused` | Control plane not listening | Check `controlplane.toml` bind addresses; check firewall |
| SQLite `database is locked` | Another process holds the lock | `fuser /var/lib/chv/controlplane.db`; kill stale processes |

## What Happens Under the Hood

1. CHV control plane is stateful — the SQLite DB is the source of truth for all metadata
2. Agents maintain local state (VM runtimes) and reconcile with the control plane on heartbeat
3. Certificates are pinned — rotating the CA requires re-enrollment of all agents
4. The backup worker is an in-process task — it resumes automatically when the control plane starts

## Related Runbooks

- [Full Site Recovery](full-site-recovery.md) — When hypervisors are also lost
- [Backup Artifact Restore](backup-artifact-restore.md) — For recovering individual VMs from shipped backups
