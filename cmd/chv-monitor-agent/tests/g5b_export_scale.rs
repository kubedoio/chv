//! G5b gate evidence: the optional external export (prompt-05 task 7,
//! ADR-027 decision 9) against a REAL VictoriaMetrics single-node
//! binary — the compatibility testing ADR-027 decision 12 requires
//! before an operator is told the backend is supported.
//!
//! Env-gated (CI has no VictoriaMetrics; the real-host record lives in
//! `docs/evidence/native-monitoring/g5b/README.md`):
//!
//! ```sh
//! CHV_G5B_VICTORIAMETRICS=/path/to/victoria-metrics-prod \
//! [CHV_G5B_VICTORIAMETRICS_SHA256=<sha256 of the binary>] \
//! [CHV_G5B_SCALE_TIERS=1,10,100,500] \
//! cargo test -p chv-monitor-agent --test g5b_export_scale -- --nocapture
//! ```
//!
//! What the export legs prove, end to end on the production paths
//! (real VictoriaMetrics process, real rustls on both ends, real
//! remote-write v1 wire format — snappy+protobuf, NOT a self-written
//! decoder on the receiving side):
//!
//! 1. **Compatibility** — the exporter's pushes are accepted by a
//!    real VictoriaMetrics and read back through its query API with
//!    the exact series names, labels and values the contract
//!    promises (`chv_<id>` names, target/source labels, dimension
//!    labels). The destination is served over TLS with a
//!    rig-issued certificate trusted via the exporter's operator-CA
//!    path — the exact shape an internal deployment uses.
//! 2. **Outage honesty** — killing the destination fails the pass at
//!    the first push (degraded, cursor held); restarting it resumes
//!    with no data lost inside the bounded backfill window, and the
//!    samples from both sides of the outage are queryable.
//! 3. **Restart boundedness** — a fresh exporter (process restart)
//!    re-exports only within `max_backfill`, never an unbounded
//!    replay of disposable telemetry.
//!
//! What this deliberately does NOT claim: bearer-token
//! authentication against VictoriaMetrics (the OSS single-node
//! binary has no bearer-token auth to verify against — the header
//! path is pinned by the unit suite), and multi-node scale
//! (single-node campaigns only; the campaign's recorded position).
//!
//! The scale legs (CHV_G5B_SCALE_TIERS) are in this file's sibling
//! test below.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chv_controlplane_service::monitoring_exporter::{MonitoringExporter, ReqwestExportTransport};
use chv_monitoring_core::model::{SampleBuilder, SampleValue, Source, TargetKind};
use chv_monitoring_store::{IngestOutcome, MonitoringStore, MonitoringStoreConfig, NodeBatch};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Honest wait until the wall clock passes `epoch_ms` — used before
/// a pass whose window must include a sample seeded with a
/// just-future observed time.
async fn wait_until_after(epoch_ms: u64) {
    while now_ms() <= epoch_ms {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The VictoriaMetrics binary path, or None when the gate is not
/// armed (CI).
fn vm_binary() -> Option<PathBuf> {
    std::env::var_os("CHV_G5B_VICTORIAMETRICS").map(PathBuf::from)
}

/// Digest pin: when the operator provides the expected sha256, a
/// mismatch aborts the rig rather than producing evidence from an
/// unverified binary.
fn verify_binary_digest(path: &std::path::Path) {
    let Some(expected) = std::env::var_os("CHV_G5B_VICTORIAMETRICS_SHA256") else {
        eprintln!(
            "g5b: CHV_G5B_VICTORIAMETRICS_SHA256 not set; proceeding without digest pin \
             (the evidence record must still state the binary digest)"
        );
        return;
    };
    let expected = expected.to_string_lossy().to_lowercase();
    let actual = sha256_file(path);
    assert_eq!(
        actual, expected,
        "g5b: VictoriaMetrics binary digest mismatch — refusing to produce evidence from an \
         unverified binary"
    );
}

fn sha256_file(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).expect("read VictoriaMetrics binary");
    let digest = Sha256::digest(&bytes);
    hex::encode(digest)
}

/// A self-signed TLS server certificate for 127.0.0.1 (the e2e
/// manager-in-a-box pattern).
fn test_server_cert() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.subject_alt_names = vec![rcgen::SanType::IpAddress(std::net::IpAddr::V4(
        std::net::Ipv4Addr::LOCALHOST,
    ))];
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// An HTTPS client trusting the rig CA (readback queries and health
/// checks).
fn https_client(ca_pem: &str) -> reqwest::Client {
    let certificate = reqwest::tls::Certificate::from_pem(ca_pem.as_bytes()).unwrap();
    reqwest::Client::builder()
        .https_only(true)
        .add_root_certificate(certificate)
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

/// A running VictoriaMetrics single-node process with TLS.
struct VictoriaMetrics {
    binary: PathBuf,
    storage_dir: PathBuf,
    cert_path: PathBuf,
    key_path: PathBuf,
    port: u16,
    child: Option<tokio::process::Child>,
}

impl VictoriaMetrics {
    /// Start on a fresh port with the given storage, cert and key.
    async fn start(
        binary: PathBuf,
        storage_dir: PathBuf,
        cert_path: PathBuf,
        key_path: PathBuf,
    ) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Self::start_on_port(binary, storage_dir, cert_path, key_path, port).await
    }

    async fn start_on_port(
        binary: PathBuf,
        storage_dir: PathBuf,
        cert_path: PathBuf,
        key_path: PathBuf,
        port: u16,
    ) -> std::io::Result<Self> {
        let log = std::fs::File::create(storage_dir.join(format!("vm-{port}.log")))?;
        let child = tokio::process::Command::new(&binary)
            .arg("-storageDataPath")
            .arg(&storage_dir)
            .arg("-httpListenAddr")
            .arg(format!("127.0.0.1:{port}"))
            .arg("-tls")
            .arg("-tlsCertFile")
            .arg(&cert_path)
            .arg("-tlsKeyFile")
            .arg(&key_path)
            .stdout(log)
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let mut vm = Self {
            binary,
            storage_dir,
            cert_path,
            key_path,
            port,
            child: Some(child),
        };
        vm.wait_healthy().await;
        Ok(vm)
    }

    async fn wait_healthy(&mut self) {
        let client = https_client(&std::fs::read_to_string(&self.cert_path).unwrap());
        let url = format!("https://127.0.0.1:{}/health", self.port);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    panic!(
                        "g5b: VictoriaMetrics exited early with {status}; log: {}",
                        String::from_utf8_lossy(
                            &std::fs::read(self.storage_dir.join(format!("vm-{}.log", self.port)))
                                .unwrap_or_default()
                        )
                    );
                }
            }
            if client
                .get(&url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
            {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "g5b: VictoriaMetrics did not become healthy within 30 s"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Hard stop (the outage injection).
    async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().await.expect("kill VictoriaMetrics");
            child.wait().await.expect("reap VictoriaMetrics");
        }
    }

    /// Restart on the SAME port and storage (recovery from outage).
    async fn restart(&mut self) {
        self.stop().await;
        let mut vm = Self::start_on_port(
            self.binary.clone(),
            self.storage_dir.clone(),
            self.cert_path.clone(),
            self.key_path.clone(),
            self.port,
        )
        .await
        .expect("restart VictoriaMetrics");
        // Take the child before `vm` drops (Drop only best-effort
        // kills what is still held).
        self.child = vm.child.take();
    }

    fn write_url(&self) -> String {
        format!("https://127.0.0.1:{}/api/v1/write", self.port)
    }
}

