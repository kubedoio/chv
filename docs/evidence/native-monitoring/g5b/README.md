# G5b — optional external export and scale/chaos qualification

**Campaign:** native monitoring implementation (#602)
**Gate:** G5b (prompt-05 task 7 — optional export; prompt-06 — scale and chaos qualification; PR-8)
**Contracts:** `docs/specs/contracts/chv-monitoring-query-alerts-v1.md` (external export section), `docs/specs/adr/027-monitoring-history-alerts-and-export.md` (decisions 9, 11, 12)
**Date:** 2026-10-11 (UTC)
**Environment:** the campaign test host — AMD EPYC 9554P (16 cores visible), kernel `6.8.0-142-generic`, 31 GiB RAM. No VM is involved in this gate by design: the external system under test is a **real VictoriaMetrics binary**, not a stub.

**Binary under test (the whole point of the gate):** `victoria-metrics-prod` (VictoriaMetrics OSS single-node, `v1.153.0`, linux-amd64). Tarball sha256 `1b495bde563825cf83dc7c0425a9d8fa03e7214858e0949f9177efc6bb1f8bfc`, verified against the official checksums file at download time; binary sha256 `f7dcb151fd1fef69364d6d6da49c0d3f1b9c2415ef4b4432b08e416a5c73a49b`, enforced **during the run** by the rig's optional digest pin (`CHV_G5B_VICTORIAMETRICS_SHA256`) — with the pin armed, a substituted binary aborts the rig, so this evidence cannot come from an unverified executable. The run below had the pin armed. The tested code state is commit `8ff935e6` (the branch tip; the working tree was identical to the commit at run time — only documentation files were uncommitted).

## Method

The env-gated rig `cmd/chv-monitor-agent/tests/g5b_export_scale.rs` (CI has no VictoriaMetrics; without the env pins every test skips). Four tests, run serially:

1. **Compatibility** — `export_push_reaches_a_real_victoriametrics`: the production exporter (real `ReqwestExportTransport`, hardened reqwest) pushes to a real VictoriaMetrics over **real TLS** (a rig-issued self-signed certificate with IP SAN `127.0.0.1`, VM started with `-tls -tlsCertFile -tlsKeyFile`, the exporter trusting it through the operator-CA path `ca_cert_path` — the exact shape of an internal deployment). Readback goes through **VictoriaMetrics' own `/api/v1/query`**, not a self-written decoder: the series must appear with the contract's exact name (`chv_node_cpu_capacity_ratio`), labels (`target_kind`, `target_id`, `source`), and value; the counter must appear as its **same-epoch delta** (1024, not the raw reads); and an allowlist-excluded series must be provably absent (asserted only after the full visibility window, so a slow flush cannot mask a leak as an absence).
2. **Outage honesty** — `destination_outage_degrades_and_recovers_without_loss`: killing the destination fails the pass at the first push (degraded, cursor held, nothing exported, no panic, nothing else blocked); restarting VictoriaMetrics on the same storage exports the held window and both sides of the outage are queryable through VictoriaMetrics' query API.
3. **Restart boundedness** — `exporter_restart_replays_only_the_bounded_backfill`: a fresh exporter (process restart) re-exports only within `max_backfill` — an hour-old sample is provably never pushed.
4. **Scale and shape** — `g5b_scale_and_shape_qualification` (`CHV_G5B_SCALE_TIERS=1,10,100,500`): tier ladder of node-batch ingest (the gRPC terminal write path) at 5 series/target; series fan-out ladder (1/5/20 series per VM at tier 100) over real registry vocabulary (dimensionless gauges plus net/block counter families fanned over interfaces/devices); guest-shape ladder (N agents through the **real mTLS path** — claim, certificate enrollment, first delivery, real spool — all concurrent as a fleet ticks); one live VictoriaMetrics absorbing the whole matrix as the export destination.

**Honest waits, not races:** VictoriaMetrics buffers fresh inserts and flushes them to searchable in-memory parts on an interval (default 5 s) — an immediate post-push query can legitimately miss data that IS stored (`vm_rows_inserted_total` counts it). The rig therefore polls readbacks with a 20 s deadline and runs absence assertions only after a 7 s visibility window. The outage leg seeds its second sample and waits for it to age into the pass window (observed time behind the pass clock — the production shape of a cadence pass).

**Rig-only override, documented:** the per-IP enrollment rate limit is raised for the guest leg only. In production that limit is per node (every guest enrolls through its own host); the rig's N agents share one loopback address, so the production default would 429 the fleet for a reason that is not what the leg measures (the shared-IP shape is the known #602 debt). Every other limit — including the per-agent batch cap of 12/min — stays at the production default, and the delivery-retry loop ticks at a 5 s cadence that respects that cap, like a production collector.

## Results (final run from commit `8ff935e6`, EXIT=0, 595.75 s total, digest pin armed)

```
test destination_outage_degrades_and_recovers_without_loss ... ok
test export_push_reaches_a_real_victoriametrics ... ok
test exporter_restart_replays_only_the_bounded_backfill ... ok
test g5b_scale_and_shape_qualification ... ok (546 s of the total)
```

### Export legs

Compatibility proven against the real backend: pushes accepted (2xx from VictoriaMetrics' own HTTP server), readback through its query API returns the exact series (`chv_node_cpu_capacity_ratio{target_kind="node", target_id="g5b-node", source="node_os"} = 0.42`; the counter series carries the delta value 1024 with `source="vmm"`), and the excluded series never crossed. Outage: pass fails at first push, exporter degrades, cursor held, zero exports during the outage, held window delivered after restart. Restart: only the bounded backfill replays; the hour-old sample never reaches the destination.

### Scale matrix

Node-batch legs (5 series per VM, 4 cadence points, real store write path):

| Targets | Samples | Ingest | Query p50/p95/p99 | Export pass | VM RSS |
|---|---|---|---|---|---|
| 1 | 20 | 6 ms (3,333/s) | 1/1/1 ms | 6 ms | 30 MB |
| 10 | 200 | 58 ms (3,448/s) | 1/1/1 ms | 25 ms | 33 MB |
| 100 | 2,000 | 582 ms (3,436/s) | 1/1/1 ms | 173 ms | 46 MB |
| 500 | 10,000 | 2,991 ms (3,343/s) | 1/1/1 ms | 930 ms | 69 MB |

Shape legs (tier 100): 1 series/target — query 0/0/0 ms, export 83 ms; 5 — 1/1/1 ms, 179 ms; 20 — 4/4/5 ms, 560 ms.

Guest mTLS legs (claim → enroll → first delivery, concurrent fleet):

| Agents | Wall time | Delivery attempts (avg/max) | Store rows |
|---|---|---|---|
| 1 | 99 ms | 1/1 | 120 |
| 10 | 45.8 s | 5.5/10 | 2,737 |
| 100 | 164.4 s | 11.3/32 | 32,556 |
| 500 | 313.7 s | 32.0/41 | 69,843 |

### Findings the rig surfaced before release

1. **The >8-metric allowlist bug (fixed).** The export config allows up to 64 `metric_ids`, but the store caps `query_history` at 8 metric ids per call. The worker passed the whole allowlist in one query, so any allowlist larger than 8 ids was rejected by the store on every pass — the exporter logged a warning, held its cursor and retried forever, **silently exporting nothing**. Found only because this rig was written against the real store; fixed by chunked reads (commit `95844324`) and pinned by a unit regression test with a 9-metric allowlist (the smallest that exceeds the cap).
2. **Fleet-concurrent guest delivery contends on the single-writer SQLite store (recorded v1 characteristic).** Concurrent first deliveries fail with `database is locked` at a rate that grows with fleet concurrency; agents spool and retry, and **no data is lost** — the whole 500-agent cold-start burst drains in ~5.2 minutes (avg 32 delivery attempts per agent at a 5 s retry cadence). Steady-state staggered collection (one batch per agent per collection interval) sits far below the per-agent batch cap. This is the burst-drain curve of the v1 store, not a defect in the export path (the node-batch legs show the write path itself sustains ~3,300 samples/s flat through 500 targets, and the export pass at 500 targets completes in 930 ms — 5.4× headroom under the 5 s cadence floor).

## What this evidence does NOT claim

- **Bearer-token authentication against the real backend.** VictoriaMetrics OSS single-node has no bearer-token auth to verify against; the token path (`Authorization: Bearer`, token-file read, redaction) is pinned by the unit suite only. The rig documents this absence honestly rather than faking it.
- **Multi-node or clustered backends.** Single-node campaigns only (the campaign's recorded position).
- **The guest-leg process RSS as a manager cost.** The ~397 MB test-process RSS at guest tier 500 includes 500 in-process agents with collectors and spools living in the same process as the manager — a rig artifact. The manager-side costs that are real are the store rows, the write contention curve and the delivery drain time above.
- **Exact exported-timestamp semantics beyond the store contract.** Exported timestamps are the history-query bucket starts (the store's documented bucketing); the compatibility leg asserts values and labels, with bucket-tolerance assertions pinned in the unit suite.
