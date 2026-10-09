//! Integration tests for node metric batch ingestion (#602 PR-2,
//! ingestion contract v1). These exercise the full service path —
//! envelope validation, sender/ownership enforcement, registry
//! validation, rate caps, and the durable store outcomes — against a
//! real (temporary-file) monitoring database and a real operational
//! store.

use crate::monitoring_ingest::{
    MonitoringIngestImplementation, MonitoringIngestService, MAX_SAMPLES_PER_BATCH,
};
use chv_controlplane_store::test_util::TestDb;
use chv_controlplane_store::{NodeRepository, ObservedStateRepository};
use chv_controlplane_types::domain::{Generation, NodeId, ResourceId};
use chv_monitoring_store::{MonitoringHealth, MonitoringStore, MonitoringStoreConfig};
use control_plane_node_api::control_plane_node_api as proto;
use std::path::PathBuf;
use std::sync::Arc;

/// A temporary monitoring store (file-backed, unlike the in-memory
/// operational test DB, so WAL/durability semantics are real).
async fn monitoring_store(dir: &std::path::Path) -> Arc<MonitoringStore> {
    Arc::new(
        MonitoringStore::connect(MonitoringStoreConfig {
            database_url: format!("sqlite://{}/monitoring.db", dir.display()),
            migrations_dir: PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../cmd/chv-controlplane/monitoring-migrations"
            )),
            ..MonitoringStoreConfig::default()
        })
        .await
        .expect("monitoring store connect"),
    )
}

struct Fixture {
    _ops_db: TestDb,
    _monitoring_dir: tempfile::TempDir,
    service: MonitoringIngestImplementation,
    node_id: String,
    vm_id: String,
}

impl Fixture {
    async fn new() -> Self {
        let ops_db = TestDb::new().await;
        let node_repo = NodeRepository::new(ops_db.pool.clone());
        let observed_state_repo = ObservedStateRepository::new(ops_db.pool.clone());
        let dir = tempfile::tempdir().unwrap();
        let store = monitoring_store(dir.path()).await;

        // An enrolled node and a VM observed on it.
        let node_id = NodeId::new("node-ingest-1").unwrap();
        node_repo
            .ensure_node_record(&node_id, Some("ingest-test-node"), None, 0)
            .await
            .unwrap();
        let vm_id = ResourceId::new("vm-ingest-1").unwrap();
        // The observed-state row has FKs into vms (desired) and nodes;
        // create both parents first, exactly like the real report path.
        chv_controlplane_store::DesiredStateRepository::new(ops_db.pool.clone())
            .upsert_vm(&chv_controlplane_store::VmDesiredStateInput {
                vm_id: vm_id.clone(),
                node_id: Some(node_id.clone()),
                display_name: "ingest-test-vm".to_string(),
                tenant_id: None,
                placement_policy: None,
                desired_generation: Generation::new(1),
                desired_status: None,
                requested_by: None,
                updated_by: None,
                target_node_id: None,
                cpu_count: None,
                memory_bytes: None,
                image_ref: None,
                boot_mode: None,
                desired_power_state: None,
                requested_unix_ms: 0,
            })
            .await
            .unwrap();
        observed_state_repo
            .upsert_vm(&chv_controlplane_store::VmObservedStateInput {
                vm_id: vm_id.clone(),
                observed_generation: Generation::new(1),
                runtime_status: "running".to_string(),
                health_status: Some("ok".to_string()),
                node_id: Some(node_id.clone()),
                cloud_hypervisor_pid: None,
                api_socket_path: None,
                last_error: None,
                last_transition_unix_ms: None,
                observed_unix_ms: 0,
            })
            .await
            .unwrap();

        let service = MonitoringIngestImplementation::new(
            Some(store),
            node_repo,
            observed_state_repo,
            MonitoringHealth::new(),
            20,
            0, // headroom floor 0: never trips in tests
        );
        Fixture {
            _ops_db: ops_db,
            _monitoring_dir: dir,
            service,
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
        }
    }

    fn request(
        &self,
        sequence: u64,
        samples: Vec<proto::MetricSampleV1>,
    ) -> proto::NodeMetricBatchRequest {
        proto::NodeMetricBatchRequest {
            meta: None,
            node_id: self.node_id.clone(),
            schema_version: 1,
            boot_id: "agent-boot-1".to_string(),
            sequence,
            sent_at_ms: 0,
            samples,
        }
    }

