# Phase A2 Host Preflight

**Recorded:** 2026-07-21T00:30:00+02:00  
**Host use decision:** rejected  
**Evidence tier:** T0 environment inspection; no OpenStack probe executed

The current workspace host was inspected without installing packages or
changing services, networking, storage, or credentials.

## Observations

```text
/dev/kvm: present (character device, root:kvm)
virtualization: kvm
operating system: Ubuntu 24.04.4 LTS
memory: 7.8 GiB total, 5.5 GiB available
swap: 0 B
workspace filesystem: 156 GiB available
systemd state: degraded
Docker client: installed
Docker daemon: unavailable
virsh: unavailable
libvirtd: unavailable
virtqemud: unavailable
cloud-hypervisor: v51.1
```

The host has active interfaces and bridges that were not created for this
discovery, including `docker0`, multiple `br-*` devices, multiple `tap-*`
devices, and active `veth*` devices. It does not contain the required
`/etc/cellhv-test-host` disposable-lab marker.

## Decision

This host is not disposable and contains unrelated virtualization networking.
Installing or running DevStack could mutate those resources and would violate
the Phase A2 environment and cleanup boundary. The checked-in preflight tool
must reject this host until an infrastructure owner provides a dedicated
machine, the exact marker, pinned inputs, and disposable credentials.

No Nova connection, libvirt API, domain XML, Neutron, Cinder, or VM lifecycle
result was observed. In particular, this environment rejection is not the
OSD-001 first Nova/libvirt blocker and does not satisfy a T5 acceptance gate.

## Reproduction

```bash
test -c /dev/kvm
systemd-detect-virt
cat /etc/os-release
free -h
df -h /root/chv
systemctl is-system-running
docker info --format '{{.ServerVersion}}'
virsh --version
libvirtd --version
virtqemud --version
cloud-hypervisor --version
ip -brief address
test -r /etc/cellhv-test-host
```

Commands that are absent or cannot connect are expected to return non-zero.
