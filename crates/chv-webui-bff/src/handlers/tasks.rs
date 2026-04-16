use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_tasks(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let page = req
        .get("page")
        .and_then(|v| v.as_u64())
        .map(|p| p.max(1) as u32)
        .unwrap_or(1);

    let page_size = req
        .get("page_size")
        .and_then(|v| v.as_u64())
        .map(|p| if p == 0 { 50 } else { p as u32 })
        .unwrap_or(50);

    let filters = req.get("filters").cloned().unwrap_or(json!({}));

    let mut query_sql = String::from(
        r#"
        SELECT
            operation_id AS task_id,
            status::text AS status,
            operation_type AS operation,
            resource_kind::text AS resource_kind,
            resource_id,
            requested_by AS actor,
            EXTRACT(EPOCH FROM requested_at)::bigint * 1000 AS started_unix_ms,
            EXTRACT(EPOCH FROM completed_at)::bigint * 1000 AS finished_unix_ms
        FROM operations
        WHERE 1=1
        "#,
    );

    if let Some(status) = filters.get("status").and_then(|v| v.as_str()) {
        if status != "all" {
            query_sql.push_str(" AND status::text = ");
            query_sql.push('\'');
            query_sql.push_str(status);
            query_sql.push('\'');
        }
    }

    if let Some(resource_kind) = filters.get("resource_kind").and_then(|v| v.as_str()) {
        if resource_kind != "all" {
            query_sql.push_str(" AND resource_kind::text = ");
            query_sql.push('\'');
            query_sql.push_str(resource_kind);
            query_sql.push('\'');
        }
    }

    if let Some(window) = filters.get("window").and_then(|v| v.as_str()) {
        match window {
            "24h" => query_sql.push_str(" AND requested_at > now() - interval '24 hours'"),
            "7d" => query_sql.push_str(" AND requested_at > now() - interval '7 days'"),
            "30d" => query_sql.push_str(" AND requested_at > now() - interval '30 days'"),
            "active" => query_sql.push_str(
                " AND status IN ('Pending', 'Accepted', 'Running')",
            ),
            _ => {}
        }
    }

    query_sql.push_str(" ORDER BY requested_at DESC");

    let rows = sqlx::query_as::<_, TaskRow>(&query_sql)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to list tasks: {}", e)))?;

    let total = rows.len() as u64;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "task_id": r.task_id,
                "status": r.status,
                "operation": r.operation,
                "resource_kind": r.resource_kind,
                "resource_id": r.resource_id.unwrap_or_default(),
                "actor": r.actor.unwrap_or_default(),
                "started_unix_ms": r.started_unix_ms,
                "finished_unix_ms": r.finished_unix_ms,
            })
        })
        .collect();

    Ok(Json(json!({
        "items": items,
        "page": {
            "page": page,
            "page_size": page_size,
            "total_items": total,
        },
        "filters": {
            "applied": {}
        },
    })))
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    task_id: String,
    status: String,
    operation: String,
    resource_kind: String,
    resource_id: Option<String>,
    actor: Option<String>,
    started_unix_ms: Option<i64>,
    finished_unix_ms: Option<i64>,
}
