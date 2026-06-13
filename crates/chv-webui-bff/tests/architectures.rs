//! Integration tests for the Phase 0 Architecture Designer BFF surface.
//!
//! These tests exercise the handlers directly (no HTTP layer) against an
//! in-memory SQLite database wired to a real repository instance. The BFF
//! crate uses this same pattern in its embedded `auth.rs` tests, so the
//! authentication / authorization paths are covered here without spinning
//! up an axum server.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::Json;
use chv_controlplane_store::{
    AlertRepository, BackupRepository, DesiredStateRepository, EventRepository, NodeRepository,
    ObservedStateRepository, OperationRepository, TopologyRepository,
};
use chv_webui_bff::auth::{BearerToken, Claims};
use chv_webui_bff::handlers::architectures::{
    apply_architecture, archive_architecture, check_fleet_architecture, create_architecture,
    destroy_architecture, destroy_plan_architecture, discard_plan_architecture,
    generate_architecture_yaml, get_architecture, get_architecture_drift, list_architecture_runs,
    list_architecture_versions, list_architectures, plan_architecture, update_architecture,
    validate_architecture, ArchiveArchitectureRequest, CreateArchitectureRequest,
    GetArchitectureRequest, ListArchitecturesRequest, UpdateArchitectureRequest,
};
use chv_webui_bff::mutations::MutationService;
use chv_webui_bff::{AppState, BffError};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;

// ---------------------------------------------------------------------------
// Test scaffolding
// ---------------------------------------------------------------------------

/// MutationService stub — none of the architecture handlers call into the
/// mutation service, so every method is unreachable in this test surface.
struct NoopMutations;

