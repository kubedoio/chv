use axum::{extract::State, response::Json};
use serde_json::Value;

use crate::router::AppState;
use crate::BffError;

pub async fn list_tasks(State(_state): State<AppState>) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented)
}
