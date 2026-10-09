//! Native monitoring sampler wiring for `chv-agent` (gate G1, prompt 01
//! task 7).
//!
//! One bounded sampling task, **independent of reconciliation and state
//! reports**, collects node-level contract samples (CPU ratio, load
//! averages, memory, swap, root filesystem, per-interface and
//! per-block-device counters) through `chv-monitoring-core`'s
//! [`run_sampler`]. Its bounds and health counters live in the sampler
//! crate; this module provides:
//!
//! - [`NodeOsSampleSource`] — the node source adapter (retained
//!   `NodeOsCollector` plus per-cycle `/proc/net/dev` and
//!   `/proc/diskstats` reads);
//! - [`LatestSamples`] — the bounded latest-sample store the sink
//!   consumer maintains (per metric+target+dimension key; PR-2's ingest
//!   replaces this with the versioned node transport);
//! - [`spawn_monitoring_sampler`] — spawns the loop and returns the
//!   health handle for the `/metrics` surface.
//!
//! VM samples still ride the existing VmStateReport transport in this
//! PR (repaired to real values); moving VM collection behind the
//! sampler + ingest is PR-2's scope.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chv_monitoring_core::model::{
    Sample, SampleBuilder, SampleQuality, SampleValue, Source, TargetKind,
};
use chv_monitoring_core::node_os::{NodeOsCollector, NodeOsSnapshot};
use chv_monitoring_core::sampler::{
    run_sampler, NodeOsSource, SamplerConfig, SamplerError, SamplerHealth, SamplerSources,
};
use tokio::sync::Mutex;

/// Retained node collector shared by the sampling task. The retention is
/// the CPU-interval fix (see `chv-monitoring-core::node_os`).
struct NodeOsSampleSource {
    node_id: String,
    collector: Mutex<NodeOsCollector>,
    proc_root: &'static Path,
}

impl NodeOsSampleSource {
    fn new(node_id: String) -> Self {
        NodeOsSampleSource {
            node_id,
            collector: Mutex::new(NodeOsCollector::new()),
            proc_root: Path::new("/proc"),
        }
    }
}

/// Build one node sample; a construction failure (should be impossible
/// for registry node metrics) is a source error, never a zero-valued
/// sample.
fn node_sample(
    node_id: &str,
    metric: &str,
    observed_ms: u64,
    value: Option<SampleValue>,
) -> Result<Sample, SamplerError> {
    let mut b = SampleBuilder::new(
        TargetKind::Node,
        node_id,
        metric,
        Source::NodeOs,
        observed_ms,
    )
    .map_err(|e| SamplerError::Source(e.to_string()))?;
    match value {
        Some(v) => b = b.value(v),
        None => b = b.quality(SampleQuality::InsufficientSamples),
    }
    b.build().map_err(|e| SamplerError::Source(e.to_string()))
}

/// Counter samples need their epoch (boot id + a stable identity for the
/// series); interface/device counters restart with the host, and the
/// agent does not survive a host reboot, so the node identity is the
/// fence.
fn node_counter_sample(
    node_id: &str,
    metric: &str,
    observed_ms: u64,
    boot_id: &str,
    series: &str,
    value: u64,
    dimension: (&str, &str),
) -> Result<Sample, SamplerError> {
    let err = |e: chv_monitoring_core::model::SampleError| SamplerError::Source(e.to_string());
    let builder = SampleBuilder::new(
        TargetKind::Node,
        node_id,
        metric,
        Source::NodeOs,
        observed_ms,
    )
    .map_err(err)?
    .dimension(dimension.0, dimension.1)
    .map_err(err)?
    .epoch(boot_id, series)
    .value(SampleValue::Integer(value));
    builder.build().map_err(err)
}

