# MVP-2: CloudHypervisor Integration Design

## Overview

Integrate CloudHypervisor (CH) as the primary hypervisor for CHV. The agent will communicate with CH via its HTTP REST API to manage VM lifecycles.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CHV Controller                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   API       │  │  Scheduler  │  │   VM Lifecycle          │  │
│  │   Layer     │◄─┤   Service   │◄─┤   Management Service    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                              │                   │
└──────────────────────────────────────────────┼──────────────────┘
                                               │ gRPC / HTTP
                                               │
┌──────────────────────────────────────────────┼──────────────────┐
│                    CHV Agent (per node)      │                  │
│  ┌───────────────────────────────────────────┘                  │
│  │  ┌────────────────────────────────────────────────────────┐  │
│  │  │         CloudHypervisor REST API (local)               │  │
│  │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐             │  │
│  │  │  │  VM API  │  │  VMM API │  │  Event   │             │  │
│  │  │  │          │  │          │  │  Source  │             │  │
│  │  │  └──────────┘  └──────────┘  └──────────┘             │  │
│  │  └────────────────────────────────────────────────────────┘  │
│  │                        │                                     │
│  │  ┌──────────────┐     │     ┌──────────────┐               │
│  │  │  VM Process  │◄────┘────►│  Cloud       │               │
│  │  │  (ch)        │           │  Hypervisor  │               │
│  │  └──────────────┘           │  Binary      │               │
│  │                             └──────────────┘               │
│  └─────────────────────────────────────────────────────────────┘
```

## CloudHypervisor REST API Mapping

### VM Lifecycle Operations

| CHV Operation | CH API Endpoint | Method | Payload |
|--------------|-----------------|--------|---------|
| Create VM | `/vm.create` | PUT | VM config JSON |
| Delete VM | `/vm.shutdown` + `/vm.delete` | PUT + PUT | - |
| Start VM | `/vm.boot` | PUT | - |
| Stop VM | `/vm.shutdown` | PUT | - |
| Reboot VM | `/vm.reboot` | PUT | - |
| Pause VM | `/vm.pause` | PUT | - |
| Resume VM | `/vm.resume` | PUT | - |

### VM Configuration (vm.create payload)

```json
{
  "cpus": {
    "boot_vcpus": 2,
    "max_vcpus": 4
  },
  "memory": {
    "size": 2147483648,
    "hotplug_method": "Acpi",
    "hotplug_size": null,
    "mergeable": false,
    "shared": false,
    "hugepages": false,
    "hugepage_size": null
  },
  "kernel": {
    "path": "/var/lib/chv/vmlinux"
  },
  "cmdline": {
    "args": "console=hvc0 root=/dev/vda1 rw"
  },
  "disks": [
    {
      "path": "/var/lib/chv/disks/vm-disk.raw",
      "readonly": false,
      "direct": false,
      "iommu": false
    }
  ],
  "net": [
    {
      "tap": "tap0",
      "mac": "AA:BB:CC:DD:EE:FF",
      "iommu": false
    }
  ],
  "console": {
    "mode": "File",
    "file": "/var/log/chv/vm-console.log"
  },
  "serial": {
    "mode": "File",
    "file": "/var/log/chv/vm-serial.log"
  },
  "payload": {
    "firmware": null
  }
}
```

### Console Access

| Feature | CH API | Implementation |
|---------|--------|----------------|
| Serial Console | File-based | Agent tails log file, streams via WebSocket |
| VGA Console | Not in MVP-2 | Future enhancement |

### Metrics Collection

| Metric | CH API Endpoint |
|--------|-----------------|
| VM Info | `/vm.info` |
| VM Counters | `/vm.counters` |
| VMM Ping | `/vmm.ping` |

## Agent Architecture

### Components

1. **CH Client** - HTTP client for CH REST API
2. **VM Manager** - Manages VM lifecycle
3. **Console Proxy** - WebSocket proxy to CH console
4. **Metrics Collector** - Polls CH for stats
5. **gRPC Server** - Communicates with Controller

### File Structure

```
internal/agent/
├── ch/                  # CloudHypervisor client
│   ├── client.go       # HTTP client
│   ├── vm.go          # VM operations
│   ├── console.go     # Console handling
│   └── metrics.go     # Metrics collection
├── manager/
│   └── vm.go          # VM lifecycle manager
├── server/
│   └── grpc.go        # gRPC server
└── main.go            # Agent entry point
```

## Implementation Phases

### Phase 1: CH Client Foundation
- [ ] HTTP client for CH REST API
- [ ] VM create/delete/start/stop/reboot operations
- [ ] Error handling and retry logic

### Phase 2: VM Lifecycle Integration
- [ ] Connect agent to controller via gRPC
- [ ] Implement VM create flow (image → disk → CH config)
- [ ] Handle VM state transitions

### Phase 3: Console Access
- [ ] Serial console log tailing
- [ ] WebSocket proxy from controller
- [ ] WebUI console integration

### Phase 4: Metrics
- [ ] VM stats collection
- [ ] Resource usage reporting
- [ ] Health checks

## Cloud-init Integration

CloudHypervisor uses a special `cloud-init` device:

```json
{
  "disks": [
    {
      "path": "/var/lib/chv/cloud-init/cloud-init.iso",
      "readonly": true
    }
  ]
}
```

Agent responsibilities:
1. Generate cloud-init ISO from user-data/meta-data
2. Mount ISO as read-only disk
3. Clean up after VM deletion

## Security Considerations

- Agent runs as non-root user (with KVM group)
- CH Unix socket restricted to agent user
- Console logs readable only by agent
- No direct CH API exposure outside node

## Testing Strategy

1. Unit tests with CH mock server
2. Integration tests with real CH (requires KVM)
3. E2E tests with controller + agent + CH
