use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_events(
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let events = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT
            event_id::text AS event_id,
            severity::text AS severity,
            event_type AS type,
            COALESCE(resource_kind::text, '') AS resource_kind,
            COALESCE(resource_id, '') AS resource_id,
            COALESCE(n.display_name, resource_id, '') AS resource_name,
            message AS summary,
            occurred_at::text AS occurred_at,
            'open' AS state
        FROM events e
        LEFT JOIN nodes n ON e.node_id = n.node_id
        ORDER BY occurred_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list events: {}", e)))?;

    let alerts = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT
            alert_id::text AS event_id,
            severity::text AS severity,
            alert_type AS type,
            COALESCE(resource_kind::text, '') AS resource_kind,
            COALESCE(resource_id, '') AS resource_id,
            COALESCE(n.display_name, resource_id, '') AS resource_name,
            message AS summary,
            opened_at::text AS occurred_at,
            CASE
                WHEN acknowledged_at IS NOT NULL THEN 'acknowledged'
                WHEN resolved_at IS NOT NULL THEN 'resolved'
                ELSE 'open'
            END AS state
        FROM alerts a
        LEFT JOIN nodes n ON a.node_id = n.node_id
        ORDER BY opened_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list alerts: {}", e)))?;

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

#[derive(sqlx::FromRow)]
struct AlertRow {
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
