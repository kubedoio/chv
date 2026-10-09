# Prompt 03 — Optional Linux agent and authenticated enrollment (G3)

Prerequisite: G2 passed. Implement [ADR-026](../../specs/adr/026-optional-monitor-agent-and-guest-identity.md), [agent spec](../../specs/component/chv-monitor-agent-spec.md), [ingestion v1](../../specs/contracts/chv-monitoring-ingestion-v1.md), and [agent security/plugin contract](../../specs/contracts/chv-monitor-agent-security-plugins-v1.md).

## Goal

Ship `chv-monitor-agent` as a separate opt-in package for Linux guests. Implement HTTPS outbound identity, enrollment, scoped ingestion, rotation/revocation, and baseline host-in-guest collectors. No remote script execution or VM authority change.

## Tasks

1. Add optional `cmd/chv-monitor-agent` executable and testable `crates/chv-monitor-collectors` library; make package release explicit and feature/off-by-default. Do not accidentally install it with the node.
2. Add a dedicated client identity and agent registry in durable manager metadata storage. Add operator-authorized issue/expire/consume/revoke/rotate endpoints scoped to a VM/project, with hashed single-use claims and full audit.
3. Build outbound HTTPS transport with authenticated server trust, scoped credentials, bounded retry/queue/spool and clock/sequence behavior. Do not trust `target_id` in self-reported data.
4. Implement server-side agent-auth ingestion separate from BFF browser-session auth. Enforce rate, size, series, source, tenant, VM identity and status. Replay/conflict behavior must follow contract.
5. Add read-only guest CPU, load, memory, uptime and OS version collectors. Guest memory must remain separately labeled from host-accounted VM memory.
6. Add safe enrollment and removal instructions in UI and `docs/install`, with security warnings for token distribution and VM images. Add last seen, credential status and data freshness views.
7. Add hardened systemd unit, permissions, deb/rpm/tarball rules and cross-architecture build matrix. Document unsupported systems instead of publishing nonworking packages.
8. Add explicit upgrades, credential rotation, cloned-image detection/re-registration and manager-disconnected behavior.

## Tests

- Real guest: install, claim, collect, stop/restart, manager offline, reconnect, revoke, purge and re-enroll.
- Security: expired/reused/stolen claim, invalid TLS, forged target/project, wrong credential, cloned VM, replay conflict, oversized batch, cardinality flood, agent faking VM status.
- Lifecycle: node continues VMs even if agent agent is removed/compromised.
- Packaging: no package installed by default; non-root restrictions verified in actual systemd.

## Gate

G3 PASS requires secure Linux guest telemetry on a real VM plus an unmodified host-only monitoring path for guests without the package. Record precise credential trust limitations; do not claim hardware attestation.
