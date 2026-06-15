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
    /// 400 — Phase 5 apply was called against a destructive plan (any
    /// `Delete`/`Replace` change, or destroy mode) without a typed-name
    /// confirmation that matches the topology name. Surfaced as
    /// `code: "MISSING_CONFIRMATION"` so the UI can pop the typed-name
    /// dialog and resubmit.
    MissingConfirmation {
        plan_id: String,
    },
    /// 400 — Phase 5 apply was called against a plan with warnings while
    /// `acknowledged_warnings=false`. Surfaced as
    /// `code: "WARNINGS_NOT_ACKNOWLEDGED"` so the UI knows to surface the
    /// warning list and re-submit with the flag set.
    WarningsNotAcknowledged {
        plan_id: String,
        warnings: usize,
    },
    /// 409 — Phase 5 apply was called against a plan whose status does not
    /// allow apply (anything other than `ready_to_apply`). Surfaced as
    /// `code: "PLAN_NOT_APPLICABLE"`.
    ///
    /// `reason` distinguishes the underlying cause so the UI can branch
    /// between "the plan moved on" (`plan_status_mismatch`), "the topology
    /// moved on" (`version_drift`), and "the plan has no actionable
    /// changes" (`empty_plan`). `current_status` always carries the actual
    /// plan status string for diagnostics, regardless of `reason`.
    PlanNotApplicable {
        plan_id: String,
        current_status: String,
        reason: Option<String>,
    },
    /// 400 — Phase 5 apply or destroy was called against a plan whose
    /// `mode` does not match the endpoint's contract (apply against a
    /// destroy plan, or vice versa). Surfaced as
    /// `code: "PLAN_MODE_MISMATCH"` so the UI can route to /destroy-plan
    /// (or /plan) without string-matching error messages.
    PlanModeMismatch {
        plan_id: String,
        expected: String,
        actual: String,
    },
    /// 400 — A change in the plan referenced a `resource_name` containing
    /// reserved separator characters (`::` or `/`). The reconcile crate
    /// rejects these to keep the operations idempotency_key unambiguous.
    /// Surfaced as `code: "INVALID_RESOURCE_NAME"`.
    InvalidResourceName {
        resource_name: String,
    },
    /// 403 — Phase 5 apply against an architecture in a `production` (or
    /// `prod`) environment requires the Admin role. Operators see this
    /// surface as `code: "PRODUCTION_REQUIRES_ADMIN"` so the UI can render
    /// a clear "ask an admin to apply this" path instead of a generic 403.
    ProductionRequiresAdmin {
        environment: String,
    },
    QuotaExceeded {
        resource: String,
        limit: i64,
        used: i64,
        requested: i64,
    },
    /// 502 — Phase 6 drift detection failed *and* the BFF could not even
    /// persist a `check_failed` drift report row (e.g. the store rejected
    /// the insert). The normal compute-failure path returns 200 with
    /// `status: check_failed` so the UI can render the failure inline; this
    /// variant fires only when persistence itself fails. Surfaced as
    /// `code: "DRIFT_CHECK_FAILED"` so callers distinguish "compute failed,
    /// here is the report" (200) from "we could not even record the
    /// failure" (502).
    DriftCheckFailed {
        architecture_id: String,
        message: String,
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
            BffError::MissingConfirmation { plan_id } => {
                let body = Json(json!({
                    "message": "apply confirmation missing or did not match topology name",
                    "code": "MISSING_CONFIRMATION",
                    "plan_id": plan_id,
                }));
                return (StatusCode::BAD_REQUEST, body).into_response();
            }
            BffError::WarningsNotAcknowledged { plan_id, warnings } => {
                let body = Json(json!({
                    "message": format!(
                        "plan {plan_id} has {warnings} warnings that must be explicitly acknowledged"
                    ),
                    "code": "WARNINGS_NOT_ACKNOWLEDGED",
                    "plan_id": plan_id,
                    "warnings": warnings,
                }));
                return (StatusCode::BAD_REQUEST, body).into_response();
            }
            BffError::PlanNotApplicable {
                plan_id,
                current_status,
                reason,
            } => {
                let body = Json(json!({
                    "message": format!(
                        "plan {plan_id} status {current_status} does not allow apply"
                    ),
                    "code": "PLAN_NOT_APPLICABLE",
                    "plan_id": plan_id,
                    "current_status": current_status,
                    "reason": reason,
                }));
                return (StatusCode::CONFLICT, body).into_response();
            }
            BffError::PlanModeMismatch {
                plan_id,
                expected,
                actual,
            } => {
                let body = Json(json!({
                    "message": format!(
                        "plan {plan_id} has mode {actual}; endpoint requires mode {expected}"
                    ),
                    "code": "PLAN_MODE_MISMATCH",
                    "plan_id": plan_id,
                    "expected": expected,
                    "actual": actual,
                }));
                return (StatusCode::BAD_REQUEST, body).into_response();
            }
            BffError::InvalidResourceName { resource_name } => {
                let body = Json(json!({
                    "message": format!(
                        "resource_name {resource_name:?} contains reserved separator characters (`::` or `/`)"
                    ),
                    "code": "INVALID_RESOURCE_NAME",
                    "resource_name": resource_name,
                }));
                return (StatusCode::BAD_REQUEST, body).into_response();
            }
            BffError::ProductionRequiresAdmin { environment } => {
                let body = Json(json!({
                    "message": format!(
                        "environment {environment} requires admin role"
                    ),
                    "code": "PRODUCTION_REQUIRES_ADMIN",
                    "environment": environment,
                }));
                return (StatusCode::FORBIDDEN, body).into_response();
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
            BffError::DriftCheckFailed {
                architecture_id,
                message,
            } => {
                let body = Json(json!({
                    "message": format!(
                        "drift check failed for architecture {architecture_id}: {message}"
                    ),
                    "code": "DRIFT_CHECK_FAILED",
                    "architecture_id": architecture_id,
                }));
                return (StatusCode::BAD_GATEWAY, body).into_response();
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

/// Translate the reconcile crate's `ApplyError` to the BFF's HTTP error
/// surface. The two-tier split (pre-condition violations vs. store/identifier
/// failures) is preserved: pre-condition variants map to dedicated 4xx codes
/// the UI gates on, store/identifier failures collapse onto 500 INTERNAL.
impl From<chv_architecture_reconcile::apply::ApplyError> for BffError {
    fn from(err: chv_architecture_reconcile::apply::ApplyError) -> Self {
        use chv_architecture_reconcile::apply::ApplyError as A;
        match err {
            A::MissingConfirmation { plan_id, .. } => BffError::MissingConfirmation { plan_id },
            A::MissingWarningAck { plan_id, warnings } => {
                BffError::WarningsNotAcknowledged { plan_id, warnings }
            }
            A::PlanNotApplicable {
                plan_id,
                current_status,
            } => BffError::PlanNotApplicable {
                plan_id,
                current_status,
                reason: Some("plan_status_mismatch".to_string()),
            },
            A::PlanExpired {
                plan_id,
                expires_at,
            } => BffError::PlanExpired {
                plan_id: plan_id.clone(),
                message: format!("plan {plan_id} expired at {expires_at}"),
            },
            A::InvalidResourceName { resource_name, .. } => {
                BffError::InvalidResourceName { resource_name }
            }
            A::Store(e) => BffError::from(e),
            A::Identifier(e) => BffError::Internal(format!("identifier error: {e}")),
        }
    }
}
