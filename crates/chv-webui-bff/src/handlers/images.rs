use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_images(
    State(_state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    // There is no dedicated images table yet; return empty schema-correct list.
    Ok(Json(json!({
        "items": [],
        "page": {
            "page": 1,
            "page_size": 50,
            "total_items": 0,
        },
        "filters": null,
    })))
}
