use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_images(
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
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
        "#,
    )
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
            "page": 1,
            "page_size": 50,
            "total_items": items.len() as u64,
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
