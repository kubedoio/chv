use axum::{extract::State, response::Json};
use serde_json::Value;

use crate::router::AppState;
use crate::BffError;

pub async fn get_overview(State(_state): State<AppState>) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented)
}
