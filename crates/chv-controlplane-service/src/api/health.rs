use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chv_controlplane_store::StorePool;
use std::sync::Arc;
use std::sync::OnceLock;

pub async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn ready_handler(State(pool): State<Arc<StorePool>>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "not_ready"})),
        ),
    }
}

static PROMETHEUS: OnceLock<Result<metrics_exporter_prometheus::PrometheusHandle, String>> =
    OnceLock::new();

pub async fn metrics_handler() -> impl IntoResponse {
    let handle = match PROMETHEUS.get_or_init(|| {
        metrics_exporter_prometheus::PrometheusBuilder::new()
            .install_recorder()
            .map_err(|e| format!("{e}"))
    }) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("metrics recorder error: {e}"),
            );
        }
    };
    (StatusCode::OK, handle.render())
}
