//! Worker tests over a real `MonitoringStore` (the evaluator-fixture
//! pattern): construction validation, cursor advance and no-dup
//! re-export, destination-failure semantics, bounded backfill
//! clamping, filters, and transport redaction.

use std::sync::Arc;
use std::time::Duration;

use chv_config::MonitoringExportConfig;
use chv_monitoring_core::model::{SampleBuilder, SampleValue, Source, TargetKind};
use chv_monitoring_store::{IngestOutcome, MonitoringStore, MonitoringStoreConfig, NodeBatch};
use prost::Message as _;

use super::remote_write::ExportPushError;
use super::remote_write::RemoteWriteBody;
use super::remote_write::WriteRequest;
use super::worker::{ExportTransport, MonitoringExporter, PassOutcome};

const BASE_MS: u64 = 10_000_000_000;

/// One decoded pushed sample: (metric name, target_id, source,
/// value, timestamp).
type PushedSample = (String, String, String, f64, i64);

/// Scripted transport: records every pushed body (decoded for
/// assertions) and succeeds or fails per the toggle.
struct MockTransport {
    fail: std::sync::atomic::AtomicBool,
    pushes: std::sync::Mutex<Vec<Vec<PushedSample>>>,
    attempts: std::sync::atomic::AtomicUsize,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            fail: std::sync::atomic::AtomicBool::new(false),
            pushes: std::sync::Mutex::new(Vec::new()),
            attempts: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn set_fail(&self, fail: bool) {
        self.fail.store(fail, std::sync::atomic::Ordering::SeqCst);
    }

    /// Every push attempt, successful or failed — the run-loop test
    /// proves the backoff schedule by watching attempts, not pushes.
    fn attempts(&self) -> usize {
        self.attempts.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The decoded pushes: (metric name, target_id, source, value,
    /// timestamp) tuples in push order.
    fn pushed(&self) -> Vec<PushedSample> {
        self.pushes
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    fn push_count(&self) -> usize {
        self.pushes.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl ExportTransport for MockTransport {
    async fn push(&self, body: &RemoteWriteBody) -> Result<(), ExportPushError> {
        self.attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ExportPushError("connect failed".to_string()));
        }
        let mut decoder = snap::raw::Decoder::new();
        let raw = decoder
            .decompress_vec(&body.bytes)
            .expect("snappy decompress");
        let request = WriteRequest::decode(raw.as_slice()).expect("prost decode");
        let mut tuples = Vec::new();
        for ts in &request.timeseries {
            let mut name = String::new();
            let mut target_id = String::new();
            let mut source = String::new();
            for label in &ts.labels {
                match label.name.as_str() {
                    "__name__" => name = label.value.clone(),
                    "target_id" => target_id = label.value.clone(),
                    "source" => source = label.value.clone(),
                    _ => {}
                }
            }
            for sample in &ts.samples {
                tuples.push((
                    name.clone(),
                    target_id.clone(),
                    source.clone(),
                    sample.value,
                    sample.timestamp,
                ));
            }
        }
        self.pushes.lock().unwrap().push(tuples);
        Ok(())
    }
}

struct Fixture {
    exporter: MonitoringExporter,
    transport: Arc<MockTransport>,
    monitoring: Arc<MonitoringStore>,
    next_sequence: std::cell::Cell<u64>,
    db_url: String,
    _dir: tempfile::TempDir,
}

fn export_config(metric_ids: &[&str]) -> MonitoringExportConfig {
    MonitoringExportConfig {
        enabled: true,
        destination_url: Some("https://victoriametrics.internal:8428/api/v1/write".into()),
        metric_ids: metric_ids.iter().map(|s| s.to_string()).collect(),
        interval_secs: 15,
        max_backfill_secs: 600,
        lag_degraded_secs: 30,
        ..Default::default()
    }
}

async fn fixture(config: &MonitoringExportConfig) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db_url = format!("sqlite://{}/monitoring.db", dir.path().display());
    let monitoring = Arc::new(
        MonitoringStore::connect(MonitoringStoreConfig {
            database_url: db_url.clone(),
            migrations_dir: std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../cmd/chv-controlplane/monitoring-migrations"
            )),
            ..MonitoringStoreConfig::default()
        })
        .await
        .expect("connect monitoring store"),
    );
    let transport = Arc::new(MockTransport::new());
    let exporter = MonitoringExporter::new(config, monitoring.clone(), transport.clone())
        .expect("valid exporter");
    Fixture {
        exporter,
        transport,
        monitoring,
        next_sequence: std::cell::Cell::new(0),
        db_url,
        _dir: dir,
    }
}

