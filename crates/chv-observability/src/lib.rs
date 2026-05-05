use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use tracing::Span;

/// ADR-009 mandated metric names.
pub const CHV_VMS_TOTAL: &str = "chv_vms_total";
pub const CHV_NODES_READY: &str = "chv_nodes_ready";
pub const CHV_OPERATION_DURATION_SECONDS: &str = "chv_operation_duration_seconds";

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
}
