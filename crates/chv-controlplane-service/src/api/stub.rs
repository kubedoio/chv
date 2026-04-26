use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
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
) -> Result<axum::response::Json<serde_json::Value>, chv_webui_bff::BffError> {
    let username = match payload.get("username").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => {
            return Err(chv_webui_bff::BffError::BadRequest(
                "missing username".into(),
            ));
        }
    };

    let password = match payload.get("password").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            return Err(chv_webui_bff::BffError::BadRequest(
                "missing password".into(),
            ));
        }
    };

    // Look up user by username
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, password_hash, role FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            return Err(chv_webui_bff::BffError::Unauthorized(
                "Invalid credentials".into(),
            ));
        }
        Err(e) => {
            tracing::error!(error = %e, "db error during login");
            return Err(chv_webui_bff::BffError::Internal("Internal error".into()));
        }
    };

    let password_ok = match bcrypt::verify(password, &row.password_hash) {
        Ok(ok) => ok,
        Err(e) => {
            tracing::error!(error = %e, "bcrypt verify error");
            return Err(chv_webui_bff::BffError::Internal("Internal error".into()));
        }
    };
    if !password_ok {
        return Err(chv_webui_bff::BffError::Unauthorized(
            "Invalid credentials".into(),
        ));
    }

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 7 * 24 * 60 * 60; // 7 days

    let claims = Claims {
        sub: row.user_id.clone(),
        username: username.to_string(),
        role: row.role.clone(),
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
            return Err(chv_webui_bff::BffError::Internal(
                "failed to generate token".into(),
            ));
        }
    };

    Ok(axum::response::Json(serde_json::json!({
        "token": token,
        "user": {
            "id": row.user_id,
            "username": username,
            "role": row.role,
        }
    })))
}

pub async fn me_handler(
    chv_webui_bff::auth::BearerToken(claims): chv_webui_bff::auth::BearerToken,
) -> impl axum::response::IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": claims.sub,
            "username": claims.username,
            "role": claims.role
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

pub async fn list_storage_pools_stub(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let rows = sqlx::query_as::<_, StoragePoolRow>(
        r#"
        SELECT pool_id, node_id, name, backend_class, path,
               total_bytes, used_bytes, status, created_at
        FROM storage_pools
        ORDER BY created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(rows) => {
            let items: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|r| {
                    let allocatable = r.total_bytes - r.used_bytes;
                    serde_json::json!({
                        "id": r.pool_id,
                        "pool_id": r.pool_id,
                        "node_id": r.node_id,
                        "name": r.name,
                        "pool_type": r.backend_class,
                        "path": r.path,
                        "capacity_bytes": r.total_bytes,
                        "allocatable_bytes": allocatable,
                        "is_default": false,
                        "status": r.status,
                        "created_at": r.created_at,
                    })
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(items)))
        }
        Err(e) => {
            tracing::error!(error = %e, "list_storage_pools db query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!([])),
            )
        }
    }
}

pub async fn create_storage_pool_stub(
    chv_webui_bff::auth::BearerToken(_claims): chv_webui_bff::auth::BearerToken,
    State(state): State<AppState>,
    AxumJson(payload): AxumJson<Value>,
) -> impl axum::response::IntoResponse {
    let name = match payload.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    serde_json::json!({"error": {"code": "BAD_REQUEST", "message": "missing name", "retryable": false}}),
                ),
            )
        }
    };

    let node_id = payload
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let backend_class = payload
        .get("pool_type")
        .or_else(|| payload.get("backend_class"))
        .and_then(|v| v.as_str())
        .unwrap_or("localdisk")
        .to_string();
    let path = payload
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let total_bytes = payload
        .get("capacity_bytes")
        .or_else(|| payload.get("total_bytes"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let pool_id = chv_common::gen_short_id();

    let result = sqlx::query(
        r#"
        INSERT INTO storage_pools (pool_id, node_id, name, backend_class, path, total_bytes, used_bytes, status)
        VALUES (?, ?, ?, ?, ?, ?, 0, 'available')
        "#,
    )
    .bind(&pool_id)
    .bind(&node_id)
    .bind(&name)
    .bind(&backend_class)
    .bind(&path)
    .bind(total_bytes)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": pool_id,
                "pool_id": pool_id,
                "node_id": node_id,
                "name": name,
                "pool_type": backend_class,
                "path": path,
                "capacity_bytes": total_bytes,
                "allocatable_bytes": total_bytes,
                "is_default": false,
                "status": "available",
            })),
        ),
        Err(e) => {
            tracing::error!(error = %e, "create_storage_pool db insert failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({"error": {"code": "INTERNAL", "message": "failed to create storage pool", "retryable": false}}),
                ),
            )
        }
    }
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

#[derive(sqlx::FromRow)]
struct UserRow {
    user_id: String,
    password_hash: String,
    role: String,
}

#[derive(sqlx::FromRow)]
struct StoragePoolRow {
    pool_id: String,
    node_id: Option<String>,
    name: String,
    backend_class: String,
    path: String,
    total_bytes: i64,
    used_bytes: i64,
    status: String,
    created_at: String,
}
