# Runbook: Monitoring Export to an External Backend (optional remote-write)

**Scenario:** CHV's optional Prometheus remote-write v1 export (ADR-027 decision 9, shipped in PR-8) needs operator intervention — enabling it, choosing the destination and metric allowlist, or diagnosing a degraded exporter, rejected pushes, or missing data at the receiver.

**Severity:** **SEV-3** for everything in this runbook by design. The export is a strictly optional, opt-in egress path. It is **never** load-bearing for CHV's native UI, alerting, or VM lifecycle: if the destination is down the control plane keeps working and the native monitoring UI is unaffected. A broken export is lost remote telemetry at an external receiver — nothing more.

**Automation level:** Export passes, batching, backoff, degraded-state marking, and bounded restart backfill are automatic. This runbook covers configuration, readback verification, and diagnosis.

## What exists where

| Piece | Location (package defaults) |
|---|---|
| Exported data | read from `monitoring.db` (`[monitoring] database_url`, default `sqlite:///var/lib/chv/monitoring/monitoring.db`) — export never touches the operational database |
| Configuration | `/etc/chv/controlplane.toml`, `[monitoring.export]` section (see `docs/examples/controlplane.toml` for the annotated example) |
| Worker | inside `chv-controlplane`: the export worker, spawned at boot |
| Observability | the exporter's own health gauges on the admin `/metrics` endpoint |
| Destination | an external receiver you operate (VictoriaMetrics/Prometheus remote-write v1 endpoint); nothing in CHV hosts or manages it |

The worker reads its configuration **at boot** — every change in this runbook requires a `systemctl restart chv-controlplane` to take effect.

## 1. Configuration

```toml
[monitoring.export]
enabled = false                  # master switch, explicit opt-in (default off)
destination_url = ""             # REQUIRED when enabled, e.g. https://victoriametrics.internal:8428/api/v1/write
bearer_token_path = ""           # optional file with a bearer token, 1..=4096 bytes, read ONCE at worker start
ca_cert_path = ""                # optional PEM CA bundle for internal destinations; system roots when unset
interval_secs = 15               # export pass cadence
max_backfill_secs = 600          # bounded catch-up window on restart/outage; older samples are skipped, never replayed unboundedly
lag_degraded_secs = 120          # beyond this lag the exporter is marked degraded (warn + gauge); never blocks anything
metric_ids = []                  # REQUIRED when enabled (non-empty): which registry metric ids to export
target_kinds = []                # optional filter, vocabulary: "node", "vm"; empty = both
sources = []                     # optional filter, vocabulary: node_os, vmm, vm_cgroup, storage_provider, network_provider, guest_agent, derived; empty = every source the metric allows
```

`destination_url`, `bearer_token_path`, and `ca_cert_path` are optional fields — an empty or absent value means unset, and unset means the behavior described above (no destination, no token, system CA roots). `metric_ids` is a list of registry metric id strings.

The default is fully off: with `enabled = false` (or the section absent) no export worker runs, no egress happens, and nothing about monitoring changes.

**Boot validation is fail-loud.** The control plane refuses to start on:

- `enabled = true` with no destination URL, or with an empty `metric_ids` selection;
- a destination that is not `https://`, carries URL credentials (`user:pass@`), has no host, or uses link-local or unspecified IP literals;
- unknown metric ids, target kinds, or sources — re-validated against the v1 registry at boot, so a typo in an allowlist entry is a boot error, not a silent skip;
- an unreadable `bearer_token_path` or `ca_cert_path` (boot error — no silent fallback), and a `ca_cert_path` that is not a valid PEM certificate bundle (boot error at transport construction);
- a bearer token file that is empty or whitespace-only.

Check `journalctl -u chv-controlplane` if the service will not start after an export config change. This is the same fail-loud contract as alerting destinations: a half-configured egress path must never look healthy.

## 2. What goes over the wire

The wire format is Prometheus remote-write v1 — snappy-compressed protobuf — pushed to the destination URL.

**Series names:** the registry metric id with a `chv_` prefix and `.` replaced by `_`, e.g. `node.cpu.capacity_ratio` exports as `chv_node_cpu_capacity_ratio`. The mapping is injective (no two metric ids collapse to the same series name) and pinned by test.

**Labels** on every series: `target_kind` (`node` or `vm`), `target_id`, and `source` (the snake_case ingestion source string), plus the metric's own dimensions — e.g. `interface_id`, `mount_id`, `block_device_id`, `window`.

**Values:** gauges export their bucketed values. Counters export same-epoch **deltas only**: a lone counter point with no predecessor carries no value. That is the store contract, not a bug — if you see counters starting only after the second sample of an epoch, that is correct behavior.

Only allowlisted `metric_ids` are exported, further filtered by `target_kinds` and `sources` when set.

## 3. Worker model, backfill, and degraded state

One pass every `interval_secs` (default 15 s): for each target (filtered by `target_kinds`/`sources`) the allowlisted metrics are read from monitoring.db and pushed. Reads are batched in chunks of the store's 8-metric-ids-per-query cap — a large allowlist means several queries per target per pass, not one.

Cursor and failure semantics:

