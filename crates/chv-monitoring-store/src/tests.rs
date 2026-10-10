use crate::config::MonitoringStoreConfig;
use crate::error::IngestOutcome;
use crate::ingest::NodeBatch;
use crate::query::Resolution;
use crate::{MonitoringStore, StoredCheck};
use chv_monitoring_core::model::{
    CheckRecord, CheckStatus, MetricKind, SampleBuilder, SampleQuality, SampleValue, Source,
    TargetKind, Unit,
};
use sqlx::Row;
use std::path::PathBuf;

const T0: u64 = 1_700_000_000_000;

fn test_config(dir: &std::path::Path) -> MonitoringStoreConfig {
    MonitoringStoreConfig {
        database_url: format!("sqlite://{}/monitoring.db", dir.display()),
        migrations_dir: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../cmd/chv-controlplane/monitoring-migrations"
        )),
        ..MonitoringStoreConfig::default()
    }
}

async fn store() -> (tempfile::TempDir, MonitoringStore) {
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let s = MonitoringStore::connect(test_config(dir.path()))
        .await
        .unwrap();
    (dir, s)
}

fn node_gauge(observed_ms: u64, value: f64) -> chv_monitoring_core::model::Sample {
    SampleBuilder::new(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        observed_ms,
    )
    .unwrap()
    .value(SampleValue::Float(value))
    .build()
    .unwrap()
}

fn node_gauge_quality(
    observed_ms: u64,
    quality: SampleQuality,
) -> chv_monitoring_core::model::Sample {
    SampleBuilder::new(
        TargetKind::Node,
        "node-1",
        "node.cpu.capacity_ratio",
        Source::NodeOs,
        observed_ms,
    )
    .unwrap()
    .quality(quality)
    .build()
    .unwrap()
}

fn iface_counter(
    observed_ms: u64,
    value: u64,
    boot: &str,
    epoch: &str,
) -> chv_monitoring_core::model::Sample {
    SampleBuilder::new(
        TargetKind::Node,
        "node-1",
        "node.net.rx_bytes_total",
        Source::NodeOs,
        observed_ms,
    )
    .unwrap()
    .dimension("interface_id", "eth0")
    .unwrap()
    .epoch(boot, epoch)
    .value(SampleValue::Integer(value))
    .build()
    .unwrap()
}

fn vm_cores(observed_ms: u64, value: f64) -> chv_monitoring_core::model::Sample {
    SampleBuilder::new(
        TargetKind::Vm,
        "vm-1",
        "vm.cpu.cores_used",
        Source::Vmm,
        observed_ms,
    )
    .unwrap()
    .value(SampleValue::Float(value))
    .build()
    .unwrap()
}

fn batch(boot: &str, sequence: u64, samples: Vec<chv_monitoring_core::model::Sample>) -> NodeBatch {
    NodeBatch {
        boot_id: boot.to_string(),
        sequence,
        sent_at_ms: T0,
        samples,
    }
}

fn check(check_id: &str, status: CheckStatus, observed_ms: u64) -> CheckRecord {
    CheckRecord {
        check_id: check_id.to_string(),
        service_key: Some("nginx.service".to_string()),
        status,
        summary: Some("active (running)".to_string()),
        observed_at_ms: observed_ms,
    }
}

#[tokio::test]
async fn ingest_deduplicates_and_detects_replay_conflicts() {
    let (_dir, s) = store().await;
    let b = batch("agent-boot-1", 0, vec![node_gauge(T0, 0.5)]);

    let out = s.ingest_node_batch("node-1", &b, T0).await.unwrap();
    assert_eq!(
        out,
        crate::IngestOutcome::Accepted { samples: 1 },
        "first delivery is durably accepted"
    );

    // Identical retry → previous acknowledgment, no re-insert.
    let out = s.ingest_node_batch("node-1", &b, T0 + 1_000).await.unwrap();
    assert_eq!(out, crate::IngestOutcome::Duplicate { samples: 1 });
    assert_eq!(s.raw_sample_count().await.unwrap(), 1);

    // Same key, different body → conflict, nothing inserted.
    let mut conflict = batch("agent-boot-1", 0, vec![node_gauge(T0, 0.9)]);
    conflict.sent_at_ms = T0 + 5_000;
    let out = s
        .ingest_node_batch("node-1", &conflict, T0 + 2_000)
        .await
        .unwrap();
    assert_eq!(out, crate::IngestOutcome::ReplayConflict);
    assert_eq!(s.raw_sample_count().await.unwrap(), 1);

    // A different sender node is a different dedup key.
    let out = s.ingest_node_batch("node-2", &b, T0).await.unwrap();
    assert!(out.is_committed());
}

