use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chv_controlplane_store::StorePool;
use std::sync::Arc;

pub async fn list_nodes(State(pool): State<Arc<StorePool>>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, NodeRow>(
        r#"SELECT node_id, hostname, display_name FROM nodes ORDER BY node_id"#,
    )
    .fetch_all(pool.as_ref())
    .await;

    match rows {
        Ok(rows) => (StatusCode::OK, Json(serde_json::json!({"nodes": rows}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

pub async fn get_node(
    Path(id): Path<String>,
    State(pool): State<Arc<StorePool>>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, NodeRow>(
        r#"SELECT node_id, hostname, display_name FROM nodes WHERE node_id = $1"#,
    )
    .bind(&id)
    .fetch_optional(pool.as_ref())
    .await;

    match row {
        Ok(Some(row)) => (StatusCode::OK, Json(serde_json::json!({"node": row}))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct NodeRow {
    node_id: String,
    hostname: String,
    display_name: String,
}