- The cursor parks at pass time on success — each pass exports the window since the last successful one.
- A target whose query is rejected does not advance; it is retried next pass, bounded by the backfill clamp.
- A destination failure fails the whole pass at the first push: the cursor is held, the worker backs off (with ±20% jitter, so multiple managers against one recovering receiver do not retry in lockstep), and the exporter is marked degraded. Recovery re-exports the held window — at-least-once delivery within `max_backfill_secs`. Receivers should tolerate duplicate samples in that window (remote-write receivers generally deduplicate by timestamp).
- A monitoring-store read failure is NOT a destination failure: the pass stops, nothing is pushed, and the next pass retries at the normal cadence — no destination backoff and no destination-degraded marking for a local store problem (the failure is visible as a warn log naming the failed read).
- On restart the same bounded backfill applies: a fresh exporter never replays older-than-backfill history. monitoring.db is disposable telemetry; export is not a durability mechanism for it.
- **Scale boundary (v1):** one pass walks at most the first 1,000 targets per kind (ordered by target id). A fleet beyond that logs a loud warning at the cap — targets beyond the first page of a kind are not exported in v1. The qualification campaign covers 500 VMs.

First-seen targets (a fresh process, or a newly discovered VM) start at the backfill window like everything else; the per-target skip warning fires for a cursor that lagged in-session (e.g. after a long destination outage), and a first-seen target's pre-window history is dropped under the same bounded-backfill rule, reported once per pass in an aggregate log line.

## 4. Observability

The exporter's own health is visible on the admin `/metrics` endpoint as three gauges:

| Gauge | Meaning |
|---|---|
| `chv_export_lag_seconds` | export backlog: age of the oldest unexported window |
| `chv_export_degraded` | 0/1 — 1 means the destination is failing (backoff + retry is automatic); store read failures do NOT set this (they retry at the normal cadence with a warn log) |
| `chv_export_samples_total` | counter of exported samples |

Degraded state is a warn-level log, never an error storm. Destination errors are redacted — no URLs and no tokens appear in logs.

## 5. Security posture

- **Egress is HTTPS-only**, with the same URL validation shape as webhook destinations: no credentials in the URL, no link-local or unspecified IP literals. Loopback and private ranges **are** allowed on purpose — an internal receiver (`https://victoriametrics.internal:8428/api/v1/write`) is a legitimate operator-configured destination. Use `ca_cert_path` when the internal receiver's certificate chains to a private CA.
- **The bearer token is read from a file at worker start** (1..=4096 bytes), sent as `Authorization: Bearer …`, never logged, and never present in the config file itself.
- **The allowlist is operator configuration only.** No API, no UI user, and no guest can add metrics, change filters, or change the destination. No new authenticated API surface exists for export — the only control is `/etc/chv/controlplane.toml` plus a restart.

## 6. Verified backend — and one honest limitation

**VictoriaMetrics OSS single-node v1.153.0** is the qualified backend. Compatibility was proven on the g5b rig: pushes accepted, readback through VictoriaMetrics' own query API with exact series names, labels, and values, plus outage and restart-backfill legs.

**Honest limitation:** VictoriaMetrics OSS single-node has **no bearer-token authentication**, so the token path is verified at unit level only, not against that backend. If your destination needs a token (e.g. a remote-write proxy in front of the receiver), configure `bearer_token_path` and verify with your receiver's audit log.

## 7. Diagnosis steps

1. **Is it enabled and healthy?** Check the three `chv_export_*` gauges on the admin `/metrics` endpoint. `chv_export_degraded` = 1 means the destination is failing — backoff and retry are automatic; nothing else is affected while you investigate.
2. **Config didn't take effect?** Config is boot-read. Restart with `sudo systemctl restart chv-controlplane` and check `journalctl -u chv-controlplane` for the fail-loud validation messages (§1) — the most common cause is a typo'd metric id or a destination URL that fails validation.
3. **Destination rejects pushes?** Check the receiver's logs. Verify TLS trust (`ca_cert_path` for private CAs), verify the token if the receiver requires one, and verify the URL points at the remote-write endpoint — `/api/v1/write` for VictoriaMetrics/Prometheus, not the query or UI endpoint.
4. **Readback verification.** Query the destination directly, e.g. for VictoriaMetrics:

   ```sh
   curl -sk 'https://HOST:8428/api/v1/query?query=chv_node_cpu_capacity_ratio'
   ```

   Check that the returned series carry the `target_kind`/`target_id`/`source` labels. **Note:** VictoriaMetrics buffers fresh inserts and flushes them to searchable parts on an interval (default 5 s) — a query immediately after a push may legitimately return empty. Wait a few seconds before concluding data is missing.
5. **Old data missing after a long outage?** That is the bounded backfill by design: samples older than `max_backfill_secs` (default 600) are skipped, with a warning in the logs. If a longer catch-up window is wanted, raise the value in `controlplane.toml` and restart — before the next outage, not after. The same bound applies to a first-seen target (fresh process, newly discovered VM): its pre-window history starts at the backfill window, reported in one aggregate log line per pass.
6. **Which series should I expect?** `chv_` + the metric id with dots replaced by underscores; only allowlisted `metric_ids`, only allowlisted kinds/sources; counters appear as same-epoch deltas (§2). If a series is absent, first check it is in the allowlist and passes the kind/source filters, then check the receiver-side flush interval (step 4).

## 8. What this deliberately does NOT do

- **No guest/API-controlled destinations or filters.** The destination and allowlist come only from operator configuration; nothing a guest or UI user does can direct telemetry anywhere.
- **No HTTP destinations.** HTTPS only, enforced at boot.
- **No unbounded replay.** Backfill is clamped to `max_backfill_secs`; older samples are skipped, never replayed without bound.
- **Export is not a backup mechanism.** monitoring.db retention rules still govern what exists to export; export reflects the store, it does not preserve it.
- **No per-tenant routing.** Export is fleet-scoped, like all v1 monitoring.
