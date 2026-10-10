# G2 — bounded history, ingest and honest UI evidence (real-host, qualified pin)

**Campaign:** native monitoring implementation (#602)
**Gate:** G2 (bounded history, authenticated API, honest UI — prompt 02)
**PR:** PR-2 (`monitoring-g2-history-ingest`)
**Date:** 2026-10-09 (UTC)
**Environment:** identical to the G0b/G1 captures — AMD EPYC 9554P,
kernel `6.8.0-142-generic`, real KVM, `cloud-hypervisor v53.0` static
binary digest-verified against `scripts/install.sh`'s pin
(`448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc`),
rust-hypervisor-fw `4a0a1e97…`, guest image `noble-qual-patched.img`
`37f7c340…` exposed read-only (`readonly=on`, symlinked under its true
`.qcow2` extension — the image is qcow2 content under an `.img` name
and the VMM refuses an extension/type mismatch; the pinned bytes are
never written).

## Method

### 1. Full agent-side pipeline on a real VM

The env-gated integration test `g2_real_vmm_samples_persist_to_bounded_history`
(`cmd/chv-agent/src/monitoring.rs`) drives the **production** path end
to end — no adopted stray process, no mocked stage:

`ProcessCloudHypervisorAdapter::create_vm` (REST `vm.create`, the real
config JSON the agent builds) → `VmRuntime::start_vm` (real `vm.boot`)
→ `VmSampleSource::collect_vm` twice, 6 s apart (the same adapter the
sampler runs on its 10 s VM cadence) → `MonitoringStore::ingest_node_batch`
(the same durable commit the control plane's ingest service performs
after validation) → `query_history` (the same query the BFF serves).

```sh
CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
cargo test -p chv-agent --bin chv-agent g2_real_vmm -- --nocapture
```

Result: **1 passed in 11.18s** (CI skips this test — no KVM; the run
above is the real-host record).

### 2. Disk-full trigger (storage failure)

`disk_full_degrades_monitoring_but_not_lifecycle`
(`crates/chv-controlplane-service/src/monitoring_ingest_tests.rs`)
induces the disk-full condition through the identical statvfs read and
comparison production performs every batch (a headroom floor above the
monitoring filesystem's real free space), against a real file-backed
monitoring store and a real operational store.

### 3. Authenticated read surface

`monitoring_routes.rs` (`crates/chv-webui-bff/tests/`) boots the real
`bff_router` with a real monitoring store and pins the query/alerts
contract v1 read API: auth required, viewer suffices, honest
value/quality/absence handling, decimal-string counter deltas, typed
error codes, overview enumeration + cap.

## Observed values (verbatim from the real-host run)

First collection (5 s after boot — no CPU interval yet):

```text
vm.cpu.cores_used             quality=insufficient_samples  value=<absent>
vm.memory.host_accounted_bytes quality=valid  value=136302592
vm.block.read_bytes_total      quality=valid  value=31562752   dim=block_device_id:_disk0
vm.block.write_bytes_total     quality=valid  value=0          dim=block_device_id:_disk0
```

Second collection (6 s later, during the noble guest's boot):

```text
vm.cpu.cores_used             quality=valid  value=1.2560770414940026  (cores)
vm.memory.host_accounted_bytes quality=valid  value=402264064
vm.block.read_bytes_total      quality=valid  value=194049024  dim=block_device_id:_disk0
vm.block.write_bytes_total     quality=valid  value=0         dim=block_device_id:_disk0
```

Both counter samples carry the same epoch fence
(`boot_id` = host boot uuid, `identity_epoch` = the VMM process's
`/proc` start-ticks identity — `pid-<pid>-start-<ticks>`), proving
same-incarnation delta-safety.

Durable ingest + read-back:

```text
g2 ingest batch 0: Accepted { samples: 4 }
g2 ingest batch 1: Accepted { samples: 4 }
```

`query_history` over the last 60 s at raw resolution returns the
`vm.cpu.cores_used` series containing the measured second observation
and the `vm.block.read_bytes_total` series with valid points — real,
bounded, persisted history on a qualified real-host VM.

Notes on the observed values:

- **31,562,752 bytes** is the exact G0b-recorded fresh-boot disk base —
  the real per-VMM-process counter starting point, not noise.
- **`vm.block.write_bytes_total` = 0 is a real zero** (the disk is
  attached `readonly=on`), distinct in kind from the absent CPU value:
  the counter is measured and validly zero; the CPU rate is honestly
  `insufficient_samples` with no value. The v1 sample path carries the
  distinction the legacy transport flattened.
- **136 MB → 402 MB RSS** is the real host-accounted VMM process memory
  while the guest touches its 512 MiB.

## Gate criteria vs. evidence

| G2 requirement | Evidence |
|---|---|
| Isolated bounded storage, durable acceptance or explicit failure | Separate SQLite `monitoring.db` (own pool/migrations/retention; `chv-monitoring-store`), typed `IngestOutcome`s; ingest integration tests: accept/dedup/replay-conflict, envelope limits, rate cap, ownership, registry validation, value/quality consistency, float-counter rejection, unavailable store (`monitoring_ingest_tests.rs`, 9 tests) |
| Rollups, reset-safe rates | idempotent raw/5m/1h rollups + epoch-fenced counter deltas (store tests, 19 tests); same-epoch fence proven live above |
| Secure history APIs | BFF `/v1/monitoring/{catalog,overview,current,history,health}` — viewer-tier, auth required, typed 400s/503, absence vocabulary, decimal-string counters (6 route tests over a real store) |
| Accurate node/VM graphs, degraded/no-data states | UI: `TargetMonitoringPanel`/`MetricChart` (gaps for non-valid points, unit badges, source/age labels, coverage), `MonitoringHealthCard`; distinct loading/degraded/error/no-data states; `Math.random` synthetic charts deleted (`NodeHealthDashboard` removed); 391 vitest tests green |
| **Monitoring store full/unavailable cannot prevent VM lifecycle** | `disk_full_degrades_monitoring_but_not_lifecycle`: typed `ingestion_unavailable`, headroom-flavored health reason, **the operational store keeps accepting state-report writes while degraded**, nothing committed is lost, automatic recovery clears degradation; the agent's batch sender owns a dedicated control-plane client so manager backpressure can never pause reconciliation |
| No guest agent or external server needed | the entire chain above runs with only the agent, the control plane, and the qualified VMM binary — no Prometheus, no OTel, no guest agent |

## Honest absences (reported, not faked)

- **Alert rules, incidents, webhooks** are PR-3+ (query/alerts contract
  v1 documents them; only the read API ships in PR-2).
- **Provider (stord/nwd) and guest-agent sources** remain unwired
  (G3/G4 scope) — no adapters faked.
- The full transport hop (agent sender → mTLS gRPC → control-plane
  service) is exercised piecewise: the sender's client/dedup/epoch
  logic is unit-tested in `cmd/chv-agent`, the service's
  validation/ownership/dedup path in `chv-controlplane-service`, and
  the wire shapes are shared generated proto types. A single-process
  real-gRPC end-to-end run is deferred to the PR-3 qualification rig.
- The VM REST-creation finding: the qualified image is qcow2 content
  under an `.img` name; the production adapter derives the VMM's
  `image_type` from the file extension and the VMM refuses the
  mismatch (`Disk image type does not match expected type`). The
  evidence run exposes the image under its true extension; the
  extension-based guess is recorded as follow-up debt on #602.

## Gate verdict

**G2 PASS** for the PR-2 scope: usable standalone native dashboards
fed by the authenticated BFF read API, real bounded persisted history
on a qualified real-host VM through the production create → sample →
ingest → query path, and the disk-full trigger proving monitoring
degrades typed-and-visible while VM lifecycle stays operational.
Deferred items are recorded above and in #602.
