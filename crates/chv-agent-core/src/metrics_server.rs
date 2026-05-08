use axum::{extract::State, response::IntoResponse, routing::get, Router};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Shared state exposed via the Prometheus metrics endpoint.
pub struct MetricsState {
    pub node_id: String,
    pub node_state: String,
    pub vm_count: usize,
    pub tick_count: u64,
    pub reconcile_failures: u32,
    pub health_failures: u32,
    pub start_time: Instant,
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
        }
    }
}

/// Handler that renders metrics in Prometheus text exposition format.
async fn metrics_handler(State(state): State<Arc<Mutex<MetricsState>>>) -> impl IntoResponse {
    let s = state.lock().await;
    let uptime = s.start_time.elapsed().as_secs();
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
         chv_agent_uptime_seconds{{node_id=\"{node_id}\"}} {uptime}\n",
        node_id = s.node_id,
        state = s.node_state,
        vms = s.vm_count,
        ticks = s.tick_count,
        rfail = s.reconcile_failures,
        hfail = s.health_failures,
        uptime = uptime,
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
