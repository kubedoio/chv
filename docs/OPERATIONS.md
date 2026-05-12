# CHV Operations Guide

Day-2 operations for CHV deployments: monitoring, troubleshooting, backups, and scaling.

---

## Monitoring

### Health Endpoints

| Endpoint | Purpose | Expected |
|----------|---------|----------|
| `curl http://127.0.0.1:8080/health` | Control plane liveness | `200 OK` |
| `curl http://127.0.0.1:8080/ready` | Control plane readiness | `200 OK` after migrations |
| `curl http://127.0.0.1:9901/metrics` | Agent Prometheus metrics | Prometheus text format |

### Key Prometheus Metrics

```
chv_vms_total{status="running"}
chv_nodes_ready
chv_operations_completed_total{status="succeeded"}
chv_operations_latency_seconds_bucket
```

### systemd Service Status

```bash
# All services
systemctl status chv-controlplane chv-agent chv-stord chv-nwd nginx

# Watch logs
journalctl -u chv-controlplane -f
journalctl -u chv-agent -f
journalctl -u chv-stord -f
journalctl -u chv-nwd -f
```

---

## Backup and Restore

### SQLite Database Backup

The database lives at `/var/lib/chv/controlplane.db`. Back up before upgrades or migrations:

```bash
# Online backup (SQLite backup API)
sqlite3 /var/lib/chv/controlplane.db ".backup '/backup/chv-$(date +%Y%m%d-%H%M%S).db'"

# Automated pre-migration backup (built into chv-controlplane)
# The control plane automatically backs up the DB before running migrations,
# keeping the last 10 backups in /var/lib/chv/backups/.
```

### Restore from Backup

```bash
sudo systemctl stop chv-controlplane chv-agent
sudo cp /backup/chv-YYYYMMDD-HHMMSS.db /var/lib/chv/controlplane.db
sudo chown chv:chv /var/lib/chv/controlplane.db
sudo systemctl start chv-controlplane
# Re-enroll the local agent if certificates were rotated
```

### Certificate Backup

```bash
sudo tar czf /backup/chv-certs-$(date +%Y%m%d).tar.gz /etc/chv/certs/
```

---

## Scaling: Multi-Node

### Add a Hypervisor-Only Host

1. **On the control plane host**, create a bootstrap token:
   ```bash
   TOKEN=$(openssl rand -hex 32)
   echo "$TOKEN" | sudo tee /etc/chv/bootstrap.token.new
   # Insert into DB (one-time use)
   ```

2. **On the new hypervisor host**, install binaries and Cloud Hypervisor:
   ```bash
   sudo apt install -y qemu-kvm bridge-utils iproute2 iptables
   # Copy chv-agent, chv-stord, chv-nwd from the control plane host
   ```

3. Configure `/etc/chv/agent.toml`:
   ```toml
   control_plane_addr = "https://<CONTROL_PLANE_IP>:8443"
   # ... other settings same as all-in-one deploy
   ```

4. Start services:
   ```bash
   sudo systemctl enable --now chv-stord chv-nwd chv-agent
   ```

5. **Verify enrollment** in the Web UI or via API:
   ```bash
   curl -s http://127.0.0.1:8080/v1/nodes | jq '.items[].name'
   ```

---

## Troubleshooting Quick Reference

### Agent Fails to Enroll
| Check | Command |
|-------|---------|
| Bootstrap token exists | `sudo cat /etc/chv/bootstrap.token` |
| Token not expired | `sqlite3 /var/lib/chv/controlplane.db "SELECT expires_at FROM bootstrap_tokens;"` |
| Control plane listening | `ss -tlnp | grep 8443` |
| Agent can reach control plane | `curl -k https://127.0.0.1:8443/health` |
| Agent logs | `journalctl -u chv-agent -n 100 --no-pager` |

### VM Won't Start
| Check | Command |
|-------|---------|
| KVM available | `ls /dev/kvm && groups chv` |
| Cloud Hypervisor binary | `cloud-hypervisor --version` |
| Storage pool exists | `ls /var/lib/chv/storage/localdisk/` |
| Volume prepared by stord | `journalctl -u chv-stord -n 50` |
| Network bridge up | `ip addr show chvbr0` |

