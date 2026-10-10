# Runbook: Native Alerting and Notifications (rules, incidents, webhook delivery)

**Scenario:** CHV's built-in alerting (ADR-027, [query/alert contract v1](../specs/contracts/chv-monitoring-query-alerts-v1.md)) needs operator intervention — enabling and tuning it, configuring webhook/Slack destinations, rotating the signing secret, or diagnosing failed deliveries.

**Severity:** **SEV-3** for everything in this runbook by design. Alerting is workflow metadata over disposable telemetry: a degraded evaluator, a dead-lettered webhook, or alerting switched off entirely **never** affects VM lifecycle, reconciliation, or state reports. Escalate beyond SEV-3 only when the symptom is really an operational-database or filesystem failure (see [control-plane DR](control-plane-dr.md)).

**Automation level:** Rule evaluation, the incident lifecycle, and delivery with retry/dead-letter are automatic. This runbook covers configuration, verification, secret rotation, disabling, and dead-letter triage.

## What exists where

| Piece | Location (package defaults) |
|---|---|
| Rules, incidents, transitions, notification outbox | operational database (`[database] url`, default `sqlite:///var/lib/chv/controlplane.db`): tables `alert_rules`, `alerts` (rows with `source = 'monitoring'`), `alert_transitions`, `notification_outbox` — **not** monitoring.db |
| Configuration | `/etc/chv/controlplane.toml`, `[monitoring.alerting]` and `[monitoring.notifications]` sections (see `docs/examples/controlplane.toml`) |
| Workers | inside `chv-controlplane`: the alert evaluator and the notification dispatcher, spawned at boot |
| UI | `/alerts` route (incidents, rules, delivery status) |
| API | `POST /v1/monitoring/alerts`, `/alerts/detail`, `/alert-rules*`, `/alerts/acknowledge`, `/alerts/silence`, `/notifications/deliveries`, `/notifications/test` (viewer reads, operator mutations, admin test) |

Both workers read their configuration **at boot** — every change in this runbook requires a `systemctl restart chv-controlplane` to take effect.

## 1. Configuration

```toml
[monitoring.alerting]
enabled = true                 # evaluator runs; a no-op with zero rules
evaluation_interval_secs = 15  # 5..=60, evaluator tick
max_rules = 200                # 1..=500, stored-rule ceiling (create beyond = 409)

[monitoring.notifications]     # ALL optional; unset = no outbound events
webhook_url = ""               # https:// only, no credentials in the URL
webhook_signing_secret = ""    # required (16..=256 bytes) iff any destination is set
slack_webhook_url = ""         # Slack incoming webhook (posts unsigned {"text":…})
webhook_ca_path = ""           # optional PEM CA bundle for internal receivers
max_attempts = 8               # 1..=20, delivery attempts before dead-letter
dispatch_interval_secs = 5     # 1..=60, dispatcher tick
max_batch = 10                 # 1..=50, events claimed per dispatch pass
```

Defaults are strict no-ops: alerting on with zero rules evaluates nothing, and with no destination configured no notification events are enqueued at all.

**Boot validation is fail-loud.** The control plane refuses to start on: out-of-range values; a destination URL that is not `https://`, carries credentials (`user:pass@`), has no host, or uses an unspecified (`0.0.0.0`, `::`) or link-local (`169.254.*`, `fe80::`) IP literal; a destination set without a signing secret; a secret shorter than 16 bytes or longer than 256; an unreadable `webhook_ca_path`. A configured CA bundle that is not valid PEM is also a boot error — never a silent fallback to system roots. Check `journalctl -u chv-controlplane` if the service will not start after an alerting config change.

## 2. The webhook destination allowlist

There is no per-rule or per-UI destination field, by design. Destinations come **only** from `[monitoring.notifications]` — that operator configuration IS the allowlist. An alert rule, a UI user, or a guest can never direct a notification anywhere.

The transport adds the remaining guardrails, enforced on every request:

- HTTPS only; HTTP URLs are rejected at boot **and** by the HTTP client at send time.
- Redirects are never followed — a webhook that answers 3xx gets a retry, not a silent hop to an attacker-chosen host.
- No credentials in the URL (boot validation rejects `@` in the authority).
- Link-local and unspecified IP literals are rejected at boot: `169.254.169.254` and friends are where cloud metadata services live, and a signed webhook must never be fetchable into them.
- Loopback and private ranges are allowed **on purpose**: an internal receiver (`https://alerts.example.internal/hook`) is a legitimate operator-configured destination. Use `webhook_ca_path` when the internal receiver's certificate chains to a private CA.