#[async_trait::async_trait]
impl NodeOsSource for NodeOsSampleSource {
    async fn collect_node(&self) -> Result<Vec<Sample>, SamplerError> {
        let snapshot = {
            let mut collector = self.collector.lock().await;
            collector.snapshot()
        };
        let mut samples = Vec::with_capacity(16);

        let NodeOsSnapshot {
            observed_at_ms,
            cpu_capacity_ratio,
            cpu_observed_at_ms,
            load1,
            load5,
            load15,
            memory_total_bytes,
            memory_available_bytes,
            swap_used_bytes,
            root_fs_total_bytes,
            root_fs_available_bytes,
            ..
        } = snapshot;

        // CPU: no valid interval yet ⇒ insufficient_samples, never zero.
        // The sample's observed_at is the measurement time, not the
        // cycle time — a retained reading (sub-interval cycle) is older
        // than this cycle and must not be stamped as fresh.
        samples.push(node_sample(
            &self.node_id,
            "node.cpu.capacity_ratio",
            if cpu_capacity_ratio.is_some() && cpu_observed_at_ms > 0 {
                cpu_observed_at_ms
            } else {
                observed_at_ms
            },
            cpu_capacity_ratio.map(SampleValue::Float),
        )?);
        for (metric, value) in [
            ("node.cpu.load1", load1),
            ("node.cpu.load5", load5),
            ("node.cpu.load15", load15),
        ] {
            samples.push(node_sample(
                &self.node_id,
                metric,
                observed_at_ms,
                value.map(SampleValue::Float),
            )?);
        }

        samples.push(node_sample(
            &self.node_id,
            "node.memory.total_bytes",
            observed_at_ms,
            Some(SampleValue::Integer(memory_total_bytes)),
        )?);
        samples.push(node_sample(
            &self.node_id,
            "node.memory.available_bytes",
            observed_at_ms,
            Some(SampleValue::Integer(memory_available_bytes)),
        )?);
        samples.push(node_sample(
            &self.node_id,
            "node.swap.used_bytes",
            observed_at_ms,
            Some(SampleValue::Integer(swap_used_bytes)),
        )?);

        for (metric, value) in [
            ("node.fs.total_bytes", root_fs_total_bytes),
            ("node.fs.available_bytes", root_fs_available_bytes),
        ] {
            let err =
                |e: chv_monitoring_core::model::SampleError| SamplerError::Source(e.to_string());
            let mut b = SampleBuilder::new(
                TargetKind::Node,
                &self.node_id,
                metric,
                Source::NodeOs,
                observed_at_ms,
            )
            .map_err(err)?
            .dimension("mount_id", "/")
            .map_err(err)?;
            match value {
                Some(v) => b = b.value(SampleValue::Integer(v)),
                None => b = b.quality(SampleQuality::Unavailable),
            }
            samples.push(b.build().map_err(err)?);
        }

        // Per-interface and per-block-device counters, labeled — never
        // summed blindly across bridges or stacked devices. Counter
        // samples need the boot epoch; if /proc is unreadable we keep
        // the gauge samples above (memory, fs, …) and skip counters
        // this cycle rather than failing the whole batch — a source
        // failure is counted by the sampler's health counters either
        // way, and the log line names the actual cause.
        let boot_id = chv_monitoring_core::process_probe::read_boot_id(self.proc_root);
        let boot_id = match &boot_id {
            Ok(b) => Some(b.as_str()),
            Err(e) => {
                tracing::warn!(error = %e, "node sampler: boot_id unreadable, skipping counter samples this cycle");
                None
            }
        };
        if let Some(boot_id) = boot_id {
            match chv_monitoring_core::proc_net::read_net_dev(self.proc_root) {
                Ok(interfaces) => {
                    for (iface, counters) in interfaces {
                        let series = format!("iface-{iface}");
                        samples.push(node_counter_sample(
                            &self.node_id,
                            "node.net.rx_bytes_total",
                            observed_at_ms,
                            boot_id,
                            &series,
                            counters.rx_bytes,
                            ("interface_id", iface.as_str()),
                        )?);
                        samples.push(node_counter_sample(
                            &self.node_id,
                            "node.net.tx_bytes_total",
                            observed_at_ms,
                            boot_id,
                            &series,
                            counters.tx_bytes,
                            ("interface_id", iface.as_str()),
                        )?);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "node sampler: /proc/net/dev unreadable, skipping interface counters this cycle");
                }
            }
            match chv_monitoring_core::proc_net::read_diskstats(self.proc_root) {
                Ok(devices) => {
                    for (device, counters) in devices {
                        let series = format!("block-{device}");
                        samples.push(node_counter_sample(
                            &self.node_id,
                            "node.block.read_bytes_total",
                            observed_at_ms,
                            boot_id,
                            &series,
                            counters.read_bytes,
                            ("block_device_id", device.as_str()),
                        )?);
                        samples.push(node_counter_sample(
                            &self.node_id,
                            "node.block.write_bytes_total",
                            observed_at_ms,
                            boot_id,
                            &series,
                            counters.write_bytes,
                            ("block_device_id", device.as_str()),
                        )?);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "node sampler: /proc/diskstats unreadable, skipping block counters this cycle");
                }
            }
        }

        Ok(samples)
    }
}

/// Bounded store of the latest sample per series key
/// (metric, target, dimensions). The node surface is inherently small
/// (registry metrics × node × interfaces/devices); the store replaces
/// each key's entry every cycle, so memory is bounded by the node's own
/// device count — the same bound the collector reads.
#[derive(Debug, Default)]
pub struct LatestSamples {
    inner: Mutex<HashMap<String, Sample>>,
}

impl LatestSamples {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(sample: &Sample) -> String {
        let dims: Vec<String> = sample
            .dimensions
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!(
            "{}|{}|{:?}|{}",
            sample.metric_id,
            sample.target_id,
            sample.source,
            dims.join(",")
        )
    }

    /// Replace the stored sample for each series in the batch.
    pub async fn replace(&self, batch: Vec<Sample>) {
        let mut inner = self.inner.lock().await;
        for sample in batch {
            inner.insert(Self::key(&sample), sample);
        }
    }