### Web UI Blank or API Errors
| Symptom | Fix |
|---------|-----|
| Blank page | Verify `/opt/chv/ui/index.html` exists; check `nginx -T \| grep root` |
| JSON parse error | Ensure nginx `proxy_pass` has NO trailing slash after the port |
| 502 Bad Gateway | Verify control plane is running: `systemctl status chv-controlplane` |
| Console disconnected | Check WebSocket proxy config; verify agent PTY process is running |

### chv-stord or chv-nwd Keep Restarting
| Check | Command |
|-------|---------|
| Binary permissions | `ls -la /usr/local/bin/chv-stord /usr/local/bin/chv-nwd` |
| Socket directory | `ls -la /run/chv/stord /run/chv/nwd` |
| Config syntax | `cat /etc/chv/stord.toml` / `cat /etc/chv/nwd.toml` |
| Daemon logs | `journalctl -u chv-stord -f` / `journalctl -u chv-nwd -f` |

### Upgrade Failures

When an upgrade fails, the `UpgradeOrchestrator` records the failure state with a reason.

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| State shows `Failed` | `chvctl upgrade status <NODE_ID>` | Check reason field; fix the issue and retry or rollback |
| Pre-check rejected | Check logs: `journalctl -u chv-controlplane \| grep "pre-check"` | Fix the blocking condition (incompatible version, active migrations, unhealthy node) |
| Health check timeout | Node didn't reach `TenantReady` within 120s | Check agent logs: `journalctl -u chv-agent -n 100`; verify binary was installed correctly |
| Drain timeout | VMs didn't evacuate within 300s | Check migration status; ensure target nodes have capacity |
| Rollback triggered | Automatic on health-check failure | Verify node is back to previous version: `chv-agent --version` |

**Recovery steps:**
```bash
# Check upgrade state
chvctl upgrade status <NODE_ID>

# Manual rollback if automated rollback failed
chvctl upgrade rollback <NODE_ID>

# Force node back to healthy state
curl -X POST http://127.0.0.1:8080/v1/nodes/mutate \
  -d '{"node_id": "<NODE_ID>", "action": "exit_maintenance"}'
```

### Migration Failures

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| `FAILED_PRECONDITION: mTLS is required` | TLS not configured on sender | Set `migration.tls.cert_path`, `migration.tls.key_path`, `migration.tls.ca_path` in `/etc/chv/stord.toml` |
| `failed to connect to peer with mTLS` | Certificate mismatch or network issue | Verify CA cert matches on both nodes; check firewall rules |
| `CRC mismatch reported by receiver` | Data corruption in transit | Check network for packet corruption; retry migration |
| `deadline_exceeded waiting for Ack` | Receiver not acknowledging | Check receiver node health; disk I/O saturation on destination |
| Migration stuck (reaper will clean up) | `chv_migration_phase` gauge stuck | Wait for reaper (2h timeout) or manually fail: update `migrations` table |
| Backpressure throttling | `slow_down_factor` in logs | Receiver is overwhelmed; reduce concurrent migrations or add I/O capacity |

**Check active migrations:**
```bash
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT migration_id, vm_id, phase, started_at FROM migrations \
   WHERE phase NOT IN ('Completed', 'Failed', 'RolledBack');"
```

**Verify mTLS configuration:**
```bash
# Check cert files exist and are readable
ls -la /etc/chv/certs/node.pem /etc/chv/certs/node-key.pem /etc/chv/certs/ca.pem

# Test TLS handshake to a peer
openssl s_client -connect <PEER_IP>:8444 \
  -cert /etc/chv/certs/node.pem \
  -key /etc/chv/certs/node-key.pem \
  -CAfile /etc/chv/certs/ca.pem
```

---

## Maintenance Windows

### Graceful Node Drain

Draining a node evacuates all VMs via live migration before allowing maintenance.

**Via CLI:**
```bash
chvctl node drain <NODE_ID>
```

**Via API:**
```bash
curl -X POST http://127.0.0.1:8080/v1/nodes/mutate \
  -H "Content-Type: application/json" \
  -d '{"node_id": "<NODE_ID>", "action": "drain"}'
```

