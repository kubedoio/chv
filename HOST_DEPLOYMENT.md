# CHV Host Deployment Guide

This guide documents running CHV (Cloud Hypervisor Platform) directly on the host system without Docker containers.

## Overview

All CHV services now run as systemd services on the host:
- **chv-controller**: HTTP API (:8082) and gRPC (:9092)
- **chv-agent**: gRPC (:9090) and HTTP (:10090)
- **chv-ui**: Web UI server (:3000)

## Quick Start

```bash
# Check status
chv status

# Start all services
chv start

# Stop all services
chv stop

# Restart all services
chv restart

# View logs
chv logs controller
chv logs agent
chv logs ui
```

## Service Details

### Controller (chv-controller)
- **Binary**: `/usr/local/bin/chv-controller`
- **Config**: `/etc/chv/controller.yaml`
- **Database**: `/var/lib/chv/chv.db` (SQLite)
- **HTTP API**: http://localhost:8082
- **gRPC**: localhost:9092
- **Service**: `systemctl {start|stop|restart|status} chv-controller`

### Agent (chv-agent)
- **Binary**: `/usr/local/bin/chv-agent`
- **Config**: `/etc/chv/agent.yaml`
- **Data**: `/var/lib/chv/`
- **gRPC**: localhost:9090
- **HTTP**: http://localhost:10090
- **Service**: `systemctl {start|stop|restart|status} chv-agent`

### Web UI (chv-ui)
- **Server**: `/usr/local/bin/chv-ui-server` (Python)
- **Files**: `/srv/data02/projects/chv/chv-ui/dist/`
- **URL**: http://localhost:3000
- **Service**: `systemctl {start|stop|restart|status} chv-ui`

## Configuration Files

### /etc/chv/controller.yaml
```yaml
http_addr: ":8082"
grpc_addr: ":9092"
database_path: "/var/lib/chv/chv.db"
log_level: "info"
log_format: "text"
jwt_secret: "change-me-in-production"
token_duration: "24h"
```

### /etc/chv/agent.yaml
```yaml
node_id: "host-node"
listen_addr: ":9091"
controller_addr: "localhost:9092"
data_dir: "/var/lib/chv"
cloud_hypervisor: "/usr/local/bin/cloud-hypervisor"
bridge_name: "chvbr0"
```

## File Locations

| Component | Location |
|-----------|----------|
| Binaries | `/usr/local/bin/chv-*` |
| Configs | `/etc/chv/` |
| Data | `/var/lib/chv/` |
| Logs | `journalctl -u chv-*` |
| Services | `/etc/systemd/system/chv-*.service` |

## Rebuilding from Source

```bash
cd /srv/data02/projects/chv

# Build binaries
go build -o chv-controller ./cmd/controller
go build -o chv-agent ./cmd/agent
go build -o chv-certgen ./cmd/certgen

# Install
sudo cp chv-* /usr/local/bin/

# Rebuild UI
cd chv-ui
npm install
npm run build

# Restart services
sudo chv restart
```

## Troubleshooting

### Check service status
```bash
systemctl status chv-controller chv-agent chv-ui
```

### View logs
```bash
journalctl -u chv-controller -f
journalctl -u chv-agent -f
journalctl -u chv-ui -f
```

### Port conflicts
If ports are already in use, edit:
- `/etc/chv/controller.yaml` - change `http_addr` and `grpc_addr`
- `/etc/chv/agent.yaml` - change `listen_addr`
- `/usr/local/bin/chv-ui-server` - change `PORT` variable

Then restart services:
```bash
chv restart
```

### Database permissions
```bash
sudo chmod 666 /var/lib/chv/chv.db
sudo chmod 666 /var/lib/chv/chv.db-*
```

## Migration from Docker

If you were previously using Docker:
1. Data persists in `/var/lib/chv/` (same location)
2. The database file is compatible
3. Configuration files have been updated for host deployment

## API Usage

```bash
# Create token
curl -X POST http://localhost:8082/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{"name":"my-token"}'

# Use token
curl http://localhost:8082/api/v1/vms \
  -H "Authorization: Bearer <token>"
```

## Web UI

Access the web UI at http://localhost:3000

Login with a token created via the API or click "Create New Token" button.
