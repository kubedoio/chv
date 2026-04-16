use axum::{extract::State, response::Json};
use serde_json::Value;

use crate::router::AppState;
use crate::BffError;

pub async fn list_vms(State(_state): State<AppState>) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented)
}

pub async fn get_vm(State(_state): State<AppState>) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented)
}

pub async fn mutate_vm(State(_state): State<AppState>) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented)
}