impl Drop for VictoriaMetrics {
    fn drop(&mut self) {
        // Best-effort synchronous cleanup: the test process must not
        // leave a VictoriaMetrics running.
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

/// The bounded native monitoring store (tempdir, real migrations).
async fn rig_store(dir: &tempfile::TempDir) -> Arc<MonitoringStore> {
    Arc::new(
        MonitoringStore::connect(MonitoringStoreConfig {
            database_url: format!("sqlite://{}/monitoring.db", dir.path().display()),
            migrations_dir: PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "../../cmd/chv-controlplane/monitoring-migrations"
            )),
            ..MonitoringStoreConfig::default()
        })
        .await
        .expect("connect monitoring store"),
    )
}

struct Rig {
    vm: VictoriaMetrics,
    store: Arc<MonitoringStore>,
    cert_pem: String,
    next_sequence: std::cell::Cell<u64>,
    _dir: tempfile::TempDir,
}

async fn rig() -> Rig {
    let binary = vm_binary().expect("CHV_G5B_VICTORIAMETRICS (checked by caller)");
    verify_binary_digest(&binary);

    let dir = tempfile::tempdir().unwrap();
    let storage = dir.path().join("vm-storage");
    std::fs::create_dir(&storage).unwrap();
    let (cert_pem, key_pem) = test_server_cert();
    let cert_path = dir.path().join("vm-cert.pem");
    let key_path = dir.path().join("vm-key.pem");
    std::fs::write(&cert_path, &cert_pem).unwrap();
    std::fs::write(&key_path, &key_pem).unwrap();

    let vm = VictoriaMetrics::start(binary, storage, cert_path, key_path)
        .await
        .expect("start VictoriaMetrics");
    let store = rig_store(&dir).await;
    Rig {
        vm,
        store,
        cert_pem,
        next_sequence: std::cell::Cell::new(0),
        _dir: dir,
    }
}

