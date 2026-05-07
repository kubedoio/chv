use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use tracing::Span;

/// ADR-009 mandated metric names.
pub const CHV_VMS_TOTAL: &str = "chv_vms_total";
pub const CHV_NODES_READY: &str = "chv_nodes_ready";
pub const CHV_OPERATION_DURATION_SECONDS: &str = "chv_operation_duration_seconds";

// ---------------------------------------------------------------------------
// Multi-node metric names (Phase 10)
// ---------------------------------------------------------------------------

// Migration metrics
pub const CHV_MIGRATION_PHASE: &str = "chv_migration_phase";
pub const CHV_MIGRATION_BYTES_TRANSFERRED: &str = "chv_migration_bytes_transferred";
pub const CHV_MIGRATION_DURATION_SECONDS: &str = "chv_migration_duration_seconds";
pub const CHV_MIGRATION_DIRTY_BLOCKS: &str = "chv_migration_dirty_blocks";

// Overlay metrics
pub const CHV_VXLAN_FDB_ENTRIES: &str = "chv_vxlan_fdb_entries";

// eBPF metrics
pub const CHV_EBPF_PACKETS_TOTAL: &str = "chv_ebpf_packets_total";
pub const CHV_EBPF_BYTES_TOTAL: &str = "chv_ebpf_bytes_total";

static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Returns the globally installed Prometheus metrics handle, if one exists.
pub fn prometheus_handle() -> Option<&'static PrometheusHandle> {
    PROMETHEUS_HANDLE.get()
}

pub fn init_logger(filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let use_json = std::env::var("CHV_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if use_json {
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .with_target(true)
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    } else {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    }

    let recorder = PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder)
        .map_err(|e| format!("failed to install metrics recorder: {e}"))?;

    let _ = PROMETHEUS_HANDLE.set(handle);

    Ok(())
}

pub fn operation_span(op_id: &str) -> Span {
    tracing::info_span!("operation", operation_id = op_id)
}

#[derive(Debug, Clone, Default)]
pub struct Metrics;

impl Metrics {
    pub fn new() -> Self {
        Self
    }

    pub fn increment_counter(&self, name: &'static str) {
        metrics::counter!(name).increment(1);
    }

    pub fn gauge(&self, name: &'static str, value: f64) {
        metrics::gauge!(name).set(value);
    }

    pub fn record_histogram(&self, name: &'static str, value: f64) {
        metrics::histogram!(name).record(value);
    }

    pub fn record_operation_duration(&self, operation: &'static str, duration_secs: f64) {
        metrics::histogram!(CHV_OPERATION_DURATION_SECONDS, "operation" => operation)
            .record(duration_secs);
    }

    // -----------------------------------------------------------------------
    // Multi-node metric helpers (Phase 10)
    // -----------------------------------------------------------------------

    /// Record the current migration phase as a gauge (0=Pending .. 7=RolledBack).
    pub fn set_migration_phase(&self, migration_id: &str, vm_id: &str, phase: f64) {
        metrics::gauge!(
            CHV_MIGRATION_PHASE,
            "migration_id" => migration_id.to_string(),
            "vm_id" => vm_id.to_string()
        )
        .set(phase);
    }

    /// Increment migration bytes transferred counter.
    pub fn add_migration_bytes(&self, migration_id: &str, vm_id: &str, bytes: u64) {
        metrics::counter!(
            CHV_MIGRATION_BYTES_TRANSFERRED,
            "migration_id" => migration_id.to_string(),
            "vm_id" => vm_id.to_string()
        )
        .increment(bytes);
    }

    /// Record total migration duration.
    pub fn record_migration_duration(&self, vm_id: &str, outcome: &str, duration_secs: f64) {
        metrics::histogram!(
            CHV_MIGRATION_DURATION_SECONDS,
            "vm_id" => vm_id.to_string(),
            "outcome" => outcome.to_string()
        )
        .record(duration_secs);
    }

    /// Set current dirty block count during migration convergence.
    pub fn set_migration_dirty_blocks(&self, migration_id: &str, blocks: f64) {
        metrics::gauge!(
            CHV_MIGRATION_DIRTY_BLOCKS,
            "migration_id" => migration_id.to_string()
        )
        .set(blocks);
    }

    /// Set current FDB entry count for a VXLAN network.
    pub fn set_vxlan_fdb_entries(&self, network_id: &str, node_id: &str, count: f64) {
        metrics::gauge!(
            CHV_VXLAN_FDB_ENTRIES,
            "network_id" => network_id.to_string(),
            "node_id" => node_id.to_string()
        )
        .set(count);
    }

    /// Increment eBPF packet counter.
    pub fn add_ebpf_packets(&self, vm_id: &str, direction: &str, action: &str, count: u64) {
        metrics::counter!(
            CHV_EBPF_PACKETS_TOTAL,
            "vm_id" => vm_id.to_string(),
            "direction" => direction.to_string(),
            "action" => action.to_string()
        )
        .increment(count);
    }

    /// Increment eBPF byte counter.
    pub fn add_ebpf_bytes(&self, vm_id: &str, direction: &str, action: &str, bytes: u64) {
        metrics::counter!(
            CHV_EBPF_BYTES_TOTAL,
            "vm_id" => vm_id.to_string(),
            "direction" => direction.to_string(),
            "action" => action.to_string()
        )
        .increment(bytes);
    }
}
