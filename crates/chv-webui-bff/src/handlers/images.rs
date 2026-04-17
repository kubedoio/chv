use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_images(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let page = payload.get("page").and_then(|v| v.as_u64()).unwrap_or(1).max(1);
    let page_size = payload.get("page_size").and_then(|v| v.as_u64()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * page_size;
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM images")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count images: {}", e)))?;
    let total_pages = (total_count as u64 + page_size - 1) / page_size;

    let rows = sqlx::query_as::<_, ImageRow>(
        r#"
        SELECT
            image_id,
            display_name AS name,
            CASE WHEN size_bytes IS NULL THEN ''
                 WHEN size_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(size_bytes AS REAL)/1073741824.0)
                 WHEN size_bytes >= 1048576 THEN printf('%.1f MiB', CAST(size_bytes AS REAL)/1048576.0)
                 WHEN size_bytes >= 1024 THEN printf('%.1f KiB', CAST(size_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', size_bytes) END AS size,
            status,
            COALESCE(os, '') AS os,
            COALESCE(version, '') AS version,
            usage_count,
            updated_at AS last_updated
        FROM images
        ORDER BY updated_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list images: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "image_id": r.image_id,
                "name": r.name,
                "size": r.size,
                "status": r.status,
                "os": r.os,
                "version": r.version,
                "usage_count": r.usage_count,
                "last_updated": r.last_updated,
            })
        })
        .collect();

    Ok(Json(json!({
        "items": items,
        "page": {
            "page": page,
            "page_size": page_size,
            "total_items": total_count,
            "total_pages": total_pages,
        },
        "filters": null,
    })))
}

#[derive(sqlx::FromRow)]
struct ImageRow {
    image_id: String,
    name: String,
    size: String,
    status: String,
    os: String,
    version: String,
    usage_count: i64,
    last_updated: String,
}
