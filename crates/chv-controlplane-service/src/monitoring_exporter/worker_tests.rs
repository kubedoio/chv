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

use super::remote_write::RemoteWriteBody;
use super::remote_write::WriteRequest;
use super::worker::{ExportPushError, ExportTransport, MonitoringExporter, PassOutcome};

const BASE_MS: u64 = 10_000_000_000;

/// One decoded pushed sample: (metric name, target_id, source,
/// value, timestamp).
type PushedSample = (String, String, String, f64, i64);

/// Scripted transport: records every pushed body (decoded for
/// assertions) and succeeds or fails per the toggle.
struct MockTransport {
    fail: std::sync::atomic::AtomicBool,
    pushes: std::sync::Mutex<Vec<Vec<PushedSample>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            fail: std::sync::atomic::AtomicBool::new(false),
            pushes: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn set_fail(&self, fail: bool) {
        self.fail.store(fail, std::sync::atomic::Ordering::SeqCst);
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
    let monitoring = Arc::new(
        MonitoringStore::connect(MonitoringStoreConfig {
            database_url: format!("sqlite://{}/monitoring.db", dir.path().display()),
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
        _dir: dir,
    }
}

impl Fixture {
    /// Ingest one gauge sample for a target.
    async fn seed(
        &self,
        kind: TargetKind,
        target_id: &str,
        metric_id: &str,
        source: Source,
        observed_at_ms: u64,
        value: f64,
    ) {
        let sample = SampleBuilder::new(kind, target_id, metric_id, source, observed_at_ms)
            .unwrap()
            .value(SampleValue::Float(value))
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
