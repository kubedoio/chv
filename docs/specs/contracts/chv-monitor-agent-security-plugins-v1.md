# CHV monitoring agent security and plugin contract v1

**Status:** Proposed  
**Authority:** [ADR-026](../adr/026-optional-monitor-agent-and-guest-identity.md)

## Trust boundaries

`chv-monitor-agent` is guest-controlled software. The manager MUST NOT trust a reported VM ID, status, process output, hostname, or a claimed virtio-vsock connection identifier. The node runtime `chv-agent` retains sole VM mutation authority. No monitoring credential authorizes any operation other than its explicit monitoring enrollment, read-only identity metadata, ingestion, and credential rotation.

## Enrollment

1. An authorized project operator requests a claim for exactly one existing VM identity. The manager stores a hashed one-time claim and an expiry no longer than 10 minutes.
2. The operator transfers the claim to that VM by an explicit user-approved path. Avoid shell arguments, cleartext terminal logs, shared image snapshots, and cloud-init outputs with world-readable secrets.
3. The guest agent validates the manager TLS trust and submits a claim over HTTPS, presenting its per-install machine identity (`install_id`). Enrollment is rate limited.
4. The manager atomically consumes the claim, binds `agent_id` to `project_id` + `target_kind=vm` + `target_id` + `credential_epoch` + the presented `install_id`, and issues scoped credentials.
5. The agent writes credentials to restricted storage, erases its plaintext claim, and begins reporting.
6. The manager checks revocation/expiry/tenant binding on each batch. Re-enrollment and replacement require an authorized operation and revoke the previous credential according to policy.

A claim is a bearer secret until consumed. This protocol prevents arbitrary self-asserted VM IDs but does not prove a compromise-free guest or prevent misuse of a stolen unexpired claim. Do not claim cryptographic hardware attestation. Harden claims with strict scope, single use, delivery controls, short TTL, auditing and operator confirmation.

## Credential lifecycle

Use bound TLS client credentials (mTLS certificate or equivalent cryptographic proof). Require server trust validation and short-lived credentials with rotation and revocation. If a signed-token transport is used for MVP, justify it in a separate security review and protect replay; there must be no permanent shared key.

**Key provisioning (decision):** the agent generates its keypair locally during claim redemption and presents the public key with the claim; the manager issues a certificate bound to that enrollment. The manager never generates, transmits, or holds an agent private key, so there is no minted private-key inventory at rest on the manager to protect, rotate, or purge. During issuance the manager stores only public material and credential metadata.

**Cloned-image handling (decision):** the installer records a per-install machine identity — a random install ID generated at install time and stored beside the credential with restricted permissions. The install ID is presented at claim redemption and echoed in every batch envelope (`install_id`), so the manager can compare it against the enrollment record; it is not part of the deduplication key. The manager flags duplicate identity when one enrolled credential is presented with a different install ID or from two simultaneously live connections, marks the agent for operator review, and requires an explicit authorized reset to recover. A full disk-image copy duplicates credential and install ID together and is invisible to this comparison (and only partially caught by simultaneous-connection observation); wholesale image cloning is out of detection scope, and rotation, revocation, and explicit reset are the recovery for that case. This detection is a best-effort signal, not attestation; the security boundary remains the credential, its scope, and its revocation.

Client states use one wire vocabulary (below); the [ADR-026 identity state machine](../adr/026-optional-monitor-agent-and-guest-identity.md) is the conceptual model and maps to it normatively:

| ADR-026 (conceptual) | Wire/API state |
|---|---|
| `not_enrolled` | `unenrolled` |
| `claim_issued` | `enrolling` |
| `enrolled`, first accepted batch not yet seen | `active` (within enrollment grace) |
| valid credential, rotation window open | `renewal_due` (sub-state of `active`) |
| `stale` (valid credential, no recent contact) | `offline` |
| `expired` | `expired` |
| `revoked` | `revoked` |

Stored private keys never appear in API responses after initial provisioning. Guest image clones must not retain active credentials; installers must detect copied machine identity or require an explicit reset.

