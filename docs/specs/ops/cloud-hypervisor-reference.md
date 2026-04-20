# Cloud Hypervisor Reference

Version: v51.1
Source of truth: [Cloud Hypervisor API docs](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/api.md)
OpenAPI spec: https://raw.githubusercontent.com/cloud-hypervisor/cloud-hypervisor/master/vmm/src/api/openapi/cloud-hypervisor.yaml

## Binaries

| Binary | Download | Install path |
|--------|----------|-------------|
| `cloud-hypervisor-static` | https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v51.1/cloud-hypervisor-static | `/usr/local/bin/cloud-hypervisor` |
| `ch-remote-static` | https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v51.1/ch-remote-static | `/usr/local/bin/ch-remote` |

## Architecture

The CLI can only be used for **launching** cloud-hypervisor. Once running, all
control goes through the REST API (on the UNIX socket) or ch-remote (which wraps
the REST API). The CLI cannot control a running VM.

```
              REST API                    Internal API
User ------> micro_http ----+
                             |
User ------> ch-remote -----+----------> VMM control loop
                             |
CLI  ------> clap ----------+
```

## CLI Options

```
cloud-hypervisor [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--cpus <cpus>` | `boot=<boot_vcpus>,max=<max_vcpus>,topology=<t>:<c>:<d>:<p>,kvm_hyperv=on\|off,max_phys_bits=<n>,affinity=<list>,features=<list>` | `boot=1,max_phys_bits=46` |
| `--platform <platform>` | `num_pci_segments=<n>,iommu_segments=<list>,serial_number=<sn>,uuid=<uuid>,oem_strings=<list>` | |
| `--memory <memory>` | `size=<size>,mergeable=on\|off,shared=on\|off,hugepages=on\|off,hugepage_size=<size>,hotplug_method=acpi\|virtio-mem,hotplug_size=<size>,hotplugged_size=<size>,prefault=on\|off,thp=on\|off` | `size=512M` |
| `--memory-zone <zone>` | Per-zone: `size=<size>,file=<path>,shared=on\|off,hugepages=on\|off,host_numa_node=<id>,id=<id>,hotplug_size=<size>,prefault=on\|off` (repeatable) | |
| `--firmware <firmware>` | Path to firmware (architecture-specific load, e.g. CLOUDHV.fd) | |
| `--kernel <kernel>` | Path to kernel or PVH-capable firmware (e.g. vmlinux) | |
| `--initramfs <initramfs>` | Path to initramfs image | |
| `--cmdline <cmdline>` | Kernel command line | |
| `--disk <disk>` | `path=<path>,readonly=on\|off,direct=on\|off,iommu=on\|off,num_queues=<n>,queue_size=<n>,vhost_user=on\|off,socket=<path>,bw_size=<bytes>,bw_one_time_burst=<bytes>,bw_refill_time=<ms>,ops_size=<io_ops>,ops_one_time_burst=<io_ops>,ops_refill_time=<ms>,id=<device_id>,pci_segment=<id>` (repeatable) | |
| `--net <net>` | `tap=<name>,ip=<ip>,mask=<mask>,mac=<mac>,fd=<fds>,iommu=on\|off,num_queues=<n>,queue_size=<n>,id=<id>,vhost_user=<bool>,socket=<path>,vhost_mode=client\|server,pci_segment=<id>,offload_tso=on\|off,offload_ufo=on\|off,offload_csum=on\|off` (repeatable) | |
| `--rng <rng>` | `src=<path>,iommu=on\|off` | `src=/dev/urandom` |
| `--balloon <balloon>` | `size=<size>,deflate_on_oom=on\|off,free_page_reporting=on\|off` | |
| `--fs <fs>` | virtio-fs: `tag=<tag>,socket=<path>,num_queues=<n>,queue_size=<n>,id=<id>,pci_segment=<id>` (repeatable) | |
| `--pmem <pmem>` | `file=<path>,size=<size>,iommu=on\|off,discard_writes=on\|off,id=<id>,pci_segment=<id>` (repeatable) | |
| `--serial <serial>` | `off\|null\|pty\|tty\|file=<path>\|socket=<path>` | `null` |
| `--console <console>` | `off\|null\|pty\|tty\|file=<path>,iommu=on\|off` | `tty` |
| `--device <device>` | VFIO passthrough: `path=<path>,iommu=on\|off,id=<id>,pci_segment=<id>` (repeatable) | |
| `--user-device <dev>` | Userspace device: `socket=<path>,id=<id>,pci_segment=<id>` (repeatable) | |
| `--vdpa <vdpa>` | `path=<path>,num_queues=<n>,iommu=on\|off,id=<id>,pci_segment=<id>` (repeatable) | |
| `--vsock <vsock>` | `cid=<cid>,socket=<path>,iommu=on\|off,id=<id>,pci_segment=<id>` | |
| `--pvpanic` | Enable pvpanic device | |
| `--numa <numa>` | `guest_numa_id=<id>,cpus=<ids>,distances=<list>,memory_zones=<list>,sgx_epc_sections=<list>,pci_segments=<list>` (repeatable) | |
| `--watchdog` | Enable virtio-watchdog | |
| `-v` | Increase debug output level (repeatable) | |
| `--log-file <path>` | Log file path (stderr if not set) | |
| `--api-socket <socket>` | HTTP API socket (UNIX domain): `path=<path>` or `fd=<fd>` | |
| `--event-monitor <monitor>` | Event file: `path=<path>` or `fd=<fd>` | |
| `--restore <restore>` | Restore from snapshot: `source_url=<url>,prefault=on\|off` | |
| `--seccomp <mode>` | `true\|false\|log` | `true` |
| `--tpm <tpm>` | TPM device: `socket=<path>` | |
| `--sgx-epc <epc>` | SGX EPC: `id=<id>,size=<size>,prefault=on\|off` (repeatable) | |