    /// The latest samples (any order). Consumed by PR-2's node ingest;
    /// test-used until then.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn all(&self) -> Vec<Sample> {
        self.inner.lock().await.values().cloned().collect()
    }

    /// Number of series currently stored. Consumed by PR-2's node
    /// ingest; test-used until then.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// Whether no series is stored. Consumed by PR-2's node ingest;
    /// test-used until then.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }
}

/// Spawn the monitoring sampler: the bounded loop plus the sink consumer
/// that maintains [`LatestSamples`]. Returns the shared health handle
/// for the `/metrics` surface and the store for future consumers.
pub fn spawn_monitoring_sampler(
    node_id: String,
    config: SamplerConfig,
) -> (Arc<SamplerHealth>, Arc<LatestSamples>) {
    let health = Arc::new(SamplerHealth::new());
    let store = Arc::new(LatestSamples::new());
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<Sample>>(256);

    let sources = SamplerSources {
        node: Arc::new(NodeOsSampleSource::new(node_id)),
        // VM samples ride the repaired VmStateReport transport in this
        // PR; PR-2's ingest moves VM collection behind the sampler.
        vms: None,
        // stord/nwd expose no attributable v1 metrics yet (volume health
        // exists but is not a registry metric) — no adapters wired, no
        // coverage faked.
        storage: None,
        network: None,
    };

    tokio::spawn(run_sampler(config, sources, tx, health.clone()));
    let consumer_store = store.clone();
    tokio::spawn(async move {
        while let Some(batch) = rx.recv().await {
            consumer_store.replace(batch).await;
        }
    });

    (health, store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn node_source_produces_contract_samples() {
        let source = NodeOsSampleSource::new("node-test".to_string());
        let samples = source.collect_node().await.unwrap();
        let ids: Vec<&str> = samples.iter().map(|s| s.metric_id.as_str()).collect();
        for expected in [
            "node.cpu.capacity_ratio",
            "node.cpu.load1",
            "node.cpu.load5",
            "node.cpu.load15",
            "node.memory.total_bytes",
            "node.memory.available_bytes",
            "node.swap.used_bytes",
            "node.fs.total_bytes",
            "node.fs.available_bytes",
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
        // Interface/block samples appear only when the host actually
        // reports them (a namespace with only `lo` or a masked
        // diskstats legitimately yields none).
        let has_interfaces =
            !chv_monitoring_core::proc_net::read_net_dev(std::path::Path::new("/proc"))
                .map(|m| m.is_empty())
                .unwrap_or(true);
        if has_interfaces {
            assert!(samples
                .iter()
                .any(|s| s.metric_id == "node.net.rx_bytes_total"));
        }
        let has_devices =
            !chv_monitoring_core::proc_net::read_diskstats(std::path::Path::new("/proc"))
                .map(|m| m.is_empty())
                .unwrap_or(true);
        if has_devices {
            assert!(samples
                .iter()
                .any(|s| s.metric_id == "node.block.read_bytes_total"));
        }
        // Counters carry their epoch.
        for s in samples
            .iter()
            .filter(|s| s.kind == chv_monitoring_core::model::MetricKind::Counter)
        {
            assert!(s.boot_id.is_some() && s.identity_epoch.is_some());
            assert!(s.value.is_some(), "counter sample without value");
        }
    }

    #[tokio::test]
    async fn first_cycle_cpu_is_insufficient_not_zero() {
        let source = NodeOsSampleSource::new("node-test".to_string());
        let samples = source.collect_node().await.unwrap();
        let cpu = samples
            .iter()
            .find(|s| s.metric_id == "node.cpu.capacity_ratio")
            .unwrap();
        assert_eq!(cpu.quality, SampleQuality::InsufficientSamples);
        assert!(cpu.value.is_none(), "no interval, no value");
    }

    #[tokio::test]
    async fn store_replaces_per_series() {
        let store = LatestSamples::new();
        let s1 = node_sample("n", "node.cpu.load1", 1, Some(SampleValue::Float(1.0))).unwrap();
        let s2 = node_sample("n", "node.cpu.load1", 2, Some(SampleValue::Float(2.0))).unwrap();
        store.replace(vec![s1]).await;
        store.replace(vec![s2]).await;
        let all = store.all().await;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].value, Some(SampleValue::Float(2.0)));
        // Different dimension = different series.
        let s3 = node_counter_sample(
            "n",
            "node.net.rx_bytes_total",
            3,
            "boot",
            "iface-eth0",
            10,
            ("interface_id", "eth0"),
        )
        .unwrap();
        store.replace(vec![s3]).await;
        assert_eq!(store.len().await, 2);
    }

    #[tokio::test]
    async fn sampler_end_to_end_fills_the_store() {
        let (health, store) = spawn_monitoring_sampler(
            "node-test".to_string(),
            SamplerConfig {
                interval: std::time::Duration::from_millis(50),
                ..SamplerConfig::default()
            },
        );
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let snap = health.snapshot();
        assert!(snap.cycles_completed >= 2, "sampler cycles ran");
        assert!(!store.is_empty().await, "store filled");
        // The sampler and consumer tasks are detached; the test runtime
        // drops them at exit once the assertions hold.
    }
}