#[tokio::test]
async fn stale_sequence_fails_closed() {
    let (_dir, s) = store().await;
    s.ingest_node_batch("node-1", &batch("b", 7, vec![node_gauge(T0, 0.5)]), T0)
        .await
        .unwrap();
    // Sequence 3 < high water 7 and no retained proof row for 3.
    let out = s
        .ingest_node_batch("node-1", &batch("b", 3, vec![node_gauge(T0, 0.4)]), T0)
        .await
        .unwrap();
    assert_eq!(out, crate::IngestOutcome::StaleSequence);
    // A new agent boot epoch restarts sequences cleanly.
    let out = s
        .ingest_node_batch("node-1", &batch("b2", 0, vec![node_gauge(T0, 0.4)]), T0)
        .await
        .unwrap();
    assert!(out.is_committed());
}

#[tokio::test]
async fn series_cap_rejects_whole_batch() {
    let (_dir, s) = store().await;
    // Default cap is 1024; drive it with distinct interfaces on one
    // counter metric.
    let mut samples = Vec::new();
    for i in 0..1025 {
        samples.push(
            SampleBuilder::new(
                TargetKind::Node,
                "node-1",
                "node.net.rx_bytes_total",
                Source::NodeOs,
                T0,
            )
            .unwrap()
            .dimension("interface_id", &format!("eth{i}"))
            .unwrap()
            .epoch("boot", "e")
            .value(SampleValue::Integer(1))
            .build()
            .unwrap(),
        );
    }
    let out = s
        .ingest_node_batch("node-1", &batch("b", 0, samples), T0)
        .await
        .unwrap();
    match out {
        crate::IngestOutcome::SeriesCapExceeded { series, cap, .. } => {
            assert_eq!(series, 1025);
            assert_eq!(cap, 1024);
        }
        other => panic!("expected series cap rejection, got {other:?}"),
    }
    assert_eq!(
        s.raw_sample_count().await.unwrap(),
        0,
        "rejected batch must not be partially applied"
    );
}

#[tokio::test]
async fn value_columns_are_typed_and_quality_guards_value() {
    let (_dir, s) = store().await;
    s.ingest_node_batch(
        "node-1",
        &batch(
            "b",
            0,
            vec![
                node_gauge(T0, 0.42),
                iface_counter(T0, 18_446_744_073_709_551_615, "boot", "e"),
                node_gauge_quality(T0 + 1_000, SampleQuality::Unavailable),
            ],
        ),
        T0,
    )
    .await
    .unwrap();

    let rows = sqlx::query(
        "SELECT metric_id, value_integer, value_real, quality FROM monitoring_samples ORDER BY metric_id",
    )
    .fetch_all(s.pool())
    .await
    .unwrap();
    let gauge = rows
        .iter()
        .find(|r| r.get::<String, _>("metric_id").contains("capacity"))
        .unwrap();
    assert_eq!(gauge.get::<Option<i64>, _>("value_integer"), None);
    assert_eq!(gauge.get::<Option<f64>, _>("value_real"), Some(0.42));

    let counter = rows
        .iter()
        .find(|r| r.get::<String, _>("metric_id").contains("rx_bytes"))
        .unwrap();
    // u64::MAX saturates to i64::MAX in the integer column — exact for
    // every realistic counter, and never a float.
    assert_eq!(
        counter.get::<Option<i64>, _>("value_integer"),
        Some(i64::MAX)
    );
    assert_eq!(counter.get::<Option<f64>, _>("value_real"), None);

    let unavailable = rows
        .iter()
        .find(|r| r.get::<String, _>("quality") == "unavailable")
        .unwrap();
    assert_eq!(unavailable.get::<Option<i64>, _>("value_integer"), None);
    assert_eq!(unavailable.get::<Option<f64>, _>("value_real"), None);
}

