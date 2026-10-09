# Prompt 06 — Optional vsock, scale and production qualification (G5)

Prerequisites: G1–G4 PASS. Follow [ADR-026](../../specs/adr/026-optional-monitor-agent-and-guest-identity.md), [agent security contract](../../specs/contracts/chv-monitor-agent-security-plugins-v1.md), and [plan](../../plans/2026-10-09-native-monitoring-implementation.md).

## Goal

Allow a Linux guest to provide enhanced monitoring without a guest management-network dependency, using qualified Cloud Hypervisor (the VMM) virtio-vsock transport. Validate system resource costs and failure isolation. Never promote optional vsock to required until acceptance evidence exists.

## Design and implementation

1. Verify virtio-vsock support against the repository's pinned VMM version and exact release. Use the pinned OpenAPI and actual KVM process, not latest-only examples.
2. Define a feature-gated VM vsock device contract in existing VM configuration and Core authority. Do not silently attach a vsock device to all VMs or reinterpret boot mode. CID allocation, uniqueness, lifecycle and restart ownership must be explicit.
3. `chv-agent` retains control of host-side vsock/Unix socket binding. The monitor transport is read-only and isolated from VMM/control sockets. Privilege, quotas and input validation apply to all messages.
4. Map guest CID to the actual fenced VMM instance and authorized stable VM identity. Present the same enrolled guest credential from HTTPS v1. A CID, peer address or socket path by itself is never authentication.
5. Define guest behavior for transport selection: HTTPS remains supported; vsock is opt-in; switching preserves identity, replay protection, last accepted sequence and monitoring history.
6. Test migration: old host/CID no longer owns guest telemetry. Recipient mapping becomes valid only after Core ownership and process identity are proven. Do not start monitoring mutation paths in a transient dual-owner window.
7. Qualify boot/reboot/pause/resume/migration/stop/delete, guest agent restart, node restart, device detach and socket cleanup with real KVM. Where VM migration itself remains unqualified, report the vsock migration leg BLOCKED rather than substitute a mock.
8. Add multi-node monitoring only after platform networking and Core operation authority have separate qualification. No monitoring-specific path may mask Core's existing remote-dispatch limitations.

## Scale and chaos qualification

Run reproducible 1/10/100/500 VM campaigns with and without optional agents, using actual metric samples and check inventory. Record node hardware, kernel, VMM binary SHA/version, agent build SHAs, CPU time, memory RSS, network bytes, ingestion throughput, SQLite/WAL size, historical query p50/p95/p99, and UI responsiveness. Vary sampling interval, 1/5/20 metrics per VM, guest discovery count and alert count.

Inject: loss of manager, loss of guest transport, dropped/reordered batches, stolen/revoked credential, partial disk full, monitoring DB corruption, slow exporter, flooding guest, plugin timeout, network partition, CPU contention, VM ID/CID reuse and producer restart.

## Release outcome

- G5 passes only for tested topology and versions.
- Produce upgrade/rollback and disaster-recovery runbooks.
- Classify each capability as `implemented`, `qualified`, `blocked`, or `unsupported`, with explicit evidence.
- Default deployment remains single binary/service Core runtime plus existing providers/control plane; optional guest executable is installed only by choice.
- Attach a final product demonstration: node and VM native charts, guest check dashboard, persistent alerts, opt-in vsock (if qualified), and optional monitoring export.

No performance, safety, migration, or availability claim is accepted without exact real-host test evidence. End with a PASS/FAIL/BLOCKED matrix and remaining PRs rather than an unsupported success statement.
