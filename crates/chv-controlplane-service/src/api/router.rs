use crate::api::{health, nodes, operations, stub};
use axum::{http::StatusCode, response::Json, routing::{get, post}, Router};
use chv_webui_bff::AppState;

async fn not_found_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": {
                "code": "NOT_IMPLEMENTED",
                "message": "This endpoint is not implemented in the current control plane build.",
                "retryable": false,
                "hint": "Use the BFF-backed /v1 routes for supported UI workflows."
            }
        })),
    )
}

pub fn admin_router(bff_state: AppState) -> Router {
    let bff_router = chv_webui_bff::bff_router();

    Router::new()
        .merge(bff_router)
        // Health & admin
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .route("/metrics", get(health::metrics_handler))
        .route("/admin/nodes", get(nodes::list_nodes))
        .route("/admin/nodes/{id}", get(nodes::get_node))
        .route("/admin/operations", get(operations::list_operations))
        .route("/admin/operations/{id}", get(operations::get_operation))
        // Auth stubs
        .route("/api/v1/auth/login", post(stub::login_handler))
        .route("/api/v1/auth/me", get(stub::me_handler))
        .route("/api/v1/auth/logout", post(stub::logout_handler))
        // Resource list stubs (return empty arrays so the UI renders empty states)
        .route("/api/v1/nodes", get(stub::list_nodes_stub))
        .route("/api/v1/vms", get(stub::list_vms_stub))
        .route("/api/v1/networks", get(stub::list_networks_stub))
        .route("/api/v1/storage-pools", get(stub::list_storage_pools_stub))
        .route("/api/v1/operations", get(stub::list_operations_stub))
        .route("/api/v1/events", get(stub::list_events_stub))
        .route("/api/v1/images", get(stub::list_images_stub))
        .route("/api/v1/vm-templates", get(stub::list_vm_templates_stub))
        .route("/api/v1/cloud-init-templates", get(stub::list_cloud_init_templates_stub))
        .route("/api/v1/backup-jobs", get(stub::list_backup_jobs_stub))
        .route("/api/v1/backup-history", get(stub::list_backup_history_stub))
        .route("/api/v1/quotas", get(stub::list_quotas_stub))
        .route("/api/v1/usage", get(stub::get_usage_stub))
        // Install stubs
        .route("/api/v1/install/status", get(stub::get_install_status_stub))
        .route("/api/v1/install/bootstrap", post(stub::bootstrap_install_stub))
        .route("/api/v1/install/repair", post(stub::repair_install_stub))
        .fallback(not_found_handler)
        .with_state(bff_state)
}
