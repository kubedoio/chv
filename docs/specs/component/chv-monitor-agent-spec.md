# CHV optional monitoring agent component specification

**Status:** Proposed  
**Decision:** [ADR-026](../adr/026-optional-monitor-agent-and-guest-identity.md)

## Purpose

`chv-monitor-agent` is a small, optional Rust collector installed inside a Linux guest. It adds file system, process, service, interface, and application metrics. It is **not** the node runtime `chv-agent` and does not own VM lifecycle, networking, or disks. The host-side monitoring experience works without it.

## Package and runtime

- Binary `chv-monitor-agent`; packages for deb/rpm/tarball; separate package and upgrade policy.
- `chv-monitor-agent.service` runs under a dedicated non-root user and reads only allowed local OS surfaces.
- No mandatory daemon dependencies beyond the host OS, no inbound port, and no external monitoring service.
- Outbound HTTPS with server authentication, per-agent authentication, and a dedicated scoped ingestion route.
- Configuration `/etc/chv-monitor/agent.toml`; credential file with restrictive permissions; persistent small state in `/var/lib/chv-monitor`.
- Initial Linux x86-64, with arm64 only after qualified release artifacts and tests. Do not claim Windows support.
- Built-in collectors stay in a small library, `chv-monitor-collectors`, so they can be unit tested without the daemon.

## Collection profiles

| Profile | Default interval | Checks |
|---|---|---|
| `resources` | 15 s | CPU, memory/swap, load, uptime, kernel, PSI when available |
| `filesystems` | 60 s | mounts, bytes, inodes, readonly, filesystem identifiers |
| `disk_io` | 15 s | interface/device counters with stable key |
| `network` | 15 s | interface counters, link state, errors/drops, TCP summary |
| `services` | 60 s | configured systemd services and bounded discovery |
| `processes` | 30 s | named processes, count, aggregate CPU/RSS; no raw command lines |
| `http_tcp_checks` | 60 s | explicitly configured and allowlisted endpoints |
| `plugins` | 60 s | administrator-enabled, opt-in plugin checks |

Avoid high-cardinality per-process or per-PID raw time series. Discovery creates service/check inventory in the manager rather than dynamic unbounded metric labels.

## Checks and discovery

A check has stable `check_id`, `target_id`, `service_key`, `status` (`ok`, `warning`, `critical`, `unknown`), `summary`, `observed_at_ms`, and limited metrics. Checks must distinguish `unknown` from successful health. A disappearing discovered service becomes `stale` after a policy interval; it is never silently deleted as healthy. Discovery has allowlists, bounded item count, and per-collection limits.

The agent reports:
- immutable agent identity, boot ID, sequence and schema version;
- OS metadata with a privacy allowlist;
- configured checks and bounded discovered resources;
- counter and gauge measurements with provenance and quality;
- delivery health (last accepted batch, queue depth, dropped batches).

## Enrollment and operational actions

A project-authorized operator creates a short-lived claim scoped to an existing VM. The user installs the agent and places the claim in a credential file through an approved path. The agent validates the server certificate and submits the claim once. The server issues bound short-lived client credentials and records one active identity. A reinstall with a different agent identity requires policy-governed replacement/revocation. A cloned VM image must not reuse enrolled credentials.

The agent can rotate credentials, update local trusted configuration, retry with backoff, spool a bounded window, and uninstall without impacting the guest's runtime. Manager restart or network partition must not make the agent spin or consume unbounded disk.

The user can: view installation instructions, enroll, revoke, rotate, see last-seen, see check inventory, and enable check profiles. Any change to a guest filesystem or process requires local operating-system administration, not CHV remote command execution.

## Plugin model

The safe baseline is **declarative built-in checks**. Executable plugins are disabled until an explicit local administrator approval:

- Manifest includes `plugin_id`, semantic version, fixed path, SHA-256 digest, allowed check names, max execution interval, output schema version and privilege profile.
- The executable resides in a local root-owned allowlisted directory. It runs with a minimal environment as the monitoring user; no shell interpolation or arbitrary code downloaded from the manager.
- Absolute wall timeout 5 seconds by default; hard ceiling 30 seconds. Limit output bytes (default 32 KiB), number of metrics, label lengths and memory/process resources when the OS supports it.
- Parse structured JSON output. Invalid/oversized output becomes an `unknown` check plus plugin error counter, never an agent crash.
- Secrets for application checks must reside in locally permissioned files and never be sent back in logs, sample labels, summaries or dashboards.
- Signature verification and centrally managed plugins are out of v1 scope. No unreviewed sudo or unrestricted privileged helper.

## Config shape (illustrative)

```toml
[agent]
interval_seconds = 15
max_queue_batches = 128
target_kind = "vm"

[transport]
mode = "https"
server = "https://manager.example.org"
credentials_file = "/etc/chv-monitor/credentials"

[collectors]
resources = true
filesystems = true
network = true
disk_io = true
services = true
processes = false
plugins = false

[plugins]
enabled = false
directory = "/etc/chv-monitor/plugins.d"
```

The v1 contract defines limits and field semantics. Config does not embed an arbitrary `vm_id` authorization assertion.

## Guest-vs-host aggregation

The BFF may show guest OS memory or service information next to host-observed VM values, but it must retain each source. An agent report cannot overwrite or reduce the node's CPU or memory measurements. Agent data cannot change `Running` or `Stopped` state. Guest freshness is independent from VM lifecycle freshness.

## Qualification

Test installation, enrollment, duplicate enrollment, credential theft/replay, token expiry, credential rotation, revocation, uninstall/reinstall, reboot, manager loss, bounded queue, high check cardinality, poisoned plugin output, plugin timeout, service changes, deleted VM, cloned VM, migration, and tenant authorization. Run tests against real Linux guests and preserve evidence. Every test of privileged monitoring uses a constrained Linux user; never grant Core socket access.
