# Cloud-Init Metadata Service

This document describes the metadata service implementation for cloud-init network configuration.

## Overview

The metadata service provides an HTTP-based alternative to the cloud-init ISO for VM configuration. It follows the same API pattern as AWS/Azure/GCP Instance Metadata Service (IMDS).

## Problem Statement

When both a boot volume and a cloud-init ISO are attached to a VM, the firmware sometimes tries to boot from the ISO instead of the boot volume. This causes boot failures.

## Solution

The metadata service serves cloud-init configuration over HTTP. VMs fetch their configuration from the host via a link-local address (169.254.169.254), eliminating the need for an attached ISO.

## Architecture

```
┌─────────────┐      HTTP GET      ┌──────────────────┐
│             │ ─────────────────► │  Metadata Server │
│     VM      │  169.254.169.254   │  (Agent)         │
│             │ ◄───────────────── │                  │
└─────────────┘   cloud-init data  └──────────────────┘
```

## API Endpoints

The metadata service provides the following endpoints:

### GET /latest/meta-data/
Returns a list of available metadata items.

**Response:**
```
instance-id
local-hostname
```

### GET /latest/meta-data/instance-id
Returns the instance ID.

**Response:**
```
vm-abc123
```

### GET /latest/meta-data/hostname
Returns the hostname.

**Response:**
```
my-vm-name
```

### GET /latest/user-data
Returns the cloud-init user-data.

**Response:**
```yaml
#cloud-config
users:
  - name: admin
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - ssh-ed25519 AAAAC3NzaC1...
```

### GET /latest/network-config
Returns the network configuration in cloud-init v2 format.

**Response:**
```json
{
  "version": 2,
  "ethernets": {
    "eth0": {
      "dhcp4": true
    }
  }
}
```

## VM Identification

VMs are identified by:

1. **X-VM-ID header**: For testing or when the VM explicitly identifies itself
2. **Source IP address**: The metadata server maps VM IP addresses to VM IDs

## Implementation

### Server Initialization

The metadata server is initialized in `internal/agent/server/grpc.go`:

```go
metadataServer := metadata.NewServer()
if err := metadataServer.Start(); err != nil {
    log.Printf("Warning: failed to start metadata server: %v", err)
}
```

### VM Registration

When a VM is provisioned, it's registered with the metadata server in `internal/agent/server/vm_handlers.go`:

```go
if s.metadataServer != nil && cloudInitConfig != nil {
    metaConfig := &metadata.Config{
        InstanceID:    req.VmId,
        Hostname:      req.VmName,
        NetworkConfig: cloudInitConfig.NetworkConfig,
        UserData:      cloudInitConfig.UserData,
        MetaData:      cloudInitConfig.MetaData,
    }
    s.metadataServer.RegisterVM(req.VmId, metaConfig)
}
```

### VM Unregistration

When a VM is stopped or deleted, it's unregistered from the metadata server:

```go
if s.metadataServer != nil {
    s.metadataServer.UnregisterVM(req.VmId)
}
```

## Network Configuration

### VM Configuration

For VMs to use the metadata service, they need to be configured to query it. This can be done via:

1. **DHCP option** (recommended): Configure DHCP to provide metadata service info
2. **Kernel command line**: Add `ds=nocloud-net;seedfrom=http://169.254.169.254/` to the kernel parameters
3. **Static configuration**: Configure cloud-init in the image to use the metadata service

### Example: Kernel Command Line

```bash
cloud-hypervisor \
  --kernel vmlinux \
  --cmdline "console=ttyS0 ds=nocloud-net;seedfrom=http://169.254.169.254/" \
  --disk path=vm-disk.raw \
  ...
```

### Example: cloud-init Configuration

Add to `/etc/cloud/cloud.cfg.d/99-metadata-service.cfg`:

```yaml
datasource:
  NoCloudNet:
    seedfrom: http://169.254.169.254/
```

## Testing

### Unit Tests

Run the metadata server tests:

```bash
go test ./internal/agent/metadata/... -v
```

### Manual Testing

1. Start the agent (which starts the metadata server)
2. Provision a VM with cloud-init configuration
3. Query the metadata endpoint:

```bash
curl -H "X-VM-ID: <vm-id>" http://localhost:8080/latest/network-config
```

## Security Considerations

1. **Link-local only**: The metadata service listens on 169.254.169.254, which is only accessible from the host and VMs on that host
2. **IP-based identification**: VMs are identified by their IP address, preventing VMs from accessing other VMs' metadata
3. **Future enhancements**: Consider implementing IMDSv2-style session authentication for additional security

## Comparison: ISO vs Metadata Service

| Feature | ISO | Metadata Service |
|---------|-----|------------------|
| Boot order issues | Yes (can be booted from) | No (no ISO attached) |
| Requires extra disk | Yes | No |
| Dynamic updates | No (requires reboot) | Yes (can update in-place) |
| Cloud-init standard | Yes (NoCloud) | Yes (NoCloudNet) |
| Network required | No | Yes |

## Future Enhancements

1. **IMDSv2 authentication**: Implement session-based authentication similar to AWS IMDSv2
2. **Metadata versioning**: Support multiple API versions
3. **Additional endpoints**: Expose more metadata (tags, placement, etc.)
4. **IPv6 support**: Support IPv6 link-local addresses