#[tokio::test]
async fn history_buckets_gauges_and_counts_counter_deltas() {
    let (_dir, s) = store().await;
    // 10 gauge observations at 1s cadence, one unavailable between
    // them (distinct timestamp: same-epoch samples at an identical
    // observed_at_ms are one observation by primary key); counters
    // climbing 100/10s with an epoch reset at t5.
    let mut samples = Vec::new();
    for i in 0..10u64 {
        samples.push(node_gauge(T0 + i * 1_000, 0.1 * (i + 1) as f64));
        let (boot, value) = if i < 5 {
            ("boot-a", 100 + i * 10)
        } else {
            ("boot-b", 50 + (i - 5) * 10)
        };
        samples.push(iface_counter(T0 + i * 1_000, value, boot, "e"));
    }
    samples.push(node_gauge_quality(T0 + 4_500, SampleQuality::Unavailable));
    s.ingest_node_batch("node-1", &batch("b", 0, samples), T0 + 10_000)
        .await
        .unwrap();

    // Gauge: max 5 points over the 10s range → 2s buckets; means of
    // valid points only.
    let series = s
        .query_history(
            &TargetKind::Node,
            "node-1",
            &["node.cpu.capacity_ratio".to_string()],
            None,
            T0,
            T0 + 10_000,
            5,
            Resolution::Raw,
        )
        .await
        .unwrap();
    assert_eq!(series.len(), 1);
    let gauge = &series[0];
    assert!(gauge.points.len() <= 5);
    // Every returned point is valid (the unavailable sample only
    // lowers coverage).
    assert!(gauge
        .points
        .iter()
        .all(|p| p.quality == SampleQuality::Valid));
    assert!(gauge.coverage_ratio > 0.5 && gauge.coverage_ratio < 1.0);

    // Counter: same-epoch deltas only. boot-a: +10/s; boot-b restarts
    // at 50 — the crossing bucket must not be a negative spike.
    let series = s
        .query_history(
            &TargetKind::Node,
            "node-1",
            &["node.net.rx_bytes_total".to_string()],
            None,
            T0,
            T0 + 10_000,
            5,
            Resolution::Raw,
        )
        .await
        .unwrap();
    let counter = &series[0];
    for p in &counter.points {
        if let Some(v) = p.value {
            assert!(v >= 0.0, "counter delta must never be negative, got {v}");
        }
    }
    // The boot-a portion sums to its total climb; the reset bucket has
    // no value.
    let valid: Vec<f64> = counter.points.iter().filter_map(|p| p.value).collect();
    assert!(!valid.is_empty());
}

