use axum::{extract::State, response::IntoResponse, routing::get, Router};
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Disks, System};
use tokio::sync::Mutex;

use crate::connectivity::ConnectivityState;

// Metric name constants for external consumers and consistency.
pub const METRIC_NODE_STATE: &str = "chv_agent_node_state";
pub const METRIC_VMS_TOTAL: &str = "chv_agent_vms_total";
pub const METRIC_RECONCILE_TICKS: &str = "chv_agent_reconcile_ticks_total";
pub const METRIC_RECONCILE_FAILURES: &str = "chv_agent_reconcile_failures_total";
pub const METRIC_HEALTH_FAILURES: &str = "chv_agent_health_failures_total";
pub const METRIC_UPTIME: &str = "chv_agent_uptime_seconds";
pub const METRIC_CP_CONNECTED: &str = "chv_agent_cp_connected";
pub const METRIC_RECONCILE_DRIFT_EMA: &str = "chv_agent_reconcile_drift_ema";
pub const METRIC_RECONCILE_DURATION: &str = "chv_agent_reconcile_duration_seconds";

pub struct HostResources {
    pub cpu_usage_percent: f32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_available_bytes: u64,
}

impl Default for HostResources {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_total_bytes: 0,
            memory_used_bytes: 0,
            disk_total_bytes: 0,
            disk_available_bytes: 0,
        }
    }
}

/// Shared state exposed via the Prometheus metrics endpoint.
pub struct MetricsState {
    pub node_id: String,
    pub node_state: String,
    pub vm_count: usize,
    pub tick_count: u64,
    pub reconcile_failures: u32,
    pub health_failures: u32,
    pub start_time: Instant,
    // Connectivity tracking
    pub cp_connectivity_state: ConnectivityState,
    pub cp_disconnected_duration_ms: i64,
    pub cp_consecutive_failures: u32,
    pub cp_total_deferred_messages: u64,
    // Host resources
    pub host: HostResources,
    // Reconcile performance metrics
    pub reconcile_drift_ema: f64,
    pub last_reconcile_duration_ms: u64,
    // Resource pressure indicators
    pub disk_pressure: bool,
    pub memory_pressure: bool,
}

impl MetricsState {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            node_state: "Bootstrapping".to_string(),
            vm_count: 0,
            tick_count: 0,
            reconcile_failures: 0,
            health_failures: 0,
            start_time: Instant::now(),
            cp_connectivity_state: ConnectivityState::Disconnected,
            cp_disconnected_duration_ms: 0,
            cp_consecutive_failures: 0,
            cp_total_deferred_messages: 0,
            host: HostResources::default(),
            reconcile_drift_ema: 0.0,
            last_reconcile_duration_ms: 0,
            disk_pressure: false,
            memory_pressure: false,
        }
    }
}

/// Collect current host resource metrics using sysinfo.
fn collect_host_resources() -> HostResources {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_total = sys.total_memory();
    let memory_used = sys.used_memory();

    let disks = Disks::new_with_refreshed_list();
    let (disk_total, disk_available) = disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| (d.total_space(), d.available_space()))
        .unwrap_or((0, 0));

    HostResources {
        cpu_usage_percent: cpu_usage,
        memory_total_bytes: memory_total,
        memory_used_bytes: memory_used,
        disk_total_bytes: disk_total,
        disk_available_bytes: disk_available,
    }
}

