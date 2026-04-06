# CloudHypervisor Configuration Examples

This directory contains example configurations for the CHV (Cloud Hypervisor Virtualization) platform. These examples demonstrate common use cases for creating and managing VMs with CloudHypervisor v51.1.

## Overview

CHV uses a controller-agent architecture where:
- **Controller** manages VM scheduling and state
- **Agent** runs on each host and controls CloudHypervisor processes via REST API over Unix sockets

## Quick Start

### 1. Prerequisites

- Linux host with KVM support
- CloudHypervisor v51.1 binary (`/usr/bin/cloud-hypervisor`)
- CHV Agent running on the host
- CHV Controller (for API access)

### 2. Basic VM Creation

```bash
# Create a VM using the controller API
curl -X POST http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d @vm-basic.json
```

## Example Files

### 1. `vm-basic.yaml`
**Basic VM configuration**

- 2 vCPUs (up to 4 with hot-plug)
- 4GB RAM
- Single virtio-blk disk
- Bridge networking

**Use case:** General purpose VMs, testing, development workloads

```bash
# See the file for JSON API request format
cat vm-basic.yaml | grep -A 30 "API Request Example"
```

### 2. `vm-ubuntu.yaml`
**Ubuntu cloud image with cloud-init**

- 4 vCPUs, 8GB RAM
- Ubuntu 22.04 cloud image
- Cloud-init provisioning (user-data, meta-data, network-config)
- SSH key injection

**Use case:** Production Ubuntu VMs with automated provisioning

**Setup:**
```bash
# Download Ubuntu cloud image
wget https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img

# Convert to raw format
qemu-img convert -f qcow2 -O raw jammy-server-cloudimg-amd64.img ubuntu-22.04.raw

# Create cloud-init ISO (see file for user-data example)
```

### 3. `vm-bridge.yaml`
**VM with Linux bridge networking**

- Multi-VM network communication
- External network access
- Optional DHCP with dnsmasq
- Network performance tuning (TSO, GSO, GRO)

**Use case:** Production environments requiring network isolation and connectivity

**Host setup:**
```bash
# Run the included setup script
sudo ./examples/setup-bridge.sh all
```

### 4. `vm-console.yaml`
**VM with serial console enabled**

- Interactive console access
- WebSocket-based browser console
- Kernel message capture
- File-based logging option

**Use case:** Debugging, troubleshooting, development

**Access console:**
```bash
# Via WebSocket
wscat -c "ws://agent-host:8081/vms/{vm-id}/console?token=YOUR_TOKEN"

# Or use browser-based console via CHV UI
```

### 5. `agent-config.yaml`
**Complete agent configuration reference**

All configuration options for the chv-agent including:
- Controller connection settings
- Data directories
- CloudHypervisor integration
- Networking configuration
- Security settings
- Systemd service file

**Use case:** Production agent deployment, customization

## Common Operations

### Creating a VM

```bash
# 1. Import an image first
curl -X POST http://localhost:8080/api/v1/images/import \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "ubuntu-22.04",
    "os_family": "ubuntu",
    "source_url": "https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img",
    "source_format": "qcow2",
    "architecture": "x86_64"
  }'

# 2. Create the VM
curl -X POST http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-vm",
    "cpu": 2,
    "memory_mb": 4096,
    "image_id": "IMAGE_ID_FROM_STEP_1",
    "disk_size_bytes": 10737418240,
    "networks": [{"network_id": "NETWORK_ID"}]
  }'

# 3. Start the VM
curl -X POST http://localhost:8080/api/v1/vms/{vm-id}/start \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### Accessing CloudHypervisor API Directly

```bash
# Get VM info via CloudHypervisor REST API
SOCKET="/var/run/chv/{vm-id}.sock"
curl --unix-socket $SOCKET http://localhost/api/v1/vm.info

# Pause VM
curl -X PUT --unix-socket $SOCKET http://localhost/api/v1/vm.pause

# Resume VM
curl -X PUT --unix-socket $SOCKET http://localhost/api/v1/vm.resume

# Graceful shutdown
curl -X PUT --unix-socket $SOCKET \
  -H "Content-Type: application/json" \
  -d '{"mode": "PowerOff"}' \
  http://localhost/api/v1/vm.shutdown
