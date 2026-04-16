use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chv_webui_bff::AppState;

pub async fn list_operations(State(state): State<AppState>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, OperationRow>(
        r#"SELECT operation_id, status::text as status FROM operations ORDER BY requested_at DESC LIMIT 100"#,
    )
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(rows) => (
            StatusCode::OK,
            Json(serde_json::json!({"operations": rows})),
        ),
        Err(e) => {
            tracing::error!("list_operations failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal server error"})),
            )
        }
    }
}

pub async fn get_operation(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, OperationRow>(
        r#"SELECT operation_id, status::text as status FROM operations WHERE operation_id = $1"#,
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(row)) => (StatusCode::OK, Json(serde_json::json!({"operation": row}))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        ),
        Err(e) => {
            tracing::error!("get_operation failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal server error"})),
            )
        }
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct OperationRow {
    operation_id: String,
    status: String,
}
