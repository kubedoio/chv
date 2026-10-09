# Runbook: Monitoring Store (history retention, damage recovery, disabled operation)

**Scenario:** The native monitoring history subsystem (`monitoring.db`,
ADR-025/027) needs operator intervention — retention tuning, storage
failure, corruption, or deliberate disablement.

**Severity:** **SEV-3** for everything in this runbook by design: the
monitoring store is disposable telemetry. A degraded, corrupted, or
disabled monitoring store **never** affects VM lifecycle, reconciliation,
or state reports. No monitoring problem should be escalated beyond SEV-3
unless it is a symptom of a wider filesystem/database failure.

**Automation level:** Degradation and recovery are automatic; this
runbook covers verification, tuning, and manual damage recovery.

## What exists where

| Piece | Location (package defaults) |
|---|---|
| Store database | one SQLite file per the `[monitoring] database_url` (default `sqlite:///var/lib/chv/monitoring/monitoring.db`; **separate** from the operational `chv.db`) |
| Schema migrations | `CHV_MONITORING_MIGRATIONS_DIR` (packaged at `/usr/local/share/chv/monitoring-migrations`) |
| Configuration | `/etc/chv/controlplane.toml`, `[monitoring]` section (see `docs/examples/controlplane.toml`) |
| Health surface | `GET /v1/monitoring/health` (BFF, authenticated); the UI's overview rail shows a Monitoring Health card; `/v1/health` carries a `monitoring: ok/degraded` line that never flips the overall status |

Defaults: raw samples 48 h, 5-minute rollups 30 days, 1-hour rollups
180 days, per-target series cap 1024, DB size budget 2 GiB, headroom
floor 256 MiB, ingest rate cap 20 batches/min/sender (the agent sends
one per 15 s), maintenance pass every 60 s.

## 1. Verifying monitoring is healthy

```sh
# Authenticated health check (any viewer-or-higher token):
curl -s -H "Authorization: Bearer $TOKEN" \
  https://<control-plane>:8080/v1/monitoring/health | jq
```

Healthy: `"available": true`, no `degraded_reason`, `last_ingest_at_ms`
within ~30 s of now (agents batch every 15 s). The UI equivalent is the
**Monitoring Health** card on the overview page.

## 2. Monitoring is degraded — check disk headroom first

The store refuses new batches and answers `ingestion_unavailable` when
the filesystem holding `monitoring.db` drops below the headroom floor
(default 256 MiB). A separate file on the same filesystem does **not**
isolate disk-full risk — this is deliberate (ADR-027): monitoring stops
itself before it can fill a disk VM lifecycle depends on.

1. Check `degraded_reason` on `/v1/monitoring/health` — headroom
   failures name the floor and the observed free bytes.
2. Free space or raise/lower the floor in `controlplane.toml`:

   ```toml
   [monitoring]
   min_headroom_mib = 512
   ```

3. Restart `chv-controlplane` (or wait for the next batch — recovery is
   automatic once statvfs reports headroom above the floor; only the
   floor value itself requires a restart).
4. Verify per §1. Degraded-period batches are **not** retroactively
   backfilled: agents keep only the latest sample per series, so the
   gap stays a gap (charts show holes, never interpolated filler).

While degraded: VM lifecycle is fully operational. The disk-full
behavior — typed rejection, health degradation, lifecycle unaffected,
automatic recovery — is pinned by the integration test
`disk_full_degrades_monitoring_but_not_lifecycle`
(`crates/chv-controlplane-service/src/monitoring_ingest_tests.rs`).

## 3. Corruption or failed migration — reset the store

The store is disposable; the operational database is **never** touched
by monitoring reset.

1. Stop the control plane: `sudo systemctl stop chv-controlplane`.
2. Move the file aside (keeps the damage for forensics if wanted):

   ```sh
   sudo mv /var/lib/chv/monitoring/monitoring.db{,.broken-$(date +%s)}
   # and its WAL/SHM siblings if present:
   sudo mv /var/lib/chv/monitoring/monitoring.db-wal{,.broken-$(date +%s)} 2>/dev/null || true
   sudo mv /var/lib/chv/monitoring/monitoring.db-shm{,.broken-$(date +%s)} 2>/dev/null || true
   ```

3. Restart: `sudo systemctl start chv-controlplane`. The store is
   recreated and migrated from scratch on boot.
4. Verify per §1. History before the reset is gone — charts show the
   absence (`not_collected` / `no_history` reasons), never zeros.

## 4. Running with monitoring disabled

Monitoring can be off permanently (small sites, or the disk budget goes
elsewhere):

```toml
[monitoring]
enabled = false
```

- Every `/v1/monitoring/*` read (except `catalog` and `health`, which
  still answer) returns **503 `MONITORING_UNAVAILABLE`** — this never
  means nodes or VMs are unhealthy, and the UI says exactly that.
- Agents keep sampling and serving their local `/metrics` Prometheus
  surface (node-level gauges/counters) — only history ingest and
  dashboards are off.
- VM lifecycle, migration, snapshots: unaffected, by construction (the
  ingest path shares nothing with the reconcile/state-report path, and
  the agent's batch sender owns a dedicated control-plane client).

Re-enabling is the reverse; the store is recreated empty and history
accumulates from that point.

## 5. Retention and size tuning

```toml
[monitoring]
raw_retention_hours = 48        # raw samples
rollup_5m_retention_days = 30   # 5-minute rollups
rollup_1h_retention_days = 180  # 1-hour rollups
max_db_gib = 2                  # hard size budget (oldest raw evicted first)
max_series_per_target = 1024    # ingestion contract cap per node/VM
batches_per_minute = 20         # per-sender rate cap
maintenance_interval_secs = 60  # rollup/retention/WAL-checkpoint pass
```

Notes:

- The DB size budget is enforced by the maintenance worker (oldest raw
  data evicted first); rollups are compact by construction.
- WAL checkpoints run in the same maintenance pass — sustained
  `monitoring.db-wal` growth beyond a few tens of MiB means the
  maintenance worker is failing; check `degraded_reason` and the
  control-plane logs (`monitoring maintenance`).
- Query ceilings are fixed by the contract (30 d detailed, 180 d
  aggregated, ≤ 1000 points/series) and are not configurable.

## 6. Escalation criteria

Escalate beyond this runbook only when:

- The **operational** database (not `monitoring.db`) is failing — that
  is a control-plane DR event: see `control-plane-dr.md`.
- The filesystem is full for reasons unrelated to monitoring — the
  headroom trip is a symptom, not the cause.
- Agents are not reporting at all (`last_ingest_at_ms` stale on a
  healthy store) — that is an agent/enrollment problem, not a
  monitoring-store problem.