    async fn ingest(
        &self,
        sequence: u64,
        samples: Vec<proto::MetricSampleV1>,
    ) -> proto::NodeMetricBatchResponse {
        self.service
            .ingest_node_metric_batch(self.request(sequence, samples))
            .await
            .expect("ingest call succeeds")
    }
}

fn node_cpu_sample(observed_at_ms: i64, value: f64) -> proto::MetricSampleV1 {
    proto::MetricSampleV1 {
        target_kind: "node".to_string(),
        target_id: "node-ingest-1".to_string(),
        metric_id: "node.cpu.capacity_ratio".to_string(),
        source: "node_os".to_string(),
        kind: "gauge".to_string(),
        unit: "ratio".to_string(),
        observed_at_ms,
        value: Some(proto::metric_sample_v1::Value::FloatValue(value)),
        quality: "valid".to_string(),
        dimensions: Default::default(),
        boot_id: String::new(),
        identity_epoch: String::new(),
    }
}

fn vm_cores_sample(vm_id: &str, observed_at_ms: i64, value: f64) -> proto::MetricSampleV1 {
    proto::MetricSampleV1 {
        target_kind: "vm".to_string(),
        target_id: vm_id.to_string(),
        metric_id: "vm.cpu.cores_used".to_string(),
        source: "vmm".to_string(),
        kind: "gauge".to_string(),
        unit: "cores".to_string(),
        observed_at_ms,
        value: Some(proto::metric_sample_v1::Value::FloatValue(value)),
        quality: "valid".to_string(),
        dimensions: Default::default(),
        boot_id: String::new(),
        identity_epoch: String::new(),
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[tokio::test]
async fn valid_batch_is_durably_accepted_then_deduplicated() {
    let f = Fixture::new().await;
    let sample = node_cpu_sample(now_ms(), 0.5);
    let resp = f.ingest(0, vec![sample.clone()]).await;
    assert_eq!(resp.outcome, "accepted");
    assert_eq!(resp.accepted_samples, 1);

    // Byte-identical retry (same timestamps): previous durable outcome.
    let resp = f.ingest(0, vec![sample.clone()]).await;
    assert_eq!(resp.outcome, "duplicate");

    // Same key, different body.
    let resp = f.ingest(0, vec![node_cpu_sample(now_ms(), 0.9)]).await;
    assert_eq!(resp.outcome, "replay_conflict");
}

#[tokio::test]
async fn unknown_or_mismetriced_samples_reject_whole_batch() {
    let f = Fixture::new().await;
    let mut unknown = node_cpu_sample(now_ms(), 0.5);
    unknown.metric_id = "node.cpu.not_a_metric".to_string();
    let resp = f.ingest(0, vec![unknown]).await;
    assert_eq!(resp.outcome, "unsupported_metric");

    // Disallowed source for the metric.
    let mut wrong_source = node_cpu_sample(now_ms(), 0.5);
    wrong_source.source = "guest_agent".to_string();
    let resp = f.ingest(0, vec![wrong_source]).await;
    assert_eq!(resp.outcome, "unsupported_metric");

    // Kind/unit must match the registry.
    let mut wrong_kind = node_cpu_sample(now_ms(), 0.5);
    wrong_kind.kind = "counter".to_string();
    let resp = f.ingest(0, vec![wrong_kind]).await;
    assert_eq!(resp.outcome, "invalid_batch");
}

#[tokio::test]
async fn value_quality_consistency_is_enforced() {
    let f = Fixture::new().await;
    // Valid quality without a value.
    let mut no_value = node_cpu_sample(now_ms(), 0.5);
    no_value.value = None;
    let resp = f.ingest(0, vec![no_value]).await;
    assert_eq!(resp.outcome, "invalid_batch");

    // Non-valid quality carrying a value.
    let mut valued = node_cpu_sample(now_ms(), 0.5);
    valued.quality = "unavailable".to_string();
    let resp = f.ingest(0, vec![valued]).await;
    assert_eq!(resp.outcome, "invalid_batch");

    // Unknown quality string.
    let mut bad_quality = node_cpu_sample(now_ms(), 0.5);
    bad_quality.quality = "probably-fine".to_string();
    let resp = f.ingest(0, vec![bad_quality]).await;
    assert_eq!(resp.outcome, "invalid_batch");
}

#[tokio::test]
async fn target_ownership_is_enforced() {
    let f = Fixture::new().await;
    // Node sample for a different node id.
    let mut foreign = node_cpu_sample(now_ms(), 0.5);
    foreign.target_id = "node-other-1".to_string();
    let resp = f.ingest(0, vec![foreign]).await;
    assert_eq!(resp.outcome, "invalid_batch");

    // VM sample for a VM not observed on this sender.
    let foreign_vm = vm_cores_sample("vm-other-1", now_ms(), 1.0);
    let resp = f.ingest(0, vec![foreign_vm]).await;
    assert_eq!(resp.outcome, "invalid_batch");

    // VM sample for a VM on this sender is accepted.
    let owned = vm_cores_sample(&f.vm_id, now_ms(), 1.5);
    let resp = f.ingest(0, vec![owned]).await;
    assert_eq!(resp.outcome, "accepted");
}

#[tokio::test]
async fn envelope_limits_reject_with_typed_outcomes() {
    let f = Fixture::new().await;

    let mut bad_schema = f.request(0, vec![node_cpu_sample(now_ms(), 0.5)]);
    bad_schema.schema_version = 2;
    let resp = f
        .service
        .ingest_node_metric_batch(bad_schema)
        .await
        .unwrap();
    assert_eq!(resp.outcome, "invalid_batch");

    let oversized = f.request(
        0,
        (0..=MAX_SAMPLES_PER_BATCH)
            .map(|i| node_cpu_sample(now_ms() + (i as i64), 0.5))
            .collect(),
    );
    let resp = f.service.ingest_node_metric_batch(oversized).await.unwrap();
    assert_eq!(resp.outcome, "batch_too_large");

    // Stale timestamp.
    let old = node_cpu_sample(now_ms() - 10 * 60 * 1000, 0.5);
    let resp = f.ingest(0, vec![old]).await;
    assert_eq!(resp.outcome, "invalid_batch");

    // Unenrolled sender.
    let mut req = f.request(0, vec![node_cpu_sample(now_ms(), 0.5)]);
    req.node_id = "node-ghost-1".to_string();
    // Keep the node-target consistent so the enrollment check is what
    // fires (not the ownership check).
    req.samples[0].target_id = req.node_id.clone();
    let resp = f.service.ingest_node_metric_batch(req).await.unwrap();
    assert_eq!(resp.outcome, "invalid_batch");
}

#[tokio::test]
async fn per_sender_rate_cap() {
    // A fresh fixture: the rate window counts every batch that passes
    // the envelope+enrollment gates, accepted or not.
    let f = Fixture::new().await;
    for seq in 0..20 {
        let resp = f.ingest(seq, vec![node_cpu_sample(now_ms(), 0.5)]).await;
        assert_eq!(
            resp.outcome, "accepted",
            "sequence {seq} should be accepted"
        );
    }
    let resp = f.ingest(20, vec![node_cpu_sample(now_ms(), 0.5)]).await;
    assert_eq!(resp.outcome, "rate_limited");
    assert!(resp.retry_after_seconds > 0);
}

#[tokio::test]
async fn unavailable_store_is_a_typed_outcome() {
    let ops_db = TestDb::new().await;
    let service = MonitoringIngestImplementation::new(
        None,
        NodeRepository::new(ops_db.pool.clone()),
        ObservedStateRepository::new(ops_db.pool.clone()),
        MonitoringHealth::new(),
        20,
        0,
    );
    let mut req = proto::NodeMetricBatchRequest {
        meta: None,
        node_id: "node-ingest-1".to_string(),
        schema_version: 1,
        boot_id: "b".to_string(),
        sequence: 0,
        sent_at_ms: 0,
        samples: vec![node_cpu_sample(now_ms(), 0.5)],
    };
    req.samples[0].target_id = req.node_id.clone();
    let resp = service.ingest_node_metric_batch(req).await.unwrap();
    assert_eq!(resp.outcome, "ingestion_unavailable");
    assert_eq!(resp.accepted_samples, 0);
    assert_eq!(service.health().snapshot().unavailable_batches, 1);
}
