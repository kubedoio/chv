# CHV monitoring ingestion contract v1

**Status:** Proposed  
**Authorities:** [ADR-025](../adr/025-native-monitoring-architecture.md), [ADR-026](../adr/026-optional-monitor-agent-and-guest-identity.md)  
**Metric schema:** [monitoring metrics v1](chv-monitoring-metrics-v1.md)

## Two transport families

| Family | Sender | Endpoint | Authentication |
|---|---|---|---|
| Node batch | `chv-agent` | New versioned method on existing authenticated node/control-plane gRPC channel | Existing mTLS node identity and node ownership |
| Guest batch | `chv-monitor-agent` | `POST /monitoring/v1/ingest` on manager HTTPS listener | Enrolled agent credential; target-bound; no browser session/cookie auth |

The node message requires a new source-controlled `.proto` contract in `proto/controlplane`. Do not hand-edit `gen/rust`. Reuse existing authenticated transport and keep the new RPC observational. If core-native mode has no manager transport, implement a separate optional telemetry adapter; do not reintroduce a second runtime authority.

The guest route is a dedicated agent-auth endpoint in `chv-controlplane`, isolated from browser BFF routes. It cannot use JWT browser sessions as agent identity. It must require TLS. Reverse proxies must preserve the authenticated connection metadata and must not permit user-supplied identity headers to bypass verification.

## Deployment exposure (default shape)

The manager's default deployment binds loopback (`127.0.0.1`) and TLS is terminated by an optional edge reverse proxy. Guest ingestion changes that exposure requirement and must be an explicit deployment decision, not a side effect:

- Enabling guest ingestion requires a manager listener **with TLS provisioned** — the control plane's own TLS configuration or its TLS-terminating edge — to be reachable from the VM network on a documented address and port. In the default shape no listener has TLS provisioned; turning on guest ingestion therefore also turns on an explicit TLS provisioning decision (control-plane TLS config or edge TLS). The plain-HTTP loopback listener MUST NOT be exposed to the VM network as the ingest endpoint. The listener must not derive any trust from network position: TLS plus the enrolled agent credential is the only authentication.
- When TLS is terminated by a reverse proxy, the proxy either forwards agent mTLS credentials unchanged to the manager, or terminates mTLS itself and forwards the verified identity over a private loopback/Unix-socket connection via a trusted header. A user-supplied identity header arriving on a public listener must be ignored.
- Deployments that do not expose the manager to VM networks keep guest ingestion disabled; node-native metrics and history are unaffected (ADR-026 keeps the agent optional).
- The G3 guest-agent qualification must verify the exact listener, interface, and proxy trust chain of the default single-node install, not only a lab topology.

## Batch envelope

```json
{
  "schema_version": 1,
  "agent_id": "a8cdd2ae-a2a0-4a56-b5fb-0b889681fd94",
  "install_id": "9f3c1a20-6d4e-4c2f-9b1a-2e8d5f7a4b6c",
  "boot_id": "b2e65145-70d1-4a6b-8eab-5fb785238e6c",
  "sequence": 37,
  "sent_at_ms": 1791576000000,
  "samples": [
    {
      "schema_version": 1,
      "target_kind": "vm",
      "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
      "metric_id": "vm.memory.guest_available_bytes",
      "source": "guest_agent",
      "kind": "gauge",
      "unit": "bytes",
      "observed_at_ms": 1791575999000,
      "value": 2147483648,
      "quality": "valid",
      "dimensions": {},
      "boot_id": "b2e65145-70d1-4a6b-8eab-5fb785238e6c",
      "identity_epoch": "agent-credential-generation-1"
    }
  ],
  "checks": [
    {
      "schema_version": 1,
      "check_id": "service:nginx.service",
      "service_key": "nginx.service",
      "status": "ok",
      "summary": "active (running)",
      "observed_at_ms": 1791575999000
    }
  ]
}
```

This JSON is illustrative v1 guest wire format. `install_id` identifies the installing image (cloned-image detection per the [security contract](chv-monitor-agent-security-plugins-v1.md)); it is authenticated metadata, not part of the deduplication key. The normative protobuf implementation MUST preserve type and optional-field semantics, especially integer counter precision. For values above `2^53 - 1`, guest JSON MUST encode exact integers as decimal strings.

The guest envelope may carry an optional `os` object with read-only guest OS identity metadata on a privacy allowlist: `name`, `version`, `kernel_release` (each bounded to 64 bytes, UTF-8, sanitized). It is registry/inventory metadata — never a metric, never an authorization input, and never extended with hostname-adjacent, user, or workload fields. The manager updates its agent record's OS fields from the envelope; absent fields leave the record unchanged.

