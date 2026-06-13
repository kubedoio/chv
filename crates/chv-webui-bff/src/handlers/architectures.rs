//! Architecture Designer BFF handlers — Phase 0.
//!
//! This module wires the BFF surface for the Architecture Designer feature
//! described in `docs/specs/architecture-designer/`. Phase 0 covers ONLY the
//! topology CRUD lifecycle (list, get, create, update, archive). All other
//! verbs in the feature surface (validate, check-fleet, generate-yaml, plan,
//! apply, destroy, drift, runs, versions) are exposed here as stubs that
//! return `BffError::NotImplemented("phase 0")` so the routing surface is
//! complete and the UI can light up endpoint-by-endpoint as later phases
//! deliver real behavior.
//!
//! # Path style
//!
//! NOTE: Path style follows the existing CHV BFF POST-only convention
//! (`/v1/architectures/<verb>`) rather than the REST verbs in
//! `docs/specs/architecture-designer/contracts/api-contract.md`. Every other
//! handler in this crate (vms, networks, volumes, snapshots, …) uses POST
//! with verbs in the path, so introducing REST verbs here for one feature
//! would split the routing convention. Documented as an intentional
//! deviation; the contract document is the design of record, the routing
//! surface adapts to the host service.
//!
//! # Roles (Phase 0)
//!
//! Phase 0 reuses the existing 3-level role hierarchy
//! (Viewer / Operator / Admin) declared in [`crate::auth::Role`]. The
//! implementation plan called for fine-grained `architecture:*` permission
//! strings, but the current RBAC has no permission-string namespace; bolting
//! one on is out of scope for the Phase 0 skeleton and is tracked
//! separately.
//!
// TODO(arch-designer/perm-fine-grained): see <issue-link-tbd> for migration
// to architecture:* permission strings.
//
//! # Optimistic concurrency
//!
//! `update_architecture` echoes `expected_version` to the topology repo,
//! which executes `WHERE version_number = expected_version` in the UPDATE
//! and returns [`StoreError::StaleVersion`] on mismatch. That maps to
//! HTTP 409 Conflict here so UIs can re-fetch and prompt the user. See
//! `docs/specs/component/architecture-designer-data-model.md` for the
//! invariant.

use axum::{extract::State, Json};
use chv_controlplane_store::{
    StoreError, TopologyCreateInput, TopologyListFilter, TopologyUpdateInput,
};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitectureStatus, ArchitectureTopology, ArchitectureVersionId,
    FleetCheckStatus, ValidationStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auth::{require_admin, require_operator_or_admin, BearerToken};
use crate::router::AppState;
use crate::BffError;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Lightweight summary returned by `list_architectures`. The API contract in
/// Phase 0 deliberately keeps the list payload close to the full topology so
/// the UI does not need a second round-trip for the dashboard view; later
/// phases may strip large fields (`design_graph_json`, `latest_yaml`) from
/// the list response if list payload size becomes a problem.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ArchitectureSummary {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub environment: Option<String>,
    pub status: ArchitectureStatus,
    pub owner_user_id: Option<String>,
    pub last_validation_status: Option<ValidationStatus>,
    pub last_fleet_check_status: Option<FleetCheckStatus>,
    pub version_number: i64,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