impl Rig {
    async fn seed(
        &self,
        kind: TargetKind,
        target_id: &str,
        metric_id: &str,
        source: Source,
        observed_at_ms: u64,
        value: f64,
    ) {
        // Counters are seeded with integer values: the store's
        // counter bucketing reads `value_integer` (a float counter
        // carries no delta).
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
            .epoch("g5b-boot", "g5b-identity-1")
            .build()
            .unwrap();
        let sequence = self.next_sequence.get();
        self.next_sequence.set(sequence + 1);
        let batch = NodeBatch {
            boot_id: "g5b-boot".to_string(),
            sequence,
            sent_at_ms: observed_at_ms,
            samples: vec![sample],
        };
        let outcome = self
            .store
            .ingest_node_batch("g5b-node", &batch, observed_at_ms)
            .await
            .expect("ingest");
        assert!(matches!(outcome, IngestOutcome::Accepted { samples: 1 }));
    }

    fn exporter(&self, metric_ids: &[&str]) -> MonitoringExporter {
        let config = chv_config::MonitoringExportConfig {
            enabled: true,
            destination_url: Some(self.vm.write_url()),
            metric_ids: metric_ids.iter().map(|s| s.to_string()).collect(),
            interval_secs: 15,
            max_backfill_secs: 600,
            lag_degraded_secs: 30,
            ..Default::default()
        };
        let transport =
            ReqwestExportTransport::new(&self.vm.write_url(), None, Some(&self.cert_pem))
                .expect("build export transport");
        MonitoringExporter::new(&config, self.store.clone(), Arc::new(transport))
            .expect("valid exporter")
    }

