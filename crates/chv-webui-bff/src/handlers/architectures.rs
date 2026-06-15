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
use chv_architecture_reconcile::FleetInventoryProvider;
use chv_architecture_validate::{
    fleet::check_fleet, parse_yaml as parse_arch_yaml, validate as validate_yaml_str,
    ValidationResult,
};
use chv_controlplane_store::{
    InventorySnapshotCreateInput, StoreError, TopologyCreateInput, TopologyListFilter,
    TopologyUpdateInput,
};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitectureStatus, ArchitectureTopology, ArchitectureVersionId, Finding,
    FleetCheckStatus, InventorySnapshotId, Severity, ValidationStatus,
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
    /// Version the client read; mirrors `update`'s optimistic-concurrency
    /// contract so concurrent edits cannot be silently overwritten by an
    /// archive. Stale-version archives surface as 409 Conflict.
    pub expected_version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ArchiveArchitectureResponse {
    pub architecture: ArchitectureSummary,
}

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

    // Skip the UPDATE entirely when the caller sent no field changes — this
    // keeps version_number stable for clients that re-submit a PATCH-style
    // form without modifying anything. The optimistic-concurrency check on
    // expected_version is preserved by passing it to `update()` only when
    // there is actual work to do; a pure-readback path uses `get()`.
    let no_field_changes = req.display_name.is_none()
        && req.description.is_none()
        && req.environment.is_none()
        && req.design_graph_json.is_none()
        && req.latest_yaml.is_none()
        && latest_version_id.is_none();
    if no_field_changes {
        let topo = state.topology_repo.get(&id).await?;
        return Ok(Json(UpdateArchitectureResponse {
            architecture: ArchitectureSummary::from(topo),
        }));
    }

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

/// Soft-delete a topology. Operator+ only. Optimistic-concurrency: the caller
/// supplies the version they read; a concurrent edit that bumped the row
/// returns 409 Conflict. Already-archived rows return 404 so callers know
/// whether they were the one that archived it.
pub async fn archive_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<ArchiveArchitectureRequest>,
) -> Result<Json<ArchiveArchitectureResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    tracing::info!(
        architecture_id = %id,
        expected_version = req.expected_version,
        "archive_architecture"
    );
    let result = state.topology_repo.archive(&id, req.expected_version).await;
    match result {
        Ok(topo) => Ok(Json(ArchiveArchitectureResponse {
            architecture: ArchitectureSummary::from(topo),
        })),
        Err(StoreError::StaleVersion {
            current, expected, ..
        }) => Err(BffError::Conflict(format!(
            "stale version: client sent {expected}, current is {current}"
        ))),
        Err(other) => Err(other.into()),
    }
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

// ---------------------------------------------------------------------------
// Phase 1 DTOs — validate, generate-yaml, import-yaml
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ValidateArchitectureRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ValidateArchitectureYamlRequest {
    pub yaml: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ValidateArchitectureResponse {
    #[serde(flatten)]
    pub result: ValidationResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GenerateYamlRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GenerateYamlResponse {
    pub yaml: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportYamlRequest {
    pub id: String,
    pub yaml: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportYamlResponse {
    pub result: ValidationResult,
}

// ---------------------------------------------------------------------------
// Phase 1 handlers
// ---------------------------------------------------------------------------

/// Validate the persisted topology's `latest_yaml`. Persists the outcome by
/// setting `last_validation_status` on the topology row (Passed when no
/// errors, Failed otherwise).
///
/// A topology with no `latest_yaml` is reported as a single SCHEMA_INVALID
/// finding (no body to validate) rather than a 4xx — the caller might be a
/// dashboard that just wants to display "no YAML yet" alongside other
/// validation state.
pub async fn validate_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<ValidateArchitectureRequest>,
) -> Result<Json<ValidateArchitectureResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    tracing::info!(architecture_id = %id, "validate_architecture");

    let topo = state.topology_repo.get(&id).await?;
    let yaml = topo.latest_yaml.as_deref().unwrap_or("");
    let result = validate_yaml_str(yaml);

    // Best-effort persistence of the outcome. If the row was bumped between
    // our get() and the set_validation_status call, surface the conflict so
    // the caller can re-read; the validation result itself was correct, but
    // the persisted status would be against a stale row.
    let new_status = if result.summary.errors == 0 {
        ValidationStatus::Passed
    } else {
        ValidationStatus::Failed
    };
    match state
        .topology_repo
        .set_validation_status(&id, topo.version_number, new_status)
        .await
    {
        Ok(_) => {}
        Err(StoreError::StaleVersion {
            current, expected, ..
        }) => {
            return Err(BffError::Conflict(format!(
                "topology was modified concurrently while persisting validation status: client sent {expected}, current is {current}"
            )));
        }
        Err(other) => return Err(other.into()),
    }

    Ok(Json(ValidateArchitectureResponse { result }))
}

/// Validate an ad-hoc YAML body without touching persistent state. Used by
/// the editor's "validate before save" path.
pub async fn validate_architecture_yaml(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(req): Json<ValidateArchitectureYamlRequest>,
) -> Result<Json<ValidateArchitectureResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    if req.yaml.trim().is_empty() {
        return Err(BffError::BadRequest("yaml must not be blank".into()));
    }
    let result = validate_yaml_str(&req.yaml);
    Ok(Json(ValidateArchitectureResponse { result }))
}

/// Phase 1 generate-yaml. Returns the topology's `latest_yaml` verbatim if
/// present; otherwise responds with a 422 carrying `code: GRAPH_EMPTY`.
///
/// The graph→YAML mapper is a Phase 2 deliverable owned by the canvas (the
/// canvas knows the node/edge schema; the validator does not). Surfacing a
/// stable code now lets the UI behave deterministically while the mapper
/// is built.
pub async fn generate_architecture_yaml(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<GenerateYamlRequest>,
) -> Result<Json<GenerateYamlResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    tracing::info!(architecture_id = %id, "generate_architecture_yaml");
    let topo = state.topology_repo.get(&id).await?;
    if let Some(yaml) = topo.latest_yaml.filter(|s| !s.trim().is_empty()) {
        return Ok(Json(GenerateYamlResponse { yaml }));
    }
    // Phase 2 will translate design_graph_json → YAML. Until then, an
    // empty graph means we have nothing to emit.
    Err(BffError::GraphEmpty)
}

