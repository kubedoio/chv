# CHV Operations Guide

Day-2 operations for CHV deployments: monitoring, troubleshooting, backups, and scaling.

---

## Monitoring

### Health Endpoints

| Endpoint | Purpose | Expected |
|----------|---------|----------|
| `curl http://127.0.0.1:8080/health` | Control plane liveness | `200 OK` |
| `curl http://127.0.0.1:8080/ready` | Control plane readiness | `200 OK` after migrations |
| `curl http://127.0.0.1:8080/health/deep` | Component-level deep health | `200 OK` (degraded) or `503` (unhealthy) |
| `curl http://127.0.0.1:9901/metrics` | Agent Prometheus metrics | Prometheus text format |

#### Deep Health Check

The `/health/deep` endpoint reports per-component health:

```bash
curl -s http://127.0.0.1:8080/health/deep | jq .
```

Example response (`healthy`):
```json
{
  "status": "healthy",
  "components": {
    "database": { "status": "healthy", "latency_ms": 2 },
    "agent_socket_dir": { "status": "healthy" },
    "agent_connectivity": { "status": "healthy" }
  }
}
```

- **`database`**: SQLite connectivity with latency measurement.
- **`agent_socket_dir`**: Agent runtime directory exists and is readable.
- **`agent_connectivity`**: Can establish a Unix socket connection to at least one agent.

Status semantics:
- `healthy` — all components pass (HTTP 200)
- `degraded` — database passes but agent issues (HTTP 200, still serving)
- `unhealthy` — database fails (HTTP 503)

### Key Prometheus Metrics

```
chv_vms_total{status="running"}
chv_nodes_ready
chv_operations_completed_total{status="succeeded"}
chv_operations_latency_seconds_bucket
```

#### Circuit Breaker Behavior

Node communication from the control plane is protected by a circuit breaker (`Closed` → `Open` → `HalfOpen`). When open, RPCs to that node are rejected immediately with `BackendUnavailable` without attempting the call. The breaker probes with single requests every 30s (default) in `HalfOpen` state and closes after 3 consecutive successes.

Agent connectivity metrics on `:9901/metrics` complement this:

```
# 1 = connected, 0 = disconnected
curl -s http://127.0.0.1:9901/metrics | grep chv_agent_cp_connected

# Duration of current disconnect in ms
curl -s http://127.0.0.1:9901/metrics | grep chv_agent_cp_disconnected_duration_ms
```

#### Partition / Autonomy Metrics

When an agent loses control-plane connectivity, it enters autonomous mode:

```
# 1 = connected, 0 = disconnected
curl -s http://127.0.0.1:9901/metrics | grep chv_agent_cp_connected

# Duration of current disconnect in ms
curl -s http://127.0.0.1:9901/metrics | grep chv_agent_cp_disconnected_duration_ms
```

During a partition:
- Existing VMs continue running.
- `CreateVm` and `MigrateVm` RPCs are rejected with `FAILED_PRECONDITION`.
- `StopVm` and `RebootVm` for existing VMs are allowed.
- Upon reconnection, pending messages are flushed in order.

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

## CLI Reference (`chvctl`)

`chvctl` is the operator CLI for the CHV platform. It communicates with the BFF HTTP API.

### Global Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--server` | `http://localhost:8080` | BFF server URL |
| `--token` | (from `~/.chv/credentials`) | Auth token override |
| `--output` | `table` | Output format: `table`, `json`, `yaml` |

### Commands

| Command | Subcommands | Description |
|---------|-------------|-------------|
| `chvctl login` | — | Authenticate and store token |
| `chvctl vm` | `list`, `show`, `create`, `start`, `stop`, `reboot`, `delete`, `resize` | VM lifecycle management |
| `chvctl node` | `list`, `show`, `drain`, `maintenance` | Node operations |
| `chvctl image` | `list`, `show`, `import`, `delete` | Disk image management |
| `chvctl volume` | `list`, `show`, `create`, `delete` | Storage volumes |
| `chvctl network` | `list`, `show`, `create`, `delete` | Network management |
| `chvctl storage` | `list-pools`, `show-pool` | Storage pools |
| `chvctl task` | `list`, `show` | Task/operation inspection |
| `chvctl backup` | `list`, `run` | Backup jobs (`show`, `create`, `restore` planned) |
| `chvctl user` | `list`, `create`, `delete` | User management (admin) |
| `chvctl migrate` | `start`, `status`, `cancel` | Live migration control |
| `chvctl upgrade` | `start`, `status`, `list`, `rollback` | Rolling upgrades |
| `chvctl health` | `cluster` | Cluster health summary |
| `chvctl version` | — | Show version and build info |