#[async_trait]
impl MutationService for NoopMutations {
    async fn mutate_vm(
        &self,
        _vm_id: String,
        _action: String,
        _force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("mutate_vm not used in architecture tests")
    }
    async fn migrate_vm(
        &self,
        _vm_id: String,
        _target_node_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("migrate_vm not used in architecture tests")
    }
    async fn snapshot_vm(
        &self,
        _vm_id: String,
        _destination: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("snapshot_vm not used in architecture tests")
    }
    async fn restore_snapshot(
        &self,
        _vm_id: String,
        _source: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("restore_snapshot not used in architecture tests")
    }
    async fn mutate_node(
        &self,
        _node_id: String,
        _action: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse, BffError> {
        unreachable!("mutate_node not used in architecture tests")
    }
    async fn mutate_volume(
        &self,
        _volume_id: String,
        _action: String,
        _force: bool,
        _resize_bytes: Option<u64>,
        _vm_id: Option<String>,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("mutate_volume not used in architecture tests")
    }
    async fn snapshot_volume(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("snapshot_volume not used in architecture tests")
    }
    async fn restore_volume_snapshot(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("restore_volume_snapshot not used in architecture tests")
    }
    async fn delete_volume_snapshot(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("delete_volume_snapshot not used in architecture tests")
    }
    async fn clone_volume(
        &self,
        _source_volume_id: String,
        _target_volume_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("clone_volume not used in architecture tests")
    }
    async fn mutate_network(
        &self,
        _network_id: String,
        _action: String,
        _force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse, BffError> {
        unreachable!("mutate_network not used in architecture tests")
    }
}

async fn build_state() -> AppState {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    chv_controlplane_store::run_migrations(&pool, None)
        .await
        .expect("run migrations");

    AppState {
        pool: pool.clone(),
        node_repo: NodeRepository::new(pool.clone()),
        operation_repo: OperationRepository::new(pool.clone()),
        event_repo: EventRepository::new(pool.clone()),
        alert_repo: AlertRepository::new(pool.clone()),
        desired_state_repo: DesiredStateRepository::new(pool.clone()),
        observed_state_repo: ObservedStateRepository::new(pool.clone()),
        backup_repo: BackupRepository::new(pool.clone()),
        topology_repo: TopologyRepository::new(pool.clone()),
        mutations: Arc::new(NoopMutations),
        jwt_secret: "test-secret".to_string(),
        agent_runtime_dir: std::path::PathBuf::from("/var/lib/chv/agent"),
        cache: chv_webui_bff::BffCache::new(5),
    }
}

fn claims_for(role: &str) -> Claims {
    Claims {
        sub: "u-tester".to_string(),
        username: "tester".to_string(),
        role: role.to_string(),
        exp: u64::MAX / 2,
        must_change_password: false,
    }
}

fn err_status(e: &BffError) -> u16 {
    match e {
        BffError::BadRequest(_) => 400,
        BffError::Unauthorized(_) => 401,
        BffError::Forbidden(_) => 403,
        BffError::NotFound(_) => 404,
        BffError::Conflict(_) => 409,
        BffError::TooManyRequests(_) => 429,
        BffError::Internal(_) => 500,
        BffError::NotImplemented(_) => 501,
        BffError::QuotaExceeded { .. } => 422,
    }
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn crud_lifecycle_create_list_get_update_archive() {
    let state = build_state().await;

    // 1. Create
    let create = create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "customer-a-prod".to_string(),
            description: Some("Customer A production".to_string()),
            environment: Some("production".to_string()),
            display_name: Some("Customer A — Production".to_string()),
            design_graph_json: Some(r#"{"nodes":[],"edges":[]}"#.to_string()),
            latest_yaml: None,
        }),
    )
    .await
    .expect("create should succeed");
    let created = create.0.architecture;
    assert_eq!(created.name, "customer-a-prod");
    assert_eq!(created.version_number, 1);
    let arch_id = created.id.clone();

    // 2. List default scope (excludes archived) — should include it
    let listed = list_architectures(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(ListArchitecturesRequest::default()),
    )
    .await
    .expect("list should succeed");
    assert_eq!(listed.0.architectures.len(), 1);
    assert_eq!(listed.0.architectures[0].id, arch_id);

    // 3. Get includes design_graph_json
    let got = get_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(GetArchitectureRequest {
            id: arch_id.clone(),
        }),
    )
    .await
    .expect("get should succeed");
    assert_eq!(got.0.architecture.id, arch_id);
    assert_eq!(
        got.0.design_graph_json.as_deref(),
        Some(r#"{"nodes":[],"edges":[]}"#)
    );

    // 4. Update bumps version_number
    let updated = update_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(UpdateArchitectureRequest {
            id: arch_id.clone(),
            expected_version: 1,
            display_name: Some("Customer A — Prod (renamed)".to_string()),
            description: None,
            environment: None,
            design_graph_json: Some(r#"{"nodes":[1],"edges":[]}"#.to_string()),
            latest_yaml: None,
            latest_version_id: None,
        }),
    )
    .await
    .expect("update should succeed");
    assert_eq!(updated.0.architecture.version_number, 2);
    assert_eq!(
        updated.0.architecture.display_name.as_deref(),
        Some("Customer A — Prod (renamed)")
    );

    // 5. Archive — supplies the bumped version_number from the update.
    let _ = archive_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(ArchiveArchitectureRequest {
            id: arch_id.clone(),
            expected_version: updated.0.architecture.version_number,
        }),
    )
    .await
    .expect("archive should succeed");

    // 6. List default excludes archived
    let listed_again = list_architectures(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(ListArchitecturesRequest::default()),
    )
    .await
    .expect("list should succeed");
    assert!(
        listed_again.0.architectures.is_empty(),
        "archived topology must be hidden from default list"
    );

    // 7. List include_archived=true surfaces it
    let listed_arch = list_architectures(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(ListArchitecturesRequest {
            include_archived: true,
        }),
    )
    .await
    .expect("list-with-archived should succeed");
    assert_eq!(listed_arch.0.architectures.len(), 1);
    assert!(listed_arch.0.architectures[0].archived_at.is_some());
}

// ---------------------------------------------------------------------------
// Optimistic concurrency
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_with_stale_version_returns_conflict() {
    let state = build_state().await;

    let created = create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "stale-version-test".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect("create should succeed")
    .0
    .architecture;
    let id = created.id.clone();
    assert_eq!(created.version_number, 1);

    // First update succeeds, bumps version to 2
    let _ = update_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(UpdateArchitectureRequest {
            id: id.clone(),
            expected_version: 1,
            display_name: Some("first".to_string()),
            description: None,
            environment: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
        }),
    )
    .await
    .expect("first update should succeed");

    // Second update with the now-stale expected_version=1 must 409
    let err = update_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(UpdateArchitectureRequest {
            id: id.clone(),
            expected_version: 1,
            display_name: Some("second".to_string()),
            description: None,
            environment: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
        }),
    )
    .await
    .expect_err("stale-version update must fail");
    assert_eq!(err_status(&err), 409, "stale version => 409 Conflict");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("stale version")
            && msg.contains("client sent 1")
            && msg.contains("current is 2"),
        "conflict message should reveal versions; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_unknown_id_returns_not_found() {
    let state = build_state().await;
    let v = claims_for("viewer");

    let err = get_architecture(
        BearerToken(v),
        State(state),
        Json(GetArchitectureRequest {
            id: "does-not-exist".to_string(),
        }),
    )
    .await
    .expect_err("unknown id must error");
    assert_eq!(err_status(&err), 404);
}