## 3. Verifying webhook signatures (receiver recipe)

Every webhook delivery carries `x-chv-signature: v1=<hex>` — an HMAC-SHA256 of the **exact raw request body** keyed by `webhook_signing_secret`, as 64 lowercase hex digits. Verify before parsing, over the raw bytes (do not re-serialize the JSON and verify that — the signature will not match):

```sh
# Receiver side. $SECRET = webhook_signing_secret, $body = the raw request body.
expected="v1=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$SECRET" | awk '{print $NF}')"
if [ "$expected" = "$X_CHV_SIGNATURE" ]; then echo verified; else echo REJECT; fi
```

Sanity-check your receiver against a known vector (RFC 4231 test case 2, also pinned in CHV's own tests):

```sh
printf '%s' 'what do ya want for nothing?' | openssl dgst -sha256 -hmac 'Jefe'
# must print ... 5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843
```

The payload itself is the contract's ten-field envelope (`schema_version`, `event_id`, `incident_id`, `event_type`, `severity`, `target_kind`, `target_id`, `summary`, `occurred_at_ms`, `resource_url`) — a closed field set, so nothing beyond it can ever be sent. Slack deliveries are the exception: they are unsigned `{"text": "…"}` posts, because Slack's incoming-webhook URL is its own credential.

## 4. Rotating the signing secret

The secret is only used to compute `v1=` HMAC-SHA256 signatures at send time; it is never logged and never appears in a payload. Rotation steps:

1. **Teach the receiver the new secret first.** A receiver that accepts both old and new secrets during the overlap avoids dropped deliveries; a receiver that hard-switches will reject everything signed with the new key.
2. Change `webhook_signing_secret` in `/etc/chv/controlplane.toml` and restart:
   `sudo systemctl restart chv-controlplane`.
3. Verify per §5 with a test notification; confirm the receiver accepted the new signature.
4. After the overlap window, remove the old secret from the receiver.

Two properties make rotation safe: the dispatcher signs **at send time** with the current boot's secret (events queued before the restart simply deliver with the new signature — no stranded old-secret signatures), and delivery is at-least-once with idempotent event IDs, so a restart mid-queue replays rather than duplicates.

## 5. Testing the notification pipeline

```sh
# Admin token required; 409 = no destination configured (an honest error):
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" -X POST \
  https://<control-plane>:8080/v1/monitoring/notifications/test | jq
# {"enqueued": true, "event_id": "…"}

# Watch it flow through the dispatcher (viewer token is enough):
curl -s -H "Authorization: Bearer $TOKEN" -X POST \
  -H 'content-type: application/json' -d '{"limit": 20}' \
  https://<control-plane>:8080/v1/monitoring/notifications/deliveries | jq
```

A healthy pipeline shows the test event moving from `pending` to `delivered` within a dispatch tick (default 5 s), with `attempts: 1` and a `2xx`-class `last_response`. The UI equivalent is the delivery-status view on the `/alerts` page.

## 6. Outage, restart, and degraded monitoring behavior

- **Incidents are durable.** They live in the operational database, not monitoring.db — an evaluator restart, a monitoring-store reset, or monitoring retention eviction never loses an active incident or its transition history.
- **The outbox is at-least-once with idempotent event IDs.** A crash between recording a transition and delivering its notification replays the enqueue as a no-op — the event is delivered exactly as many times as it was accepted, never duplicated.
- **Restart resumes delivery.** Pending outbox events survive restarts and retry with capped exponential backoff (5 s doubling to a 1-hour ceiling, ±20% jitter) until `max_attempts` (default 8), then dead-letter.
- **A crashed dispatch batch returns on its own.** Claimed events carry a 5-minute lease; a dispatcher that dies mid-batch has its claims expire and the events retry — no reaper, no manual unstick.
- **Monitoring-store degradation degrades alerting only.** Without a connected monitoring store the evaluator does not run (boot) or skips rules with warnings (runtime) — it never fires on the absence itself. The dispatcher keeps delivering events already enqueued, and VM lifecycle is unaffected throughout. See the [monitoring store runbook](monitoring-store.md) for the store side.
- **A destination removed from config dead-letters honestly.** Events already enqueued for the removed channel are marked `dead` with `"webhook destination removed from configuration"` rather than being dropped silently.
- **Deleting a rule retires its incidents — no ghosts.** A deleted rule's firing incident would otherwise fire forever with no recovery path (the rule that could recover it is gone). Deletion resolves it (reason `rule deleted`, resolved notification unless silenced) and deletes any never-fired pending; the response reports `retired_incidents`. The same retirement happens when an update changes the rule's dimension match (the incident identity changes with it). **Disabling** a rule is different and recoverable: incidents hold in place and resume when it is re-enabled.

## 7. Running UI-only, and disabling alerting

- **No destination configured** (the default): incidents still open, fire, and resolve in the UI — the UI is always a channel — but no outbox rows accumulate, because events are only enqueued when a destination exists.
- **`[monitoring.alerting] enabled = false`:** the evaluator is not started; existing incidents persist untouched and every alerting read (incidents, rules, delivery audit) keeps working, since they are operational-database reads. Re-enabling resumes evaluation from the stored incident state.

Neither state affects the monitoring dashboards or VM lifecycle.

## 8. Dead-letter triage

A `dead` status means CHV gave up on that event — either permanently rejected, or retried to exhaustion:

1. **Read the delivery audit** (`/notifications/deliveries` or the UI delivery view) and find rows with `status: "dead"`. `last_response` says why: `http 404` (permanent 4xx — wrong URL or the receiver rejects the payload), `429`/`5xx`/`transport: …` retried to `max_attempts`, or the removed-destination note from §6.
2. **Check the courtesy event.** When an event dead-letters, CHV enqueues one `delivery_failed` notification for the same incident — routed to a surviving configured channel when the dead event's own channel is gone — so the outage is visible on the alerting surfaces, and appends a durable audit event (`monitoring.notification.dead_letter`, actor `system:notification-dispatcher`) to the events feed. When no destination remains configured at all, the courtesy notification is skipped (it could only dead-letter too); the audit event still records the failure.
3. **Fix and re-test.** Permanent 4xx usually means a wrong `webhook_url` or a receiver that moved; fix the config, restart, and send a test per §5. Dead-lettered events are **not** automatically retried after the fix — the incident's later transitions (still firing, eventual resolved) notify normally; use the test endpoint to confirm the pipeline.

## 9. What acknowledge and silence do — and do not

Both are **overlays on active (pending/firing) incidents**, set from the `/alerts` UI or the BFF by an operator:

- **Acknowledge** records who looked at the incident and when. It does **not** resolve the incident, does not stop the condition from evaluating, and does not suppress firing/resolved notifications.
- **Silence** suppresses notification **enqueue** while the deadline is in the future (relative `duration_minutes`, 1..=10080, or absolute `until_ms` — never both, and the absolute form is bounded to the same 7-day horizon). The transition itself still happens and remains fully visible; when the deadline passes, later transitions notify again. Silence does **not** resolve the incident.

Only the recovery window — the condition being observably false for `recovery_seconds` — resolves an incident. Neither overlay can substitute for fixing the underlying problem.

## 10. Growth and retention (v1 boundary)

Resolved incidents (with their transition history) and delivered/dead outbox rows are **retained indefinitely in v1**: they are durable audit state in the operational database, reads are paged (incidents/rules ≤100 per page, deliveries ≤50), and nothing is deleted on a schedule. On a long-lived fleet these tables grow with incident volume; a bounded retention/archival policy is recorded as follow-up debt on the campaign tracking issue (#602). Until then, treat operational-database growth monitoring as part of normal control-plane capacity planning — the DR guidance in [control-plane-dr.md](control-plane-dr.md) covers the database these rows live in.

## 11. Escalation criteria

Escalate beyond this runbook only when:

- The **operational** database is failing (incident reads and rule lists error too) — that is a control-plane DR event: see [control-plane-dr.md](control-plane-dr.md).
- The filesystem is full or corrupt — the same symptom will be visible in the monitoring store runbook's health checks.
- You suspect the signing secret or a destination URL has been exposed — treat it as a credential rotation (§4) plus a review of the delivery audit for unexpected destinations; destinations can only ever have come from `controlplane.toml`.
