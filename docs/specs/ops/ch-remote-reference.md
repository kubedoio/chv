# ch-remote CLI Reference

Version: v51.1
Binary: `ch-remote-static`
Download: https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v51.1/ch-remote-static
Install path: `/usr/local/bin/ch-remote`

## Usage

```
ch-remote --api-socket <socket_path> <COMMAND>
```

The `--api-socket` flag points to the UNIX domain socket created by cloud-hypervisor's `--api-socket` flag.

For CHV-managed VMs, the socket is at:
```
/var/lib/chv/agent/vms/{vm_id}/vm.sock
```

## Commands

| Command | Description | Example |
|---------|-------------|---------|
| `ping` | Check VMM API availability | `ch-remote --api-socket vm.sock ping` |
| `info` | Get VM state and configuration | `ch-remote --api-socket vm.sock info` |
| `boot` | Boot a created VM | `ch-remote --api-socket vm.sock boot` |
| `pause` | Pause the VM | `ch-remote --api-socket vm.sock pause` |
| `resume` | Resume a paused VM | `ch-remote --api-socket vm.sock resume` |
| `reboot` | Reboot the VM | `ch-remote --api-socket vm.sock reboot` |
| `shutdown` | Graceful VM shutdown | `ch-remote --api-socket vm.sock shutdown` |
| `shutdown-vmm` | Shutdown the VMM process | `ch-remote --api-socket vm.sock shutdown-vmm` |
| `power-button` | Send ACPI power button | `ch-remote --api-socket vm.sock power-button` |
| `delete` | Delete a VM | `ch-remote --api-socket vm.sock delete` |
| `resize` | Resize vCPUs/memory | `ch-remote --api-socket vm.sock resize --cpus 4 --memory 2G` |
| `resize-zone` | Resize a memory zone | `ch-remote --api-socket vm.sock resize-zone --id zone0 --size 1G` |
| `add-disk` | Hot-add a block device | `ch-remote --api-socket vm.sock add-disk path=/path/to/disk.img` |
| `add-net` | Hot-add a network device | `ch-remote --api-socket vm.sock add-net tap=tap0,mac=aa:bb:cc:dd:ee:ff` |
| `add-fs` | Hot-add a virtio-fs device | `ch-remote --api-socket vm.sock add-fs tag=myfs,socket=/path/to/virtiofsd.sock` |
| `add-pmem` | Hot-add persistent memory | `ch-remote --api-socket vm.sock add-pmem file=/path/to/pmem,size=1G` |
| `add-device` | Hot-add a VFIO device | `ch-remote --api-socket vm.sock add-device path=/sys/bus/pci/devices/0000:01:00.0` |
| `add-user-device` | Hot-add a userspace device | `ch-remote --api-socket vm.sock add-user-device socket=/path/to/socket` |
| `add-vdpa` | Hot-add a vDPA device | `ch-remote --api-socket vm.sock add-vdpa path=/dev/vhost-vdpa-0` |
| `add-vsock` | Hot-add a vsock device | `ch-remote --api-socket vm.sock add-vsock cid=3,socket=/path/to/vsock` |
| `remove-device` | Hot-remove a device | `ch-remote --api-socket vm.sock remove-device --id device_id` |
| `snapshot` | Create VM snapshot | `ch-remote --api-socket vm.sock snapshot file:///path/to/snapshot` |
| `restore` | Restore VM from snapshot | `ch-remote --api-socket vm.sock restore source_url=file:///path/to/snapshot` |
| `coredump` | Generate VM coredump | `ch-remote --api-socket vm.sock coredump file:///path/to/coredump` |
| `counters` | Get VM performance counters | `ch-remote --api-socket vm.sock counters` |
| `send-migration` | Initiate live migration | `ch-remote --api-socket vm.sock send-migration --url tcp://dest:port` |
| `receive-migration` | Receive live migration | `ch-remote --api-socket vm.sock receive-migration --url tcp://0.0.0.0:port` |
| `create` | Create VM from JSON config | `ch-remote --api-socket vm.sock create path/to/vm-config.json` |

## CHV Operations Examples

### Check if a VM is running
```bash
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock ping
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock info
```

### Snapshot and restore a VM
```bash
# Take snapshot (VM must be paused first for memory-consistent snapshot)
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock pause
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock snapshot file:///var/lib/chv/agent/vms/{vm_id}/snapshots/snap1
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock resume

# Restore from snapshot
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock restore source_url=file:///var/lib/chv/agent/vms/{vm_id}/snapshots/snap1
```

### Hot-add a disk to a running VM
```bash
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock add-disk path=/var/lib/chv/storage/localdisk/extra-disk.img,id=extra-disk
```

### Hot-add a network interface
```bash
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock add-net tap=tap-{nic_id},mac=aa:bb:cc:dd:ee:ff
```

### Resize a running VM
```bash
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock resize --cpus 4 --memory 4G
```

### Graceful shutdown vs force kill
```bash
# Graceful: sends ACPI shutdown signal to guest OS
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock power-button

# Forceful: immediately shuts down the VM
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock shutdown

# Kill the VMM process entirely
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock shutdown-vmm
```

### Debug: get performance counters
```bash
ch-remote --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock counters
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Connection refused` | VM not running or socket path wrong | Check VM status, verify socket exists |
| `No such file or directory` | API socket doesn't exist | VM process may have crashed; check `cloud-hypervisor.stderr.log` |
| `vm.boot returned non-success` | VM already booted | Use `info` to check current state |
| Snapshot fails | VM must be paused first for memory-consistent snapshots | Pause before snapshot |
