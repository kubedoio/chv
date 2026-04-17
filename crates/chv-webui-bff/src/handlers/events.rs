use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_events(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let page = payload.get("page").and_then(|v| v.as_u64()).unwrap_or(1).max(1);
    let page_size = payload.get("page_size").and_then(|v| v.as_u64()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * page_size;

    let total_count: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM events) + (SELECT COUNT(*) FROM alerts)",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to count events: {}", e)))?;

    let total_pages = (total_count as u64 + page_size - 1) / page_size;

    let items_raw = sqlx::query_as::<_, UnifiedEventRow>(
        r#"
        SELECT event_id, severity, type, resource_kind, resource_id, resource_name, summary, occurred_at, state
        FROM (
            SELECT
                event_id,
                severity,
                event_type AS type,
                COALESCE(resource_kind, '') AS resource_kind,
                COALESCE(resource_id, '') AS resource_id,
                COALESCE(n.display_name, e.resource_id, '') AS resource_name,
                message AS summary,
                occurred_at,
                'open' AS state
            FROM events e
            LEFT JOIN nodes n ON e.node_id = n.node_id
            UNION ALL
            SELECT
                alert_id AS event_id,
                severity,
                alert_type AS type,
                COALESCE(resource_kind, '') AS resource_kind,
                COALESCE(resource_id, '') AS resource_id,
                COALESCE(n.display_name, a.resource_id, '') AS resource_name,
                message AS summary,
                opened_at AS occurred_at,
                CASE
                    WHEN acknowledged_at IS NOT NULL THEN 'acknowledged'
                    WHEN resolved_at IS NOT NULL THEN 'resolved'
                    ELSE 'open'
                END AS state
            FROM alerts a
            LEFT JOIN nodes n ON a.node_id = n.node_id
        ) combined
        ORDER BY occurred_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list events: {}", e)))?;

    let items: Vec<Value> = items_raw
        .into_iter()
        .map(|r| {
            json!({
                "event_id": r.event_id,
                "severity": r.severity.to_lowercase(),
                "type": r.r#type,
                "resource_kind": r.resource_kind,
                "resource_id": r.resource_id,
                "resource_name": r.resource_name,
                "summary": r.summary,
                "state": r.state,
                "occurred_at": r.occurred_at,
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

pub async fn list_events_for_vm(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?;

    let events = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT
            event_id,
            severity,
            event_type AS type,
            COALESCE(resource_kind, '') AS resource_kind,
            COALESCE(resource_id, '') AS resource_id,
            COALESCE(v.display_name, resource_id, '') AS resource_name,
            message AS summary,
            occurred_at,
            'open' AS state
        FROM events e
        LEFT JOIN vms v ON e.resource_id = v.vm_id
        WHERE e.resource_kind = 'vm' AND e.resource_id = $1
        ORDER BY occurred_at DESC
        LIMIT 50
        "#,
    )
    .bind(vm_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list vm events: {}", e)))?;

    let alerts = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT
            alert_id AS event_id,
            severity,
            alert_type AS type,
            COALESCE(resource_kind, '') AS resource_kind,
            COALESCE(resource_id, '') AS resource_id,
            COALESCE(v.display_name, resource_id, '') AS resource_name,
            message AS summary,
            opened_at AS occurred_at,
            CASE
                WHEN acknowledged_at IS NOT NULL THEN 'acknowledged'
                WHEN resolved_at IS NOT NULL THEN 'resolved'
                ELSE 'open'
            END AS state
        FROM alerts a
        LEFT JOIN vms v ON a.resource_id = v.vm_id
        WHERE a.resource_kind = 'vm' AND a.resource_id = $1
        ORDER BY opened_at DESC
        LIMIT 50
        "#,
    )
    .bind(vm_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list vm alerts: {}", e)))?;

    let mut items: Vec<Value> = Vec::new();
    for r in events {
        items.push(json!({
            "event_id": r.event_id,
            "severity": r.severity.to_lowercase(),
            "type": r.r#type,
            "resource_kind": r.resource_kind,
            "resource_id": r.resource_id,
            "resource_name": r.resource_name,
            "summary": r.summary,
            "state": r.state,
            "occurred_at": r.occurred_at,
        }));
    }
    for r in alerts {
        items.push(json!({
            "event_id": r.event_id,
            "severity": r.severity.to_lowercase(),
            "type": r.r#type,
            "resource_kind": r.resource_kind,
            "resource_id": r.resource_id,
            "resource_name": r.resource_name,
            "summary": r.summary,
            "state": r.state,
            "occurred_at": r.occurred_at,
        }));
    }

    items.sort_by(|a, b| {
        let a_ts = a.get("occurred_at").and_then(|v| v.as_str()).unwrap_or("");
        let b_ts = b.get("occurred_at").and_then(|v| v.as_str()).unwrap_or("");
        b_ts.cmp(a_ts)
    });

    let total = items.len() as u64;
    Ok(Json(json!({
        "items": items,
        "page": {
            "page": 1,
            "page_size": 50,
            "total_items": total,
        },
        "filters": null,
    })))
}

#[derive(sqlx::FromRow)]
struct EventRow {
    event_id: String,
    severity: String,
    r#type: String,
    resource_kind: String,
    resource_id: String,
    resource_name: String,
    summary: String,
    occurred_at: String,
    state: String,
}

// AlertRow has the same shape as EventRow; kept separate for query clarity.
type AlertRow = EventRow;

// Unified row type for the UNION ALL query in list_events.
type UnifiedEventRow = EventRow;