**The CLI does NOT have `--user-data` or any cloud-init flag.** Cloud-init userdata must be injected via a seed disk image.

## REST API

Available as soon as cloud-hypervisor starts, on the UNIX socket from `--api-socket`.

### VMM Actions

| Action | Endpoint | Request Body | Response Body | Prerequisites |
|--------|----------|-------------|---------------|---------------|
| Ping | `/vmm.ping` | N/A | `VmmPingResponse` | N/A |
| Shutdown VMM | `/vmm.shutdown` | N/A | N/A | VMM running |

### VM Actions

| Action | Endpoint | Request Body | Response Body | Prerequisites |
|--------|----------|-------------|---------------|---------------|
| Create VM | `/vm.create` | `VmConfig` | N/A | VM not created yet |
| Delete VM | `/vm.delete` | N/A | N/A | N/A |
| Boot VM | `/vm.boot` | N/A | N/A | Created but not booted |
| Shutdown VM | `/vm.shutdown` | N/A | N/A | VM booted |
| Reboot VM | `/vm.reboot` | N/A | N/A | VM booted |
| Power button | `/vm.power-button` | N/A | N/A | VM booted |
| Pause VM | `/vm.pause` | N/A | N/A | VM booted |
| Resume VM | `/vm.resume` | N/A | N/A | VM paused |
| Snapshot VM | `/vm.snapshot` | `VmSnapshotConfig` | N/A | **VM paused** |
| Coredump VM | `/vm.coredump` | `VmCoredumpData` | N/A | **VM paused** (x86_64 + guest_debug only) |
| Restore VM | `/vm.restore` | `RestoreConfig` | N/A | Created but not booted |
| Resize VM | `/vm.resize` | `VmResize` | N/A | VM booted |
| Resize disk | `/vm.resize-disk` | `VmResizeDisk` | N/A | VM created |
| Resize memory zone | `/vm.resize-zone` | `VmResizeZone` | N/A | VM booted |
| VM info | `/vm.info` | N/A | `VmInfo` | VM created |
| VM counters | `/vm.counters` | N/A | `VmCounters` | VM booted |
| Inject NMI | `/vm.nmi` | N/A | N/A | VM booted |
| Add VFIO device | `/vm.add-device` | `VmAddDevice` | `PciDeviceInfo` | VM booted |
| Add disk | `/vm.add-disk` | `DiskConfig` | `PciDeviceInfo` | VM booted |
| Add fs | `/vm.add-fs` | `FsConfig` | `PciDeviceInfo` | VM booted |
| Add pmem | `/vm.add-pmem` | `PmemConfig` | `PciDeviceInfo` | VM booted |
| Add net | `/vm.add-net` | `NetConfig` | `PciDeviceInfo` | VM booted |
| Add user device | `/vm.add-user-device` | `VmAddUserDevice` | `PciDeviceInfo` | VM booted |
| Add vdpa | `/vm.add-vdpa` | `VdpaConfig` | `PciDeviceInfo` | VM booted |
| Add vsock | `/vm.add-vsock` | `VsockConfig` | `PciDeviceInfo` | VM booted |
| Add generic vhost-user | `/vm.add-generic-vhost-user` | `GenericVhostUserConfig` | `PciDeviceInfo` | VM booted |
| Remove device | `/vm.remove-device` | `VmRemoveDevice` | N/A | VM booted |
| Receive migration | `/vm.receive-migration` | `ReceiveMigrationData` | N/A | N/A |
| Send migration | `/vm.send-migration` | `SendMigrationData` | N/A | VM booted + shared mem or hugepages |

