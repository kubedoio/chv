use crate::api::{health, nodes, operations};
use axum::{routing::get, Router};
use chv_controlplane_store::StorePool;
use std::sync::Arc;

pub fn admin_router(pool: StorePool) -> Router {
    Router::new()
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .route("/metrics", get(health::metrics_handler))
        .route("/admin/nodes", get(nodes::list_nodes))
        .route("/admin/nodes/{id}", get(nodes::get_node))
        .route("/admin/operations", get(operations::list_operations))
        .route("/admin/operations/{id}", get(operations::get_operation))
        .with_state(Arc::new(pool))
}
