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

## Batch envelope

```json
{
  "schema_version": 1,
  "agent_id": "a8cdd2ae-a2a0-4a56-b5fb-0b889681fd94",
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
  "checks": []
}
```

This JSON is illustrative v1 guest wire format. The normative protobuf implementation MUST preserve type and optional-field semantics, especially integer counter precision. For values above `2^53 - 1`, guest JSON MUST encode exact integers as decimal strings.

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