**What happens:**
1. Node transitions to `Draining` — scheduling is paused immediately
2. Agent reconcile loop issues migration requests for each running VM
3. VMs are live-migrated to other `TenantReady` nodes
4. When all VMs are evacuated, node transitions to `Maintenance` automatically
5. Perform maintenance (kernel update, hardware swap, etc.)
6. Restart agent: `sudo systemctl start chv-agent`
7. Mark node ready: set desired state to `TenantReady` via API or Web UI

**Monitor drain progress:**
```bash
# Check remaining VMs
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT count(*) FROM vms v JOIN vm_observed_state o ON v.vm_id = o.vm_id \
   WHERE v.node_id = '<NODE_ID>' AND o.runtime_status NOT IN ('Stopped', 'Deleted');"

# Watch via metrics
watch -n5 'curl -s http://127.0.0.1:9901/metrics | grep chv_node_vm_count'
```

### Rolling Upgrade

**Via CLI:**
```bash
# Start upgrade on a specific node
chvctl upgrade start <NODE_ID> --version 0.5.0

# Check upgrade status
chvctl upgrade status <NODE_ID>

# List all upgrades
chvctl upgrade list

# Rollback if upgrade failed
chvctl upgrade rollback <NODE_ID>
```

**Via API:**
```bash
# Initiate upgrade
curl -X POST http://127.0.0.1:8080/v1/upgrades \
  -H "Content-Type: application/json" \
  -d '{"node_id": "<NODE_ID>", "version": "0.5.0"}'

# Check status
curl http://127.0.0.1:8080/v1/upgrades/<NODE_ID>
```

**Upgrade flow (automated per node):**
1. Pre-checks: version compatibility, disk space, no active migrations, node health
2. Drain node (evacuate VMs)
3. Record upgrade intent in `node_desired_state`
4. Agent performs binary swap + systemd restart
5. Control plane polls for health (up to 120s timeout)
6. If healthy → un-drain and proceed to next node
7. If unhealthy → automatic rollback to previous version

### Compatibility Matrix

The compatibility matrix defines allowed version ranges per component. Located at `/etc/chv/compat-matrix.toml`:

```toml
[compatibility]
[[compatibility.entry]]
component = "agent"
min_version = "0.1.0"
max_version = "1.0.0"

[[compatibility.entry]]
component = "stord"
min_version = "0.2.0"
max_version = "1.0.0"
```

**Check compatibility via API:**
```bash
chvctl health cluster
```

The control plane validates the matrix at upgrade time. If the target version falls outside the allowed range, the upgrade is rejected before any drain begins.

### Upgrade Procedure (Manual)

1. Back up database and certificates
2. Build or download new release tarball
3. Stop services in order: agent → stord/nwd → control plane
4. Install new binaries
5. Start control plane (runs migrations automatically)
6. Start stord and nwd
7. Start agent
8. Verify: `systemctl status` and `curl /health`

---

## Multi-Node Operations

### Migration Monitoring

Key Prometheus metrics to watch during live migration:

| Metric | Type | What it tells you |
|--------|------|-------------------|
| `chv_migration_phase` | Gauge | Current phase (0=Pending, 1=PreCopyDisk, 2=ConvergingDisk, 3=MemoryMigration, 4=Paused, 5=Completed, 6=Failed, 7=RolledBack) |
| `chv_migration_bytes_transferred` | Counter | Total bytes copied so far |
| `chv_migration_duration_seconds` | Histogram | End-to-end migration time by outcome |
| `chv_migration_dirty_blocks` | Gauge | Remaining dirty blocks during convergence |

**Expected phase durations** (100GB disk, 16GB RAM):

| Phase | Typical | Alarm threshold |
|-------|---------|-----------------|
| PreCopyDisk | 10-60 min | >90 min |
| ConvergingDisk | 1-10 min per round | >30 min total |
| MemoryMigration | 30s-5 min | >10 min |
| Paused (final sync) | <5s | >30s |

**When to intervene:**

- `dirty_blocks` not converging after 5 rounds: check I/O write rate on source VM
- Phase stuck in MemoryMigration >10 min: possible network partition between nodes
- Migration rolled back repeatedly: check source/destination connectivity and disk I/O saturation