impl From<ArchitectureTopology> for ArchitectureSummary {
    fn from(t: ArchitectureTopology) -> Self {
        Self {
            id: t.id.into_inner(),
            name: t.name,
            display_name: t.display_name,
            description: t.description,
            environment: t.environment,
            status: t.status,
            owner_user_id: t.owner_user_id,
            last_validation_status: t.last_validation_status,
            last_fleet_check_status: t.last_fleet_check_status,
            version_number: t.version_number,
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
            archived_at: t.archived_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListArchitecturesRequest {
    /// When true, archived topologies are included in the result. Defaults
    /// to false because the dashboard view excludes archived items.
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListArchitecturesResponse {
    pub architectures: Vec<ArchitectureSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GetArchitectureRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GetArchitectureResponse {
    pub architecture: ArchitectureSummary,
    /// Full design graph JSON (kept on the get response so the editor can
    /// open without a second round-trip).
    pub design_graph_json: Option<String>,
    pub latest_yaml: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateArchitectureRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    /// Optional initial design graph JSON. Most clients create then update.
    #[serde(default)]
    pub design_graph_json: Option<String>,
    #[serde(default)]
    pub latest_yaml: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateArchitectureResponse {
    pub architecture: ArchitectureSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateArchitectureRequest {
    pub id: String,
    /// Version the client read; server fails with 409 Conflict if the
    /// current row has moved on. See module-level optimistic-concurrency
    /// note.
    pub expected_version: i64,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub design_graph_json: Option<String>,
    #[serde(default)]
    pub latest_yaml: Option<String>,
    #[serde(default)]
    pub latest_version_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateArchitectureResponse {
    pub architecture: ArchitectureSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ArchiveArchitectureRequest {
    pub id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ArchiveArchitectureResponse {}

// ---------------------------------------------------------------------------
// CRUD handlers
// ---------------------------------------------------------------------------

/// List topologies. Viewer-accessible. Default scope excludes archived.
pub async fn list_architectures(
    BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<ListArchitecturesRequest>,
) -> Result<Json<ListArchitecturesResponse>, BffError> {
    tracing::info!(
        include_archived = req.include_archived,
        "list_architectures"
    );
    let topologies = state
        .topology_repo
        .list(TopologyListFilter {
            include_archived: req.include_archived,
        })
        .await?;
    Ok(Json(ListArchitecturesResponse {
        architectures: topologies
            .into_iter()
            .map(ArchitectureSummary::from)
            .collect(),
    }))
}

/// Read a single topology including its design graph and latest yaml.
pub async fn get_architecture(
    BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<GetArchitectureRequest>,
) -> Result<Json<GetArchitectureResponse>, BffError> {
    let id = parse_id(&req.id)?;
    tracing::info!(architecture_id = %id, "get_architecture");
    let topo = state.topology_repo.get(&id).await?;
    let design_graph_json = topo.design_graph_json.clone();
    let latest_yaml = topo.latest_yaml.clone();
    Ok(Json(GetArchitectureResponse {
        architecture: ArchitectureSummary::from(topo),
        design_graph_json,
        latest_yaml,
    }))
}

/// Create a new draft topology. Operator+ only.
pub async fn create_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<CreateArchitectureRequest>,
) -> Result<Json<CreateArchitectureResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    if req.name.trim().is_empty() {
        return Err(BffError::BadRequest("name must not be blank".into()));
    }
    let id = ArchitectureId::new(chv_common::gen_short_id())
        .map_err(|e| BffError::Internal(format!("failed to mint architecture id: {e}")))?;
    tracing::info!(architecture_id = %id, name = %req.name, "create_architecture");
    let topo = state
        .topology_repo
        .create(TopologyCreateInput {
            id,
            name: req.name,
            display_name: req.display_name,
            description: req.description,
            environment: req.environment,
            status: ArchitectureStatus::Draft,
            owner_user_id: Some(claims.sub.clone()),
            design_graph_json: req.design_graph_json,
            latest_yaml: req.latest_yaml,
        })
        .await?;
    Ok(Json(CreateArchitectureResponse {
        architecture: ArchitectureSummary::from(topo),
    }))
}

/// Update a topology with optimistic-concurrency check.
pub async fn update_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<UpdateArchitectureRequest>,
) -> Result<Json<UpdateArchitectureResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    let latest_version_id = match req.latest_version_id {
        None => None,
        Some(v) => Some(
            ArchitectureVersionId::new(v)
                .map_err(|e| BffError::BadRequest(format!("invalid latest_version_id: {e}")))?,
        ),
    };
    tracing::info!(
        architecture_id = %id,
        expected_version = req.expected_version,
        "update_architecture"
    );
    let result = state
        .topology_repo
        .update(TopologyUpdateInput {
            id: id.clone(),
            expected_version: req.expected_version,
            display_name: req.display_name,
            description: req.description,
            environment: req.environment,
            // Status transitions are owned by validate/plan/apply phases —
            // Phase 0 CRUD never moves a topology out of Draft.
            status: None,
            design_graph_json: req.design_graph_json,
            latest_yaml: req.latest_yaml,
            latest_version_id,
            last_validation_status: None,
            last_fleet_check_status: None,
        })
        .await;
    match result {
        Ok(topo) => Ok(Json(UpdateArchitectureResponse {
            architecture: ArchitectureSummary::from(topo),
        })),
        // Surface a stable, human-readable conflict message — the
        // From<StoreError> impl on BffError already does this, but we
        // explicitly build the message here to match the wording the
        // dispatch contract calls out.
        Err(StoreError::StaleVersion {
            current, expected, ..
        }) => Err(BffError::Conflict(format!(
            "stale version: client sent {expected}, current is {current}"
        ))),
        Err(other) => Err(other.into()),
    }
}

/// Soft-delete a topology. Operator+ only. Idempotent at the routing layer:
/// archiving an already-archived row returns 404, so callers know whether
/// they were the one that archived it.
pub async fn archive_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<ArchiveArchitectureRequest>,
) -> Result<Json<ArchiveArchitectureResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    tracing::info!(architecture_id = %id, "archive_architecture");
    state.topology_repo.archive(&id).await?;
    Ok(Json(ArchiveArchitectureResponse::default()))
}

// ---------------------------------------------------------------------------
// Stub handlers (Phase 0)
// ---------------------------------------------------------------------------
//
// Each stub validates the role gate that the real handler will use so the
// behavior is identical from the routing perspective once the real
// implementation lands. Handlers accept and ignore `Json<Value>` so the
// stubs are tolerant of any body shape — clients can probe the surface
// without crafting endpoint-specific payloads.

pub async fn validate_architecture(
    BearerToken(_claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn check_fleet_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_operator_or_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn generate_architecture_yaml(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_operator_or_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn plan_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_operator_or_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn destroy_plan_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_operator_or_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn discard_plan_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_operator_or_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn apply_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn destroy_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn get_architecture_drift(
    BearerToken(_claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn list_architecture_runs(
    BearerToken(_claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented("phase 0".into()))
}

pub async fn list_architecture_versions(
    BearerToken(_claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    Err(BffError::NotImplemented("phase 0".into()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_id(id: &str) -> Result<ArchitectureId, BffError> {
    ArchitectureId::new(id)
        .map_err(|e| BffError::BadRequest(format!("invalid architecture id: {e}")))
}