### Examples

```bash
# List VMs
chvctl vm list

# Show node details
chvctl node show <NODE_ID>

# Drain a node for maintenance
chvctl node drain <NODE_ID>

# Check cluster health
chvctl health cluster

# Start a rolling upgrade
chvctl upgrade start <NODE_ID> --version 0.5.0

# Resize a VM
chvctl vm resize <VM_ID> --cpu 4 --memory-mb 8192
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

### Disaster Recovery Runbooks

For detailed step-by-step procedures covering VM snapshot restore, volume snapshot restore, backup artifact restore, control plane DR, and full site recovery, see the runbooks in [`docs/runbooks/`](runbooks/).

| Runbook | Scenario |
|---------|----------|
| [VM Snapshot Restore](runbooks/vm-snapshot-restore.md) | Restore a VM from a local snapshot (automated) |
| [Volume Snapshot Restore](runbooks/volume-snapshot-restore.md) | Restore a volume from a local snapshot (local/Ceph automated; LVM/iSCSI manual) |
| [Backup Artifact Restore](runbooks/backup-artifact-restore.md) | Restore a VM from a shipped S3/NFS backup artifact (manual) |
| [Control Plane DR](runbooks/control-plane-dr.md) | Recover the control plane after host failure or DB corruption |
| [Full Site Recovery](runbooks/full-site-recovery.md) | Rebuild the entire infrastructure from backups |

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

---

## Architecture Designer day-2 operations

The Architecture Designer (see [`docs/specs/architecture-designer/`](specs/architecture-designer/)) persists desired-state topologies, plans, apply runs, drift reports, and fleet inventory snapshots in the controlplane SQLite database. Day-2 ops mirror the rest of CHV: SQLite backups + targeted retention.

### Backup

The Architecture Designer ships six tables (migrations `0046`–`0051`):

| Table | Purpose |
|-------|---------|
| `architecture_topologies` | Saved topology metadata (id, name, owner, latest version pointer) |
| `architecture_versions` | Immutable, append-only history of every saved YAML revision |
| `architecture_plans` | Generated plans pending or completed (apply / destroy modes) |
| `architecture_apply_runs` | Apply / destroy execution records linked to CHV tasks |
| `architecture_drift_reports` | Drift findings per architecture per check |
| `inventory_snapshots` | Fleet observed state captured for plan determinism and drift baselines |

The standard SQLite backup procedure documented earlier (`sqlite3 ... ".backup"`) covers all six tables — they live in `/var/lib/chv/controlplane.db` along with everything else. No separate dump is required.

**Recommended cadence:**
- Hourly online backup retained 24h.
- Daily backup retained 30d.
- Weekly off-host backup retained 12 months.

**Recovery posture:**
- **RPO** ≤ 1 hour (last hourly backup).
- **RTO** ≤ 15 minutes for control-plane restore (stop service → copy file → start) on a single-node deployment.
- A restored database loses any plans / apply runs created after the backup window. Topologies and accepted versions are unaffected if backups are taken before user-facing edits.

### Starter topologies — opt-out and re-seed

On first boot the controlplane seeds six canonical reference topologies into `architecture_topologies` (see [`docs/plans/2026-06-16-starter-topologies-and-auto-seed.md`](plans/2026-06-16-starter-topologies-and-auto-seed.md)). They land as `status = draft`, `owner_user_id = NULL`, named `starter-NN-<slug>`, and the dashboard never auto-applies them — operators clone, not edit, starters to make their own.

The seed is gated by a sentinel row `seed_starters_completed` in `system_settings`. A single atomic `UPSERT … WHERE value = '0'` flips the sentinel to `'1'` and claims the seed; only the process whose UPDATE affects exactly one row owns the run. Subsequent boots see `value = '1'` and skip cleanly.

**Opt out before first boot** (e.g. on a real production deployment where the operator does not want demo content):

```bash
sudo systemctl stop chv-controlplane
sqlite3 /var/lib/chv/controlplane.db \
  "INSERT OR REPLACE INTO system_settings (key, value, updated_at)
   VALUES ('seed_starters_completed', '1', strftime('%Y-%m-%dT%H:%M:%fZ','now'));"
sudo systemctl start chv-controlplane
# verify: dashboard shows zero topologies; controlplane logs note "starter topologies already seeded; skipping"
```

**Re-seed missing starters** (operator deleted some starter rows and wants them back):

```bash
sudo systemctl stop chv-controlplane
sqlite3 /var/lib/chv/controlplane.db \
  "UPDATE system_settings SET value = '0',
       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
   WHERE key = 'seed_starters_completed';"
