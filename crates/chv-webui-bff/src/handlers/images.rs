use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::router::AppState;
use crate::BffError;

pub async fn list_images(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let page = payload
        .get("page")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .max(1);
    let page_size = payload
        .get("page_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .clamp(1, 200);
    let offset = (page - 1) * page_size;
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM images")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count images: {}", e)))?;
    let total_pages = (total_count as u64).div_ceil(page_size);

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
        "filters": {
            "applied": {}
        },
    })))
}

pub async fn import_image(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?;

    let source_url = payload
        .get("source_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let format = payload
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("qcow2");

    let os = payload
        .get("os")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let architecture = payload
        .get("architecture")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let checksum = payload
        .get("checksum")
        .and_then(|v| v.as_str());

    // Check for duplicate by source_url
    if !source_url.is_empty() {
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM images WHERE source_url = ?"
        )
        .bind(source_url)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to check existing image: {}", e)))?;

        if existing > 0 {
            return Err(BffError::BadRequest(
                "An image with this source URL already exists".into(),
            ));
        }
    }

    let image_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"INSERT INTO images
           (image_id, display_name, image_type, format, size_bytes, checksum, source_url, os, version, status, node_id, created_at, updated_at)
           VALUES (?, ?, 'disk', ?, NULL, ?, ?, ?, ?, 'available', NULL, datetime('now'), datetime('now'))"#,
    )
    .bind(&image_id)
    .bind(name)
    .bind(format)
    .bind(checksum)
    .bind(source_url)
    .bind(os)
    .bind(architecture)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert image: {}", e)))?;

    Ok(Json(json!({
        "image_id": image_id,
        "name": name,
        "source_url": source_url,
        "format": format,
        "status": "available",
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
