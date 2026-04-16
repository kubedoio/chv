use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chv_webui_bff::AppState;

pub async fn list_nodes(State(state): State<AppState>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, NodeRow>(
        r#"SELECT node_id, hostname, display_name FROM nodes ORDER BY node_id"#,
    )
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(rows) => (StatusCode::OK, Json(serde_json::json!({"nodes": rows}))),
        Err(e) => {
            tracing::error!("list_nodes failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal server error"})),
            )
        }
    }
}

pub async fn get_node(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, NodeRow>(
        r#"SELECT node_id, hostname, display_name FROM nodes WHERE node_id = $1"#,
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(row)) => (StatusCode::OK, Json(serde_json::json!({"node": row}))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        ),
        Err(e) => {
            tracing::error!("get_node failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal server error"})),
            )
        }
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct NodeRow {
    node_id: String,
    hostname: String,
    display_name: String,
}
