# CHV Product Roadmap

This document outlines the phased development of the `chv` virtualization platform. We strictly adhere to a lean, minimalist philosophy to ensure stability before expanding feature sets.

## Stage 1: Consolidation & Foundation (Current)
*Focus: Stabilizing the underlying plumbing and architectural split.*
- [x] Separate `chv-controller` (API/State) from `chv-agent` (Host execution).
- [x] Implement opaque bearer token authentication.
- [x] Define default networking (`chvbr0`) and storage paths (`/var/lib/chv/`).
- [ ] Finalize SQLite schema with Write-Ahead Logging (WAL) for concurrency.
- [ ] Refine `install.sh` for reliable bare-metal bootstrapping.

## Stage 2: Core Stability (MVP-v1 Target)
*Focus: Reliable single-node VM lifecycle management.*
- [ ] **Agent Robustness:** Reliable Start, Stop, Hard Reset, and Destroy operations.
- [ ] **State Reconciliation:** Agent reports accurate VM states (Running, Stopped, Crashed) back to the controller.
- [ ] **Cloud-Init:** Reliable local generation and mounting of seed ISOs for user-data and network-config.
- [ ] **Operator Console:** SvelteKit UI fully wired to the Go API for managing Nodes, VMs, and API Tokens.
- [ ] **Security:** Basic `nftables` isolation rules managed per-VM by the agent.

## Stage 3: Telemetry & Management Polish
*Focus: Usability and operational visibility.*
- [ ] **Metrics:** Expose basic CPU/RAM utilization from Cloud Hypervisor to the Web UI.
- [ ] **Image Management:** UI/API endpoints to pull external `.qcow2` images into the local storage pool.
- [ ] **Visual Architecture:** Dashboard enhancements to conceptually visualize nodes, VMs, and active bridge connections.
- [ ] **Local Snapshots:** Basic non-live disk snapshots.

## Stage 4: Advanced Infrastructure (Post-MVP)
*Focus: Scaling beyond the single node and expanding OS support.*
- [ ] Multi-node controller support (polling multiple remote agents).
- [ ] Windows 10/Server support (Virtio driver injection).
- [ ] High Availability / Automated node failover.
- [ ] Distributed storage support plugins (Ceph/NFS).
