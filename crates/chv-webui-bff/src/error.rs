use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;

#[derive(Debug)]
pub enum BffError {
    NotImplemented,
    Internal(String),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Conflict(String),
}

impl IntoResponse for BffError {
    fn into_response(self) -> axum::response::Response {
        let (status, message, code) = match &self {
            BffError::NotImplemented => (
                StatusCode::NOT_IMPLEMENTED,
                "Not implemented".to_string(),
                "NOT_IMPLEMENTED",
            ),
            BffError::Internal(msg) => {
                tracing::error!(error = %msg, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    "INTERNAL_ERROR",
                )
            }
            BffError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone(), "NOT_FOUND"),
            BffError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone(), "BAD_REQUEST"),
            BffError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone(), "UNAUTHORIZED"),
            BffError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone(), "CONFLICT"),
        };

        let body = Json(json!({
            "message": message,
            "code": code,
        }));

        (status, body).into_response()
    }
}

impl From<chv_controlplane_store::StoreError> for BffError {
    fn from(err: chv_controlplane_store::StoreError) -> Self {
        match err {
            chv_controlplane_store::StoreError::NotFound { entity, id } => {
                BffError::NotFound(format!("{} {} not found", entity, id))
            }
            _ => BffError::Internal(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for BffError {
    fn from(err: serde_json::Error) -> Self {
        BffError::Internal(err.to_string())
    }
}
