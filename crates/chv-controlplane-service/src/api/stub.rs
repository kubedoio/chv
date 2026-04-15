use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    Json as AxumJson,
};
use serde_json::Value;
use std::collections::HashMap;

pub async fn login_handler(AxumJson(payload): AxumJson<Value>) -> impl axum::response::IntoResponse {
    let username = payload
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "token": "mock-token-chv-all-in-one",
            "user": {
                "id": "00000000-0000-0000-0000-000000000001",
                "username": username,
                "role": "admin"
            }
        })),
    )
}

pub async fn me_handler() -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "username": "admin",
            "role": "admin"
        })),
    )
}

pub async fn logout_handler() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

pub async fn list_nodes_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"nodes": []})))
}

pub async fn list_vms_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"vms": []})))
}

pub async fn list_networks_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"networks": []})))
}

pub async fn list_storage_pools_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"storage_pools": []})))
}

pub async fn list_operations_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"operations": []})))
}

pub async fn list_events_stub(Query(_params): Query<HashMap<String, String>>) -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"events": []})))
}

pub async fn list_images_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"images": []})))
}

pub async fn list_vm_templates_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"vm_templates": []})))
}

pub async fn list_cloud_init_templates_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"cloud_init_templates": []})))
}

pub async fn list_backup_jobs_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"backup_jobs": []})))
}

pub async fn list_backup_history_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"backup_history": []})))
}

pub async fn list_quotas_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"quotas": []})))
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

pub async fn repair_install_stub(AxumJson(_payload): AxumJson<Value>) -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "Repair completed"
        })),
    )
}