## Privileges and packaging

The default service has a dedicated OS user, `NoNewPrivileges=true`, `ProtectSystem=strict`, `ProtectHome=true`, controlled `ReadWritePaths`, reduced capabilities and network egress limited by deployment policy. Report unsupported privileged readings instead of running as root. Avoid reading other tenants' files, process environment, command lines, SSH keys, shell history, credentials, or application data. Package uninstall stops the service, removes software, and preserves data/credentials unless explicit purge is requested. Securely revoke manager credentials before or during purge when reachable.

## Plugin manifest v1

```json
{
  "schema_version": 1,
  "plugin_id": "example.http-health",
  "plugin_version": "1.0.0",
  "executable": "/etc/chv-monitor/plugins.d/http-health",
  "sha256": "<sha256-of-approved-binary>",
  "checks": ["plugin:example.http-health"],
  "interval_seconds": 60,
  "timeout_seconds": 5,
  "max_output_bytes": 32768,
  "privilege_profile": "unprivileged"
}
```

The local root administrator owns manifest and executable updates. The manager cannot send arbitrary script content or privileged commands for the guest to execute. An administrator must explicitly enable each plugin. The agent verifies manifest fields and file metadata/digest before execution. No symlink traversal outside the allowlisted directory. No shell invocation and no user-controlled command interpolation.

## Plugin output v1

```json
{
  "schema_version": 1,
  "check_id": "plugin:example.http-health",
  "status": "ok",
  "summary": "Endpoint responded",
  "metrics": [
    {
      "metric_id": "check.duration_seconds",
      "value": 0.043,
      "unit": "seconds"
    }
  ]
}
```

Status enum: `ok`, `warning`, `critical`, `unknown`. For invalid JSON, unexpected check IDs, oversize output, forbidden dimensions, timeouts or non-zero process exits, the agent reports `unknown` plus a structured local error. It never marks them `ok`. Summaries have length limits and sanitization; never render raw HTML.

## Execution limits

Default: 5-second timeout, 32-KiB output, 8 checks/plugin, 4 dimensions/metric, a per-agent global plugin concurrency cap of 2. Hard ceilings: 30 seconds timeout, 256 KiB output. Enforce resource isolation using available systemd/cgroup controls. Network checks use an endpoint allowlist, disallow metadata addresses and loopback by default for remote configured targets, resolve DNS safely, and defend against DNS rebinding/redirect to forbidden IPs. Localhost checks are allowed only through explicit local configuration.

## Vsock extension requirements

The host transport maps the trusted live VMM process to its authorized guest CID and runtime incarnation. A guest must also present a valid agent credential. A CID alone is never a project/VM authorization grant. Handle CID reuse, live migration, restart, stale socket, and guest replacement before enabling by default. Node runtime does not accept guest-originated VM lifecycle RPCs.

## Authorization and privacy

Manager API enforces role checks for enrollment (operator-tier claim issuance), deletion and data views. **v1 boundary, recorded precisely:** CHV v1 has a single fleet scope — roles are fleet-wide, VM reads (including every monitoring data view: `current`, `history`, `overview`, `checks`) are visible to the Viewer role, and per-resource ownership gates mutations, console access and enrollment claim issuance. This means a Viewer can read guest check inventory (service names, summaries, filesystem layouts) of any VM in the fleet — the exposure operators accept by granting the Viewer role. The project-isolation requirement is the multi-tenancy target state: once CHV gains project-scoped authorization, different projects must not query one another's guest processes, filenames, service names, filesystem layouts, or plugins. Guest inventory defaults to minimal fields. A user may disable sensitive collector families. Logs, webhooks, and Prometheus export must redact secrets and sensitive field values.

## Required negative tests

Stolen enrollment claim, double consumption, claim expiry, replayed credential, revoked certificate, cloned VM image, fake `target_id`, alternate project, modified plugin executable, symlink attack, path traversal, oversized plugin output, hanging plugin, malicious plugin labels, SSRF, manager command injection attempt, and vsock CID reuse. Every failure must preserve VM lifecycle availability.