#[tokio::test]
async fn rollups_are_idempotent_and_reset_safe() {
    let (_dir, s) = store().await;
    let mut samples = Vec::new();
    for i in 0..20u64 {
        samples.push(node_gauge(T0 + i * 1_000, 0.5));
        // Monotonic within boot-a, then a reset to a lower base.
        let (boot, value) = if i < 10 {
            ("a", 1000 + i * 10)
        } else {
            ("b", 5 + i)
        };
        samples.push(iface_counter(T0 + i * 1_000, value, boot, "e"));
    }
    s.ingest_node_batch("node-1", &batch("b", 0, samples), T0 + 20_000)
        .await
        .unwrap();

    let report1 = s.run_maintenance(T0 + 60_000).await.unwrap();
    assert!(report1.rollup_windows >= 2, "5m and 1h windows computed");
    // Re-running the pass is a no-op (cursor advanced, nothing touched).
    let report2 = s.run_maintenance(T0 + 120_000).await.unwrap();
    assert_eq!(report2.rollup_windows, 0, "idempotent: nothing re-rolled");

    // The counter rollup for this window has NO delta (reset inside).
    let row = sqlx::query(
        "SELECT counter_first, counter_last, counter_delta, value_count FROM monitoring_rollups
         WHERE tier = '5m' AND metric_id = 'node.net.rx_bytes_total'",
    )
    .fetch_one(s.pool())
    .await
    .unwrap();
    let (first, last, delta, count): (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = (
        row.get("counter_first"),
        row.get("counter_last"),
        row.get("counter_delta"),
        row.get("value_count"),
    );
    assert_eq!(first, Some(1000));
    assert_eq!(last, Some(5 + 19));
    assert_eq!(
        delta, None,
        "a reset makes the window delta honestly absent"
    );
    assert_eq!(count, Some(20));

    // Gauge rollup aggregates over valid points.
    let row = sqlx::query(
        "SELECT value_min, value_max, value_sum, value_count FROM monitoring_rollups
         WHERE tier = '5m' AND metric_id = 'node.cpu.capacity_ratio'",
    )
    .fetch_one(s.pool())
    .await
    .unwrap();
    let (min, max): (Option<f64>, Option<f64>) = (row.get("value_min"), row.get("value_max"));
    assert_eq!(min, Some(0.5));
    assert_eq!(max, Some(0.5));
}

#[tokio::test]
async fn retention_deletes_old_raw_and_dedup() {
    let (_dir, s) = store().await;
    s.ingest_node_batch(
        "node-1",
        &batch(
            "b",
            0,
            vec![node_gauge(T0, 0.5), iface_counter(T0, 10, "boot", "e")],
        ),
        T0,
    )
    .await
    .unwrap();

    // 49 hours later: raw retention (48h) has passed.
    let later = T0 + 49 * 60 * 60 * 1000;
    let report = s.run_maintenance(later).await.unwrap();
    assert!(report.raw_deleted >= 2, "raw rows past retention deleted");
    assert!(
        report.dedup_deleted >= 1,
        "dedup proofs past retention deleted"
    );
    assert_eq!(s.raw_sample_count().await.unwrap(), 0);
}

#[tokio::test]
async fn size_budget_evicts_oldest_raw_first() {
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let mut config = test_config(dir.path());
    // Absurdly small budget: any real page count exceeds it.
    config.max_db_bytes = 1;
    let s = MonitoringStore::connect(config).await.unwrap();

    s.ingest_node_batch("node-1", &batch("b", 0, vec![node_gauge(T0, 0.5)]), T0)
        .await
        .unwrap();
    let report = s.run_maintenance(T0 + 1_000).await.unwrap();
    assert!(report.evicted_raw >= 1, "over-budget store evicts raw data");
    assert_eq!(s.raw_sample_count().await.unwrap(), 0);
}

#[tokio::test]
async fn current_reports_latest_with_staleness() {
    let (_dir, s) = store().await;
    s.ingest_node_batch("node-1", &batch("b", 0, vec![node_gauge(T0, 0.5)]), T0)
        .await
        .unwrap();

    let now = T0 + 10_000;
    let current = s
        .query_current(&TargetKind::Node, "node-1", &[], None, now)
        .await
        .unwrap();
    let cpu = current
        .iter()
        .find(|c| c.metric_id == "node.cpu.capacity_ratio")
        .unwrap();
    assert_eq!(cpu.value, Some(0.5));
    assert!(!cpu.stale, "10s-old node sample is fresh (60s threshold)");
    assert_eq!(cpu.unit, Unit::Ratio);
    assert_eq!(cpu.kind, MetricKind::Gauge);

    let current = s
        .query_current(&TargetKind::Node, "node-1", &[], None, T0 + 120_000)
        .await
        .unwrap();
    let cpu = current
        .iter()
        .find(|c| c.metric_id == "node.cpu.capacity_ratio")
        .unwrap();
    assert!(cpu.stale, "2-minute-old node sample is stale");
}

#[tokio::test]
async fn current_applies_metric_specific_staleness_thresholds() {
    let (_dir, s) = store().await;
    // A 60-second cadence family (vm.guest.fs.*) and a 15-second
    // family (vm.guest.cpu.utilization_ratio), both observed 100
    // seconds ago: the fs sample stays fresh under its 180-second
    // metric-specific window while the 15-second family is stale
    // under the default non-node threshold.
    let fs = SampleBuilder::new(
        TargetKind::Vm,
        "vm-1",
        "vm.guest.fs.available_bytes",
        Source::GuestAgent,
        T0,
    )
    .unwrap()
    .dimension("mount_id", "ext4:/")
    .unwrap()
    .value(SampleValue::Integer(1_000))
    .build()
    .unwrap();
    let cpu = SampleBuilder::new(
        TargetKind::Vm,
        "vm-1",
        "vm.guest.cpu.utilization_ratio",
        Source::GuestAgent,
        T0,
    )
    .unwrap()
    .value(SampleValue::Float(0.5))
    .build()
    .unwrap();
    s.ingest_node_batch("vm-1", &batch("boot-1", 0, vec![fs, cpu]), T0)
        .await
        .unwrap();

    let current = s
        .query_current(&TargetKind::Vm, "vm-1", &[], None, T0 + 100_000)
        .await
        .unwrap();
    let fs = current
        .iter()
        .find(|c| c.metric_id == "vm.guest.fs.available_bytes")
        .unwrap();
    assert!(!fs.stale, "100s-old fs sample is fresh (180s window)");
    let cpu = current
        .iter()
        .find(|c| c.metric_id == "vm.guest.cpu.utilization_ratio")
        .unwrap();
    assert!(
        cpu.stale,
        "100s-old 15s-family sample is stale (90s default)"
    );

    // The fs window is finite too.
    let current = s
        .query_current(&TargetKind::Vm, "vm-1", &[], None, T0 + 200_000)
        .await
        .unwrap();
    let fs = current
        .iter()
        .find(|c| c.metric_id == "vm.guest.fs.available_bytes")
        .unwrap();
    assert!(fs.stale, "200s-old fs sample is stale");
}

#[tokio::test]
async fn current_survives_retention_eviction_as_stale() {
    let (_dir, s) = store().await;
    s.ingest_node_batch("node-1", &batch("b", 0, vec![node_gauge(T0, 0.5)]), T0)
        .await
        .unwrap();
    // Evict raw via retention; the series row keeps the memory.
    s.run_maintenance(T0 + 49 * 60 * 60 * 1000).await.unwrap();
    let current = s
        .query_current(
            &TargetKind::Node,
            "node-1",
            &[],
            None,
            T0 + 49 * 60 * 60 * 1000,
        )
        .await
        .unwrap();
    assert!(
        current.is_empty(),
        "series row past retention is forgotten too"
    );
}

#[tokio::test]
async fn vm_history_rejects_bad_ranges_and_ceiling() {
    let (_dir, s) = store().await;
    s.ingest_node_batch("node-1", &batch("b", 0, vec![vm_cores(T0, 1.25)]), T0)
        .await
        .unwrap();

    let err = s
        .query_history(
            &TargetKind::Vm,
            "vm-1",
            &["vm.cpu.cores_used".to_string()],
            None,
            T0,
            T0, // from == to
            240,
            Resolution::Auto,
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("from_ms must be before to_ms"));

    let err = s
        .query_history(
            &TargetKind::Vm,
            "vm-1",
            &["vm.cpu.cores_used".to_string()],
            None,
            T0,
            T0 + 200 * 24 * 60 * 60 * 1000, // 200 days
            240,
            Resolution::Auto,
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("ceiling"));

    let ok = s
        .query_history(
            &TargetKind::Vm,
            "vm-1",
            &["vm.cpu.cores_used".to_string()],
            None,
            T0 - 1_000,
            T0 + 1_000,
            240,
            Resolution::Raw,
        )
        .await
        .unwrap();
    assert_eq!(ok.len(), 1);
    assert_eq!(ok[0].points.len(), 1);
    assert_eq!(ok[0].points[0].value, Some(1.25));
    assert_eq!(ok[0].unit, Unit::Cores);
}

#[tokio::test]
async fn history_absence_classification() {
    let (_dir, s) = store().await;
    // Nothing ingested for this target at all → no_history.
    let series = s
        .query_history(
            &TargetKind::Vm,
            "vm-404",
            &["vm.cpu.cores_used".to_string()],
            None,
            T0,
            T0 + 60_000,
            240,
            Resolution::Raw,
        )
        .await
        .unwrap();
    assert!(
        series.is_empty(),
        "no series rows → no series (target unknown)"
    );

    // Ingest old data; query a range with nothing in it.
    s.ingest_node_batch("node-1", &batch("b", 0, vec![vm_cores(T0, 1.0)]), T0)
        .await
        .unwrap();
    let series = s
        .query_history(
            &TargetKind::Vm,
            "vm-1",
            &["vm.cpu.cores_used".to_string()],
            None,
            T0 + 10 * 60 * 1000,
            T0 + 20 * 60 * 1000,
            240,
            Resolution::Raw,
        )
        .await
        .unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].reason.as_ref().map(|r| r.as_str()), Some("stale"));
    assert!(series[0].points.is_empty());
}

