//! Operator health endpoints (`chvctl health ...`).
//!
//! `chvctl health check | cluster | report <node_id>` issue GET requests
//! against `/v1/health`, `/v1/cluster/health`, and
//! `/v1/nodes/{node_id}/health`. These routes were previously unimplemented
//! (the CLI always failed with NOT_IMPLEMENTED/404 on a healthy deployment —
//! issue #320). They are read-only aggregates over the control-plane store:
//!
//! - `/v1/health` — liveness plus a database ping. Authentication is still
//!   required (viewer tier); it is not an anonymous probe endpoint — the
//!   BFF surface is authenticated by design and `chvctl` carries the token.
//! - `/v1/cluster/health` — fleet aggregate of `node_observed_state`.
//! - `/v1/nodes/{node_id}/health` — per-node observed health.
//!
//! Health-status vocabulary matches the rest of the BFF
//! (`healthy`, `degraded`, `warning`, `critical`, `unknown` for NULL/absent
//! rows — see `handlers::overview`).

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use crate::auth::BearerToken;
use crate::{AppState, BffError};

/// GET /v1/health — liveness + database reachability.
pub async fn health(
    BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    // Any successful round-trip to the store proves the pool answers;
    // `SELECT 1` avoids depending on table shape.
    let db_ok: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("health: database unreachable: {}", e)))?;

    Ok(Json(json!({
        "status": if db_ok == 1 { "ok" } else { "degraded" },
        "database": "reachable",
    })))
}

/// GET /v1/cluster/health — aggregate node health across the fleet.
///
/// Overall status: `critical` if any node reports critical, `degraded` if
/// any node reports degraded/warning, `unknown` if there are no nodes (or
/// none have reported yet), otherwise `healthy`.
pub async fn cluster_health(
    BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT
            CASE
                WHEN nos.health_status IS NULL OR nos.health_status = '' THEN 'unknown'
                ELSE LOWER(nos.health_status)
            END AS health,
            COUNT(*) AS count
        FROM nodes n
        LEFT JOIN node_observed_state nos ON n.node_id = nos.node_id
        GROUP BY health
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("cluster health: query failed: {}", e)))?;

    let mut counts = serde_json::Map::new();
    let mut total: i64 = 0;
    for (status, count) in rows {
        total += count;
        counts.insert(status, json!(count));
    }
    // Always-present keys so consumers can rely on the shape.
    for key in ["healthy", "degraded", "warning", "critical", "unknown"] {
        counts.entry(key.to_string()).or_insert(json!(0));
    }

    let overall = if total == 0 || counts["unknown"].as_i64() == Some(total) {
        "unknown"
    } else if counts["critical"].as_i64().unwrap_or(0) > 0 {
        "critical"
    } else if counts["degraded"].as_i64().unwrap_or(0) > 0
        || counts["warning"].as_i64().unwrap_or(0) > 0
    {
        "degraded"
    } else {
        "healthy"
    };

    let mut body = json!({
        "status": overall,
        "total_nodes": total,
    });
    if let Value::Object(map) = &mut body {
        for (k, v) in counts {
            map.insert(k, v);
        }
    }
    Ok(Json(body))
}

/// GET /v1/nodes/{node_id}/health — per-node observed health.
pub async fn node_health(
    BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    let row = sqlx::query_as::<_, (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )>(
        r#"
        SELECT
            n.node_id,
            n.hostname,
            COALESCE(nos.observed_state, 'Unknown'),
            COALESCE(nos.health_status, 'unknown'),
            nos.runtime_status,
            nos.state_reason,
            nos.entered_at,
            nos.observed_at
        FROM nodes n
        LEFT JOIN node_observed_state nos ON n.node_id = nos.node_id
        WHERE n.node_id = ?
        "#,
    )
    .bind(&node_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("node health: query failed: {}", e)))?;

    let row = row
        .ok_or_else(|| BffError::NotFound(format!("node {} not found", node_id)))?;

    let (
        node_id,
        hostname,
        state_,
        health_status,
        runtime_status,
        state_reason,
        entered_at,
        observed_at,
    ) = row;

    Ok(Json(json!({
        "node_id": node_id,
        "hostname": hostname,
        "state": state_,
        "health": health_status,
        "runtime_status": runtime_status,
        "state_reason": state_reason,
        "entered_at": entered_at,
        "observed_at": observed_at,
    })))
}