The `checks` array carries check records (agent spec "Check records"; the metrics contract's `check.*` families). Each check object has exactly these fields:

| Field | Rules |
|---|---|
| `schema_version` | Exactly `1` |
| `check_id` | Stable namespaced identifier, ≤ 128 bytes, charset `[A-Za-z0-9._:/@-]` — for example `service:nginx.service`, `service:user@1000.service` (systemd instance units), `http:local:8080`, `plugin:example.http-health` |
| `service_key` | Optional; same rules as `check_id` (the systemd unit name for service checks) |
| `status` | One of `ok`, `warning`, `critical`, `unknown` — `unknown` is never conflated with healthy |
| `summary` | Optional, ≤ 256 bytes, printable (the manager rejects control characters); plain text, never rendered as HTML |
| `observed_at_ms` | Same past-age and future-skew bounds as samples |

Check records carry no values: a check's numeric time series travel as regular samples (`check.status`, `check.duration_seconds`, dimensioned by `check_id`) in the same batch; the check record itself is latest-status inventory. `check_id` must be unique within a batch. Checks follow the same whole-batch rejection semantics as samples — one invalid check record rejects the batch — and are recorded only when the batch is accepted (a duplicate replay does not re-record). The manager keeps the latest record per `(target, check_id)` as inventory; a delayed older batch never regresses a newer record. Check inventory staleness uses the query contract's 180-second window for the 60-second collection cadence.

## Limits (initial defaults, validated before release)

- Max compressed/uncompressed request: 256 KiB uncompressed; compressed uploads disabled until decompression bombs are tested.
- Max samples/batch: 512. Max discovered checks/batch: 128.
- Max dimensions/sample: 4; max distinct metric series per VM agent: 1024.
- Max guest send frequency: one batch per 5 seconds; defaults every 15 seconds.
- Max past timestamp age: 5 minutes for live raw ingestion; older bounded replay uses a separate offline-recovery policy and is never silently promoted to current.
- Max future timestamp skew: 2 minutes. Server stamps receipt time independently.
- Per-agent concurrency: one accepted request in flight; bounded rate and queue limits; return `429` with capped retry hint.
- Node sampling and manager queue budget are independently configurable and enforced.

All defaults are candidate limits. Re-evaluate with measured memory and CPU usage. Do not silently increase limits based on request-supplied data.

## Authentication, ownership and deduplication

- A guest agent authenticates with its own scoped credential. The server looks up the single permitted `target_kind`, `target_id`, and project. These are never read from unverified payload to determine authorization.
- A node agent authenticates through mTLS. The manager checks the node's authoritative ownership mapping for every VM target and current incarnation. A stale owner must not submit measurements for the successor node.
- Reject revoked, unknown, deleted or mismatched targets, and credential epochs outside allowed grace windows.
- Deduplicate by `(authenticated_sender_id, boot_id, sequence)`. The same batch digest returns previous acknowledgment. The same triple with different body produces `409 replay_conflict`.
- A durable high-water mark or bounded deduplication window survives manager restart. Replay outside retained proof fails closed or receives an explicit `resync_required`, never an uncontrolled insert.
- ACK means **durably accepted into the ingestion queue or store according to the declared persistence mode**. Default mode must only ACK after the batch is persisted or safely durably spooled. It must not acknowledge volatile in-memory enqueue as durable.
- A failed batch cannot modify operational VM state. Return structured errors with a trace ID; never echo secret-bearing request fields.

## Responses

| HTTP | Code | Meaning |
|---|---|---|
| 202 | `accepted` | Batch durably accepted with accepted sample count |
| 200 | `duplicate` | Identical batch already committed |
| 400 | `invalid_batch` | Bad schema/unit/value/dimensions |
| 401 | `unauthenticated` | No valid agent credential |
| 403 | `forbidden_target` | Sender cannot write to target |
| 409 | `replay_conflict` | Same batch key, different digest |
| 413 | `batch_too_large` | Payload exceeds limits |
| 422 | `unsupported_metric` | Unsupported registry ID or source |
| 429 | `rate_limited` | Bounded retry allowed |
| 503 | `ingestion_unavailable` | No durable write accepted |

All errors return `{ "error": { "code": "...", "request_id": "..." } }`. Retry only idempotent batches after bounded backoff and jitter. Treat `409` and validation errors as permanent until corrected. If a batch contains invalid identity, reject the entire batch. If measurements are partially unsupported, reject the entire batch for v1; do not ACK an ambiguous subset.

## Delivery guarantees and failures

Data delivery is **at least once** under retries, deduplicated at ingestion. Exactly-once end-to-end delivery is not claimed across credential rotation and disconnected clients. Loss is observable through `dropped_batches_total`, gaps and sample quality. The agent's local spool is optional, size-limited and non-authoritative. Off-line monitoring cannot block VM operations.

Version contract tests must include malformed field, wrong target, stolen/revoked credential, duplicate, conflicting replay, time skew, oversize, rapid flood, manager restart, outage, queue full, and VM migration with old owner.
