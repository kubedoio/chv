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

---

## Maintenance Windows

### Graceful Node Drain

1. Mark node as `Draining` in the Web UI or API
2. VMs will be migrated or stopped per the drain policy
3. Wait for `vm_count` to reach 0
4. Stop agent: `sudo systemctl stop chv-agent`
5. Perform maintenance
6. Restart agent: `sudo systemctl start chv-agent`
7. Mark node as `TenantReady`

### Upgrade Procedure

1. Back up database and certificates
2. Build or download new release tarball
3. Stop services in order: agent → stord/nwd → control plane
4. Install new binaries
5. Start control plane (runs migrations automatically)
6. Start stord and nwd
7. Start agent
8. Verify: `systemctl status` and `curl /health`

---

## Security Hardening

- Replace self-signed CA with organization PKI
- Rotate bootstrap tokens after each use
- Restrict `/etc/chv/certs/` to `root:chv` with `640` permissions
- Run `chv-stord` under a dedicated service account with device/path allowlists
- Enable firewall rules limiting gRPC port 8443 to known hypervisor IPs
