# ADR-026: Optional guest monitoring agent and identity

**Date:** 2026-10-09  
**Status:** Proposed  
**Authority:** ADR-016, ADR-017, ADR-025

## Context

Native hypervisor statistics cannot reveal every guest filesystem, process, systemd service, or application condition. A lightweight optional agent can provide Checkmk-style discovery and checks without requiring a Checkmk installation. It must not duplicate `chv-agent` VM authority.

Guest-originated data is untrusted. A VM identifier in a client payload is not identity proof. A virtio-vsock connection identifier alone is also insufficient authentication. Agents can run in customer-controlled VMs, including guests with a different privilege and tenancy boundary from the node.

## Decision

1. Introduce an **optional** Rust executable and package named `chv-monitor-agent`. Its service name is `chv-monitor-agent.service`.
2. Default installation has **no guest agent**. Native host-observed metrics remain available. The guest agent is explicitly installed by a user or via an opt-in image/provisioning path.
3. The first qualified agent platform is Linux x86-64. Linux ARM64 follows a separate packaging gate. Windows remains a future contract extension, not a current implementation claim.
4. The agent collects node-independent guest metrics: CPU, load, memory/swap, filesystems, inode use, disk/network interfaces, uptime, systemd, process checks, TCP/HTTP checks, and safe discovery.
5. An agent only reports observations. It cannot invoke VM create, start, stop, resize, migrate, or provider mutation APIs.
6. **HTTPS outbound is the v1 transport.** The agent makes authenticated requests to a scoped monitoring ingestion endpoint. No guest inbound port is required.
7. **Virtio-vsock is a later transport.** Cloud Hypervisor v53.0 provides vsock device support, but CHV must implement and qualify device lifecycle, host mapping, credentials, migration, and recovery first. Transport must not grant identity by itself.
8. Use a one-time, expiring enrollment claim **scoped to exactly one VM identity** or explicit external-node identity. Issue scoped, renewable client credentials after claim consumption. The server binds `agent_id`, project, target, credential, and revocation state. A guest cannot select another target in its metric payload.
9. Remote enrollment needs an authenticated operator action and an explicit delivery step. Tokens MUST NOT appear in logs, shell history examples, public boot metadata, URL query parameters, or UI history. A stolen bootstrap token remains a credential theft risk until expiry or consumption.
10. The monitoring agent runs as a non-root service by default. Any privileged collectors require separately reviewed, constrained helpers. It has no access to the Core Unix socket or VMM sockets.
11. Plugins are **disabled by default**. Local administrator-controlled plugins use an explicit allowlist, fixed executable path, manifest digest, controlled environment, timeout, output cap, and constrained user. No shell-interpreted command strings and no control-plane-originated arbitrary command execution.
12. The server treats agent checks and readings as untrusted inputs. It validates limits, timestamp skew, labels, schema, sample count, and tenant identity. A malformed or compromised agent cannot exhaust the manager.
13. Agent removal, credential revocation, reinstall, guest restart, VM migration, and VM deletion have explicit identity and history behavior. Removing or revoking an agent does not affect the VM.

## Identity state machine

```text
not_enrolled -> claim_issued -> enrolled -> active
                     |               |          |
                     +-> expired      +-> revoked|
                                                +-> stale -> active
                                                +-> revoked
```

An enrollment claim is single-use and expires after at most 10 minutes by default. Credentials rotate independently of VM lifecycles. The server deduplicates batches by `(agent_id, boot_id, sequence)` and checks authenticated identity (for node batches the first element is the authenticated node identity). Registry metadata may use the main control-plane database; time-series samples must not. The [security contract](../contracts/chv-monitor-agent-security-plugins-v1.md) defines the single wire/API state vocabulary: this diagram's `stale` is `offline` on the wire, `enrolled` is observed as `enrolling` → `active` on the first accepted batch, and implementations must use the wire vocabulary in APIs.

## Transport policy

| Mode | Default | Identity proof | Qualification |
|---|---|---|---|
| Outbound HTTPS | Yes for installed agent | Credential bound to target, server TLS validation | Must pass before agent GA |
| Guest-to-node vsock | No | Same credential plus trusted host-side VM/CID mapping | Separate opt-in phase |
| External Linux machine | No | Explicit external target registration | After guest agent qualification |
| Guest push to Prometheus | No | Not a CHV control-plane ingestion path | External integration only |

## Alternatives rejected

- Force a guest agent on every VM: breaks guest independence.
- Enable remote arbitrary script execution for monitoring: unsafe command execution boundary.
- Treat claimed `vm_id` or vsock CID as authentication: allows identity spoofing.
- Install Checkmk server by default: unnecessary product dependency.
- Manage monitoring-guest configuration through VM reconciliation writes: risks VM lifecycle authority.

## Acceptance

A malicious test guest must not impersonate a second VM, exfiltrate enrollment secrets, issue a VM lifecycle RPC, or create unbounded series. Uninstall and certificate revocation must work. Offline guests must report `stale`, not `healthy`. Host-only monitoring must work unchanged without the optional agent.

## Rationale

- Guest-originated data is untrusted by definition: a VM identifier or vsock CID in a payload is a claim, not identity, so identity must come from a manager-issued scoped credential bound at enrollment.
- Outbound HTTPS as the v1 transport: no guest inbound port, no new host-side attack surface, and it works on every qualified guest OS today; vsock is deferred until device lifecycle, CID reuse, and migration handling are qualified.
- Agent-generated keys with manager-issued certificates: the manager never generates, transmits, or holds a guest private key, removing an entire class of at-rest secret inventory on the manager.
- Plugins disabled by default behind local-administrator allowlists: remote execution of monitoring code is the classic monitoring-agent compromise path; this design refuses it.

## Consequences

- Enrollment is an operator action with a short-lived single-use claim — deliberate friction; bulk image deployment needs an explicit provisioning path, never a shared baked-in credential.
- The manager must maintain credential lifecycle (rotation, revocation, epochs, dedup windows) and treat cloned images as a detected-and-reset condition rather than a silent failure.
- Guest-derived metrics remain permanently second-class in trust: validated, bounded, and never authoritative for VM state.
- The vsock transport, when it lands, adds host-side ownership/CID mapping qualification obligations before it can be default-enabled.

See [agent spec](../component/chv-monitor-agent-spec.md), [agent security contract](../contracts/chv-monitor-agent-security-plugins-v1.md), and [ingestion contract](../contracts/chv-monitoring-ingestion-v1.md).
