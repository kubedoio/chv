use crate::api::{health, nodes, operations};
use axum::{http::StatusCode, response::Json, routing::get, Router};
use chv_controlplane_store::StorePool;
use std::sync::Arc;

async fn not_found_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "not_found",
            "message": "This endpoint is not implemented in the current control plane build."
        })),
    )
}

pub fn admin_router(pool: StorePool) -> Router {
    Router::new()
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .route("/metrics", get(health::metrics_handler))
        .route("/admin/nodes", get(nodes::list_nodes))
        .route("/admin/nodes/{id}", get(nodes::get_node))
        .route("/admin/operations", get(operations::list_operations))
        .route("/admin/operations/{id}", get(operations::get_operation))
        .fallback(not_found_handler)
        .with_state(Arc::new(pool))
}