impl Fixture {
    /// Ingest one sample for a target. Counters are seeded with
    /// integer values: the store's counter bucketing reads
    /// `value_integer` (a float counter carries no delta).
    async fn seed(
        &self,
        kind: TargetKind,
        target_id: &str,
        metric_id: &str,
        source: Source,
        observed_at_ms: u64,
        value: f64,
    ) {
        let is_counter = chv_monitoring_core::registry::lookup(metric_id)
            .map(|d| d.kind == chv_monitoring_core::model::MetricKind::Counter)
            .unwrap_or(false);
        let value = if is_counter {
            SampleValue::Integer(value as u64)
        } else {
            SampleValue::Float(value)
        };
        let sample = SampleBuilder::new(kind, target_id, metric_id, source, observed_at_ms)
            .unwrap()
            .value(value)
            // Counters require the epoch pair; gauges carry it
            // harmlessly.
            .epoch("boot-1", "identity-1")
            .build()
            .unwrap();
        let sequence = self.next_sequence.get();
        self.next_sequence.set(sequence + 1);
        let batch = NodeBatch {
            boot_id: "boot-1".to_string(),
            sequence,
            sent_at_ms: observed_at_ms,
            samples: vec![sample],
        };
        let outcome = self
            .monitoring
            .ingest_node_batch(target_id, &batch, observed_at_ms)
            .await
            .expect("ingest");
        assert!(matches!(outcome, IngestOutcome::Accepted { samples: 1 }));
    }
}

#[tokio::test]
async fn construction_rejects_invalid_allowlists_loudly() {
    let dir = tempfile::tempdir().unwrap();
    let monitoring = Arc::new(
        MonitoringStore::connect(MonitoringStoreConfig {
            database_url: format!("sqlite://{}/m.db", dir.path().display()),
            migrations_dir: std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../cmd/chv-controlplane/monitoring-migrations"
            )),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let transport = Arc::new(MockTransport::new());

    // Unknown metric id.
    let cfg = export_config(&["not.a.registry.metric"]);
    let err = MonitoringExporter::new(&cfg, monitoring.clone(), transport.clone())
        .map(|_| ())
        .expect_err("unknown metric");
    assert!(
        err.contains("not.a.registry.metric") && err.contains("registry"),
        "{err}"
    );

    // Target kind that carries no monitoring series.
    let mut cfg = export_config(&["node.cpu.capacity_ratio"]);
    cfg.target_kinds = vec!["volume".into()];
    let err = MonitoringExporter::new(&cfg, monitoring.clone(), transport.clone())
        .map(|_| ())
        .expect_err("bad kind");
    assert!(
        err.contains("target_kinds") && err.contains("node and vm"),
        "{err}"
    );

    // Unknown source vocabulary.
    let mut cfg = export_config(&["node.cpu.capacity_ratio"]);
    cfg.sources = vec!["guest".into()];
    let err = MonitoringExporter::new(&cfg, monitoring.clone(), transport.clone())
        .map(|_| ())
        .expect_err("bad source");
    assert!(err.contains("sources"), "{err}");

    // A source filter that would exclude every allowed source of the
    // metric: the metric would export nothing forever.
    let mut cfg = export_config(&["node.cpu.capacity_ratio"]);
    cfg.sources = vec!["guest_agent".into()];
    let err = MonitoringExporter::new(&cfg, monitoring, transport)
        .map(|_| ())
        .expect_err("empty filter");
    assert!(err.contains("excludes every allowed source"), "{err}");
}

#[tokio::test]
async fn passes_export_new_samples_once_and_advance_the_cursor() {
    let mut f = fixture(&export_config(&["node.cpu.capacity_ratio"])).await;
    let t0 = BASE_MS;
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0,
        0.25,
    )
    .await;
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 + 15_000,
        0.5,
    )
    .await;

    // First pass: the whole (bounded) backfill window is new.
    let now = t0 + 60_000;
    let outcome = f.exporter.run_pass(now).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert!(!f.exporter.is_degraded());
    assert_eq!(f.transport.push_count(), 1);
    assert_eq!(
        f.transport.pushed(),
        vec![
            (
                "chv_node_cpu_capacity_ratio".into(),
                "node-1".into(),
                "node_os".into(),
                0.25,
                t0 as i64
            ),
            (
                "chv_node_cpu_capacity_ratio".into(),
                "node-1".into(),
                "node_os".into(),
                0.5,
                (t0 + 15_000) as i64
            ),
        ]
    );
    assert_eq!(f.exporter.exported_samples(), 2);
    // Cursor parks at `now` after a successful pass: the whole
    // window is exported, and the lag signal measures export backlog
    // (not the data staleness of quiet targets).
    assert_eq!(f.exporter.cursor(&TargetKind::Node, "node-1"), Some(now));

    // Second pass with no new data: no push, cursor parked at now.
    let now2 = t0 + 120_000;
    let outcome = f.exporter.run_pass(now2).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert_eq!(f.transport.push_count(), 1, "no re-export without new data");
    assert_eq!(f.exporter.cursor(&TargetKind::Node, "node-1"), Some(now2));

    // New sample: exactly the delta is pushed (the timestamp is the
    // store's bucket start, within one 60 ms bucket of the observed
    // time).
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 + 130_000,
        0.75,
    )
    .await;
    let outcome = f.exporter.run_pass(t0 + 180_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert_eq!(f.transport.push_count(), 2);
    let delta = &f.transport.pushed()[2];
    assert_eq!(delta.0, "chv_node_cpu_capacity_ratio");
    assert_eq!(delta.3, 0.75);
    let ts = delta.4 as u64;
    assert!(ts <= t0 + 130_000 && ts > t0 + 130_000 - 60, "ts={ts}");
}