/// Handler that renders metrics in Prometheus text exposition format.
async fn metrics_handler(State(state): State<Arc<Mutex<MetricsState>>>) -> impl IntoResponse {
    let host = collect_host_resources();
    let s = state.lock().await;
    let uptime = s.start_time.elapsed().as_secs();
    let reconcile_duration_secs = s.last_reconcile_duration_ms as f64 / 1000.0;
    let body = format!(
        "# HELP chv_agent_node_state Current node state (1=active)\n\
         # TYPE chv_agent_node_state gauge\n\
         chv_agent_node_state{{node_id=\"{node_id}\",state=\"{state}\"}} 1\n\
         # HELP chv_agent_vms_total Number of VMs managed\n\
         # TYPE chv_agent_vms_total gauge\n\
         chv_agent_vms_total{{node_id=\"{node_id}\"}} {vms}\n\
         # HELP chv_agent_reconcile_ticks_total Total reconciliation ticks\n\
         # TYPE chv_agent_reconcile_ticks_total counter\n\
         chv_agent_reconcile_ticks_total{{node_id=\"{node_id}\"}} {ticks}\n\
         # HELP chv_agent_reconcile_failures_total Total reconciliation failures\n\
         # TYPE chv_agent_reconcile_failures_total counter\n\
         chv_agent_reconcile_failures_total{{node_id=\"{node_id}\"}} {rfail}\n\
         # HELP chv_agent_health_failures_total Total health check failures\n\
         # TYPE chv_agent_health_failures_total counter\n\
         chv_agent_health_failures_total{{node_id=\"{node_id}\"}} {hfail}\n\
         # HELP chv_agent_uptime_seconds Agent uptime in seconds\n\
         # TYPE chv_agent_uptime_seconds gauge\n\
         chv_agent_uptime_seconds{{node_id=\"{node_id}\"}} {uptime}\n\
         # HELP chv_agent_cp_connected Control plane connectivity (1=connected, 0=disconnected)\n\
         # TYPE chv_agent_cp_connected gauge\n\
         chv_agent_cp_connected{{node_id=\"{node_id}\",state=\"{cp_state}\"}} {cp_connected}\n\
         # HELP chv_agent_cp_disconnected_duration_ms Duration of current control plane disconnect in milliseconds\n\
         # TYPE chv_agent_cp_disconnected_duration_ms gauge\n\
         chv_agent_cp_disconnected_duration_ms{{node_id=\"{node_id}\"}} {cp_disconnect_ms}\n\
         # HELP chv_agent_cp_consecutive_failures Consecutive control plane contact failures\n\
         # TYPE chv_agent_cp_consecutive_failures gauge\n\
         chv_agent_cp_consecutive_failures{{node_id=\"{node_id}\"}} {cp_failures}\n\
         # HELP chv_agent_cp_deferred_messages_total Total messages deferred due to control plane unavailability\n\
         # TYPE chv_agent_cp_deferred_messages_total counter\n\
         chv_agent_cp_deferred_messages_total{{node_id=\"{node_id}\"}} {cp_deferred}\n\
         # HELP chv_agent_cpu_usage_percent Overall CPU utilization percentage\n\
         # TYPE chv_agent_cpu_usage_percent gauge\n\
         chv_agent_cpu_usage_percent{{node_id=\"{node_id}\"}} {cpu_usage}\n\
         # HELP chv_agent_memory_total_bytes Total system RAM in bytes\n\
         # TYPE chv_agent_memory_total_bytes gauge\n\
         chv_agent_memory_total_bytes{{node_id=\"{node_id}\"}} {mem_total}\n\
         # HELP chv_agent_memory_used_bytes Used system RAM in bytes\n\
         # TYPE chv_agent_memory_used_bytes gauge\n\
         chv_agent_memory_used_bytes{{node_id=\"{node_id}\"}} {mem_used}\n\
         # HELP chv_agent_disk_total_bytes Total disk space on root mount in bytes\n\
         # TYPE chv_agent_disk_total_bytes gauge\n\
         chv_agent_disk_total_bytes{{node_id=\"{node_id}\"}} {disk_total}\n\
         # HELP chv_agent_disk_available_bytes Available disk space on root mount in bytes\n\
         # TYPE chv_agent_disk_available_bytes gauge\n\
         chv_agent_disk_available_bytes{{node_id=\"{node_id}\"}} {disk_avail}\n\
         # HELP chv_agent_reconcile_drift_ema Exponential moving average of reconcile drift\n\
         # TYPE chv_agent_reconcile_drift_ema gauge\n\
         chv_agent_reconcile_drift_ema{{node_id=\"{node_id}\"}} {drift_ema}\n\
         # HELP chv_agent_reconcile_duration_seconds Last reconcile tick duration\n\
         # TYPE chv_agent_reconcile_duration_seconds gauge\n\
         chv_agent_reconcile_duration_seconds{{node_id=\"{node_id}\"}} {reconcile_dur}\n\
         # HELP chv_agent_disk_pressure Whether disk is under pressure (1=yes, 0=no)\n\
         # TYPE chv_agent_disk_pressure gauge\n\
         chv_agent_disk_pressure{{node_id=\"{node_id}\"}} {disk_pressure}\n\
         # HELP chv_agent_memory_pressure Whether memory is under pressure (1=yes, 0=no)\n\
         # TYPE chv_agent_memory_pressure gauge\n\
         chv_agent_memory_pressure{{node_id=\"{node_id}\"}} {mem_pressure}\n",
        node_id = s.node_id,
        state = s.node_state,
        vms = s.vm_count,
        ticks = s.tick_count,
        rfail = s.reconcile_failures,
        hfail = s.health_failures,
        uptime = uptime,
        cp_state = s.cp_connectivity_state.as_str(),
        cp_connected = if s.cp_connectivity_state == ConnectivityState::Connected { 1 } else { 0 },
        cp_disconnect_ms = s.cp_disconnected_duration_ms,
        cp_failures = s.cp_consecutive_failures,
        cp_deferred = s.cp_total_deferred_messages,
        cpu_usage = host.cpu_usage_percent,
        mem_total = host.memory_total_bytes,
        mem_used = host.memory_used_bytes,
        disk_total = host.disk_total_bytes,
        disk_avail = host.disk_available_bytes,
        drift_ema = s.reconcile_drift_ema,
        reconcile_dur = reconcile_duration_secs,
        disk_pressure = if s.disk_pressure { 1 } else { 0 },
        mem_pressure = if s.memory_pressure { 1 } else { 0 },
    );
    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Build an axum Router for the `/metrics` endpoint.
pub fn metrics_router(state: Arc<Mutex<MetricsState>>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}
