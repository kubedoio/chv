use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_backup_jobs(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
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

    let (rows, total_count) = state
        .backup_job_repo
        .list_jobs(page_size as i64, offset as i64)
        .await
        .map_err(|e| BffError::Internal(format!("failed to list backup jobs: {}", e)))?;

    let total_pages = (total_count as u64).div_ceil(page_size);

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "job_id": r.job_id,
                "vm_id": r.vm_id,
                "name": r.name,
                "schedule": r.schedule,
                "destination": r.destination,
                "retention_days": r.retention_days,
                "enabled": r.enabled,
                "last_run_at": r.last_run_at,
                "next_run_at": r.next_run_at,
                "created_at": r.created_at,
                "updated_at": r.updated_at,
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

pub async fn list_backup_history(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
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

    let job_id = payload.get("job_id").and_then(|v| v.as_str());

    let (rows, total_count) = if let Some(job_id) = job_id {
        state
            .backup_job_repo
            .list_history_for_job(job_id, page_size as i64, offset as i64)
            .await
            .map_err(|e| BffError::Internal(format!("failed to list backup history: {}", e)))?
    } else {
        state
            .backup_job_repo
            .list_recent_history(page_size as i64, offset as i64)
            .await
            .map_err(|e| BffError::Internal(format!("failed to list backup history: {}", e)))?
    };

    let total_pages = (total_count as u64).div_ceil(page_size);

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "history_id": r.history_id,
                "job_id": r.job_id,
                "vm_id": r.vm_id,
                "started_at": r.started_at,
                "completed_at": r.completed_at,
                "status": r.status,
                "size_bytes": r.size_bytes,
                "error_message": r.error_message,
                "created_at": r.created_at,
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
