# ADR-0002: Use privileged bootstrap container, but host-native node runtime

## Status
Accepted

## Context
A privileged container can simplify installation and upgrades, but making the full hypervisor runtime depend on a long-lived privileged container adds debugging and operational indirection.

## Decision
Use:
- privileged bootstrap container for installation and upgrades
- host-native `chv-agent` systemd service
- host-native `cloud-hypervisor` processes for VMs

## Consequences
### Positive
- easier debugging of KVM, bridges, disks, and VM processes
- cleaner ownership model
- simpler host troubleshooting

### Negative
- slightly less "containerized" aesthetic
- more host integration code in installer