/// Replace a topology's `latest_yaml` with caller-supplied YAML. Validates
/// the YAML, persists `latest_yaml` and `last_validation_status` together
/// in one optimistic-concurrency-checked update. Validation failure does
/// NOT block the import — the YAML is stored and the row is marked
/// `last_validation_status = failed` so the operator can iterate.
pub async fn import_yaml_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<ImportYamlRequest>,
) -> Result<Json<ImportYamlResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    if req.yaml.trim().is_empty() {
        return Err(BffError::BadRequest("yaml must not be blank".into()));
    }
    tracing::info!(architecture_id = %id, "import_yaml_architecture");

    let topo = state.topology_repo.get(&id).await?;
    let result = validate_yaml_str(&req.yaml);
    let new_status = if result.summary.errors == 0 {
        ValidationStatus::Passed
    } else {
        ValidationStatus::Failed
    };

    let update_result = state
        .topology_repo
        .update(TopologyUpdateInput {
            id: id.clone(),
            expected_version: topo.version_number,
            display_name: None,
            description: None,
            environment: None,
            status: None,
            design_graph_json: None,
            latest_yaml: Some(req.yaml),
            latest_version_id: None,
            last_validation_status: Some(new_status),
            last_fleet_check_status: None,
        })
        .await;
    match update_result {
        Ok(_) => Ok(Json(ImportYamlResponse { result })),
        Err(StoreError::StaleVersion {
            current, expected, ..
        }) => Err(BffError::Conflict(format!(
            "stale version: client sent {expected}, current is {current}"
        ))),
        Err(other) => Err(other.into()),
    }
}

pub async fn plan_architecture(
    BearerToken(claims): BearerToken,
    State(_state): State<AppState>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, BffError> {
    require_operator_or_admin(&claims)?;
    Err(BffError::NotImplemented("phase 0".into()))
}

// ---------------------------------------------------------------------------
// Phase 3 DTOs — check-fleet
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CheckFleetRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CheckFleetResponse {
    pub status: ValidationStatusKind,
    pub inventory_snapshot_id: String,
    pub checked_at: String,
    pub findings: Vec<Finding>,
}

/// Mirrors `chv_architecture_validate::ValidationStatusKind` so the BFF
/// surface owns the wire enum without re-exporting it. Keeping the
/// duplication local keeps the contract stable if the validator tag
/// changes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatusKind {
    Valid,
    Warning,
    Invalid,
}