#[tokio::test]
async fn corrupt_database_reports_degraded_not_panics() {
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let db_path = dir.path().join("monitoring.db");
    std::fs::write(&db_path, b"this is not a sqlite database at all").unwrap();
    let config = MonitoringStoreConfig {
        database_url: format!("sqlite://{}", db_path.display()),
        ..test_config(dir.path())
    };
    let err = MonitoringStore::connect(config).await.unwrap_err();
    assert!(
        err.to_string().contains("degraded") || err.to_string().contains("database"),
        "corruption is an error signal, got: {err}"
    );
}

/// Integer-valued gauges (byte counts, counts) must aggregate through
/// both raw bucketing and rollups: they are stored in the exact
/// integer column, and an aggregation that only reads the float column
/// would turn every one of their history points into an absence.
#[tokio::test]
async fn integer_gauges_aggregate_in_history_and_rollups() {
    let (_dir, s) = store().await;
    let mut samples = Vec::new();
    for i in 0..10u64 {
        samples.push(
            SampleBuilder::new(
                TargetKind::Node,
                "node-1",
                "node.memory.available_bytes",
                Source::NodeOs,
                T0 + i * 1_000,
            )
            .unwrap()
            .value(SampleValue::Integer(2_000_000_000 + i * 1_000_000))
            .build()
            .unwrap(),
        );
    }
    s.ingest_node_batch("node-1", &batch("b", 0, samples), T0 + 10_000)
        .await
        .unwrap();

    // Raw bucketing (5 points over the 10s range forces 2s buckets):
    // every bucket must carry the mean of its integer observations.
    let series = s
        .query_history(
            &TargetKind::Node,
            "node-1",
            &["node.memory.available_bytes".to_string()],
            None,
            T0,
            T0 + 10_000,
            5,
            Resolution::Raw,
        )
        .await
        .unwrap();
    let gauge = &series[0];
    assert!(!gauge.points.is_empty());
    for p in &gauge.points {
        assert_eq!(p.quality, SampleQuality::Valid);
        assert!(p.value.is_some(), "integer gauge bucket has a value");
    }

    // Rollups: min/max/sum must aggregate the integer values, and the
    // rollup history point must be Valid with a value.
    s.run_maintenance(T0 + 60_000).await.unwrap();
    let row = sqlx::query(
        "SELECT value_min, value_max, value_count FROM monitoring_rollups
         WHERE tier = '5m' AND metric_id = 'node.memory.available_bytes'",
    )
    .fetch_one(s.pool())
    .await
    .unwrap();
    let (min, max, count): (Option<f64>, Option<f64>, Option<i64>) = (
        row.get("value_min"),
        row.get("value_max"),
        row.get("value_count"),
    );
    assert_eq!(min, Some(2_000_000_000.0));
    assert_eq!(max, Some(2_009_000_000.0));
    assert_eq!(count, Some(10));

    let series = s
        .query_history(
            &TargetKind::Node,
            "node-1",
            &["node.memory.available_bytes".to_string()],
            None,
            // The 5m window containing T0 starts before T0 — query
            // from the window boundary or the rollup row falls outside
            // the range.
            T0 - (T0 % 300_000),
            T0 - (T0 % 300_000) + 300_000,
            5,
            Resolution::FiveMinute,
        )
        .await
        .unwrap();
    let rolled = &series[0];
    assert!(rolled
        .points
        .iter()
        .any(|p| p.quality == SampleQuality::Valid && p.value.is_some()));
}