sudo systemctl start chv-controlplane
# the seeder's per-starter AlreadyExists branch means existing rows are left untouched;
# only the missing ones get re-inserted. The sentinel flips back to '1' on success.
```

**Caveats:**
- The sentinel must be exactly `'0'` to trigger re-seed. Trailing whitespace (`'0 '`) does NOT match — by design, to keep typo-resistant operator overrides idempotent.
- A control plane that cannot read or write `system_settings` refuses to start. Sentinel-write failures are fail-closed; per-fixture parse/insert failures are fail-open and logged via `tracing::error!` so a single bad fixture does not block boot.

### Retention

Until the periodic pruner ships (tracked in [`docs/plans/2026-06-16-snapshot-pruner-followup.md`](plans/2026-06-16-snapshot-pruner-followup.md)), retention is best-effort and operator-driven. Recommended policy:

| Table | Retention | Rationale |
|-------|-----------|-----------|
| `architecture_topologies` | Indefinite | Small, user-curated, no churn. |
| `architecture_versions` | Indefinite | Append-only history of intent; per ADR-003-Designer the YAML is the source of truth. Small. |
| `architecture_plans` | 30 days post-terminal status | Per ADR-005-Designer; expired/discarded plans are not re-runnable. Active plans never deleted. |
| `architecture_apply_runs` | 90d for `succeeded`, indefinite for `failed`/`partially_failed` | Failed runs are audit material. Successful runs are operational telemetry. |
| `architecture_drift_reports` | 14 days, **but** retain latest report per architecture indefinitely | Most recent report is the user-facing "current drift" view. |
| `inventory_snapshots` | 7 days | High churn; only the recent ones drive plan determinism. |

Operators who need to free space before the pruner ships can run targeted deletes:

```bash
# Drop drift reports older than 14 days, keeping the latest per architecture
sqlite3 /var/lib/chv/controlplane.db <<'SQL'
DELETE FROM architecture_drift_reports
WHERE id NOT IN (
    SELECT id FROM (
        SELECT id, ROW_NUMBER() OVER (PARTITION BY architecture_id ORDER BY created_at DESC) AS rn
        FROM architecture_drift_reports
    ) WHERE rn = 1
)
AND created_at < datetime('now', '-14 days');
SQL

# Drop inventory snapshots older than 7 days
sqlite3 /var/lib/chv/controlplane.db \
  "DELETE FROM inventory_snapshots WHERE created_at < datetime('now', '-7 days');"
```

Always take a backup before manual deletes.

### Monitoring

The Architecture Designer exposes Prometheus metrics on the controlplane `:9901/metrics` endpoint. The label vocabularies below match the actual emitter source (`crates/chv-webui-bff/src/metrics_apply.rs` and `metrics_drift.rs`); writing alerts against label values the emitter does not produce yields silent dead-on-arrival rules.

```
# Apply attempts (counter)
# status ∈ { started, enqueued, failed }
#   - started:  incremented on every POST /v1/architectures/apply entry,
#               before pre-condition guards run
#   - enqueued: apply_plan returned Ok and the run was accepted for execution
#   - failed:   apply_plan returned Err (4xx pre-condition or 5xx store)
# Note: this counter records the BFF-side enqueue lifecycle. Terminal
# orchestrator outcomes (succeeded / partially_failed / cancelled) are
# recorded in the `architecture_apply_runs.status` column today; a future
# emitter on the orchestrator terminal-state writeback can extend this
# counter or introduce `chv_architecture_apply_terminal_total`.
chv_architecture_apply_total{status="started|enqueued|failed"}

# Apply latency (histogram, BFF handler entry → response)
chv_architecture_apply_duration_seconds_bucket{...}

# Drift checks (counter)
# status ∈ { no_drift, drifted, unknown, check_failed, cache_hit }
#   - no_drift:     fresh compute returned an empty findings list
#   - drifted:      fresh compute returned at least one finding
#   - unknown:      reserved for the wire enum's DriftStatus::Unknown variant
#   - check_failed: snapshot capture or YAML parse failed; the BFF persisted
#                   a check_failed report and returned 200
#   - cache_hit:    the most recent persisted report was within the 5-minute
#                   TTL and force_refresh was false
chv_architecture_drift_total{status="no_drift|drifted|unknown|check_failed|cache_hit"}
```

Recommended alert templates (Prometheus `for:` durations are starting points; tune per environment):

```yaml
- alert: ArchitectureDriftCheckFailing
  expr: increase(chv_architecture_drift_total{status="check_failed"}[5m]) > 0
  for: 5m
  labels: { severity: page }
  annotations:
    summary: "Architecture drift checker is failing"
    description: "Drift checks have returned check_failed in the last 5 minutes — fleet visibility is degraded."

