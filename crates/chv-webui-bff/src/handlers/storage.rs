use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

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

pub async fn list_storage_pools(
    State(state): State<AppState>,
    axum::Json(_payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let rows = sqlx::query_as::<_, StoragePoolRow>(
        r#"
        SELECT pool_id, node_id, name, backend_class, path,
               total_bytes, used_bytes, status, created_at
        FROM storage_pools
        ORDER BY created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list storage pools: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            let allocatable = r.total_bytes - r.used_bytes;
            json!({
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

    Ok(Json(json!(items)))
}

pub async fn create_storage_pool(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?
        .to_string();

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

    sqlx::query(
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
    .await
    .map_err(|e| BffError::Internal(format!("failed to create storage pool: {}", e)))?;

    Ok(Json(json!({
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
    })))
}