/// A counter delta that crosses an ingestion gap must be rated against
/// the real time between its two samples: attributing it to a single
/// bucket would inflate the rate by the gap's length.
#[tokio::test]
async fn counter_bucket_window_spans_the_real_gap() {
    let (_dir, s) = store().await;
    // Two observations 60s apart (a 50s silence between them), 100s
    // range with 10 buckets ⇒ 10s nominal buckets.
    let samples = vec![
        iface_counter(T0, 1_000, "boot-a", "e"),
        iface_counter(T0 + 60_000, 4_000, "boot-a", "e"),
    ];
    s.ingest_node_batch("node-1", &batch("b", 0, samples), T0 + 60_000)
        .await
        .unwrap();

    let series = s
        .query_history(
            &TargetKind::Node,
            "node-1",
            &["node.net.rx_bytes_total".to_string()],
            None,
            T0,
            T0 + 100_000,
            10,
            Resolution::Raw,
        )
        .await
        .unwrap();
    let counter = &series[0];
    let delta_point = counter
        .points
        .iter()
        .find(|p| p.value.is_some())
        .expect("the gap-crossing delta is present");
    assert_eq!(delta_point.integer_value, Some(3_000));
    assert_eq!(
        delta_point.window_ms, 60_000,
        "window_ms is the observed span between the two samples, not the nominal bucket"
    );
    // The rate a consumer computes from the wire: 3000 bytes over the
    // 60s observed span = 50 B/s (not 3000 over one 10s bucket).
    let rate_per_second =
        delta_point.integer_value.unwrap() as f64 * 1000.0 / delta_point.window_ms as f64;
    assert!(
        (rate_per_second - 50.0).abs() < 1e-9,
        "rate is the true average, got {rate_per_second}"
    );
}

/// A rollup window with a single valid counter observation carries no
/// interval: the delta stays honestly absent (Valid always means the
/// point has a number — a lone sample must not fabricate a 0 rate).
#[tokio::test]
async fn rollup_lone_counter_sample_has_no_delta() {
    let (_dir, s) = store().await;
    let samples = vec![iface_counter(T0, 1_234, "boot-a", "e")];
    s.ingest_node_batch("node-1", &batch("b", 0, samples), T0 + 1_000)
        .await
        .unwrap();
    s.run_maintenance(T0 + 60_000).await.unwrap();

    let row = sqlx::query(
        "SELECT counter_delta FROM monitoring_rollups
         WHERE tier = '5m' AND metric_id = 'node.net.rx_bytes_total'",
    )
    .fetch_one(s.pool())
    .await
    .unwrap();
    let delta: Option<i64> = row.get("counter_delta");
    assert_eq!(delta, None, "a lone sample carries no interval");

    let series = s
        .query_history(
            &TargetKind::Node,
            "node-1",
            &["node.net.rx_bytes_total".to_string()],
            None,
            // The 5m window containing T0 starts before T0 — query
            // from the window boundary or the rollup row falls outside
            // the range.
            T0 - (T0 % 300_000),
            T0 - (T0 % 300_000) + 300_000,
            5,
            Resolution::FiveMinute,
        )
        .await
        .unwrap();
    let rolled = &series[0];
    let point = rolled
        .points
        .iter()
        .find(|p| p.value.is_none())
        .expect("the lone-sample window is visible");
    assert_ne!(
        point.quality,
        SampleQuality::Valid,
        "Valid always means has-a-number"
    );
}