```bash
# Check active migrations
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT migration_id, vm_id, phase, bytes_transferred, dirty_blocks_remaining \
   FROM migrations WHERE completed_at IS NULL;"

# Watch migration progress
watch -n5 'curl -s http://127.0.0.1:9901/metrics | grep chv_migration'
```

### VXLAN Overlay Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| Stale FDB entries | `bridge fdb show dev vxlan<VNI>` | Trigger reconcile: restart chv-agent on affected node |
| VXLAN interface down | `ip link show \| grep vxlan` | Check `chv-nwd` logs; verify VTEP registration in DB |
| VNI exhaustion | `sqlite3 /var/lib/chv/controlplane.db "SELECT count(*) FROM vni_allocations WHERE released_at IS NULL;"` | Release unused VNIs or expand VNI range |
| Cross-node VM unreachable | `tcpdump -i <vtep_interface> udp port 4789` | Verify UDP/4789 not blocked by firewall between nodes |
| MTU issues / fragmentation | `ping -M do -s 1400 <remote_vm_ip>` | Check `[overlay] inner_mtu` in nwd.toml; ensure outer MTU >= inner + 50 |

```bash
# Verify VTEP registry
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT node_id, vtep_ip, vtep_port FROM vtep_entries;"

# Check VNI allocation for a network
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT network_id, vni, allocated_at FROM vni_allocations WHERE network_id = '<ID>';"

# Force overlay reconciliation on a node
systemctl restart chv-nwd
```

### eBPF Policy Troubleshooting

| Symptom | Diagnostic | Resolution |
|---------|-----------|------------|
| Program load failure | `journalctl -u chv-nwd \| grep "eBPF"` | Verify `/usr/lib/chv/ebpf/policy_tc.o` exists; check kernel version >= 5.10 |
| Rules not applied | `tc filter show dev <tap> egress` | Check that clsact qdisc is attached; verify rule_map entries |
| Stats all zeros | `journalctl -u chv-nwd \| grep "stats_map"` | eBPF programs may be in stub mode (libbpf-rs not available) |
| VM traffic blocked unexpectedly | Check `chv_ebpf_packets_total{action="denied"}` metric | Review security rules via API; check default_action in `[ebpf]` config |
| Rate limiting too aggressive | Check `chv_ebpf_bytes_total` vs configured rate | Adjust rate_bps in the VM's rate limit policy |

```bash
# Check if eBPF programs are loaded on a tap interface
tc filter show dev tap-<vm_short_id> egress

# View eBPF stats (via metrics endpoint)
curl -s http://127.0.0.1:9901/metrics | grep chv_ebpf

# Verify eBPF object files exist
ls -la /usr/lib/chv/ebpf/policy_tc.o
```

### Backup & Recovery for Multi-Node State

The SQLite backup (see above) automatically includes all multi-node state:

- **VTEP registry**: `vtep_entries` table (node-to-VTEP-IP mapping)
- **VNI allocations**: `vni_allocations` table (network-to-VNI mapping with cooldown)
- **Migration records**: `migrations` table (active and completed migrations)
- **FDB state**: Reconstructed at recovery time from VTEP registry + VM placement

**Recovery procedure for overlay state:**

1. Restore SQLite backup (standard procedure above)
2. Restart control plane: `sudo systemctl restart chv-controlplane`
3. Restart agents on all nodes: `sudo systemctl restart chv-agent`
4. Agents will re-register VTEPs and control plane will reconcile overlay state
5. Verify: `curl -s http://127.0.0.1:8080/v1/nodes | jq '.[].vtep_ip'`

**If overlay state is corrupt but VMs are running:**

```bash
# VMs continue running with stale FDB — connectivity may be intermittent
# Force full overlay rebuild:
sqlite3 /var/lib/chv/controlplane.db "DELETE FROM vtep_entries;"
# Then restart all agents to re-register
```

---

## Security Hardening

- Replace self-signed CA with organization PKI
- Rotate bootstrap tokens after each use
- Restrict `/etc/chv/certs/` to `root:chv` with `640` permissions
- Run `chv-stord` under a dedicated service account with device/path allowlists
- Enable firewall rules limiting gRPC port 8443 to known hypervisor IPs
