# CHV - Cloud Hypervisor Virtualization Platform

[![Go Version](https://img.shields.io/badge/go-1.26+-blue.svg)](https://golang.org/dl/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Test Coverage](https://img.shields.io/badge/coverage-75%25-yellow.svg)]()

A Linux-first, cloud-image-first virtualization platform for sovereign private cloud and edge cloud environments.

> **Status**: MVP-1 Released | [Documentation](docs/) | [Contributing](CONTRIBUTING.md) | [Security](SECURITY.md)

## Overview

CHV is built on [Cloud Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) and provides a modern VM platform with:

- **Cloud-image-first** provisioning (no ISO workflows)
- **API-driven** control plane
- **Linux bridge** networking
- **Local and NFS** storage
- **Raw runtime disks** for simplicity
- **PostgreSQL-backed** control plane

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Controller    │────▶│  PostgreSQL DB  │◀────│   Scheduler     │
│   (REST/gRPC)   │     │   (State Store) │     │   (Placement)   │
└────────┬────────┘     └─────────────────┘     └─────────────────┘
         │
         │ gRPC
         ▼
┌─────────────────┐     ┌─────────────────┐
│   chv-agent     │────▶│  Cloud Hypervisor│
│  (Per-node)     │     │   (VM processes) │
└─────────────────┘     └─────────────────┘
```

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Linux host with KVM support (for production deployment)
- Go 1.22+ (for development)

### Development Environment

1. **Clone and setup:**
```bash
git clone <repository>
cd chv
make setup
```

2. **Start the services:**
```bash
make docker-up
```

This starts:
- PostgreSQL database on port 5432
- Controller API on port 8080
- Controller gRPC on port 9090

3. **Create an API token:**
```bash
curl -X POST http://localhost:8080/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{"name": "admin-token", "expires_in": "24h"}'
```

Save the returned token for subsequent requests.

4. **Register a node:**
```bash
curl -X POST http://localhost:8080/api/v1/nodes/register \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "hostname": "node1",
    "management_ip": "192.168.1.10",
    "total_cpu_cores": 8,
    "total_ram_mb": 16384
  }'
```

5. **Create a network:**
```bash
curl -X POST http://localhost:8080/api/v1/networks \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "default",
    "bridge_name": "br0",
    "cidr": "192.168.100.0/24",
    "gateway_ip": "192.168.100.1"
  }'
```

6. **Create a storage pool:**
```bash
curl -X POST http://localhost:8080/api/v1/storage-pools \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "local",
    "pool_type": "local",
    "path_or_export": "/var/lib/chv/volumes",
    "supports_online_resize": true
  }'
```

7. **Import a cloud image:**
```bash
curl -X POST http://localhost:8080/api/v1/images/import \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "ubuntu-22.04",
    "os_family": "ubuntu",
    "source_url": "https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img",
    "source_format": "qcow2",
    "architecture": "x86_64",
    "cloud_init_supported": true
  }'
```

8. **Create and start a VM:**
```bash
curl -X POST http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "vm1",
    "cpu": 2,
    "memory_mb": 4096,
    "image_id": "YOUR_IMAGE_ID",
    "disk_size_bytes": 10737418240,
    "networks": [{"network_id": "YOUR_NETWORK_ID"}]
  }'