/// Truncation is honest: a series whose points EXCEEDED the ceiling
/// and were thinned reports `truncated: true` (with at most
/// `max_points` points); a series merely AT the ceiling is complete
/// and stays `false`. The wire contract's downsampling signal must
/// never fire for a full-fidelity series.
#[tokio::test]
async fn history_reports_truncation_only_when_thinned() {
    let (_dir, s) = store().await;
    // 20 distinct gauge observations.
    let mut samples = Vec::new();
    for i in 0..20u64 {
        samples.push(node_gauge(T0 + i * 1_000, 0.1 * (i + 1) as f64));
    }
    s.ingest_node_batch("node-1", &batch("b", 0, samples), T0 + 20_000)
        .await
        .unwrap();

    let query = |to_ms: u64, max_points: usize| {
        let s = &s;
        async move {
            s.query_history(
                &TargetKind::Node,
                "node-1",
                &["node.cpu.capacity_ratio".to_string()],
                None,
                T0,
                to_ms,
                max_points,
                Resolution::Raw,
            )
            .await
            .unwrap()
        }
    };

    // 20 samples over a 20s range with a 20-point ceiling: one bucket
    // per second, exactly at the ceiling — complete, not truncated.
    let series = query(T0 + 20_000, 20).await;
    assert_eq!(series[0].points.len(), 20);
    assert!(!series[0].truncated, "at-ceiling is not truncation");

    // The same data with the range ending ON the last sample and a
    // 5-point ceiling: the boundary bucket overflows the ceiling, the
    // series is thinned and must say so.
    let series = query(T0 + 19_000, 5).await;
    assert_eq!(series[0].points.len(), 5);
    assert!(series[0].truncated, "over-ceiling is truncated");

    // The degenerate 1-point ceiling keeps exactly one point.
    let series = query(T0 + 19_000, 1).await;
    assert_eq!(series[0].points.len(), 1, "one-point ceiling is honored");
    assert!(series[0].truncated);
}

/// A retry that is byte-identical except the send timestamp is a
/// Duplicate (the previous durable outcome), never a ReplayConflict:
/// `sent_at_ms` is send-attempt metadata, not observation content.
#[tokio::test]
async fn later_send_time_alone_is_a_duplicate_not_a_conflict() {
    let (_dir, s) = store().await;
    let samples = vec![node_gauge(T0, 0.5)];
    let first = s
        .ingest_node_batch("node-1", &batch("b", 7, samples.clone()), T0 + 1_000)
        .await
        .unwrap();
    assert!(matches!(first, IngestOutcome::Accepted { .. }));

    // Same key, same samples, a later sent_at_ms (a retry after an
    // unknown-fate transport failure, re-read from the latest store).
    let mut retried = batch("b", 7, samples);
    retried.sent_at_ms = T0 + 30_000;
    let second = s
        .ingest_node_batch("node-1", &retried, T0 + 31_000)
        .await
        .unwrap();
    assert!(
        matches!(second, IngestOutcome::Duplicate { .. }),
        "identical samples at a later send time: {second:?}"
    );
}

// -- check inventory -------------------------------------------------------

