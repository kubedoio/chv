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
//! The scale legs (CHV_G5B_SCALE_TIERS, the sibling test at the
//! bottom of this file) qualify the monitoring system's resource
//! curve at fleet tiers: node-batch ingest shape, guest mTLS shape,
//! series fan-out per target, query latency percentiles and export
//! pass cost against the live destination.

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

/// A throwaway self-signed agent CA (the e2e manager-in-a-box
/// pattern) — signs the enrolled guest client certificates.
fn test_ca() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "g5b-scale-agent-ca");
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
            // Rig-only knob, and an honest one: VictoriaMetrics
            // persists in-memory data to disk parts on the
            // `-inmemoryDataFlushInterval` cadence (default 5 s), and
            // only DISK parts survive a SIGKILL. Measured on this
            // exact pinned binary: inserts ≤5 s old are still lost on
            // SIGKILL even at the 1 s minimum (fresh inserts sit in a
            // pre-part buffer whose formation dominates), while ≥10 s
            // old inserts survive. The outage leg therefore waits out
            // a measured window — see the comment there — before the
            // kill, so the receiver's crash window is never the
            // reason the pre-outage readback fails.
            .arg("-inmemoryDataFlushInterval=1s")
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

    /// Wait until the instant query at `time_ms` reports `expected`
    /// as the latest value of the (single) series. Presence alone is
    /// not enough when older samples are already queryable: a sample
    /// loaded from disk parts answers immediately while a freshly
    /// pushed one needs VictoriaMetrics' insert-visibility window,
    /// so a `wait_for_series(.., 1)` poll would return early with the
    /// OLD value and the "latest" assertion would race.
    async fn wait_for_latest_value(&self, query: &str, time_ms: u64, expected: f64) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let result = self.query(query, time_ms).await;
            if result.len() == 1 && (result[0].1 - expected).abs() < 1e-9 {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "g5b: series {query:?} did not report {expected} as the latest value \
                 within 20 s (last result: {result:?})"
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

    // Let the receiver make the pushed sample SIGKILL-durable before
    // the outage. Measured on the pinned VictoriaMetrics binary with
    // the rig's 1 s flush flag: inserts ≤5 s old were lost on
    // SIGKILL (pre-part buffer), ≥10 s old survived — so the leg
    // waits 20 s, 2× the highest surviving-measurement boundary. The
    // EXPORTER has done its job the moment the push was accepted;
    // this wait is about the receiver's crash window, not ours.
    tokio::time::sleep(Duration::from_secs(20)).await;

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

    // The post-recovery sample must be the LATEST at a later query
    // time. Value-aware wait: the pre-outage sample (retained on
    // disk) answers instantly, while this freshly pushed one needs
    // the insert-visibility window first.
    rig.wait_for_latest_value("chv_node_cpu_capacity_ratio", t0 + 60_000, 0.2)
        .await;

    // BOTH sides of the outage are queryable at their own times: the
    // pre-outage sample (pushed before the outage) must still be
    // readable at its timestamp — a receiver that dropped the oldest
    // sample would otherwise pass the assertion above.
    let pre = rig
        .wait_for_series("chv_node_cpu_capacity_ratio", t0, 1)
        .await;
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0].1, 0.1, "the pre-outage sample survived the outage");

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

// ---------------------------------------------------------------------------
// G5b scale legs (prompt-06): the monitoring system's resource curve at
// fleet tiers. Node-batch ingest shape through the real store write
// path, guest shape through the real mTLS enrollment + delivery path,
// series fan-out per target, query latency percentiles, and export
// pass cost against the live VictoriaMetrics destination. Every number
// the evidence record cites is printed by this test under --nocapture.
// ---------------------------------------------------------------------------

