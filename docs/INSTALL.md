# CHV Installation Guide

This guide covers the complete installation of CHV (Cloud Hypervisor Virtualization Platform), including Cloud Hypervisor, the CHV Agent, and the CHV Controller.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Cloud Hypervisor Installation](#cloud-hypervisor-installation)
- [Kernel and Firmware Requirements](#kernel-and-firmware-requirements)
- [Agent Setup](#agent-setup)
- [Controller Setup](#controller-setup)
- [Verification Steps](#verification-steps)
- [Troubleshooting](#troubleshooting)

---

## Overview

CHV consists of three main components:

1. **Cloud Hypervisor**: The underlying VMM (Virtual Machine Monitor) that runs VMs
2. **CHV Agent**: Runs on each compute node, manages VMs via Cloud Hypervisor
3. **CHV Controller**: Central control plane that manages multiple agents

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

---

## Prerequisites

### System Requirements

#### Compute Nodes (Agent + Cloud Hypervisor)

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| Linux Kernel | 5.10+ | 6.1+ |
| CPU | x86_64 with VT-x/AMD-V | Multi-core with VT-x/AMD-V |
| RAM | 4 GB | 16 GB+ |
| Disk | 20 GB | 100 GB+ SSD |
| KVM Support | Required | Required |

#### Controller Node

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Linux (any modern distribution) | Ubuntu 22.04 LTS |
| RAM | 2 GB | 4 GB+ |
| Disk | 10 GB | 50 GB+ |
| PostgreSQL | 14+ | 16+ |

### Supported Linux Distributions

- Ubuntu 20.04 LTS, 22.04 LTS, 24.04 LTS
- Debian 11, 12
- RHEL 8, 9
- Rocky Linux 8, 9
- AlmaLinux 8, 9

### Checking Prerequisites

#### 1. Verify Kernel Version

```bash
uname -r
# Should output 5.10 or higher
```

#### 2. Check for KVM Support

```bash
# Check if KVM module is loaded
lsmod | grep kvm

# Output should show:
# kvm_intel             ...
# kvm                   ...

# Or for AMD:
# kvm_amd               ...
# kvm                   ...

# Check if /dev/kvm exists
ls -la /dev/kvm
# Should show: crw-rw----+ 1 root kvm 10, 232 ...
```

#### 3. Verify CPU Virtualization Support

```bash
# Intel CPUs
grep -c vmx /proc/cpuinfo

# AMD CPUs  
grep -c svm /proc/cpuinfo

# Either should return a number greater than 0
```

#### 4. Enable KVM (if not already enabled)

```bash
# Load KVM modules
sudo modprobe kvm
sudo modprobe kvm_intel   # For Intel CPUs
# OR
sudo modprobe kvm_amd     # For AMD CPUs

# Ensure modules load on boot
echo "kvm" | sudo tee /etc/modules-load.d/kvm.conf
echo "kvm_intel" | sudo tee -a /etc/modules-load.d/kvm.conf  # Intel
# OR
echo "kvm_amd" | sudo tee -a /etc/modules-load.d/kvm.conf    # AMD
```

---

## Cloud Hypervisor Installation

### Download and Install

Cloud Hypervisor v51.1 is the recommended version for CHV.

```bash
# Create installation directory
sudo mkdir -p /usr/local/bin

# Download Cloud Hypervisor (static binary)
# Current version: v51.1
sudo curl -L -o /usr/local/bin/cloud-hypervisor \
  https://github.com/cloud-hypervisor/cloud-hypervisor/releases/latest/download/cloud-hypervisor-static

# Make executable
sudo chmod +x /usr/local/bin/cloud-hypervisor

# Verify installation
cloud-hypervisor --version
# Output: cloud-hypervisor v51.1 (or newer)
```

### Alternative: Using Pre-downloaded Binary

If you have the binary in your CHV project:

```bash
# Copy from project bin directory
sudo cp bin/cloud-hypervisor /usr/local/bin/
sudo chmod +x /usr/local/bin/cloud-hypervisor

# Verify
cloud-hypervisor --version
```

### Verify Binary Integrity

```bash
# Check binary is not corrupted
file /usr/local/bin/cloud-hypervisor
# Should show: ELF 64-bit LSB executable, x86-64

# Test execution
cloud-hypervisor --help
```

---

## Kernel and Firmware Requirements

### Guest Kernel (vmlinux)

Cloud Hypervisor uses a direct kernel boot method. You need a Linux kernel image (`vmlinux` or `vmlinuz`) for the guests.

#### Option 1: Download Pre-built Kernel

```bash
# Create kernels directory
sudo mkdir -p /var/lib/chv/kernels

# Download cloud-hypervisor compatible kernel
# Example: Ubuntu generic kernel
cd /var/lib/chv/kernels
sudo curl -LO https://cloud-images.ubuntu.com/jammy/current/unpacked/

# Or use the kernel from cloud-hypervisor releases
sudo curl -LO https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v51.1/linux-6.12.6-amd

# Rename for clarity
sudo mv linux-6.12.6-amd vmlinux-6.12.6
sudo chmod 644 vmlinux-6.12.6
```

#### Option 2: Extract from Distribution Package

```bash
# For Ubuntu/Debian
sudo apt-get install linux-image-generic

# Extract vmlinux from vmlinuz
sudo cp /boot/vmlinuz-$(uname -r) /tmp/vmlinuz
sudo dd if=/tmp/vmlinuz of=/var/lib/chv/kernels/vmlinux-$(uname -r) bs=1 skip=$(
  od -A d -t x1 -N 4 /tmp/vmlinuz | head -1 | awk '{print $2}' | xargs -I {} printf '%d' 0x{}
) 2>/dev/null || true

# Alternative: Use extract-vmlinux script
/usr/src/linux-headers-$(uname -r)/scripts/extract-vmlinux /boot/vmlinuz-$(uname -r) > \
  /var/lib/chv/kernels/vmlinux-$(uname -r)
```

#### Option 3: Build Custom Kernel

```bash
# Download kernel source
git clone https://github.com/torvalds/linux.git
cd linux
git checkout v6.12

# Configure for Cloud Hypervisor
make x86_64_defconfig
make menuconfig  # Enable VirtIO drivers

# Build
make -j$(nproc)

# Install
sudo cp vmlinux /var/lib/chv/kernels/vmlinux-custom
```

### Required Kernel Configuration

Ensure your guest kernel has these options enabled:

```
CONFIG_VIRTIO=y
CONFIG_VIRTIO_PCI=y
CONFIG_VIRTIO_NET=y
CONFIG_VIRTIO_BLK=y
CONFIG_VIRTIO_CONSOLE=y
CONFIG_VIRTIO_BALLOON=y
CONFIG_VIRTIO_INPUT=y
CONFIG_VIRTIO_MMIO=y
CONFIG_VIRTIO_FS=y
CONFIG_DRM_VIRTIO_GPU=y
CONFIG_NET_9P=y
CONFIG_NET_9P_VIRTIO=y
CONFIG_SCSI_VIRTIO=y
```

### Cloud-init Support

CHV uses cloud-init for VM provisioning. Ensure your guest kernel supports:

```
CONFIG_BLK_DEV_SR=y          # SCSI CD-ROM (for cloud-init ISO)
CONFIG_ISO9660_FS=y          # ISO 9660 filesystem
CONFIG_VFAT_FS=y             # FAT filesystem (for ESP)
```

---

## Agent Setup

### 1. Create System User and Directories

```bash
# Create chv user
sudo useradd --system --no-create-home --shell /usr/sbin/nologin chv

# Create required directories
sudo mkdir -p /var/lib/chv/{images,volumes,run}
sudo mkdir -p /etc/chv
sudo mkdir -p /var/log/chv

# Set ownership
sudo chown -R chv:chv /var/lib/chv
sudo chown -R chv:chv /var/log/chv
sudo chmod 755 /var/lib/chv
```

### 2. Install CHV Agent Binary

```bash
# Build from source (if needed)
make build-agent

# Install binary
sudo cp bin/chv-agent /usr/local/bin/
sudo chmod +x /usr/local/bin/chv-agent

# Or download from releases
# sudo curl -L -o /usr/local/bin/chv-agent https://github.com/yourorg/chv/releases/download/v0.1.0/chv-agent
# sudo chmod +x /usr/local/bin/chv-agent
```

### 3. Create Agent Configuration

Create `/etc/chv/agent.yaml`:

```yaml
# CHV Agent Configuration

# Unique node identifier (auto-generated if not specified)
# node_id: "node-001"

# Controller gRPC address
controller_addr: "controller.example.com:9090"

# Agent listen address
listen_addr: ":9090"

# Data directories
data_dir: "/var/lib/chv"
image_dir: "/var/lib/chv/images"
volume_dir: "/var/lib/chv/volumes"

# Cloud Hypervisor binary path
cloud_hypervisor_path: "/usr/local/bin/cloud-hypervisor"

# Heartbeat interval
heartbeat_interval: "30s"
```

### 4. Install Systemd Service

Create `/etc/systemd/system/chv-agent.service`:

```ini
[Unit]
Description=CHV Agent
After=network.target
Wants=network.target

[Service]
Type=simple
User=chv
Group=chv
ExecStart=/usr/local/bin/chv-agent -config=/etc/chv/agent.yaml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/chv /var/log/chv
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
RestrictRealtime=true
RestrictNamespaces=true
LockPersonality=true
MemoryDenyWriteExecute=true

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable chv-agent

# Start service
sudo systemctl start chv-agent

# Check status
sudo systemctl status chv-agent
```

### 5. Configure Firewall

```bash
# Allow agent gRPC port (9090)
sudo firewall-cmd --permanent --add-port=9090/tcp
sudo firewall-cmd --reload

# Or using ufw
sudo ufw allow 9090/tcp
```

---

## Controller Setup

### 1. Install PostgreSQL

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install postgresql-16 postgresql-contrib

# RHEL/Rocky/AlmaLinux
sudo dnf install postgresql-server postgresql-contrib
sudo postgresql-setup --initdb
sudo systemctl enable --now postgresql
```

### 2. Create Database and User

```bash
# Switch to postgres user
sudo -u postgres psql

# In PostgreSQL prompt:
CREATE DATABASE chv;
CREATE USER chv WITH PASSWORD 'your_secure_password';
GRANT ALL PRIVILEGES ON DATABASE chv TO chv;
\q
```

### 3. Initialize Database Schema

```bash
# Apply schema
sudo -u postgres psql -d chv -f configs/schema.sql
```

### 4. Create System User and Directories

```bash
# Create chv user
sudo useradd --system --no-create-home --shell /usr/sbin/nologin chv-controller

# Create directories
sudo mkdir -p /etc/chv
sudo mkdir -p /var/log/chv

# Set ownership
sudo chown chv-controller:chv-controller /var/log/chv
```

### 5. Install CHV Controller Binary

```bash
# Build from source
make build-controller

# Install binary
sudo cp bin/chv-controller /usr/local/bin/
sudo chmod +x /usr/local/bin/chv-controller
```

### 6. Create Controller Configuration

Create `/etc/chv/controller.yaml`:

```yaml
# CHV Controller Configuration

# Database connection string
database_url: "postgres://chv:your_secure_password@localhost:5432/chv?sslmode=disable"

# HTTP API address
http_addr: ":8080"

# gRPC address for agent communication
grpc_addr: ":9090"

# Logging level (debug, info, warn, error)
log_level: "info"

# Reconciliation interval
reconcile_interval: "30s"

# Heartbeat timeout for nodes
heartbeat_timeout: "2m"
```

### 7. Install Systemd Service

Create `/etc/systemd/system/chv-controller.service`:

```ini
[Unit]
Description=CHV Controller
After=network.target postgresql.service
Wants=network.target postgresql.service

[Service]
Type=simple
User=chv-controller
Group=chv-controller
ExecStart=/usr/local/bin/chv-controller -config=/etc/chv/controller.yaml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/chv
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
RestrictRealtime=true
RestrictNamespaces=true

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable chv-controller

# Start service
sudo systemctl start chv-controller

# Check status
sudo systemctl status chv-controller
```

### 8. Configure Firewall

```bash
# Allow controller ports
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --permanent --add-port=9090/tcp
sudo firewall-cmd --reload

# Or using ufw
sudo ufw allow 8080/tcp
sudo ufw allow 9090/tcp
```

### 9. Create Initial API Token

```bash
# Start controller first
sudo systemctl start chv-controller

# Wait for it to be ready
sleep 5

# Create admin token
curl -X POST http://localhost:8080/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{"name": "admin-token", "expires_in": "8760h"}'

# Save the returned token securely
```

---

## Verification Steps

### 1. Verify Cloud Hypervisor

```bash
# Check version
cloud-hypervisor --version

# Expected: cloud-hypervisor v51.1

# Verify KVM access
ls -la /dev/kvm

# Test Cloud Hypervisor can access KVM
sudo cloud-hypervisor --kernel /var/lib/chv/kernels/vmlinux-6.12.6 --disk path=/dev/null --cmdline "console=hvc0" --memory size=128M --api-socket /tmp/chv-test.sock &
sleep 2
curl --unix-socket /tmp/chv-test.sock http://localhost/api/v1/vmm.ping
kill %1
```

### 2. Verify Agent

```bash
# Check service status
sudo systemctl status chv-agent

# View logs
sudo journalctl -u chv-agent -f

# Check agent is listening
ss -tlnp | grep 9090

# Test validation
cd /srv/data02/projects/chv
sudo go run ./cmd/chv-agent -config /etc/chv/agent.yaml 2>&1 | head -20
# Should show: "Host validation passed"
```

### 3. Verify Controller

```bash
# Check service status
sudo systemctl status chv-controller

# View logs
sudo journalctl -u chv-controller -f

# Check health endpoint
curl http://localhost:8080/health

# Expected: {"status":"healthy"}

# List nodes (requires token)
curl http://localhost:8080/api/v1/nodes \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### 4. End-to-End Test

```bash
# 1. Register the agent node
curl -X POST http://localhost:8080/api/v1/nodes/register \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "hostname": "node1",
    "management_ip": "192.168.1.10",
    "total_cpu_cores": 8,
    "total_ram_mb": 16384
  }'

# 2. Create a network
curl -X POST http://localhost:8080/api/v1/networks \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "default",
    "bridge_name": "br0",
    "cidr": "192.168.100.0/24",
    "gateway_ip": "192.168.100.1"
  }'

# 3. Create a storage pool
curl -X POST http://localhost:8080/api/v1/storage-pools \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "local",
    "pool_type": "local",
    "path_or_export": "/var/lib/chv/volumes",
    "supports_online_resize": true
  }'

# 4. Import a cloud image
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

# 5. Create a VM
curl -X POST http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-vm",
    "cpu": 2,
    "memory_mb": 4096,
    "image_id": "YOUR_IMAGE_ID",
    "disk_size_bytes": 10737418240,
    "networks": [{"network_id": "YOUR_NETWORK_ID"}]
  }'

# 6. Start the VM
curl -X POST http://localhost:8080/api/v1/vms/VM_ID/start \
  -H "Authorization: Bearer YOUR_TOKEN"

# 7. Check VM status
curl http://localhost:8080/api/v1/vms/VM_ID \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## Troubleshooting

### Cloud Hypervisor Issues

#### "Permission denied" on /dev/kvm

```bash
# Add user to kvm group
sudo usermod -a -G kvm chv
# Log out and back in for changes to take effect

# Or check permissions
sudo chmod 666 /dev/kvm  # Temporary, not recommended for production
```

#### Kernel Too Old

```bash
# Check kernel version
uname -r

# If < 5.10, upgrade kernel
# Ubuntu:
sudo apt-get install linux-generic-hwe-22.04

# RHEL/Rocky:
sudo dnf update kernel
```

### Agent Issues

#### "Host validation failed: KVM_NOT_AVAILABLE"

```bash
# Check KVM module
lsmod | grep kvm

# Load if missing
sudo modprobe kvm
sudo modprobe kvm_intel  # or kvm_amd

# Check BIOS settings - ensure VT-x/AMD-V is enabled
```

#### "CLOUD_HV_NOT_FOUND"

```bash
# Verify cloud-hypervisor is in PATH
which cloud-hypervisor

# Check version
cloud-hypervisor --version

# Update config if installed elsewhere
# Edit /etc/chv/agent.yaml:
# cloud_hypervisor_path: "/custom/path/cloud-hypervisor"
```

### Controller Issues

#### Database Connection Failed

```bash
# Test PostgreSQL connection
psql -h localhost -U chv -d chv

# Check PostgreSQL is running
sudo systemctl status postgresql

# Verify database exists
sudo -u postgres psql -c "\l" | grep chv

# Check pg_hba.conf for authentication settings
sudo cat /etc/postgresql/16/main/pg_hba.conf | grep -v "^#"
```

#### Port Already in Use

```bash
# Find process using port 8080
sudo lsof -i :8080

# Change port in config
# Edit /etc/chv/controller.yaml:
# http_addr: ":8081"
```

### Network Issues

#### Agents Can't Connect to Controller

```bash
# Test connectivity from agent node
telnet controller-ip 9090

# Check firewall
sudo iptables -L -n | grep 9090
sudo firewall-cmd --list-ports

# Verify gRPC is listening on correct interface
ss -tlnp | grep 9090
# Should show 0.0.0.0:9090 or specific IP, not 127.0.0.1:9090
```

### Logs and Debugging

```bash
# View agent logs
sudo journalctl -u chv-agent -n 100 --no-pager

# View controller logs
sudo journalctl -u chv-controller -n 100 --no-pager

# Enable debug logging
# Edit config and set: log_level: "debug"
sudo systemctl restart chv-agent  # or chv-controller

# Follow logs in real-time
sudo journalctl -u chv-agent -f
```

---

## Next Steps

- Read the [API Documentation](API.md) for complete API reference
- See [Development Guide](../DEVELOPMENT.md) for development setup
- Review [Architecture Documentation](architecture/mvp1-architecture.md) for system design

---

## Reference

### File Locations

| Component | Path |
|-----------|------|
| Cloud Hypervisor | `/usr/local/bin/cloud-hypervisor` |
| CHV Agent | `/usr/local/bin/chv-agent` |
| CHV Controller | `/usr/local/bin/chv-controller` |
| Agent Config | `/etc/chv/agent.yaml` |
| Controller Config | `/etc/chv/controller.yaml` |
| Data Directory | `/var/lib/chv/` |
| Images | `/var/lib/chv/images/` |
| Volumes | `/var/lib/chv/volumes/` |
| Kernels | `/var/lib/chv/kernels/` |
| Logs | `/var/log/chv/` |
| Systemd (Agent) | `/etc/systemd/system/chv-agent.service` |
| Systemd (Controller) | `/etc/systemd/system/chv-controller.service` |

### Default Ports

| Service | Port | Protocol |
|---------|------|----------|
| Controller HTTP | 8080 | TCP |
| Controller gRPC | 9090 | TCP |
| Agent gRPC | 9090 | TCP |
| PostgreSQL | 5432 | TCP |

### Environment Variables

#### Agent

| Variable | Description | Default |
|----------|-------------|---------|
| `CHV_NODE_ID` | Unique node identifier | Auto-generated |
| `CHV_CONTROLLER_ADDR` | Controller gRPC address | `localhost:9090` |
| `CHV_LISTEN_ADDR` | Agent listen address | `:9090` |
| `CHV_DATA_DIR` | Data directory | `/var/lib/chv` |
| `CHV_IMAGE_DIR` | Image storage path | `/var/lib/chv/images` |
| `CHV_VOLUME_DIR` | Volume storage path | `/var/lib/chv/volumes` |
| `CHV_CLOUD_HV_PATH` | Cloud Hypervisor binary path | `/usr/local/bin/cloud-hypervisor` |
| `CHV_HEARTBEAT_INTERVAL` | Heartbeat interval | `30s` |

#### Controller

| Variable | Description | Default |
|----------|-------------|---------|
| `CHV_DATABASE_URL` | PostgreSQL connection string | - |
| `CHV_HTTP_ADDR` | HTTP API bind address | `:8080` |
| `CHV_GRPC_ADDR` | gRPC bind address | `:9090` |
| `CHV_LOG_LEVEL` | Logging level | `info` |
