use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;

#[derive(Debug)]
pub enum BffError {
    Internal(String),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    TooManyRequests(String),
    /// Endpoint exists in the routing surface but is not yet wired to a real
    /// implementation. Mapped to HTTP 501 with `code: "NOT_IMPLEMENTED"` so
    /// callers get a deterministic signal during phased rollouts. The string
    /// payload describes the phase / reason (e.g., `"phase 0"`).
    NotImplemented(String),
    /// 422 — Phase 1 generate-yaml was called but the topology has no
    /// `latest_yaml` and no graph→YAML mapper has shipped yet (Phase 2).
    /// Surfaced as a stable code so the UI can render a deterministic
    /// "design something on the canvas first" state.
    GraphEmpty,
    /// 409 — Phase 4 plan TTL has passed; an `apply` or `confirm` call
    /// against the plan must be rejected with `code: "PLAN_EXPIRED"` so the
    /// UI re-runs `plan` rather than silently widening the apply window.
    PlanExpired {
        plan_id: String,
        message: String,
    },
    /// 409 — discard-plan was called against a plan in a terminal state
    /// (`Applying`, `Applied`, `Failed`, `Expired`). The Phase-4 contract
    /// only allows discard from non-terminal states (`Draft`,
    /// `FailedValidation`, `RequiresConfirmation`, `ReadyToApply`). Surfaced
    /// as `code: "PLAN_NOT_DISCARDABLE"` so the UI can render a clear
    /// "this plan can no longer be discarded" message.
    PlanNotDiscardable {
        plan_id: String,
        current_status: chv_controlplane_types::architecture::PlanStatus,
    },
    QuotaExceeded {
        resource: String,
        limit: i64,
        used: i64,
        requested: i64,
    },
}

impl IntoResponse for BffError {
    fn into_response(self) -> axum::response::Response {
        let (status, message, code) = match &self {
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
            BffError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone(), "FORBIDDEN"),
            BffError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone(), "CONFLICT"),
            BffError::TooManyRequests(msg) => {
                (StatusCode::TOO_MANY_REQUESTS, msg.clone(), "RATE_LIMITED")
            }
            BffError::NotImplemented(msg) => {
                (StatusCode::NOT_IMPLEMENTED, msg.clone(), "NOT_IMPLEMENTED")
            }
            BffError::GraphEmpty => {
                let body = Json(json!({
                    "message": "topology graph is empty; YAML generation requires a non-empty graph (Phase 2 deliverable)",
                    "code": "GRAPH_EMPTY",
                }));
                return (StatusCode::UNPROCESSABLE_ENTITY, body).into_response();
            }
            BffError::PlanExpired { plan_id, message } => {
                let body = Json(json!({
                    "message": message,
                    "code": "PLAN_EXPIRED",
                    "plan_id": plan_id,
                }));
                return (StatusCode::CONFLICT, body).into_response();
            }
            BffError::PlanNotDiscardable {
                plan_id,
                current_status,
            } => {
                let body = Json(json!({
                    "message": format!(
                        "plan {plan_id} cannot be discarded from status {}",
                        current_status.as_str()
                    ),
                    "code": "PLAN_NOT_DISCARDABLE",
                    "plan_id": plan_id,
                    "current_status": current_status.as_str(),
                }));
                return (StatusCode::CONFLICT, body).into_response();
            }
            BffError::QuotaExceeded {
                resource,
                limit,
                used,
                requested,
            } => {
                let body = Json(json!({
                    "message": format!("{} quota exceeded", resource),
                    "code": "QUOTA_EXCEEDED",
                    "resource": resource,
                    "limit": limit,
                    "used": used,
                    "requested": requested,
                }));
                return (StatusCode::UNPROCESSABLE_ENTITY, body).into_response();
            }
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
            chv_controlplane_store::StoreError::StaleGeneration { entity, id, .. } => {
                BffError::Conflict(format!(
                    "{} {} was modified by another request (stale generation)",
                    entity, id
                ))
            }
            chv_controlplane_store::StoreError::StaleVersion {
                entity,
                id,
                current,
                expected,
            } => BffError::Conflict(format!(
                "{} {} stale version: client sent {expected}, current is {current}",
                entity, id
            )),
            chv_controlplane_store::StoreError::Conflict { entity, id, reason } => {
                BffError::Conflict(format!("{} '{}': {}", entity, id, reason))
            }
            chv_controlplane_store::StoreError::NotImplemented { reason } => {
                BffError::NotImplemented(reason.to_string())
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