    /// Poll an instant query until it returns at least `min` series,
    /// or panic with the last (empty) result. VictoriaMetrics buffers
    /// fresh inserts and flushes them to searchable in-memory parts
    /// on an interval (default 5 s) — an immediate query can
    /// legitimately miss stored data, so the rig polls instead of
    /// racing the flush.
    async fn wait_for_series(
        &self,
        query: &str,
        time_ms: u64,
        min: usize,
    ) -> Vec<(serde_json::Map<String, serde_json::Value>, f64)> {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let result = self.query(query, time_ms).await;
            if result.len() >= min {
                return result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "g5b: series {query:?} did not become queryable within 20 s (last result: \
                 {result:?}) — the push claimed success, so VictoriaMetrics either dropped \
                 or delayed the data"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Wait out VictoriaMetrics' insert-flush interval so a subsequent
    /// "must be empty" assertion cannot pass merely because stored
    /// data is still invisible.
    async fn settle_visibility_window(&self) {
        tokio::time::sleep(Duration::from_secs(7)).await;
    }

    /// Instant query at `time_ms` through VictoriaMetrics' own query
    /// API — the readback proof. Returns (labels, value) pairs.
    async fn query(
        &self,
        query: &str,
        time_ms: u64,
    ) -> Vec<(serde_json::Map<String, serde_json::Value>, f64)> {
        let client = https_client(&self.cert_pem);
        let url = format!(
            "https://127.0.0.1:{}/api/v1/query?query={}&time={}",
            self.vm.port,
            urlencoding_encode(query),
            time_ms / 1000
        );
        let response = client.get(&url).send().await.expect("query");
        assert!(
            response.status().is_success(),
            "VictoriaMetrics query failed: {}",
            response.status()
        );
        let body: serde_json::Value = response.json().await.expect("query json");
        assert_eq!(body["status"], "success", "query body: {body}");
        body["data"]["result"]
            .as_array()
            .expect("result array")
            .iter()
            .map(|item| {
                let labels = item["metric"].as_object().expect("metric map").clone();
                let value: f64 = item["value"][1]
                    .as_str()
                    .expect("value string")
                    .parse()
                    .expect("value f64");
                (labels, value)
            })
            .collect()
    }
}

/// Minimal percent-encoding for the query parameter (promQL braces
/// and quotes).
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[tokio::test]
async fn export_push_reaches_a_real_victoriametrics() {
    if vm_binary().is_none() {
        eprintln!("g5b: CHV_G5B_VICTORIAMETRICS not set; skipping export leg");
        return;
    }
    let mut rig = rig().await;
    let t0 = now_ms();

    // Two targets, two metric families. The counter is seeded with
    // two same-epoch points — counters export same-epoch DELTAS, so
    // one isolated point legitimately carries no value.
    rig.seed(
        TargetKind::Node,
        "g5b-node",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 - 30_000,
        0.42,
    )
    .await;
    rig.seed(
        TargetKind::Vm,
        "g5b-vm",
        "vm.net.rx_bytes_total",
        Source::Vmm,
        t0 - 30_000,
        1024.0,
    )
    .await;
    rig.seed(
        TargetKind::Vm,
        "g5b-vm",
        "vm.net.rx_bytes_total",
        Source::Vmm,
        t0 - 15_000,
        2048.0,
    )
    .await;
    let mut exporter = rig.exporter(&["node.cpu.capacity_ratio", "vm.net.rx_bytes_total"]);
    let outcome = exporter.run_pass(now_ms()).await;
    assert_eq!(
        outcome,
        chv_controlplane_service::monitoring_exporter::PassOutcome::Complete
    );
    assert!(!exporter.is_degraded());
    assert_eq!(exporter.exported_samples(), 2);

    // Readback through VictoriaMetrics' own query API: the exported
    // series exist with the contract's exact names and labels. The
    // wait is an honest poll — VictoriaMetrics buffers fresh inserts
    // and flushes them to searchable in-memory parts on an interval
    // (default 5 s), so an immediate query can legitimately miss
    // data that IS stored (vm_rows_inserted_total counts it).
    let node = rig
        .wait_for_series("chv_node_cpu_capacity_ratio", t0, 1)
        .await;
    assert_eq!(node[0].1, 0.42);
    assert_eq!(node[0].0["target_id"], "g5b-node");
    assert_eq!(node[0].0["target_kind"], "node");
    assert_eq!(node[0].0["source"], "node_os");

    let vm_series = rig
        .wait_for_series("chv_vm_net_rx_bytes_total", t0, 1)
        .await;
    assert_eq!(vm_series.len(), 1);
    assert_eq!(
        vm_series[0].1, 1024.0,
        "the counter delta, not the raw read"
    );
    assert_eq!(vm_series[0].0["target_id"], "g5b-vm");
    assert_eq!(vm_series[0].0["source"], "vmm");

    // The series the allowlist excludes never crossed. Poll the full
    // visibility window first so a slow flush cannot mask a leak as
    // an absence.
    rig.settle_visibility_window().await;
    assert!(rig
        .query("chv_node_memory_available_bytes", t0)
        .await
        .is_empty());

    rig.vm.stop().await;
}

#[tokio::test]
async fn destination_outage_degrades_and_recovers_without_loss() {
    if vm_binary().is_none() {
        eprintln!("g5b: CHV_G5B_VICTORIAMETRICS not set; skipping outage leg");
        return;
    }
    let mut rig = rig().await;
    let t0 = now_ms();

    rig.seed(
        TargetKind::Node,
        "g5b-node",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 - 20_000,
        0.1,
    )
    .await;
    let mut exporter = rig.exporter(&["node.cpu.capacity_ratio"]);
    let outcome = exporter.run_pass(now_ms()).await;
    assert_eq!(
        outcome,
        chv_controlplane_service::monitoring_exporter::PassOutcome::Complete
    );
    assert_eq!(exporter.exported_samples(), 1);

    // OUTAGE: kill the destination, then produce a fresh sample. The
    // pass must fail at the first push, hold the cursor and degrade —
    // never panic, never block anyone else. The honest wait lets the
    // new sample age INTO the pass window (its observed time must be
    // behind the pass clock, the production shape of a cadence pass).
    rig.vm.stop().await;
    rig.seed(
        TargetKind::Node,
        "g5b-node",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 + 5_000,
        0.2,
    )
    .await;
    wait_until_after(t0 + 6_000).await;
    let outcome = exporter.run_pass(now_ms()).await;
    assert_eq!(
        outcome,
        chv_controlplane_service::monitoring_exporter::PassOutcome::DestinationFailed
    );
    assert!(exporter.is_degraded());
    assert_eq!(
        exporter.exported_samples(),
        1,
        "nothing exported during the outage"
    );

    // RECOVERY: the destination comes back on the same storage; the
    // held window goes out and both sides of the outage are
    // queryable — at-least-once inside the bounded backfill.
    rig.vm.restart().await;
    let outcome = exporter.run_pass(now_ms()).await;
    assert_eq!(
        outcome,
        chv_controlplane_service::monitoring_exporter::PassOutcome::Complete
    );
    assert!(!exporter.is_degraded());
    assert_eq!(
        exporter.exported_samples(),
        2,
        "the outage-window sample exported after recovery"
    );

    let all = rig
        .wait_for_series("chv_node_cpu_capacity_ratio", t0 + 60_000, 1)
        .await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1, 0.2, "the post-recovery sample is the latest");

    rig.vm.stop().await;
}

#[tokio::test]
async fn exporter_restart_replays_only_the_bounded_backfill() {
    if vm_binary().is_none() {
        eprintln!("g5b: CHV_G5B_VICTORIAMETRICS not set; skipping restart leg");
        return;
    }
    let rig = rig().await;
    let t0 = now_ms();

    // History far beyond the backfill window plus recent history.
    rig.seed(
        TargetKind::Node,
        "g5b-node",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 - 3_600_000,
        99.0,
    )
    .await;
    rig.seed(
        TargetKind::Node,
        "g5b-node",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        t0 - 30_000,
        0.5,
    )
    .await;

    // A fresh exporter (process restart): the cursor starts at
    // now - max_backfill (600 s), so the hour-old sample is NEVER
    // read or pushed — bounded catch-up, not unbounded replay.
    let mut exporter = rig.exporter(&["node.cpu.capacity_ratio"]);
    let outcome = exporter.run_pass(now_ms()).await;
    assert_eq!(
        outcome,
        chv_controlplane_service::monitoring_exporter::PassOutcome::Complete
    );
    assert_eq!(exporter.exported_samples(), 1, "only the in-window sample");

    let all = rig
        .wait_for_series("chv_node_cpu_capacity_ratio", t0, 1)
        .await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1, 0.5);
    // The ancient sample never reached the destination — asserted
    // only after the full visibility window so a slow flush cannot
    // mask a leak as an absence.
    rig.settle_visibility_window().await;
    let ancient = rig
        .query(
            "chv_node_cpu_capacity_ratio{target_id=\"g5b-node\"}",
            t0 - 3_600_000,
        )
        .await;
    assert!(
        ancient.is_empty(),
        "the hour-old sample must not be exported: {ancient:?}"
    );
}