```

## API Documentation

### Authentication

All protected endpoints require a Bearer token:
```
Authorization: Bearer chv_...
```

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | /api/v1/tokens | Create API token |
| POST | /api/v1/nodes/register | Register a node |
| GET | /api/v1/nodes | List nodes |
| GET | /api/v1/nodes/{id} | Get node details |
| POST | /api/v1/nodes/{id}/maintenance | Toggle maintenance mode |
| POST | /api/v1/networks | Create network |
| GET | /api/v1/networks | List networks |
| POST | /api/v1/storage-pools | Create storage pool |
| GET | /api/v1/storage-pools | List storage pools |
| POST | /api/v1/images/import | Import cloud image |
| GET | /api/v1/images | List images |
| POST | /api/v1/vms | Create VM |
| GET | /api/v1/vms | List VMs |
| GET | /api/v1/vms/{id} | Get VM details |
| POST | /api/v1/vms/{id}/start | Start VM |
| POST | /api/v1/vms/{id}/stop | Stop VM |
| POST | /api/v1/vms/{id}/reboot | Reboot VM |
| POST | /api/v1/vms/{id}/resize-disk | Resize VM disk |
| DELETE | /api/v1/vms/{id} | Delete VM |

## Web UI

CHV includes a Vue.js 3 web interface with an enterprise virtualization console design inspired by VMware vSphere and Proxmox VE.

### Quick Start (UI)

**Option 1: Development Server**
```bash
# Start the UI development server (requires Node.js 20+)
make ui-dev
```

Then open http://localhost:5173

**Option 2: Docker (Production Build)**
```bash
# Build and run the UI in Docker
make ui-docker
```

Then open http://localhost:3000

**Option 3: Full Stack with UI**
```bash
# Start controller, postgres, agent, and UI
make docker-up-all
```

### UI Features

- **Three-pane layout** (sidebar navigation, main content, details panel)
- **VM lifecycle management** (create, start, stop, reboot, delete)
- **Real-time status monitoring** with color-coded indicators
- **Inventory management** for nodes, networks, storage pools, and images
- **Responsive design** for desktop and tablet
- **Enterprise aesthetic** - industrial, data-dense, functional

### UI Architecture

```
chv-ui/
├── src/
│   ├── assets/         # Global styles and design tokens
│   ├── components/     # Vue components
│   ├── stores/         # Pinia state management
│   ├── views/          # Page components
│   ├── types/          # TypeScript type definitions
│   └── router/         # Vue Router configuration
├── public/             # Static assets
└── package.json        # Dependencies
```

## Project Structure

```
.
├── cmd/
│   ├── chv-controller/     # Controller entry point
│   ├── chv-agent/          # Agent entry point
│   └── chv-bootstrap/      # Bootstrap installer
├── internal/
│   ├── api/                # HTTP API handlers
│   ├── auth/               # Token authentication
│   ├── cloudinit/          # Cloud-init generation
│   ├── models/             # Domain models
│   ├── network/            # Network management
│   ├── nodevalidate/       # Host validation
│   ├── operations/         # Operation tracking
│   ├── pb/agent/           # Protocol buffers
│   ├── reconcile/          # State reconciliation
│   ├── scheduler/          # VM placement
│   ├── storage/            # Storage management
│   └── store/              # Database layer
├── pkg/
│   ├── errorsx/            # Structured errors
│   └── uuidx/              # UUID utilities
├── deploy/
│   ├── systemd/            # Systemd unit files
│   ├── controller.Dockerfile
│   ├── agent.Dockerfile
│   └── bootstrap.Dockerfile
├── configs/
│   └── schema.sql          # Database schema
├── docs/
│   ├── specs/              # Product specifications
│   ├── contracts/          # System contracts
│   ├── adr/                # Architecture decisions
│   └── architecture/       # Architecture docs
├── chv-ui/                 # Vue.js 3 Web UI
│   ├── src/
│   ├── public/
│   └── package.json
├── docker-compose.yml
├── Makefile
└── README.md
```

## Development

### Building

```bash
# Build all binaries
make build

# Build specific components
make build-controller
make build-agent
make build-bootstrap
```

### Testing

```bash
# Run tests
make test

# Run with coverage
make test-coverage
```

### Docker Compose

```bash
# Start services
make docker-up

# View logs
make docker-logs

# Stop services
make docker-down

# Clean everything
make docker-clean
```

## MVP-1 Scope

### Included
- Linux guests only
- Cloud Hypervisor VMM
- Controller + Agent model
- PostgreSQL database
- Linux bridge networking
- Local and NFS storage
- Raw runtime disks
- Cloud-init provisioning
- VM lifecycle (create, start, stop, reboot, delete)
- Basic online disk resize
- Node inventory and heartbeats
- Scheduler with placement rules
- Maintenance/drain mode
- Desired vs actual state reconciliation

### Explicitly Not Included
- ISO installation
- Windows guests
- GPU/VFIO
- Distributed block storage
- VXLAN/eBPF networking
- Live migration
- LXC/containers

## Architecture Decisions

See [docs/adr/](docs/adr/) for detailed architecture decisions:

1. **ADR-0001**: Linux-first and cloud-image-first
2. **ADR-0002**: Privileged bootstrap container, host-native runtime
3. **ADR-0003**: Linux bridges in MVP-1
4. **ADR-0004**: Local + NFS storage only
5. **ADR-0005**: Raw runtime disks
6. **ADR-0006**: Opaque bearer tokens
7. **ADR-0007**: Migration as validated-matrix feature

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute to this project.

Please read our [Code of Conduct](CODE_OF_CONDUCT.md) to understand our community standards.

## Security

For security-related information, including vulnerability reporting, see [SECURITY.md](SECURITY.md).

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and release notes.

## Development

For detailed development setup instructions, see [DEVELOPMENT.md](DEVELOPMENT.md).