/// Tier list from CHV_G5B_SCALE_TIERS ("1,10,100,500"); None when the
/// gate is not armed.
fn parse_tiers() -> Option<Vec<usize>> {
    let raw = std::env::var("CHV_G5B_SCALE_TIERS").ok()?;
    let tiers = raw
        .split(',')
        .map(|t| t.trim().parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .expect("CHV_G5B_SCALE_TIERS must be comma-separated usize tiers");
    assert!(!tiers.is_empty(), "CHV_G5B_SCALE_TIERS must not be empty");
    Some(tiers)
}

/// One series of the node-batch scale shape: a real registry metric a
/// node reports for a VM through the store's node-batch write path
/// (the terminal write path the gRPC ingest service lands in — the
/// transport layer itself is measured by the guest mTLS leg, not this
/// one) (dimensionless gauges, then net/block counter families fanned
/// out over interfaces/devices — the production shape of a busy VM).
struct SeriesSpec {
    metric_id: &'static str,
    dimension: Option<(&'static str, String)>,
    counter: bool,
}

/// `n` series per VM target from the closed registry vocabulary.
fn vm_series_set(n: usize) -> Vec<SeriesSpec> {
    let mut out = Vec::new();
    if n >= 1 {
        out.push(SeriesSpec {
            metric_id: "vm.cpu.cores_used",
            dimension: None,
            counter: false,
        });
    }
    if n >= 2 {
        out.push(SeriesSpec {
            metric_id: "vm.memory.host_accounted_bytes",
            dimension: None,
            counter: false,
        });
    }
    // Counter families: net (interface_id) then block (block_device_id),
    // each pair adding two series.
    let mut i = 0usize;
    while out.len() < n {
        let interface = format!("eth{i}");
        out.push(SeriesSpec {
            metric_id: "vm.net.rx_bytes_total",
            dimension: Some(("interface_id", interface.clone())),
            counter: true,
        });
        if out.len() == n {
            break;
        }
        out.push(SeriesSpec {
            metric_id: "vm.net.tx_bytes_total",
            dimension: Some(("interface_id", interface)),
            counter: true,
        });
        i += 1;
    }
    let mut d = 0usize;
    while out.len() < n {
        let device = format!("vd{}", (b'a' + d as u8) as char);
        out.push(SeriesSpec {
            metric_id: "vm.block.read_bytes_total",
            dimension: Some(("block_device_id", device.clone())),
            counter: true,
        });
        if out.len() == n {
            break;
        }
        out.push(SeriesSpec {
            metric_id: "vm.block.write_bytes_total",
            dimension: Some(("block_device_id", device)),
            counter: true,
        });
        d += 1;
    }
    out
}

/// (rss_kb, cpu_ms) of this process — leg deltas, not absolutes.
fn proc_rss_kb_cpu_ms() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let rss = status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the comm ')' start at state; utime/stime are the
    // 12th/13th there (fields 14/15 of the full line). USER_HZ is 100
    // on Linux.
    let fields: Vec<&str> = stat
        .rsplit(')')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    let utime: u64 = fields.get(11).and_then(|v| v.parse().ok()).unwrap_or(0);
    let stime: u64 = fields.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
    (rss, (utime + stime) * 10)
}

/// The VictoriaMetrics process RSS (destination cost at scale).
fn vm_rss_kb(vm: &VictoriaMetrics) -> u64 {
    vm.child
        .as_ref()
        .and_then(|child| child.id())
        .and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/status")).ok())
        .and_then(|status| {
            status
                .lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .unwrap_or(0)
}

fn percentile_ms(mut samples: Vec<u128>, p: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    let idx = (p * (samples.len() - 1)) / 100;
    samples[idx]
}

fn file_bytes(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// One node-batch scale leg: ingest `targets` VMs × `series_per_target`
/// series × 4 cadence points through the real store write path, then
/// measure query latency percentiles and one export pass against the
/// live destination. Prints the evidence row.
async fn node_batch_scale_leg(
    leg: &str,
    vm: &VictoriaMetrics,
    cert_pem: &str,
    targets: usize,
    series_per_target: usize,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = rig_store(&dir).await;
    let specs = vm_series_set(series_per_target);
    assert_eq!(specs.len(), series_per_target);
    let metric_ids: Vec<String> = {
        // Order-preserving unique (the specs alternate families, so
        // `dedup` on consecutive duplicates would not do).
        let mut ids: Vec<&'static str> = Vec::new();
        for spec in &specs {
            if !ids.contains(&spec.metric_id) {
                ids.push(spec.metric_id);
            }
        }
        ids.iter().map(|s| s.to_string()).collect()
    };
    let (rss0, cpu0) = proc_rss_kb_cpu_ms();

    // 4 cadence points, 15 s apart, ending ~30 s before the pass.
    let t0 = now_ms();
    let points: Vec<u64> = (0..4).map(|p| t0 - 30_000 - (3 - p) * 15_000).collect();
    let mut sequence = 0u64;
    let ingested = std::time::Instant::now();
    let mut samples_total = 0u64;
    for (p, observed) in points.iter().enumerate() {
        for tgt in 0..targets {
            let target_id = format!("scale-vm-{tgt}");
            let mut samples = Vec::with_capacity(specs.len());
            for (s, spec) in specs.iter().enumerate() {
                let mut builder = SampleBuilder::new(
                    TargetKind::Vm,
                    &target_id,
                    spec.metric_id,
                    Source::Vmm,
                    *observed,
                )
                .unwrap();
                if let Some((key, value)) = &spec.dimension {
                    builder = builder.dimension(key, value).unwrap();
                }
                let value = if spec.counter {
                    SampleValue::Integer(100_000 + p as u64 * 1_000 + s as u64)
                } else {
                    SampleValue::Float(((tgt + p) % 97) as f64 + 0.5)
                };
                samples.push(
                    builder
                        .value(value)
                        .epoch("scale-boot", "scale-identity")
                        .build()
                        .unwrap(),
                );
            }
            samples_total += samples.len() as u64;
            let batch = NodeBatch {
                boot_id: "scale-boot".to_string(),
                sequence,
                sent_at_ms: *observed,
                samples,
            };
            sequence += 1;
            let outcome = store
                .ingest_node_batch("scale-node", &batch, t0)
                .await
                .expect("ingest");
            assert!(
                matches!(outcome, IngestOutcome::Accepted { .. }),
                "{outcome:?}"
            );
        }
    }
    let ingest_ms = ingested.elapsed().as_millis();

    let db = dir.path().join("monitoring.db");
    let wal = dir.path().join("monitoring.db-wal");
    let (db_bytes, wal_bytes) = (file_bytes(&db), file_bytes(&wal));

    // Query latency percentiles across every target (the UI/alerting
    // read shape: one windowed history query per target).
    let mut query_ms = Vec::new();
    for tgt in 0..targets {
        let target_id = format!("scale-vm-{tgt}");
        let start = std::time::Instant::now();
        let series = store
            .query_history(
                &TargetKind::Vm,
                &target_id,
                &metric_ids,
                None,
                points[0],
                t0,
                1000,
                chv_monitoring_store::Resolution::Auto,
            )
            .await
            .expect("query");
        query_ms.push(start.elapsed().as_millis());
        assert!(!series.is_empty(), "target {target_id} must query");
    }

    // One export pass against the live destination (fresh exporter:
    // restart cursor = now - backfill, covering all seeded points).
    let config = chv_config::MonitoringExportConfig {
        enabled: true,
        destination_url: Some(vm.write_url()),
        metric_ids: metric_ids.clone(),
        interval_secs: 5,
        max_backfill_secs: 600,
        lag_degraded_secs: 30,
        ..Default::default()
    };
    let transport =
        ReqwestExportTransport::new(&vm.write_url(), None, Some(cert_pem)).expect("transport");
    let mut exporter =
        MonitoringExporter::new(&config, store.clone(), Arc::new(transport)).expect("exporter");
    let pass = std::time::Instant::now();
    let outcome = exporter.run_pass(now_ms()).await;
    let pass_ms = pass.elapsed().as_millis();

    let (rss1, cpu1) = proc_rss_kb_cpu_ms();
    let cadence_ok = pass_ms < 5_000;
    eprintln!(
        "g5b-scale | {leg} | targets={targets} series/target={series_per_target} | \
         ingest {samples_total} samples in {ingest_ms} ms ({:.0} samples/s) | \
         db {} B + wal {} B | query p50/p95/p99 {}/{}/{} ms | \
         export pass {pass_ms} ms ({} samples, cadence_5s={cadence_ok}) | \
         degraded={} | test-proc rss {rss0}->{rss1} kB cpu +{} ms | vm rss {} kB",
        if ingest_ms > 0 {
            samples_total as f64 * 1000.0 / ingest_ms as f64
        } else {
            0.0
        },
        db_bytes,
        wal_bytes,
        percentile_ms(query_ms.clone(), 50),
        percentile_ms(query_ms.clone(), 95),
        percentile_ms(query_ms, 99),
        exporter.exported_samples(),
        exporter.is_degraded(),
        cpu1.saturating_sub(cpu0),
        vm_rss_kb(vm),
    );
    assert_eq!(
        outcome,
        chv_controlplane_service::monitoring_exporter::PassOutcome::Complete
    );
    assert!(!exporter.is_degraded());
    assert!(exporter.exported_samples() > 0, "data must cross at scale");
    assert!(
        cadence_ok,
        "export pass at {targets} targets took {pass_ms} ms — beyond the 5 s interval floor"
    );
}

/// The manager-in-a-box for the guest shape (the e2e Manager pattern,
/// N VMs instead of one): real operational DB, real monitoring store,
/// real agent CA + mTLS listener, real MonitoringAgentService.
struct ScaleManager {
    _ops: chv_controlplane_store::test_util::TestDb,
    _monitoring_dir: tempfile::TempDir,
    store: Arc<MonitoringStore>,
    service: Arc<chv_controlplane_service::monitoring_agent::MonitoringAgentService>,
    server_cert_pem: String,
    base_url: String,
    /// Held for the listener's lifetime.
    _shutdown_tx: tokio::sync::watch::Sender<()>,
}

impl ScaleManager {
    async fn new(targets: usize) -> Self {
        use chv_controlplane_service::monitoring_agent::{
            AgentCertificateIssuer, GuestIngestionLimits, MonitoringAgentService,
        };

        let ops = chv_controlplane_store::test_util::TestDb::new().await;
        let monitoring_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            MonitoringStore::connect(MonitoringStoreConfig {
                database_url: format!("sqlite://{}/monitoring.db", monitoring_dir.path().display()),
                migrations_dir: PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "../../cmd/chv-controlplane/monitoring-migrations"
                )),
                ..MonitoringStoreConfig::default()
            })
            .await
            .expect("connect monitoring store"),
        );

        for tgt in 0..targets {
            let vm_id = format!("vm-g5b-{tgt}");
            sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ($1, $1)")
                .bind(&vm_id)
                .execute(&ops.pool)
                .await
                .unwrap();
        }

        let (agent_ca_pem, agent_ca_key) = test_ca();
        let issuer = Arc::new(AgentCertificateIssuer::new(&agent_ca_pem, &agent_ca_key).unwrap());
        // The per-IP enrollment rate limit is per NODE in production
        // (every guest enrolls through its own host); the rig's N
        // agents all share one loopback address, so the limit is
        // raised for the rig only (the known #602 debt is not what
        // this leg measures). Every other limit — including the
        // per-agent batch cap — stays at the production default.
        let limits = GuestIngestionLimits {
            enroll_attempts_per_minute: (targets + targets / 2 + 10) as u32,
            ..GuestIngestionLimits::default()
        };
        let service = Arc::new(MonitoringAgentService::new(
            chv_controlplane_store::MonitoringAgentRepository::new(ops.pool.clone()),
            issuer,
            Some(store.clone()),
            chv_monitoring_store::MonitoringHealth::new(),
            chv_controlplane_store::EventRepository::new(ops.pool.clone()),
            limits,
            None,
        ));

        let (server_cert_pem, server_key_pem) = test_server_cert();
        let https = chv_controlplane_service::api::tls::build_https_config(
            &server_cert_pem,
            &server_key_pem,
            Some(&agent_ca_pem),
        )
        .unwrap();
        let router = chv_controlplane_service::api::agent_routes::agent_routes(axum::Router::new())
            .layer(axum::Extension(service.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
        tokio::spawn(async move {
            let _ =
                chv_controlplane_service::api::tls::serve_tls(listener, https, router, shutdown_rx)
                    .await;
        });

        Self {
            _ops: ops,
            _monitoring_dir: monitoring_dir,
            store,
            service,
            server_cert_pem,
            base_url: format!("https://127.0.0.1:{port}"),
            _shutdown_tx: shutdown_tx,
        }
    }
}

/// The guest shape leg: N agents through the REAL path — claim, mTLS
/// enrollment with an issued client certificate, first delivery with
/// the default collectors — all concurrent, as a fleet ticks.
async fn guest_shape_leg(targets: usize) {
    use chv_monitor_agent::{Agent, TickOutcome};

    let manager = Arc::new(ScaleManager::new(targets).await);
    let (rss0, cpu0) = proc_rss_kb_cpu_ms();
    let leg = std::time::Instant::now();

    let mut fleet = tokio::task::JoinSet::new();
    for tgt in 0..targets {
        let manager = manager.clone();
        fleet.spawn(async move {
            let vm_id = format!("vm-g5b-{tgt}");
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            std::fs::write(root.join("manager-ca.pem"), &manager.server_cert_pem).unwrap();
            let claim = manager
                .service
                .issue_claim(&vm_id, "g5b-scale", now_ms() as i64)
                .await
                .expect("issue claim");
            std::fs::write(root.join("claim"), &claim.token).unwrap();
            let config_path = root.join("agent.toml");
            std::fs::write(
                &config_path,
                format!(
                    r#"
server_url = "{url}"
manager_ca_path = "{ca}"
claim_path = "{claim}"
credential_path = "{cred}"
state_dir = "{state}"
spool_dir = "{spool}"
interval_seconds = 1
max_spool_batches = 100
"#,
                    url = manager.base_url,
                    ca = root.join("manager-ca.pem").display(),
                    claim = root.join("claim").display(),
                    cred = root.join("credential.json").display(),
                    state = root.join("state").display(),
                    spool = root.join("spool").display(),
                ),
            )
            .unwrap();
            let config = chv_monitor_agent::config::AgentConfig::load(&config_path).unwrap();
            let mut agent = Agent::start(config).unwrap();
            assert_eq!(agent.tick().await, TickOutcome::Enrolled, "{vm_id}");
            // First delivery under fleet-wide concurrency: the
            // single-writer SQLite store contends, deliveries fail,
            // and the agent SPOOLS and retries — the production
            // never-lose-data path. Tick until the first batch is
            // delivered (the runtime's own cadence), bounded.
            let mut attempts = 0u32;
            loop {
                attempts += 1;
                match agent.tick().await {
                    TickOutcome::Delivered { .. } => break attempts,
                    TickOutcome::Spooled => {}
                    other => panic!("{vm_id} unexpected tick outcome: {other:?}"),
                }
                // The runtime's own cadence between attempts: the
                // per-agent batch cap is 12/min (production default),
                // so a hot retry loop would burn it on 429s; 5 s
                // stays inside it, like a production collector tick.
                tokio::time::sleep(Duration::from_secs(5)).await;
                assert!(
                    attempts < 60,
                    "{vm_id} never delivered its first batch within {attempts} ticks"
                );
            }
        });
    }
    let mut enrolled_delivered = 0usize;
    let mut attempts_total = 0u64;
    let mut attempts_max = 0u32;
    while let Some(result) = fleet.join_next().await {
        let attempts = result.expect("agent task");
        attempts_total += attempts as u64;
        attempts_max = attempts_max.max(attempts);
        enrolled_delivered += 1;
    }
    let leg_ms = leg.elapsed().as_millis();
    let (rss1, cpu1) = proc_rss_kb_cpu_ms();
    let rows = manager.store.raw_sample_count().await.expect("row count");
    eprintln!(
        "g5b-scale | guest-mtls | targets={targets} | {enrolled_delivered} agents enrolled + \
         delivered in {leg_ms} ms ({:.1} agents/s) | delivery attempts avg {:.1} max \
         {attempts_max} (SQLite write contention drains through the spool-retry path) | \
         store rows {rows} | test-proc rss {rss0}->{rss1} kB cpu +{} ms",
        if leg_ms > 0 {
            targets as f64 * 1000.0 / leg_ms as f64
        } else {
            0.0
        },
        if targets > 0 {
            attempts_total as f64 / targets as f64
        } else {
            0.0
        },
        cpu1.saturating_sub(cpu0),
    );
    assert_eq!(enrolled_delivered, targets);
    assert!(rows > 0, "guest samples must be durable");
}

#[tokio::test]
async fn g5b_scale_and_shape_qualification() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("chv_controlplane_service=debug,chv_monitor_agent=debug,warn")
        .with_test_writer()
        .try_init();
    let (Some(tiers), Some(binary)) = (parse_tiers(), vm_binary()) else {
        eprintln!(
            "g5b: CHV_G5B_SCALE_TIERS and CHV_G5B_VICTORIAMETRICS not both set; \
             skipping scale legs"
        );
        return;
    };
    verify_binary_digest(&binary);

    // One live destination for the whole matrix — cumulative, like a
    // real deployment absorbing a fleet.
    let dir = tempfile::tempdir().unwrap();
    let storage = dir.path().join("vm-storage");
    std::fs::create_dir(&storage).unwrap();
    let (cert_pem, key_pem) = test_server_cert();
    let cert_path = dir.path().join("vm-cert.pem");
    let key_path = dir.path().join("vm-key.pem");
    std::fs::write(&cert_path, &cert_pem).unwrap();
    std::fs::write(&key_path, &key_pem).unwrap();
    let mut vm = VictoriaMetrics::start(binary, storage, cert_path, key_path)
        .await
        .expect("start VictoriaMetrics");

    eprintln!(
        "g5b-scale | leg | targets series/target | ingest | storage | query percentiles | \
         export pass | resources"
    );

    // Tier ladder at the production shape (5 series per VM).
    for tier in &tiers {
        node_batch_scale_leg("node-batch", &vm, &cert_pem, *tier, 5).await;
    }

    // Shape ladder at a fixed tier: 1 / 5 / 20 series per VM.
    let shape_tier = tiers
        .iter()
        .copied()
        .find(|t| *t >= 100)
        .unwrap_or(tiers[0]);
    for shape in [1usize, 5, 20] {
        node_batch_scale_leg("shape", &vm, &cert_pem, shape_tier, shape).await;
    }

    // Guest shape through the real mTLS path.
    for tier in &tiers {
        guest_shape_leg(*tier).await;
    }

    vm.stop().await;
}
