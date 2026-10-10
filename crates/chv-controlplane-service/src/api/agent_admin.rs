//! Browser-authenticated (BFF) routes for the optional guest
//! monitoring agent (query/alerts contract v1's
//! `/v1/monitoring/agents` surface; ADR-026, campaign #602 prompt 03).
//!
//! These are the operator surfaces: agent inventory (viewer tier),
//! claim issuance and credential lifecycle actions (operator tier).
//! They live beside — never inside — the agent-authenticated ingest
//! routes (`agent_routes.rs`): browser sessions and agent credentials
//! are different authentication domains and must never share a
//! middleware.
//!
//! Honesty rules:
//! - The enrollment claim is shown exactly once at issuance and never
//!   persisted in plaintext or echoed back afterwards.
//! - Agent state uses the security contract's wire vocabulary
//!   (`unenrolled`, `enrolling`, `active`, `renewal_due`, `offline`,
//!   `expired`, `revoked`) plus one documented extension,
//!   `identity_conflict`, for the cloned-image signal (a flagged agent
//!   is blocked from reporting until an authorized reset — that is
//!   neither `active` nor honestly `revoked`).
//! - When guest ingestion is not configured, these routes answer
//!   typed `guest_ingestion_disabled` errors, not empty lists that
//!   would read as "no agents enrolled".

use crate::monitoring_agent::MonitoringAgentService;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use chv_controlplane_store::MonitoringAgentRow;
use chv_webui_bff::auth::BearerToken;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Cap on the inventory listing (paginated in v1 by `limit`).
const MAX_LIST_AGENTS: u32 = 200;

pub struct AgentAdminState {
    pub service: Arc<MonitoringAgentService>,
    /// Derived-state thresholds mirrored from the guest ingestion
    /// config (kept here so the wire state derivation is testable
    /// without a live service).
    pub offline_after_ms: i64,
    pub enrollment_grace_ms: i64,
    pub renewal_window_ms: i64,
}

/// Viewer-tier routes (read-only inventory and per-VM status).
pub fn agent_viewer_router(state: Arc<AgentAdminState>) -> Router<chv_webui_bff::router::AppState> {
    Router::new()
        .route("/v1/monitoring/agents", post(list_agents))
        .layer(Extension(state))
}

/// Operator-tier routes (claim issuance, revoke, force-rotate, reset).
pub fn agent_operator_router(
    state: Arc<AgentAdminState>,
) -> Router<chv_webui_bff::router::AppState> {
    Router::new()
        .route("/v1/monitoring/agents/claim", post(issue_claim))
        .route("/v1/monitoring/agents/revoke", post(revoke_agent))
        .route("/v1/monitoring/agents/rotate", post(force_rotation))
        .route("/v1/monitoring/agents/reset", post(reset_agent))
        .layer(Extension(state))
}

fn err(status: StatusCode, code: &str, message: impl std::fmt::Display) -> Response {
    (
        status,
        Json(json!({
            "error": { "code": code, "message": message.to_string() }
        })),
    )
        .into_response()
}

/// The security contract's wire-state vocabulary (plus the documented
/// `identity_conflict` extension for the cloned-image signal).
fn wire_state(row: &MonitoringAgentRow, st: &AgentAdminState, now_ms: i64) -> &'static str {
    if row.status == "revoked" {
        return "revoked";
    }
    if row.identity_conflict {
        return "identity_conflict";
    }
    if now_ms >= row.cert_not_after_ms {
        return "expired";
    }
    // renewal_due is a sub-state of active: valid credential, rotation
    // window open (operator-forced or the expiry window).
    let renewal_due =
        row.rotation_pending || (row.cert_not_after_ms - now_ms) <= st.renewal_window_ms;
    match row.last_seen_at_ms {
        Some(seen) if now_ms.saturating_sub(seen) <= st.offline_after_ms => {
            if renewal_due {
                "renewal_due"
            } else {
                "active"
            }
        }
        // Fresh enrollment: active within the grace window even before
        // the first accepted batch (security contract state table).
        None if now_ms.saturating_sub(row.enrolled_at_ms) <= st.enrollment_grace_ms => {
            if renewal_due {
                "renewal_due"
            } else {
                "active"
            }
        }
        _ => "offline",
    }
}

fn agent_json(row: &MonitoringAgentRow, st: &AgentAdminState, now_ms: i64) -> serde_json::Value {
    json!({
        "agent_id": row.agent_id,
        "vm_id": row.vm_id,
        "state": wire_state(row, st, now_ms),
        "install_id": row.install_id,
        "credential_epoch": row.credential_epoch,
        "credential_expires_at_ms": row.cert_not_after_ms,
        "rotation_pending": row.rotation_pending,
        "identity_conflict": row.identity_conflict,
        "conflict_reason": row.conflict_reason,
        "enrolled_at_ms": row.enrolled_at_ms,
        "last_seen_at_ms": row.last_seen_at_ms,
        "last_seen_age_seconds": row.last_seen_at_ms
            .map(|seen| ((now_ms - seen) / 1000).max(0)),
        "os": {
            "name": row.os_name,
            "version": row.os_version,
            "kernel_release": row.os_kernel_release,
        },
    })
}