// ---------------------------------------------------------------------------
// AuthZ
// ---------------------------------------------------------------------------

#[tokio::test]
async fn viewer_cannot_create_architecture() {
    let state = build_state().await;
    let v = claims_for("viewer");

    let err = create_architecture(
        BearerToken(v),
        State(state),
        Json(CreateArchitectureRequest {
            name: "viewer-cannot".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect_err("viewer must not be able to create");
    assert_eq!(err_status(&err), 403);
}

#[tokio::test]
async fn viewer_cannot_update_architecture() {
    let state = build_state().await;
    let created = create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "viewer-update-test".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect("create")
    .0
    .architecture;

    let v = claims_for("viewer");
    let err = update_architecture(
        BearerToken(v),
        State(state.clone()),
        Json(UpdateArchitectureRequest {
            id: created.id.clone(),
            expected_version: 1,
            display_name: Some("nope".to_string()),
            description: None,
            environment: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
        }),
    )
    .await
    .expect_err("viewer update must be forbidden");
    assert_eq!(err_status(&err), 403);
}

#[tokio::test]
async fn viewer_cannot_archive_architecture() {
    let state = build_state().await;
    let created = create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "viewer-archive-test".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect("create")
    .0
    .architecture;

    let v = claims_for("viewer");
    let err = archive_architecture(
        BearerToken(v),
        State(state),
        Json(ArchiveArchitectureRequest {
            id: created.id.clone(),
            expected_version: 1,
        }),
    )
    .await
    .expect_err("viewer archive must be forbidden");
    assert_eq!(err_status(&err), 403);
}

// Note on 401 / unauthenticated:
//
// 401 enforcement lives in the `BearerToken` extractor and the role-gating
// middleware in `crate::auth`, neither of which can be exercised by calling
// handlers as plain functions. The middleware paths are covered by
// `crate::auth` tests in the BFF crate, and the role-required arms are
// covered by the `viewer_cannot_*` tests above. The architecture handlers
// themselves are gate-only behind those layers, so a request without a
// bearer token never reaches them.

// ---------------------------------------------------------------------------
// Stub endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn validate_stub_returns_501_for_operator() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = validate_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn validate_stub_forbids_viewer_before_501() {
    // Validate is operator+ even before the real handler lands; verify the
    // role gate sticks so a viewer never gets a misleading 501.
    let state = build_state().await;
    let v = claims_for("viewer");
    let err = validate_architecture(BearerToken(v), State(state), Json(json!({})))
        .await
        .expect_err("viewer must be forbidden");
    assert_eq!(err_status(&err), 403);
}

#[tokio::test]
async fn check_fleet_stub_returns_501_for_operator() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = check_fleet_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn check_fleet_stub_forbids_viewer_before_501() {
    // Verify the stub still enforces the role gate so callers don't get a
    // misleading 501 when they actually lack permission. Stubs that gate on
    // operator+ MUST return 403 to a viewer.
    let state = build_state().await;
    let v = claims_for("viewer");
    let err = check_fleet_architecture(BearerToken(v), State(state), Json(json!({})))
        .await
        .expect_err("viewer must be forbidden");
    assert_eq!(err_status(&err), 403);
}

#[tokio::test]
async fn generate_yaml_stub_returns_501_for_operator() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = generate_architecture_yaml(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn plan_stub_returns_501_for_operator() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = plan_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn destroy_plan_stub_returns_501_for_operator() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = destroy_plan_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn discard_plan_stub_returns_501_for_operator() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = discard_plan_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn apply_stub_returns_501_for_admin() {
    let state = build_state().await;
    let admin = claims_for("admin");
    let err = apply_architecture(BearerToken(admin), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn apply_stub_forbids_operator_before_501() {
    // The apply verb is admin-only — an operator hitting it must see 403,
    // not 501. This guards the routing decision in §2 of the plan.
    let state = build_state().await;
    let op = claims_for("operator");
    let err = apply_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("operator must be forbidden from apply");
    assert_eq!(err_status(&err), 403);
}

#[tokio::test]
async fn destroy_stub_returns_501_for_admin() {
    let state = build_state().await;
    let admin = claims_for("admin");
    let err = destroy_architecture(BearerToken(admin), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn destroy_stub_forbids_operator_before_501() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = destroy_architecture(BearerToken(op), State(state), Json(json!({})))
        .await
        .expect_err("operator must be forbidden from destroy");
    assert_eq!(err_status(&err), 403);
}

#[tokio::test]
async fn drift_stub_returns_501_for_viewer() {
    let state = build_state().await;
    let v = claims_for("viewer");
    let err = get_architecture_drift(BearerToken(v), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn runs_list_stub_returns_501_for_viewer() {
    let state = build_state().await;
    let v = claims_for("viewer");
    let err = list_architecture_runs(BearerToken(v), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

#[tokio::test]
async fn versions_list_stub_returns_501_for_viewer() {
    let state = build_state().await;
    let v = claims_for("viewer");
    let err = list_architecture_versions(BearerToken(v), State(state), Json(json!({})))
        .await
        .expect_err("stub must error");
    assert_eq!(err_status(&err), 501);
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_with_blank_name_returns_400() {
    let state = build_state().await;
    let op = claims_for("operator");
    let err = create_architecture(
        BearerToken(op),
        State(state),
        Json(CreateArchitectureRequest {
            name: "   ".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect_err("blank name must 400");
    assert_eq!(err_status(&err), 400);
}

#[tokio::test]
async fn get_with_blank_id_returns_400() {
    let state = build_state().await;
    let v = claims_for("viewer");
    let err = get_architecture(
        BearerToken(v),
        State(state),
        Json(GetArchitectureRequest { id: "".to_string() }),
    )
    .await
    .expect_err("blank id must 400");
    assert_eq!(err_status(&err), 400);
}

// ---------------------------------------------------------------------------
// Optimistic-concurrency on archive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn archive_with_stale_expected_version_returns_409() {
    let state = build_state().await;

    let created = create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "stale-archive".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect("create")
    .0
    .architecture;
    assert_eq!(created.version_number, 1);

    // Bump the version via update; client B still holds v=1.
    let _ = update_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(UpdateArchitectureRequest {
            id: created.id.clone(),
            expected_version: 1,
            display_name: Some("renamed".to_string()),
            description: None,
            environment: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
        }),
    )
    .await
    .expect("update");

    // Stale archive must surface as 409 with the documented version banner.
    let err = archive_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ArchiveArchitectureRequest {
            id: created.id.clone(),
            expected_version: 1,
        }),
    )
    .await
    .expect_err("stale archive must 409");
    assert_eq!(err_status(&err), 409);
    let msg = format!("{err:?}");
    assert!(
        msg.contains("stale version")
            && msg.contains("client sent 1")
            && msg.contains("current is 2"),
        "conflict message should reveal versions; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Duplicate name → 409 (UNIQUE violation surfaces as Conflict, not 500)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let state = build_state().await;

    create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "dup-name".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect("first create");

    let err = create_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(CreateArchitectureRequest {
            name: "dup-name".to_string(),
            description: None,
            environment: None,
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect_err("duplicate name must fail");
    assert_eq!(err_status(&err), 409, "duplicate name => 409 Conflict");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("dup-name") && msg.contains("name already exists"),
        "conflict message should name the offending field; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// No-op update keeps version stable (M1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_with_no_field_changes_does_not_bump_version() {
    let state = build_state().await;

    let created = create_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: "noop-update".to_string(),
            description: Some("d".to_string()),
            environment: Some("test".to_string()),
            display_name: None,
            design_graph_json: None,
            latest_yaml: None,
        }),
    )
    .await
    .expect("create")
    .0
    .architecture;
    assert_eq!(created.version_number, 1);

    let result = update_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(UpdateArchitectureRequest {
            id: created.id.clone(),
            expected_version: 1,
            display_name: None,
            description: None,
            environment: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
        }),
    )
    .await
    .expect("noop update should succeed");
    assert_eq!(
        result.0.architecture.version_number, 1,
        "no field changes must NOT bump the version"
    );
}