### REST API Examples

Create a VM:
```bash
curl --unix-socket /tmp/ch.sock -i \
     -X PUT 'http://localhost/api/v1/vm.create' \
     -H 'Accept: application/json' \
     -H 'Content-Type: application/json' \
     -d '{
         "cpus":{"boot_vcpus": 4, "max_vcpus": 4},
         "payload":{"kernel":"/opt/clh/kernel/vmlinux", "cmdline":"console=ttyS0 root=/dev/vda1 rw"},
         "disks":[{"path":"/opt/clh/images/focal.raw"}],
         "rng":{"src":"/dev/urandom"},
         "net":[{"ip":"192.168.10.10", "mask":"255.255.255.0", "mac":"12:34:56:78:90:01"}]
     }'
```

Boot, info, reboot, shutdown:
```bash
curl --unix-socket /tmp/ch.sock -i -X PUT 'http://localhost/api/v1/vm.boot'
curl --unix-socket /tmp/ch.sock -i -X GET 'http://localhost/api/v1/vm.info'
curl --unix-socket /tmp/ch.sock -i -X PUT 'http://localhost/api/v1/vm.reboot'
curl --unix-socket /tmp/ch.sock -i -X PUT 'http://localhost/api/v1/vm.shutdown'
```

## CHV Usage Pattern

CHV spawns cloud-hypervisor with these standard flags:

```bash
cloud-hypervisor \
  --api-socket /var/lib/chv/agent/vms/{vm_id}/vm.sock \
  --cpus boot={cpus} \
  --memory size={memory_bytes} \
  --firmware /var/lib/chv/hypervisor-fw \
  --disk path={disk_path} \
  --net mac={mac},tap={tap_name} \
  --console off \
  --serial tty={pty_slave_path}
```

After spawning, the VM is controlled entirely via the HTTP API on the UNIX socket.
The CHV agent uses direct HTTP requests; operators can use `ch-remote` for manual interaction.

## Important Notes for CHV Development

1. The CLI is for **launching only**. All runtime control uses the REST API.
2. There is **no `--user-data` flag**. Cloud-init must use a seed disk.
3. `--firmware` is for UEFI boot (CLOUDHV.fd); `--kernel` is for direct kernel boot (vmlinux).
4. `--net` requires a pre-created tap device (nwd creates these via `ip tuntap add`).
5. `--api-socket` creates the UNIX socket used by ch-remote and the CHV agent.
6. **Snapshots require the VM to be paused first** (`/vm.pause` then `/vm.snapshot`).
7. **Restore requires the VM to be in Created-but-not-booted state** (`/vm.restore`).
8. VM coredump is x86_64-only and requires the `guest_debug` feature.