#[derive(Deserialize)]
struct ListAgentsRequest {
    #[serde(default)]
    vm_id: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

async fn list_agents(
    Extension(st): Extension<Arc<AgentAdminState>>,
    _token: BearerToken,
    Json(req): Json<ListAgentsRequest>,
) -> Response {
    let limit = req.limit.unwrap_or(50).min(MAX_LIST_AGENTS);
    let now_ms = chrono::Utc::now().timestamp_millis();
    match st
        .service
        .repo()
        .list_agents(req.vm_id.as_deref(), limit)
        .await
    {
        Ok(rows) => (
            StatusCode::OK,
            Json(json!({
                "schema_version": 1,
                "agents": rows.iter().map(|r| agent_json(r, &st, now_ms)).collect::<Vec<_>>(),
                "generated_at_ms": now_ms,
                "truncated": rows.len() as u32 == limit,
            })),
        )
            .into_response(),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            format!("agent inventory query failed: {e}"),
        ),
    }
}

#[derive(Deserialize)]
struct ClaimRequest {
    vm_id: String,
}

async fn issue_claim(
    Extension(st): Extension<Arc<AgentAdminState>>,
    token: BearerToken,
    Json(req): Json<ClaimRequest>,
) -> Response {
    let now_ms = chrono::Utc::now().timestamp_millis();
    match st
        .service
        .issue_claim(&req.vm_id, &token.0.username, now_ms)
        .await
    {
        Ok(claim) => {
            // The plaintext claim is returned exactly once. The UI
            // renders it immediately; it is never persisted server-side
            // and cannot be re-displayed.
            (
                StatusCode::CREATED,
                Json(json!({
                    "schema_version": 1,
                    "vm_id": claim.vm_id,
                    "claim_token": claim.token,
                    "expires_at_ms": claim.expires_at_ms,
                    "server_url": st.service.public_base_url(),
                    "ca_fingerprint": st.service.issuer().ca_fingerprint(),
                })),
            )
                .into_response()
        }
        Err(crate::error::ControlPlaneServiceError::Store(
            chv_controlplane_store::StoreError::NotFound { .. },
        )) => err(
            StatusCode::NOT_FOUND,
            "vm_not_found",
            format!("vm {} does not exist", req.vm_id),
        ),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            format!("claim issuance failed: {e}"),
        ),
    }
}

#[derive(Deserialize)]
struct AgentActionRequest {
    agent_id: String,
}

async fn revoke_agent(
    Extension(st): Extension<Arc<AgentAdminState>>,
    token: BearerToken,
    Json(req): Json<AgentActionRequest>,
) -> Response {
    let now_ms = chrono::Utc::now().timestamp_millis();
    match st
        .service
        .repo()
        .revoke_agent(&req.agent_id, &token.0.username, now_ms)
        .await
    {
        Ok(row) => {
            st.service
                .audit(
                    now_ms,
                    Some(&row.vm_id),
                    &token.0.username,
                    "monitoring_agent.revoked",
                    format!("guest monitoring agent {} revoked", req.agent_id),
                )
                .await;
            (
                StatusCode::OK,
                Json(json!({
                    "schema_version": 1,
                    "agent_id": row.agent_id,
                    "state": "revoked",
                })),
            )
                .into_response()
        }
        Err(chv_controlplane_store::StoreError::NotFound { .. }) => err(
            StatusCode::NOT_FOUND,
            "agent_not_found",
            format!("agent {} does not exist or is not active", req.agent_id),
        ),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            format!("revoke failed: {e}"),
        ),
    }
}

async fn force_rotation(
    Extension(st): Extension<Arc<AgentAdminState>>,
    token: BearerToken,
    Json(req): Json<AgentActionRequest>,
) -> Response {
    let now_ms = chrono::Utc::now().timestamp_millis();
    match st.service.repo().set_rotation_pending(&req.agent_id).await {
        Ok(()) => {
            st.service
                .audit(
                    now_ms,
                    None,
                    &token.0.username,
                    "monitoring_agent.rotation_forced",
                    format!(
                        "credential rotation forced for guest monitoring agent {}",
                        req.agent_id
                    ),
                )
                .await;
            (
                StatusCode::OK,
                Json(json!({
                    "schema_version": 1,
                    "agent_id": req.agent_id,
                    "rotation_pending": true,
                })),
            )
                .into_response()
        }
        Err(chv_controlplane_store::StoreError::NotFound { .. }) => err(
            StatusCode::NOT_FOUND,
            "agent_not_found",
            format!("agent {} does not exist or is not active", req.agent_id),
        ),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            format!("forcing rotation failed: {e}"),
        ),
    }
}

async fn reset_agent(
    Extension(st): Extension<Arc<AgentAdminState>>,
    token: BearerToken,
    Json(req): Json<AgentActionRequest>,
) -> Response {
    // An explicit authorized reset: clear the cloned-image conflict
    // flag so the credential may report again. Only meaningful for a
    // flagged agent — resetting a healthy one is a no-op error, not a
    // silent success.
    let row = match st.service.repo().find_agent(&req.agent_id).await {
        Ok(Some(row)) => row,
        _ => {
            return err(
                StatusCode::NOT_FOUND,
                "agent_not_found",
                format!("agent {} does not exist", req.agent_id),
            )
        }
    };
    if !row.identity_conflict {
        return err(
            StatusCode::CONFLICT,
            "no_conflict",
            format!("agent {} has no identity conflict to reset", req.agent_id),
        );
    }
    match st.service.repo().clear_conflict(&req.agent_id).await {
        Ok(()) => {
            st.service
                .audit(
                    chrono::Utc::now().timestamp_millis(),
                    Some(&row.vm_id),
                    &token.0.username,
                    "monitoring_agent.conflict_cleared",
                    format!(
                        "identity conflict cleared for guest monitoring agent {}",
                        req.agent_id
                    ),
                )
                .await;
            (
                StatusCode::OK,
                Json(json!({
                    "schema_version": 1,
                    "agent_id": req.agent_id,
                    "identity_conflict": false,
                })),
            )
                .into_response()
        }
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            format!("reset failed: {e}"),
        ),
    }
}
