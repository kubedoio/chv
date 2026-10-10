# Guest monitoring plugins (optional, opt-in, local-only)

Plugins are the extension point of the optional guest monitoring
agent ([ADR-026](../../specs/adr/026-optional-monitor-agent-and-guest-identity.md),
[security and plugins contract
v1](../../specs/contracts/chv-monitor-agent-security-plugins-v1.md)).
They let a VM's local administrator add application-level checks
(HTTP endpoints, database readiness, queue health) to the agent's
telemetry without touching the agent itself.

This directory ships **examples**. Nothing here is installed by any
package, and plugin support is **disabled by default** in the agent.

## The security model, in one breath

The manager can NEVER push executable code to a guest. Plugins are a
purely local, root-administrator-owned mechanism: the agent executes
only binaries that the local root administrator placed in a
root-owned allowlisted directory, pinned by SHA-256 digest in a
manifest that the same administrator wrote. There is no
manager-to-agent plugin channel in v1 at all — enablement and policy
are local configuration, and the wire protocol carries no code in
either direction.

## How the agent runs a plugin

- **No shell.** The executable is exec'd directly with an empty
  environment. No command interpolation exists anywhere on the path.
- **Non-root.** The agent already runs as the dedicated `chv-monitor`
  user (hardened unit: `NoNewPrivileges`, `ProtectSystem=strict`,
  `ProtectHome`); plugins inherit that. A plugin needing privilege
  must not exist in v1 — report the unsupported reading instead.
- **Pinned.** Before every execution the agent re-verifies the
  manifest (regular file, root-owned, no symlinks, inside the
  allowlist directory) and the executable (regular file, no symlink,
  no traversal outside the directory, SHA-256 matches the manifest).
  A modified executable after manifest verification fails the check
  as `unknown` — it is never executed.
- **Bounded.** Default 5 s timeout (hard ceiling 30 s), 32 KiB output
  cap (hard ceiling 256 KiB), at most 8 checks per plugin, and a
  per-agent global concurrency cap of 2. A hanging plugin is killed
  together with its whole process group.
- **Fail-soft.** Invalid JSON, an unexpected `check_id`, oversized
  output, unregistered metric ids, a timeout, or a non-zero exit all
  degrade that check to `unknown` with a structured local error —
  never `ok`, never an agent crash, never a VM lifecycle change.

## Manifest format (v1)

One `*.json` file per plugin in the allowlist directory
(`/etc/chv-monitor/plugins.d` by default):

```json
{
  "schema_version": 1,
  "plugin_id": "example.http-health",
  "plugin_version": "1.0.0",
  "executable": "/etc/chv-monitor/plugins.d/http-health.py",
  "sha256": "<sha256 of the executable — compute at install, see below>",
  "checks": ["example.http-health"],
  "interval_seconds": 60,
  "timeout_seconds": 5,
  "max_output_bytes": 32768,
  "privilege_profile": "unprivileged"
}
```

| Field | Meaning |
|---|---|
| `schema_version` | Exactly `1` |
| `plugin_id` | Stable identity, ≤ 128 bytes, `[A-Za-z0-9._:/-]` |
| `plugin_version` | Semantic version of the plugin, for humans |
| `executable` | Absolute path, must resolve inside the allowlist directory, regular file, no symlinks |
| `sha256` | SHA-256 of the executable bytes; re-verified before every run |
| `checks` | `check_id`s this plugin may report (≤ 8) — anything else in its output degrades to `unknown` |
| `interval_seconds` | Execution interval (the agent also caps total load) |
| `timeout_seconds` | 1–30; default 5 |
| `max_output_bytes` | ≤ 262144; default 32768 |
| `privilege_profile` | Must be `unprivileged` in v1 |

## Plugin output format (v1)

A plugin prints one JSON object on stdout and exits `0`:

```json
{
  "schema_version": 1,
  "check_id": "example.http-health",
  "status": "ok",
  "summary": "Endpoint responded",
  "metrics": [
    { "metric_id": "check.duration_seconds", "value": 0.043, "unit": "seconds" }
  ]
}
```

`status` is one of `ok`, `warning`, `critical`, `unknown`. Summaries
are short (≤ 200 bytes recommended; the manager rejects > 256 bytes),
printable, and rendered as plain text — never as HTML. Metric ids
must exist in the metric registry (in practice:
`check.duration_seconds`); the agent itself measures wall-clock
duration authoritatively. **Never put credentials, tokens, URLs with
embedded secrets, or raw error bodies in a summary** — summaries are
monitoring telemetry, stored and displayed by the manager.

## Installing an example

The examples are self-contained Python 3 scripts with their
configuration in constants at the top (the examples deliberately
avoid configuration files and arguments — a plugin is a fixed
root-owned executable, and `python3` is the only dependency). The
PostgreSQL example additionally needs `pg_isready` (typically in the
`postgresql-client` package).

```bash
# 1. Copy and configure the script (edit the constants as root).
sudo cp docs/examples/plugins/http-health.py \
    /etc/chv-monitor/plugins.d/http-health.py
sudo chown root:root /etc/chv-monitor/plugins.d/http-health.py
sudo chmod 0755 /etc/chv-monitor/plugins.d/http-health.py

# 2. Pin the digest in the manifest.
sudo sha256sum /etc/chv-monitor/plugins.d/http-health.py
#    -> put that hex digest in the manifest's "sha256" field

# 3. Write the manifest (root-owned, 0644, .json).
sudo editor /etc/chv-monitor/plugins.d/http-health.json

# 4. Opt in: enable plugins in /etc/chv-monitor/agent.toml
#    [plugins]
#    enabled = true
sudo systemctl restart chv-monitor-agent
```

The check appears in the VM's monitoring dashboard (checks card) on
the next collection tick (60 s by default) as
`plugin:example.http-health`.

## The examples

| Example | Check | Needs |
|---|---|---|
| `http-health.py` | One local HTTP endpoint answers 200 | python3 |
| `postgres-readiness.py` | PostgreSQL is accepting connections (`pg_isready`, no credentials) | python3, `pg_isready` |
| `redis-ping.py` | Redis answers `PING` (no credentials; an auth error is a warning, not a failure) | python3 |

All three exit `0` and print plugin-output-v1 JSON; failures are
reported as statuses, not crashes, and summaries carry exception
class names instead of messages so a secret-bearing error can never
leak into telemetry.

Application-specific deep checks beyond this level are possible with
the same mechanism but require separate security review — they are
not prerequisites for anything.
