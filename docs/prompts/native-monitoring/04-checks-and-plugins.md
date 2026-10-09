# Prompt 04 — Linux guest discovery, checks, and safe plugins (G4 part 1)

Prerequisite: G3 passed. Follow [agent spec](../../specs/component/chv-monitor-agent-spec.md), [metrics v1](../../specs/contracts/chv-monitoring-metrics-v1.md), and [security/plugin contract](../../specs/contracts/chv-monitor-agent-security-plugins-v1.md).

## Goal

Add Checkmk-style OS and service visibility to the optional `chv-monitor-agent` without deploying Checkmk itself, granting root by default, collecting private data indiscriminately, or allowing remote commands.

## Tasks

1. Add read-only Linux filesystem monitoring: mount identity, size, available bytes, read-only state, inode utilization, and file system types. Reject path traversal and bound discovery.
2. Add Linux network monitoring: interface/link, byte counters, packet drops, RX/TX errors and TCP summary. Distinguish virtual, loopback, bridge and physical interfaces.
3. Add configurable systemd checks and bounded service discovery: `ok`, `warning`, `critical`, `unknown`; service not installed vs stopped vs not queried must be distinct.
4. Add optional process selectors by stable executable/name and aggregate count/CPU/memory. Do not collect raw command lines, process environments, secrets or unlimited PID labels.
5. Add declarative local HTTP/TCP checks with strict endpoint allowlist and SSRF defenses. Local checks require local opt-in.
6. Implement normalized `check_id`, service key and check freshness; versioned checks protocol; human-readable short summaries with sanitization.
7. Implement opt-in local plugins using root-owned manifest/executable allowlist, pinned digest, no shell, non-root execution, per-plugin timeout/output and global concurrency limits. Manager sends policy/enablement only; it MUST NOT push executable code.
8. Add safe admin-facing per-guest dashboard: filesystems, services, checks, applications, process inventory, last success and error cause. Preserve project RBAC and source attribution.
9. Add example plugins for local HTTP endpoint, PostgreSQL readiness and Redis ping without storing credentials in manager telemetry. Advanced application-specific deep plugins require separate review and are not prerequisites for this stage.

## Tests

- Real guest: filesystem fill/recovery, inode exhaustion simulation, service stop/restart/disable, network error counter, process start/exit, valid and failing HTTP/TCP checks.
- Security: rogue symlinks, modified executable after manifest verification, plugin timeout/killing descendants, oversized output, JSON injection, extra metrics, repeated service discovery, SSRF/DNS rebinding, secret-bearing error output.
- Failure: unknown status for unsupported/failed plugins, no guest agent panic, no Core VM lifecycle changes.
- UI: no unexpected leakage of process/service names across tenants; check stale information is clearly marked.

## Gate

The check inventory and trend charts must reflect real OS behavior. Arbitrary code deployment from manager must remain impossible. Plugin execution must be disabled by default and constrained when enabled. This completes the guest inspection portion of G4, not alerting G4.