- alert: ArchitectureApplySlow
  expr: histogram_quantile(0.99, rate(chv_architecture_apply_duration_seconds_bucket[15m])) > 600
  for: 15m
  labels: { severity: ticket }
  annotations:
    summary: "Architecture apply p99 > 10 minutes"
    description: "Apply runs are taking longer than expected; investigate task-system backlog."

- alert: ArchitectureApplyEnqueueFailureRate
  # `failed` here is the BFF enqueue-time failure (pre-condition or store);
  # it does NOT cover orchestrator-side terminal failures because those are
  # not surfaced as a Prometheus counter today (see Monitoring note above).
  expr: |
    sum(rate(chv_architecture_apply_total{status="failed"}[15m]))
      / clamp_min(sum(rate(chv_architecture_apply_total{status=~"started|failed"}[15m])), 1e-9) > 0.2
  for: 30m
  labels: { severity: ticket }
  annotations:
    summary: "Architecture apply enqueue failure rate > 20% over 15m"
    description: >
      More than 20% of POST /v1/architectures/apply requests are failing the
      pre-condition or store guard. For terminal orchestrator outcomes
      (succeeded / partially_failed / cancelled), query
      `architecture_apply_runs` directly until a dedicated counter ships.
```

> **Label-vocabulary contract**: the bundled rules in `monitoring/rules/chv.yml` and `recording.yml` reference only labels the emitter actually sets. CI runs `scripts/check-metric-labels.sh` on every push to enforce that contract — see [`docs/specs/adr/009-logging-and-observability.md`](specs/adr/009-logging-and-observability.md). If you extend an emitter to add a new label key, run the gate locally before opening the PR.

### Troubleshooting

**Drift checks return `CheckFailed` repeatedly**

| Diagnostic | Resolution |
|-----------|------------|
| `journalctl -u chv-controlplane \| grep architecture_drift` for the underlying error | Most common cause: a referenced resource (network, datastore) was deleted out-of-band. Re-pin the architecture's baseline by re-running `apply` after editing the topology, or accept the current drift by writing a new version. |
| Inventory snapshot stale (look for `inventory_snapshots.created_at` > 1h old) | Restart the controlplane snapshot loop: `systemctl restart chv-controlplane`. The next drift check uses a fresh snapshot. |

**Apply stuck in `Applying` state**

The apply state machine uses CAS guards (compare-and-set on `status`) to ensure exactly one writer. A stuck `Applying` row usually means the worker process died mid-run.

```bash
# Inspect the run
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT id, architecture_id, plan_id, status, started_at, updated_at \
   FROM architecture_apply_runs WHERE status = 'applying';"

# Check that no live task is still in flight
sqlite3 /var/lib/chv/controlplane.db \
  "SELECT operation_id, kind, state FROM operations \
   WHERE meta_json LIKE '%<APPLY_RUN_ID>%';"
```

If the linked task is `Failed` / `Cancelled` and no controlplane process is holding the row (no recent `updated_at`), reset the run to `failed` so a retry is possible:

```bash
sqlite3 /var/lib/chv/controlplane.db <<SQL
UPDATE architecture_apply_runs
SET status = 'failed',
    error_summary = 'manual recovery: stuck applying',
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
WHERE id = '<APPLY_RUN_ID>' AND status = 'applying';
SQL
```

The CAS guard makes this safe: if a live worker still owns the row, the `WHERE status = 'applying'` clause races the update without corrupting state. Always restart the controlplane first if there is any doubt.

**Plan returns `PLAN_EXPIRED` on retry within 15 minutes**

Plans expire 15 minutes after generation (per ADR-005-Designer). If a fresh plan is rejected as expired, suspect clock skew between the BFF process and the controlplane.

| Diagnostic | Resolution |
|-----------|------------|
| `date -u` on each host running CHV components | Sync clocks: `sudo chronyc -a makestep` or `sudo systemctl restart systemd-timesyncd`. |
| Inspect the plan row: `SELECT created_at, expires_at FROM architecture_plans WHERE id = '<PLAN_ID>';` | If `expires_at - created_at` is not 15 minutes, the issuing process has wrong wall-clock; restart that process after the clock fix. |
| Container deployments with shared host clock | Verify NTP is enabled on the host; containers inherit host time, so a host-level fix propagates. |

If clocks are correct and the plan still expires immediately, file a bug — the controlplane and BFF should always agree on plan expiry.