#[tokio::test]
async fn destination_failure_fails_the_pass_keeps_the_cursor_then_retries() {
    let mut f = fixture(&export_config(&["node.cpu.capacity_ratio"])).await;
    let t0 = BASE_MS;
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0,
        0.25,
    )
    .await;

    f.transport.set_fail(true);
    let outcome = f.exporter.run_pass(t0 + 30_000).await;
    assert_eq!(outcome, PassOutcome::DestinationFailed);
    assert!(f.exporter.is_degraded());
    assert_eq!(f.exporter.last_error(), Some("connect failed"));
    assert_eq!(f.transport.push_count(), 0);
    // Cursor unchanged (still the first-pass window start): the next
    // pass retries exactly this window — at-least-once within the
    // backfill bound.
    assert_eq!(
        f.exporter.cursor(&TargetKind::Node, "node-1"),
        Some(t0 + 30_000 - 600_000)
    );

    // Recovery: the same samples go out on the next pass.
    f.transport.set_fail(false);
    let outcome = f.exporter.run_pass(t0 + 45_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert!(!f.exporter.is_degraded());
    assert_eq!(f.transport.push_count(), 1);
    assert_eq!(f.transport.pushed().len(), 1);
}

#[tokio::test]
async fn backlog_beyond_max_backfill_is_clamped() {
    let mut cfg = export_config(&["node.cpu.capacity_ratio"]);
    cfg.max_backfill_secs = 60; // config floor
    let mut f = fixture(&cfg).await;
    let t0 = BASE_MS;
    let now = t0 + 300_000; // 5 minutes of history, 60 s window

    // One sample inside the window, one far beyond it.
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        now - 10_000,
        0.9,
    )
    .await;
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        now - 240_000,
        0.1,
    )
    .await;

    let outcome = f.exporter.run_pass(now).await;
    assert_eq!(outcome, PassOutcome::Complete);
    let pushed = f.transport.pushed();
    // Only the in-window sample crossed; the ancient one was skipped
    // by the clamp, never read or pushed. The exported timestamp is
    // the store's bucket start (query_history buckets raw points),
    // i.e. within one 60 ms bucket of the observed time.
    assert_eq!(pushed.len(), 1);
    let ts = pushed[0].4 as u64;
    assert!(ts <= now - 10_000 && ts > now - 10_000 - 60, "ts={ts}");
    assert_eq!(pushed[0].3, 0.9);
}

