use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    Json as AxumJson,
};
use chv_webui_bff::auth::Claims;
use chv_webui_bff::AppState;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn login_handler(
    State(state): State<AppState>,
    AxumJson(payload): AxumJson<Value>,
) -> impl axum::response::IntoResponse {
    let username = payload
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");

    let password = payload
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if username != "admin" || password != "admin" {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid credentials"})),
        )
            .into_response();
    }

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 60 * 60; // 7 days

    let claims = Claims {
        sub: username.to_string(),
        username: username.to_string(),
        role: "admin".to_string(),
        exp,
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let token = match jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to encode jwt token");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to generate token"})),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "token": token,
            "user": {
                "id": "00000000-0000-0000-0000-000000000001",
                "username": username,
                "role": "admin"
            }
        })),
    )
        .into_response()
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
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_vms_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_networks_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_storage_pools_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_operations_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_events_stub(Query(_params): Query<HashMap<String, String>>) -> impl axum::response::IntoResponse {
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

pub async fn list_backup_jobs_stub() -> impl axum::response::IntoResponse {
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn list_backup_history_stub() -> impl axum::response::IntoResponse {
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

pub async fn repair_install_stub(AxumJson(_payload): AxumJson<Value>) -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "Repair completed"
        })),
    )
}
