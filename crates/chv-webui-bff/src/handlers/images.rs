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
            image_type,
            format,
            CASE WHEN size_bytes IS NULL THEN ''
                 WHEN size_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(size_bytes AS REAL)/1073741824.0)
                 WHEN size_bytes >= 1048576 THEN printf('%.1f MiB', CAST(size_bytes AS REAL)/1048576.0)
                 WHEN size_bytes >= 1024 THEN printf('%.1f KiB', CAST(size_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', size_bytes) END AS size,
            status,
            node_id,
            created_at
        FROM images
        ORDER BY created_at DESC
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
                "image_type": r.image_type,
                "format": r.format,
                "size": r.size,
                "status": r.status,
                "node_id": r.node_id,
                "created_at": r.created_at,
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
    image_type: String,
    format: String,
    size: String,
    status: String,
    node_id: Option<String>,
    created_at: String,
}