/// The inventory holds exactly the LATEST record per check_id: a
/// newer observation replaces the row, and re-recording the same
/// observation is idempotent.
#[tokio::test]
async fn check_inventory_upserts_latest_per_check() {
    let (_dir, s) = store().await;
    s.record_checks(
        "agent:a1",
        &TargetKind::Vm,
        "vm-1",
        &[
            check("service:nginx.service", CheckStatus::Ok, T0),
            check("http:local:8080", CheckStatus::Warning, T0),
        ],
        T0 + 1_000,
    )
    .await
    .unwrap();

    // A later batch flips one status and introduces a new check.
    s.record_checks(
        "agent:a1",
        &TargetKind::Vm,
        "vm-1",
        &[
            check("service:nginx.service", CheckStatus::Critical, T0 + 60_000),
            check("plugin:example.http-health", CheckStatus::Ok, T0 + 60_000),
        ],
        T0 + 61_000,
    )
    .await
    .unwrap();

    let stored = s
        .query_checks(&TargetKind::Vm, "vm-1", T0 + 61_000)
        .await
        .unwrap();
    assert_eq!(stored.len(), 3, "one row per check_id: {stored:?}");
    let nginx = stored
        .iter()
        .find(|c| c.check_id == "service:nginx.service")
        .unwrap();
    assert_eq!(nginx.status, CheckStatus::Critical);
    assert_eq!(nginx.observed_at_ms, T0 + 60_000);
    assert_eq!(nginx.agent_id, "agent:a1");
    // The untouched check keeps its previous record.
    let http = stored
        .iter()
        .find(|c| c.check_id == "http:local:8080")
        .unwrap();
    assert_eq!(http.status, CheckStatus::Warning);
    assert_eq!(http.observed_at_ms, T0);

    // Re-recording the same observation is idempotent (equal
    // observed_at_ms refreshes receipt metadata only).
    s.record_checks(
        "agent:a1",
        &TargetKind::Vm,
        "vm-1",
        &[check(
            "service:nginx.service",
            CheckStatus::Critical,
            T0 + 60_000,
        )],
        T0 + 62_000,
    )
    .await
    .unwrap();
    let stored = s
        .query_checks(&TargetKind::Vm, "vm-1", T0 + 62_000)
        .await
        .unwrap();
    assert_eq!(stored.len(), 3);
    let nginx = stored
        .iter()
        .find(|c| c.check_id == "service:nginx.service")
        .unwrap();
    assert_eq!(nginx.status, CheckStatus::Critical);
    assert_eq!(nginx.received_at_ms, T0 + 62_000);
}

/// Rows only move forward: a delayed batch with an OLDER
/// observed_at_ms must not regress a newer inventory record.
#[tokio::test]
async fn older_check_batch_does_not_regress_the_inventory() {
    let (_dir, s) = store().await;
    s.record_checks(
        "agent:a1",
        &TargetKind::Vm,
        "vm-1",
        &[check(
            "service:nginx.service",
            CheckStatus::Critical,
            T0 + 120_000,
        )],
        T0 + 121_000,
    )
    .await
    .unwrap();

    // A delayed batch (e.g. reordered delivery) carrying an older
    // observation with a rosier status: the inventory must keep the
    // newer record.
    s.record_checks(
        "agent:a1",
        &TargetKind::Vm,
        "vm-1",
        &[check("service:nginx.service", CheckStatus::Ok, T0 + 60_000)],
        T0 + 122_000,
    )
    .await
    .unwrap();

    let stored = s
        .query_checks(&TargetKind::Vm, "vm-1", T0 + 122_000)
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, CheckStatus::Critical, "newer record wins");
    assert_eq!(stored[0].observed_at_ms, T0 + 120_000);
    assert_eq!(stored[0].received_at_ms, T0 + 121_000);
}

/// Query marks staleness with the registry's `check.*` window (60 s
/// cadence → 180 s): one missed collection is not stale; three are.
#[tokio::test]
async fn check_query_reports_staleness_and_empty_targets() {
    let (_dir, s) = store().await;
    s.record_checks(
        "agent:a1",
        &TargetKind::Vm,
        "vm-1",
        &[
            check("service:nginx.service", CheckStatus::Ok, T0),
            check("http:local:8080", CheckStatus::Ok, T0 + 120_000),
        ],
        T0 + 1_000,
    )
    .await
    .unwrap();

    // 150 s after the first observation (30 s after the second):
    // both inside the 180 s window.
    let stored = s
        .query_checks(&TargetKind::Vm, "vm-1", T0 + 150_000)
        .await
        .unwrap();
    assert!(
        stored.iter().all(|c| !c.stale),
        "inside 180s window: {stored:?}"
    );

    // 181 s after the T0 observation: that check is stale; the 120 s
    // one (61 s old) is not.
    let stored = s
        .query_checks(&TargetKind::Vm, "vm-1", T0 + 181_000)
        .await
        .unwrap();
    let nginx = stored
        .iter()
        .find(|c| c.check_id == "service:nginx.service")
        .unwrap();
    assert!(nginx.stale, "181s-old check is stale");
    let http = stored
        .iter()
        .find(|c| c.check_id == "http:local:8080")
        .unwrap();
    assert!(!http.stale, "61s-old check is fresh");

    // A target with no recorded checks: honest absence, empty vec.
    let empty = s.query_checks(&TargetKind::Vm, "vm-404", T0).await.unwrap();
    assert_eq!(empty, Vec::<StoredCheck>::new());
    // ...and kind separation holds (a VM's checks are not a node's).
    let empty = s.query_checks(&TargetKind::Node, "vm-1", T0).await.unwrap();
    assert!(empty.is_empty());
}