```

### Disk Management

```bash
# Create a raw disk image
qemu-img create -f raw /var/lib/chv/volumes/my-disk.raw 20G

# Resize a disk (VM must be stopped)
qemu-img resize /var/lib/chv/volumes/my-disk.raw +10G

# Convert qcow2 to raw
qemu-img convert -f qcow2 -O raw input.qcow2 output.raw

# Create copy-on-write overlay
qemu-img create -f raw -b base.raw -F raw overlay.raw
```

### Network Configuration

```bash
# Create Linux bridge
sudo ip link add name br0 type bridge
sudo ip addr add 192.168.100.1/24 dev br0
sudo ip link set br0 up

# Create TAP device and add to bridge
sudo ip tuntap add mode tap tap0
sudo ip link set tap0 master br0
sudo ip link set tap0 up

# Enable IP forwarding
sudo sysctl -w net.ipv4.ip_forward=1

# Add NAT for external access
sudo iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE
```

## Troubleshooting

### VM Won't Start

1. **Check CloudHypervisor binary:**
   ```bash
   which cloud-hypervisor
   cloud-hypervisor --version
   ```

2. **Verify KVM support:**
   ```bash
   ls -la /dev/kvm
   kvm-ok  # or: ls /dev/kvm
   ```

3. **Check agent logs:**
   ```bash
   journalctl -u chv-agent -f
   ```

4. **Verify disk image:**
   ```bash
   qemu-img info /path/to/disk.raw
   ```

### No Network Connectivity

1. **Check bridge configuration:**
   ```bash
   ip link show br0
   bridge link show
   ```

2. **Verify TAP device:**
   ```bash
   ip tuntap show
   ip link show tap0
   ```

3. **Check IP forwarding:**
   ```bash
   sysctl net.ipv4.ip_forward
   ```

4. **Verify firewall rules:**
   ```bash
   iptables -t nat -L -v -n
   iptables -L FORWARD -v -n
   ```

### Console Access Issues

1. **Check agent HTTP server:**
   ```bash
   curl http://localhost:8081/health
   ```

2. **Verify WebSocket endpoint:**
   ```bash
   curl -i -N \
     -H "Connection: Upgrade" \
     -H "Upgrade: websocket" \
     -H "Host: localhost:8081" \
     -H "Origin: http://localhost:8081" \
     http://localhost:8081/vms/{vm-id}/console
   ```

## Configuration Reference

### VM Spec Fields

| Field | Type | Description |
|-------|------|-------------|
| `cpu.boot_vcpus` | int | Number of vCPUs at boot |
| `cpu.max_vcpus` | int | Maximum vCPUs (for hot-plug) |
| `memory.size` | string | Memory size (e.g., "4096M", "8G") |
| `memory.hotplug_size` | string | Max memory with hot-plug |
| `boot.mode` | string | `cloud_image`, `direct_kernel`, `uefi` |
| `disks[].path` | string | Path to disk image |
| `disks[].bus` | string | `virtio-blk` or `virtio-scsi` |
| `networks[].tap` | string | TAP device name |
| `networks[].mac` | string | MAC address |
| `console.mode` | string | `off`, `tty`, `file` |
| `serial.mode` | string | `off`, `tty`, `file` |
| `api_socket.path` | string | Unix socket path |

### CloudHypervisor API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/vm.info` | VM status and config |
| PUT | `/api/v1/vm.create` | Create VM |
| PUT | `/api/v1/vm.boot` | Boot VM |
| PUT | `/api/v1/vm.shutdown` | Shutdown VM |
| PUT | `/api/v1/vm.pause` | Pause VM |
| PUT | `/api/v1/vm.resume` | Resume VM |
| GET | `/api/v1/vm.counters` | Performance counters |
| PUT | `/api/v1/vm.resize` | Resize VM (CPU/memory) |

## Resources

- [CloudHypervisor Documentation](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/README.md)
- [CloudHypervisor REST API](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/api.md)
- [cloud-init Documentation](https://cloudinit.readthedocs.io/)
- [CHV Project README](../README.md)

## Contributing

When adding new examples:
1. Include full configuration spec
2. Add comments explaining each field
3. Document required resources
4. Provide usage instructions
5. Include troubleshooting tips

## License

These examples are provided under the same license as the CHV project (MIT).