#[tokio::test]
async fn counter_series_export_as_same_epoch_deltas() {
    // Counters never export raw reads: the store emits same-epoch
    // deltas (the query/alerts contract), so the exported series is
    // the delta, and an isolated single counter point carries no
    // value at all (nothing to subtract).
    let mut f = fixture(&export_config(&["vm.net.rx_bytes_total"])).await;
    let t0 = BASE_MS;
    f.seed(
        TargetKind::Vm,
        "vm-1",
        "vm.net.rx_bytes_total",
        Source::Vmm,
        t0,
        1024.0,
    )
    .await;
    f.seed(
        TargetKind::Vm,
        "vm-1",
        "vm.net.rx_bytes_total",
        Source::Vmm,
        t0 + 15_000,
        2048.0,
    )
    .await;
    let outcome = f.exporter.run_pass(t0 + 60_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    let pushed = f.transport.pushed();
    assert_eq!(pushed.len(), 1, "exactly the delta point: {pushed:?}");
    assert_eq!(pushed[0].0, "chv_vm_net_rx_bytes_total");
    assert_eq!(pushed[0].3, 1024.0, "2048 - 1024, not the raw read");
}

#[tokio::test]
async fn allowlists_beyond_the_store_query_cap_are_batched() {
    // The config allows up to 64 metric ids but the store caps
    // query_history at 8 metric ids per call — the worker must batch
    // the reads and merge the series, or a 9-metric allowlist would
    // be rejected by the store and export nothing, forever, silently
    // retrying. Found by writing the g5b scale rig; pinned here.
    let nine = [
        "node.cpu.capacity_ratio",
        "node.cpu.load1",
        "node.cpu.load5",
        "node.cpu.load15",
        "node.memory.total_bytes",
        "node.memory.available_bytes",
        "node.swap.used_bytes",
        "vm.cpu.cores_used",
        "vm.memory.host_accounted_bytes",
    ];
    let mut f = fixture(&export_config(&nine)).await;
    let t0 = BASE_MS;
    // Seven node gauges on the node target, two vm gauges on the vm
    // target (the registry binds metric families to target kinds).
    for (i, metric) in nine.iter().take(7).enumerate() {
        f.seed(
            TargetKind::Node,
            "node-1",
            metric,
            Source::NodeOs,
            t0,
            i as f64,
        )
        .await;
    }
    for (i, metric) in nine.iter().skip(7).enumerate() {
        f.seed(TargetKind::Vm, "vm-1", metric, Source::Vmm, t0, i as f64)
            .await;
    }
    let outcome = f.exporter.run_pass(t0 + 30_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert_eq!(
        f.exporter.exported_samples(),
        9,
        "every metric of a >8-id allowlist must cross via chunked reads"
    );
    let pushed = f.transport.pushed();
    assert_eq!(pushed.len(), 9);
    assert!(pushed
        .iter()
        .any(|p| p.0 == "chv_node_swap_used_bytes" && p.3 == 6.0));
    assert!(pushed
        .iter()
        .any(|p| p.0 == "chv_vm_memory_host_accounted_bytes" && p.3 == 1.0));
}

#[tokio::test]
async fn empty_passes_never_push() {
    let mut f = fixture(&export_config(&["node.cpu.capacity_ratio"])).await;
    let outcome = f.exporter.run_pass(BASE_MS).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert_eq!(f.transport.push_count(), 0);
    assert!(!f.exporter.is_degraded());
    assert_eq!(f.exporter.exported_samples(), 0);
}

#[tokio::test]
async fn target_kinds_filter_excludes_other_kinds() {
    let mut cfg = export_config(&["vm.cpu.cores_used", "node.cpu.capacity_ratio"]);
    cfg.target_kinds = vec!["vm".into()];
    let mut f = fixture(&cfg).await;
    let t0 = BASE_MS;

    // A node sample (excluded by target_kinds) and two vm samples.
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0,
        0.5,
    )
    .await;
    f.seed(
        TargetKind::Vm,
        "vm-1",
        "vm.cpu.cores_used",
        Source::Vmm,
        t0,
        1.0,
    )
    .await;
    f.seed(
        TargetKind::Vm,
        "vm-2",
        "vm.cpu.cores_used",
        Source::VmCgroup,
        t0,
        3.0,
    )
    .await;

    let outcome = f.exporter.run_pass(t0 + 30_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    let pushed = f.transport.pushed();
    assert_eq!(pushed.len(), 2);
    assert!(pushed.iter().all(|p| p.1.starts_with("vm-")));
    assert!(pushed.iter().any(|p| p.1 == "vm-1" && p.3 == 1.0));
    assert!(pushed.iter().any(|p| p.1 == "vm-2" && p.3 == 3.0));
}

#[tokio::test]
async fn sources_filter_excludes_other_sources() {
    let mut cfg = export_config(&["vm.cpu.cores_used"]);
    cfg.sources = vec!["vmm".into()];
    let mut f = fixture(&cfg).await;
    let t0 = BASE_MS;

    // vm_cgroup is an allowed source of the metric but excluded by
    // the filter; only the vmm series crosses.
    f.seed(
        TargetKind::Vm,
        "vm-1",
        "vm.cpu.cores_used",
        Source::VmCgroup,
        t0,
        2.0,
    )
    .await;
    f.seed(
        TargetKind::Vm,
        "vm-1",
        "vm.cpu.cores_used",
        Source::Vmm,
        t0,
        1.0,
    )
    .await;

    let outcome = f.exporter.run_pass(t0 + 30_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    let pushed = f.transport.pushed();
    assert_eq!(pushed.len(), 1);
    assert_eq!(pushed[0].1, "vm-1");
    assert_eq!(pushed[0].2, "vmm");
    assert_eq!(pushed[0].3, 1.0);
}

#[tokio::test]
async fn export_transport_errors_never_expose_the_destination_url() {
    // Same contract as the notification dispatcher: reqwest's Display
    // embeds the URL, and a remote-write URL can carry a credential
    // in its path or query. The redacted classification must contain
    // none of it.
    let client = reqwest::Client::builder().https_only(true).build().unwrap();
    let secret = "token-secret-in-path";
    let error = client
        .post(format!("https://nonexistent.invalid/write/{secret}"))
        .body(vec![0u8; 4])
        .send()
        .await
        .expect_err("the .invalid domain cannot resolve");
    let raw = error.to_string();
    assert!(raw.contains("nonexistent.invalid"), "sanity: {raw}");
    let redacted = super::worker::redact_transport_error(&error);
    assert!(
        !redacted.contains("nonexistent.invalid")
            && !redacted.contains(secret)
            && !redacted.contains("https://"),
        "the redacted classification must not carry the destination: {redacted}"
    );
}

#[test]
fn settings_are_bounded_by_the_config_validation() {
    // The worker trusts the config layer's bounds; pin the
    // relationship the degraded marker relies on.
    let cfg = export_config(&["node.cpu.capacity_ratio"]);
    assert!(cfg.lag_degraded_secs <= cfg.max_backfill_secs);
    assert!(Duration::from_secs(cfg.interval_secs) >= Duration::from_secs(5));
}

#[test]
fn export_telemetry_is_additive_to_the_preserved_instrumentation() {
    // Prompt-05 task 7 first clause: PRESERVE the existing
    // `/metrics` instrumentation and the `monitoring/rules/*.yml`
    // files that consume it. This pin holds the additive direction:
    // the three export gauges are new, distinct names, and no
    // exported monitoring series can ever collide with a preserved
    // control-plane family (a receiver — and the rule files — must
    // never conflate the two).
    use chv_observability::{
        CHV_CP_CONSECUTIVE_DRIFT_TICKS, CHV_CP_CONVERGENCE_AVG_MS, CHV_CP_DRIFT_COUNT,
        CHV_CP_PENDING_OPERATIONS, CHV_EBPF_BYTES_TOTAL, CHV_EBPF_PACKETS_TOTAL,
        CHV_GRPC_SERVER_DURATION_SECONDS, CHV_GRPC_SERVER_REQUESTS_TOTAL,
        CHV_MIGRATION_BYTES_TRANSFERRED, CHV_MIGRATION_DIRTY_BLOCKS,
        CHV_MIGRATION_DURATION_SECONDS, CHV_MIGRATION_PHASE, CHV_NODES_READY,
        CHV_OPERATION_DURATION_SECONDS, CHV_VMS_TOTAL, CHV_VM_OPS_TOTAL,
        CHV_VM_OP_DURATION_SECONDS, CHV_VXLAN_FDB_ENTRIES, NWD_FABRIC_APPLY_TOTAL,
        NWD_FABRIC_REMOVE_TOTAL,
    };
    let preserved = [
        CHV_VMS_TOTAL,
        CHV_NODES_READY,
        CHV_OPERATION_DURATION_SECONDS,
        CHV_GRPC_SERVER_REQUESTS_TOTAL,
        CHV_GRPC_SERVER_DURATION_SECONDS,
        CHV_VM_OPS_TOTAL,
        CHV_VM_OP_DURATION_SECONDS,
        CHV_MIGRATION_PHASE,
        CHV_MIGRATION_BYTES_TRANSFERRED,
        CHV_MIGRATION_DURATION_SECONDS,
        CHV_MIGRATION_DIRTY_BLOCKS,
        CHV_VXLAN_FDB_ENTRIES,
        CHV_EBPF_PACKETS_TOTAL,
        CHV_EBPF_BYTES_TOTAL,
        CHV_CP_DRIFT_COUNT,
        CHV_CP_PENDING_OPERATIONS,
        CHV_CP_CONVERGENCE_AVG_MS,
        CHV_CP_CONSECUTIVE_DRIFT_TICKS,
        NWD_FABRIC_APPLY_TOTAL,
        NWD_FABRIC_REMOVE_TOTAL,
    ];
    for name in [
        super::worker::METRIC_EXPORT_LAG_SECONDS,
        super::worker::METRIC_EXPORT_DEGRADED,
        super::worker::METRIC_EXPORT_SAMPLES_TOTAL,
    ] {
        assert!(!preserved.contains(&name), "export gauge {name} collides");
    }
    for def in chv_monitoring_core::registry::all() {
        let name = chv_monitoring_core::registry::prometheus_name(def.id).unwrap();
        assert!(
            !preserved.contains(&name.as_str()),
            "exported series name {name} collides with a preserved family"
        );
    }
}

#[tokio::test]
async fn store_read_failures_are_not_destination_failures() {
    // A store that cannot be read must fail the pass as a STORE
    // failure: no destination-degraded marking, no destination
    // backoff ladder — mislabeling a local problem as a destination
    // outage would both lie on the gauge and delay recovery. The
    // pass retries at the normal cadence.
    let mut f = fixture(&export_config(&["node.cpu.capacity_ratio"])).await;
    let t0 = BASE_MS;
    f.seed(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0,
        0.5,
    )
    .await;
    let outcome = f.exporter.run_pass(t0 + 30_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert!(!f.exporter.is_degraded());

    // Break the store's series table out from under the exporter.
    let pool = sqlx::SqlitePool::connect(&f.db_url).await.unwrap();
    sqlx::query("DROP TABLE monitoring_series")
        .execute(&pool)
        .await
        .unwrap();

    let outcome = f.exporter.run_pass(t0 + 60_000).await;
    assert_eq!(
        outcome,
        PassOutcome::StoreFailed,
        "a store read failure must be typed as StoreFailed, not DestinationFailed"
    );
    assert!(
        !f.exporter.is_degraded(),
        "a store read failure must not mark the destination degraded"
    );
    assert!(
        f.exporter.last_error().unwrap().contains("list_targets"),
        "the failure must name the failed read: {:?}",
        f.exporter.last_error()
    );
    assert_eq!(f.exporter.exported_samples(), 1, "nothing new exported");
}

/// Poll until `cond` holds, panicking with the timeout otherwise.
/// The run-loop test below watches real-time cadence and backoff, so
/// it polls with generous deadlines instead of sleeping blindly.
async fn eventually(timeout: std::time::Duration, cond: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + timeout;
    while !cond() {
        assert!(
            std::time::Instant::now() < deadline,
            "run-loop condition not met within {timeout:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn run_loop_exports_on_cadence_backs_off_recovers_and_stops() {
    // The production entry point (`run`) — its cadence sleep, its
    // destination backoff with jitter, and its shutdown — none of
    // which the run_pass tests reach. The worker honors the
    // configured interval verbatim; 1 s keeps the loop observable
    // without waiting out the config-validation floor. run() stamps
    // passes with the real clock, so every seed lives in the real
    // now, and each phase seeds fresh data: a quiet target produces
    // no push, which would make cadence unobservable.
    let mut config = export_config(&["node.cpu.capacity_ratio"]);
    config.interval_secs = 1;
    let f = fixture(&config).await;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    f.seed(
        TargetKind::Node,
        "node-run-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t - 30_000,
        0.5,
    )
    .await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    // The task owns the fixture (and therefore the tempdir) for as
    // long as it runs; the test observes through the shared
    // transport and seeds through the shared store.
    let transport = f.transport.clone();
    let monitoring = f.monitoring.clone();
    let mut sequence = 1u64;
    let handle = tokio::spawn(async move { f.exporter.run(shutdown_rx).await });

    // A fresh sample at the real now: the cursor parks at each pass
    // time, so anything observed before the last pass is outside
    // every future window — the seed must be at (or after) the
    // current instant for the NEXT pass to carry it. (The closure
    // form cannot express an async block borrowing its argument.)
    async fn seed_fresh(monitoring: &MonitoringStore, sequence: &mut u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let sample = SampleBuilder::new(
            TargetKind::Node,
            "node-run-1",
            "node.cpu.capacity_ratio",
            Source::NodeOs,
            now,
        )
        .unwrap()
        .value(SampleValue::Float(0.5))
        .epoch("boot-1", "identity-1")
        .build()
        .unwrap();
        let seq = *sequence;
        *sequence += 1;
        let batch = NodeBatch {
            boot_id: "boot-1".to_string(),
            sequence: seq,
            sent_at_ms: now,
            samples: vec![sample],
        };
        let outcome = monitoring
            .ingest_node_batch("node-run-1", &batch, now)
            .await
            .unwrap();
        assert!(
            matches!(outcome, IngestOutcome::Accepted { .. }),
            "{outcome:?}"
        );
    }

    // First pass lands within the cadence and exports the seed.
    eventually(std::time::Duration::from_secs(15), || {
        transport.push_count() >= 1
    })
    .await;

    // Cadence continues: fresh data on the next interval is picked
    // up by the NEXT pass, not lost.
    seed_fresh(&monitoring, &mut sequence).await;
    eventually(std::time::Duration::from_secs(10), || {
        transport.push_count() >= 2
    })
    .await;

    // Destination outage: the next cadence pass with data fails at
    // the push, and the retry schedule is the backoff ladder (5 s ±
    // 20% at one failure — at least 4 s), not the 1 s cadence.
    transport.set_fail(true);
    seed_fresh(&monitoring, &mut sequence).await;
    let baseline = transport.attempts();
    eventually(std::time::Duration::from_secs(10), || {
        transport.attempts() > baseline
    })
    .await;
    let failed_attempt = transport.attempts();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert_eq!(
        transport.attempts(),
        failed_attempt,
        "the retry after a destination failure must be held back by the backoff ladder"
    );

    // Recovery: the destination returns, the held window is pushed,
    // the loop keeps running.
    transport.set_fail(false);
    eventually(std::time::Duration::from_secs(20), || {
        transport.attempts() > failed_attempt
    })
    .await;
    eventually(std::time::Duration::from_secs(5), || {
        transport.push_count() >= 3
    })
    .await;

    // Shutdown: the loop exits cleanly.
    drop(shutdown_tx);
    tokio::time::timeout(std::time::Duration::from_secs(10), handle)
        .await
        .expect("run() must exit after shutdown")
        .unwrap();
}

#[tokio::test]
async fn export_gauges_reach_the_prometheus_exposition() {
    // The disjointness pin above proves the export gauge NAMES are
    // safe; this test proves the WIRING — a running exporter's
    // gauges reach the exact exposition the admin /metrics route
    // renders (the route serves `prometheus_handle()`'s render, so
    // what this test reads is what an operator scrapes). Together
    // with the route test in `tests.rs`
    // (`webui_metrics_stays_prometheus_and_renamed_ui_route_serves_the_spa`)
    // this is the full pin the interface asked for.
    chv_observability::install_prometheus_recorder().expect("install prometheus recorder");
    let handle = chv_observability::prometheus_handle().expect("recorder handle");

    let mut f = fixture(&export_config(&["node.cpu.capacity_ratio"])).await;
    let t0 = BASE_MS;
    f.seed(
        TargetKind::Node,
        "node-gauge-pin",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0,
        0.5,
    )
    .await;
    let outcome = f.exporter.run_pass(t0 + 30_000).await;
    assert_eq!(outcome, PassOutcome::Complete);
    assert_eq!(f.exporter.exported_samples(), 1);

    let exposition = handle.render();
    for name in [
        "chv_export_lag_seconds",
        "chv_export_degraded",
        "chv_export_samples_total",
    ] {
        assert!(
            exposition.contains(name),
            "{name} missing from the /metrics exposition:\n{exposition}"
        );
    }
    assert!(
        exposition.contains("chv_export_samples_total{outcome=\"exported\"}"),
        "the exported-samples counter must carry its outcome label:\n{exposition}"
    );
}