/// Phase 3 fleet check. Captures a fresh inventory snapshot, runs
/// layer-2 checks against the topology's `latest_yaml`, persists the
/// snapshot, updates `last_fleet_check_status`, and returns the
/// findings.
///
/// Role-gated to Operator+ via the router. The `caller_can_deploy`
/// signal passed to the inventory provider is `true` until the
/// `architecture:apply` permission ships in Phase 4 — without it we
/// would emit `PERMISSION_DENIED_DEPLOY` on every call.
pub async fn check_fleet_architecture(
    BearerToken(claims): BearerToken,
    State(state): State<AppState>,
    Json(req): Json<CheckFleetRequest>,
) -> Result<Json<CheckFleetResponse>, BffError> {
    require_operator_or_admin(&claims)?;
    let id = parse_id(&req.id)?;
    tracing::info!(architecture_id = %id, "check_fleet_architecture");

    let topo = state.topology_repo.get(&id).await?;
    let yaml = topo
        .latest_yaml
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if yaml.is_empty() {
        return Err(BffError::BadRequest(
            "topology has no latest_yaml; import or generate YAML first".into(),
        ));
    }

    // Capture the fleet snapshot. Phase 4 wires deploy_allowed_for_caller
    // to the architecture:apply permission check; Phase 3 short-circuits
    // to true so PERMISSION_DENIED_DEPLOY does not fire spuriously.
    let provider = FleetInventoryProvider {
        nodes: state.node_repo.clone(),
        networks: state.network_repo.clone(),
        images: state.image_repo.clone(),
        deploy_allowed_for_caller: true,
    };
    let snapshot = chv_architecture_reconcile::capture(&provider, "bff/check-fleet")
        .await
        .map_err(|e| BffError::Internal(format!("capture inventory: {e}")))?;

    // Persist the snapshot for plan/drift downstream.
    let snapshot_id = InventorySnapshotId::new(chv_common::gen_short_id())
        .map_err(|e| BffError::Internal(format!("mint inventory snapshot id: {e}")))?;
    let snapshot_json = serde_json::to_string(&snapshot)?;
    let summary_json = Some(
        serde_json::json!({
            "totals": {
                "hosts": snapshot.nodes.len(),
                "networks": snapshot.networks.len(),
                "datastores": snapshot.datastores.len(),
                "images": snapshot.images.len(),
                "backup_targets": snapshot.backup_targets.len(),
            },
            "backup_targets_complete": snapshot.backup_targets_complete,
            "captured_by": "bff/check-fleet",
        })
        .to_string(),
    );
    let persisted = state.topology_repo.pool();
    // Use the existing snapshot repository so persistence stays
    // single-source-of-truth.
    let snapshot_repo = chv_controlplane_store::InventorySnapshotRepository::new(persisted.clone());
    let saved = snapshot_repo
        .create(InventorySnapshotCreateInput {
            id: snapshot_id.clone(),
            source: snapshot.source.clone(),
            snapshot_json,
            summary_json,
            captured_by: Some(claims.sub.clone()),
        })
        .await?;

    // Parse the model and run pure-data fleet checks.
    let model = parse_arch_yaml(yaml)
        .map_err(|e| BffError::BadRequest(format!("latest_yaml parse failed: {e}")))?;
    let findings: Vec<Finding> = check_fleet(&model, &snapshot);

    // Compute status from finding severities. Errors → invalid;
    // warnings → warning; clean → valid.
    let mut errors = 0usize;
    let mut warnings = 0usize;
    for f in &findings {
        match f.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => {}
        }
    }
    let status = if errors > 0 {
        ValidationStatusKind::Invalid
    } else if warnings > 0 {
        ValidationStatusKind::Warning
    } else {
        ValidationStatusKind::Valid
    };

    // Persist last_fleet_check_status. Failed when any error finding,
    // Passed otherwise (warnings still count as a successful check).
    let new_status = if errors > 0 {
        FleetCheckStatus::Failed
    } else {
        FleetCheckStatus::Passed
    };
    let update_result = state
        .topology_repo
        .update(TopologyUpdateInput {
            id: id.clone(),
            expected_version: topo.version_number,
            display_name: None,
            description: None,
            environment: None,
            status: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
            last_validation_status: None,
            last_fleet_check_status: Some(new_status),
        })
        .await;
    match update_result {
        Ok(_) => {}
        Err(StoreError::StaleVersion {
            current, expected, ..
        }) => {
            return Err(BffError::Conflict(format!(
                "topology was modified concurrently while persisting fleet check status: client sent {expected}, current is {current}"
            )));
        }
        Err(other) => return Err(other.into()),
    }

    Ok(Json(CheckFleetResponse {
        status,
        inventory_snapshot_id: saved.id.into_inner(),
        checked_at: snapshot.captured_at.to_rfc3339(),
        findings,
    }))
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
