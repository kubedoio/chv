use axum::{extract::Query, http::StatusCode, response::Json, Json as AxumJson};
use serde_json::Value;
use std::collections::HashMap;

pub async fn list_nodes_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_vms_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_networks_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_operations_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_events_stub(
    Query(_params): Query<HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_images_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_vm_templates_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_cloud_init_templates_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_quotas_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn get_usage_stub() -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "usage": {
                "vms": 0,
                "cpu_cores": 0,
                "memory_mb": 0,
                "disk_gb": 0
            },
            "quota": null
        })),
    )
}

pub async fn get_install_status_stub() -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ready",
            "initialized": true,
            "message": "CHV is installed and ready"
        })),
    )
}

pub async fn bootstrap_install_stub() -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "Bootstrap completed"
        })),
    )
}

pub async fn repair_install_stub(
    AxumJson(_payload): AxumJson<Value>,
) -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "Repair completed"
        })),
    )
}
