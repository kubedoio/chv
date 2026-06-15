//! Integration tests for the Phase 5 Architecture Designer apply / destroy
//! handlers (`POST /v1/architectures/apply`, `POST /v1/architectures/destroy`).
//!
//! Mirrors the harness in `tests/architectures_plan.rs` (in-memory SQLite,
//! direct handler invocation, no HTTP layer) but exercises the apply path
//! end-to-end against an injected `ManualClock`. Tests cover:
//!
//! * happy-path apply (plan → apply → run row + operations enqueued)
//! * typed-name confirmation guard for destructive plans
//! * warnings-acknowledged guard for plans carrying warnings
//! * production-environment role escalation (operator → 403,
//!   admin → 200)
//! * plan TTL expiry
//! * plan-status guard (only ReadyToApply applies; everything else 409)
//! * idempotency on retry — a second apply with the same plan_id must not
//!   duplicate operations
//! * destroy endpoint rejecting apply-mode plans
//! * `requested_by` actor stamping on the apply_run row
//! * tracing emission contract (spot-checked via a non-blocking subscriber)

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::Json;
use chrono::{Duration, TimeZone, Utc};
use chv_common::ManualClock;
use chv_controlplane_store::{
    AlertRepository, ApplyRunRepository, BackupRepository, DesiredStateRepository, EventRepository,
    ImageRepository, NetworkRepository, NodeRepository, ObservedStateRepository,
    OperationRepository, PlanRepository, PlanStatusUpdateInput, TopologyRepository,
};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitecturePlanId, PlanMode, PlanStatus, RunStatus,
};
use chv_webui_bff::auth::{BearerToken, Claims};
use chv_webui_bff::handlers::architectures::{
    apply_architecture, create_architecture, destroy_architecture, destroy_plan_architecture,
    plan_architecture, ApplyArchitectureRequest, ConfirmationDto, CreateArchitectureRequest,
    PlanArchitectureRequest,
};
use chv_webui_bff::mutations::MutationService;
use chv_webui_bff::{AppState, BffError};
use sqlx::sqlite::SqlitePoolOptions;

// ---------------------------------------------------------------------------
// Test scaffolding
// ---------------------------------------------------------------------------

/// MutationService stub — none of the architecture handlers call into the
/// mutation service in this test surface. The trait still has to be
/// satisfied because `AppState` carries an `Arc<dyn MutationService>`.
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
        unreachable!("mutate_vm not used")
    }
    async fn migrate_vm(
        &self,
        _vm_id: String,
        _target_node_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("migrate_vm not used")
    }
    async fn snapshot_vm(
        &self,
        _vm_id: String,
        _destination: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("snapshot_vm not used")
    }
    async fn restore_snapshot(
        &self,
        _vm_id: String,
        _source: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!("restore_snapshot not used")
    }
    async fn mutate_node(
        &self,
        _node_id: String,
        _action: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse, BffError> {
        unreachable!("mutate_node not used")
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
        unreachable!("mutate_volume not used")
    }
    async fn snapshot_volume(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("snapshot_volume not used")
    }
    async fn restore_volume_snapshot(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("restore_volume_snapshot not used")
    }
    async fn delete_volume_snapshot(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("delete_volume_snapshot not used")
    }
    async fn mutate_network(
        &self,
        _network_id: String,
        _action: String,
        _force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse, BffError> {
        unreachable!("mutate_network not used")
    }
    async fn clone_volume(
        &self,
        _volume_id: String,
        _new_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!("clone_volume not used")
    }
}

fn t0() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 15, 10, 0, 0).unwrap()
}

async fn build_state_with_clock(clock: ManualClock) -> AppState {
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
        network_repo: NetworkRepository::new(pool.clone()),
        image_repo: ImageRepository::new(pool.clone()),
        apply_runs: Arc::new(ApplyRunRepository::new(pool.clone())),
        mutations: Arc::new(NoopMutations),
        jwt_secret: "test-secret".to_string(),
        agent_runtime_dir: std::path::PathBuf::from("/var/lib/chv/agent"),
        cache: chv_webui_bff::BffCache::new(5),
        clock: Arc::new(clock),
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
        BffError::GraphEmpty => 422,
        BffError::PlanExpired { .. } => 409,
        BffError::PlanNotDiscardable { .. } => 409,
        BffError::MissingConfirmation { .. } => 400,
        BffError::WarningsNotAcknowledged { .. } => 400,
        BffError::PlanNotApplicable { .. } => 409,
        BffError::ProductionRequiresAdmin { .. } => 403,
        BffError::PlanModeMismatch { .. } => 400,
        BffError::InvalidResourceName { .. } => 400,
    }
}

/// Seed one schedulable host with plenty of headroom so fleet checks pass
/// for the happy-path YAML used below. Mirrors the seeding helper in
/// `tests/architectures_plan.rs`.
async fn seed_capable_host(state: &AppState) {
    sqlx::query(
        r#"INSERT INTO nodes (node_id, hostname, display_name)
           VALUES ('n1', 'host-1', 'host-1')"#,
    )
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes)
           VALUES ('n1', 'x86_64', 16, ?1)"#,
    )
    .bind(64i64 * 1024 * 1024 * 1024)
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO node_desired_state
           (node_id, desired_generation, desired_state, scheduling_paused)
           VALUES ('n1', 1, 'Running', 0)"#,
    )
    .execute(&state.pool)
    .await
    .unwrap();
}

const HAPPY_YAML: &str = r#"apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: happy
templates:
  - name: small
    image: ubuntu-24.04
    cpu: 1
    memory_mb: 1024
instances:
  - name: app-a
    template: small
    placement:
      server: host-1
  - name: app-b
    template: small
    placement:
      server: host-1
"#;

async fn create_arch(
    state: &AppState,
    name: &str,
    yaml: Option<String>,
    environment: Option<String>,
) -> String {
    // Production-environment topologies require Admin per Phase-5 reviewer F2.
    let creator_role = match environment.as_deref() {
        Some(env) => {
            let normalized = env.trim().to_ascii_lowercase();
            if matches!(normalized.as_str(), "production" | "prod") {
                "admin"
            } else {
                "operator"
            }
        }
        None => "operator",
    };
    create_architecture(
        BearerToken(claims_for(creator_role)),
        State(state.clone()),
        Json(CreateArchitectureRequest {
            name: name.to_string(),
            description: None,
            environment,
            display_name: None,
            design_graph_json: None,
            latest_yaml: yaml,
        }),
    )
    .await
    .expect("create")
    .0
    .architecture
    .id
}

/// Generate a `ready_to_apply` plan and force its status to `ReadyToApply`
/// in case the diff happens to land on `RequiresConfirmation` (e.g., when
/// a future test introduces destructive changes). The current `HAPPY_YAML`
/// produces only Creates so this is a no-op for it, but keeping the helper
/// symmetric protects the tests against diff drift.
async fn generate_ready_plan(
    state: &AppState,
    arch_id: &str,
) -> chv_webui_bff::handlers::architectures::PlanResponse {
    let plan = plan_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(PlanArchitectureRequest {
            id: arch_id.to_string(),
            allow_warnings: None,
            refresh_inventory: None,
        }),
    )
    .await
    .expect("plan should succeed")
    .0;
    if plan.status != PlanStatus::ReadyToApply {
        // Force-promote the row so the apply guard accepts it. Phase 4
        // ships a confirm-plan flow that the UI will trigger between plan
        // and apply; until that lands we drive the transition by hand.
        let plan_repo = PlanRepository::new(state.pool.clone());
        plan_repo
            .update_status(PlanStatusUpdateInput {
                id: ArchitecturePlanId::new(plan.plan_id.clone()).unwrap(),
                status: PlanStatus::ReadyToApply,
                confirmed_by: Some("u-tester".to_string()),
                mark_confirmed: true,
                mark_discarded: false,
                discarded_by: None,
            })
            .await
            .expect("force-ready");
    }
    plan
}

// ---------------------------------------------------------------------------
// 1. apply happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_happy_path() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "happy-apply", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    let resp = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(ApplyArchitectureRequest {
            id: arch_id.clone(),
            plan_id: plan.plan_id.clone(),
            // HAPPY_YAML produces only Creates → not destructive, no typed
            // name required.
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect("apply should succeed")
    .0;

    assert_eq!(resp.architecture_id, arch_id);
    assert_eq!(resp.plan_id, plan.plan_id);
    assert_eq!(resp.architecture_version_id, plan.architecture_version_id);
    assert_eq!(resp.status, "running");
    assert!(resp.task_id.is_some(), "first operation id must be wired");
    assert!(!resp.run_id.is_empty());

    // Verify the run row exists in the DB and matches.
    let runs_repo = ApplyRunRepository::new(state.pool.clone());
    let arch = ArchitectureId::new(arch_id).unwrap();
    let runs = runs_repo
        .list_for_architecture(&arch)
        .await
        .expect("list runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Running);
    assert_eq!(runs[0].requested_by.as_deref(), Some("u-tester"));
}

// ---------------------------------------------------------------------------
// 2. apply with missing typed-name on destructive returns 400
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_missing_typed_name_on_destructive_returns_400_missing_confirmation() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "destroy-me", Some(HAPPY_YAML.to_string()), None).await;

    // Destroy plan: every change is a Delete → destructive.
    let plan = destroy_plan_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(PlanArchitectureRequest {
            id: arch_id.clone(),
            allow_warnings: None,
            refresh_inventory: None,
        }),
    )
    .await
    .expect("destroy-plan")
    .0;

    // Force the plan into ReadyToApply (destroy plans land in
    // RequiresConfirmation by default).
    let plan_repo = PlanRepository::new(state.pool.clone());
    plan_repo
        .update_status(PlanStatusUpdateInput {
            id: ArchitecturePlanId::new(plan.plan_id.clone()).unwrap(),
            status: PlanStatus::ReadyToApply,
            confirmed_by: Some("u-tester".to_string()),
            mark_confirmed: true,
            mark_discarded: false,
            discarded_by: None,
        })
        .await
        .expect("force-ready");

    let err = destroy_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id.clone(),
            confirmation: ConfirmationDto { typed_name: None },
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect_err("destroy without typed name must 400");
    assert_eq!(err_status(&err), 400);
    assert!(matches!(err, BffError::MissingConfirmation { .. }));
}

// ---------------------------------------------------------------------------
// 3. apply with warnings unacknowledged returns 400
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_with_warnings_unacknowledged_returns_400_warnings_not_acknowledged() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "warned", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    // Inject a warning into the plan_json so `apply_plan` rejects unack'd.
    // We piggy-back on the same row by reading the existing plan_json,
    // rewriting it with a synthetic warning, and writing it back.
    let plan_id = ArchitecturePlanId::new(plan.plan_id.clone()).unwrap();
    let row =
        sqlx::query_scalar::<_, String>("SELECT plan_json FROM architecture_plans WHERE id = $1")
            .bind(plan_id.as_str())
            .fetch_one(&state.pool)
            .await
            .expect("fetch plan_json");
    let mut plan_struct: chv_architecture_reconcile::Plan =
        serde_json::from_str(&row).expect("plan_json is parseable");
    plan_struct
        .warnings
        .push("synthetic warning for test".to_string());
    plan_struct.summary.warnings = plan_struct.warnings.len() as u32;
    let new_json = serde_json::to_string(&plan_struct).unwrap();
    sqlx::query("UPDATE architecture_plans SET plan_json = $1 WHERE id = $2")
        .bind(&new_json)
        .bind(plan_id.as_str())
        .execute(&state.pool)
        .await
        .expect("rewrite plan_json");

    let err = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect_err("must reject unacknowledged warnings");
    assert_eq!(err_status(&err), 400);
    match err {
        BffError::WarningsNotAcknowledged { warnings, .. } => assert_eq!(warnings, 1),
        other => panic!("expected WarningsNotAcknowledged, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 4. production environment + operator returns 403
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_production_environment_as_operator_returns_403_production_requires_admin() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(
        &state,
        "prod-app",
        Some(HAPPY_YAML.to_string()),
        Some("production".to_string()),
    )
    .await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    let err = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect_err("operator must be blocked from production apply");
    assert_eq!(err_status(&err), 403);
    assert!(matches!(err, BffError::ProductionRequiresAdmin { .. }));
}

// ---------------------------------------------------------------------------
// 5. production environment as admin succeeds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_production_environment_as_admin_succeeds() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(
        &state,
        "prod-app",
        Some(HAPPY_YAML.to_string()),
        Some("production".to_string()),
    )
    .await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    let resp = apply_architecture(
        BearerToken(claims_for("admin")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            // HAPPY_YAML is non-destructive — no typed name required even
            // for production. The reconcile crate's destructive-apply
            // guard is plan-driven, not env-driven.
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect("admin must clear production guard");
    assert_eq!(resp.0.status, "running");
}

// ---------------------------------------------------------------------------
// 6. expired plan returns 409 PLAN_EXPIRED
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_expired_plan_returns_409_plan_expired() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock.clone()).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "expired", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    // Advance the clock past the 15-minute TTL.
    clock.set(t0() + Duration::minutes(20));

    let err = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect_err("expired plan must be rejected");
    assert_eq!(err_status(&err), 409);
    assert!(matches!(err, BffError::PlanExpired { .. }));
}

// ---------------------------------------------------------------------------
// 7. plan in draft / non-ready status returns 409 PLAN_NOT_APPLICABLE
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_plan_in_draft_status_returns_409_plan_not_applicable() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "drafty", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    // Force the plan back to Draft.
    let plan_repo = PlanRepository::new(state.pool.clone());
    plan_repo
        .update_status(PlanStatusUpdateInput {
            id: ArchitecturePlanId::new(plan.plan_id.clone()).unwrap(),
            status: PlanStatus::Draft,
            confirmed_by: None,
            mark_confirmed: false,
            mark_discarded: false,
            discarded_by: None,
        })
        .await
        .expect("force-draft");

    let err = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect_err("draft plan must be rejected");
    assert_eq!(err_status(&err), 409);
    assert!(matches!(err, BffError::PlanNotApplicable { .. }));
}

// ---------------------------------------------------------------------------
// 8. apply idempotent on retry — no operation duplication
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_idempotent_on_retry() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "retry", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    let req = || ApplyArchitectureRequest {
        id: arch_id.clone(),
        plan_id: plan.plan_id.clone(),
        confirmation: ConfirmationDto::default(),
        acknowledged_warnings: false,
    };

    let _first = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(req()),
    )
    .await
    .expect("first apply");

    // Count operations in the table before re-apply.
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations")
        .fetch_one(&state.pool)
        .await
        .unwrap();

    let _second = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(req()),
    )
    .await
    .expect("second apply must succeed (idempotent)");

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations")
        .fetch_one(&state.pool)
        .await
        .unwrap();

    assert_eq!(
        before, after,
        "second apply must not insert duplicate operations (idempotency_key collision)"
    );
}

// ---------------------------------------------------------------------------
// 9. destroy with apply-mode plan returns 400
// ---------------------------------------------------------------------------

#[tokio::test]
async fn destroy_with_apply_mode_plan_returns_400() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "wrong-mode", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;
    assert_eq!(
        plan.mode,
        PlanMode::Apply,
        "test precondition: apply-mode plan"
    );

    let err = destroy_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            // The mode mismatch fires before the typed-name check; even a
            // matching typed_name does not make this OK.
            confirmation: ConfirmationDto {
                typed_name: Some("wrong-mode".to_string()),
            },
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect_err("apply-mode plan against /destroy must be rejected");
    assert_eq!(err_status(&err), 400);
    assert!(matches!(err, BffError::BadRequest(_)));
}

// ---------------------------------------------------------------------------
// 10. apply records actor in run.requested_by
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_records_actor_in_run() {
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "audit", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    let resp = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state.clone()),
        Json(ApplyArchitectureRequest {
            id: arch_id.clone(),
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect("apply")
    .0;

    let runs_repo = ApplyRunRepository::new(state.pool.clone());
    let run = runs_repo
        .get(
            &chv_controlplane_types::architecture::ArchitectureApplyRunId::new(resp.run_id)
                .unwrap(),
        )
        .await
        .expect("run row exists");
    assert_eq!(
        run.requested_by.as_deref(),
        Some("u-tester"),
        "run.requested_by must record the caller subject"
    );
}

// ---------------------------------------------------------------------------
// 11. tracing emission spot-check
// ---------------------------------------------------------------------------
//
// We attach a non-blocking subscriber via `tracing-subscriber::fmt` writing
// to an in-memory buffer for one apply call, then assert the
// `architecture.apply` target appears with `apply_plan invoked` /
// `apply_plan succeeded` events. This protects the contract called out in
// the Phase-5 task plan ("every handler emits structured tracing events
// with the contract fields").

/// Tracing emission test. We install a process-wide subscriber once
/// (gated by an `OnceLock`) that pipes events to a shared in-memory
/// buffer. We then assert that calling `apply_architecture` produces at
/// least one event tagged with target `architecture.apply`.
///
/// `set_global_default` is one-shot per process; the buffer outlives
/// individual tests so we just check what landed in it after our apply.
static TRACE_BUF: OnceLock<Arc<std::sync::Mutex<Vec<u8>>>> = OnceLock::new();
static TRACE_INSTALLED: OnceLock<()> = OnceLock::new();

fn install_tracing_subscriber() -> Arc<std::sync::Mutex<Vec<u8>>> {
    let buf = TRACE_BUF
        .get_or_init(|| Arc::new(std::sync::Mutex::new(Vec::new())))
        .clone();
    TRACE_INSTALLED.get_or_init(|| {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        let writer = MakeBuf(buf.clone());
        let layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_target(true)
            .with_ansi(false);
        let _ = tracing_subscriber::Registry::default()
            .with(layer)
            .try_init();
    });
    buf
}

#[tokio::test]
async fn apply_emits_structured_tracing() {
    let buf = install_tracing_subscriber();
    // Snapshot the buffer length before our apply so we only inspect what
    // *this* test emits, isolating the assertion from concurrent tests.
    let baseline = buf.lock().unwrap().len();

    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "trace", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;
    let _ = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect("apply");

    let captured = {
        let guard = buf.lock().unwrap();
        String::from_utf8(guard[baseline..].to_vec()).expect("utf8")
    };
    assert!(
        captured.contains("architecture.apply"),
        "expected architecture.apply target in trace output, got:\n{captured}"
    );
    assert!(
        captured.contains("apply_plan invoked") || captured.contains("apply_plan succeeded"),
        "expected one of the apply tracing events:\n{captured}"
    );
}

// In-memory MakeWriter implementation.
#[derive(Clone)]
struct MakeBuf(Arc<std::sync::Mutex<Vec<u8>>>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MakeBuf {
    type Writer = BufWriter;
    fn make_writer(&'a self) -> Self::Writer {
        BufWriter(self.0.clone())
    }
}

struct BufWriter(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for BufWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 12. metrics scrape — apply_total{status="enqueued"} >= 1
// ---------------------------------------------------------------------------
//
// Installs an in-process Prometheus recorder on first use and asserts the
// rendered metrics text after a successful apply contains the
// `chv_architecture_apply_total{status="enqueued"}` counter at >= 1.
// `set_global_recorder` is one-shot per process; the `OnceLock` makes the
// install survive across test invocations and multiple apply calls in the
// same test binary.

use std::sync::OnceLock;

static METRICS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

fn ensure_metrics_installed() -> &'static metrics_exporter_prometheus::PrometheusHandle {
    METRICS_HANDLE.get_or_init(|| {
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        // Best-effort install; if some other test crate already installed
        // a recorder we silently fall back to the existing handle. The
        // handle in METRICS_HANDLE is the one bound to *this* recorder, so
        // a foreign recorder would break the assertion below — accept it
        // and let the test fail loudly rather than masking the conflict.
        let _ = metrics::set_global_recorder(recorder);
        handle
    })
}

#[tokio::test]
async fn apply_records_metrics_for_scrape() {
    let handle = ensure_metrics_installed();
    let clock = ManualClock::new(t0());
    let state = build_state_with_clock(clock).await;
    seed_capable_host(&state).await;
    let arch_id = create_arch(&state, "metrics", Some(HAPPY_YAML.to_string()), None).await;
    let plan = generate_ready_plan(&state, &arch_id).await;

    let _ = apply_architecture(
        BearerToken(claims_for("operator")),
        State(state),
        Json(ApplyArchitectureRequest {
            id: arch_id,
            plan_id: plan.plan_id,
            confirmation: ConfirmationDto::default(),
            acknowledged_warnings: false,
        }),
    )
    .await
    .expect("apply");

    let scrape = handle.render();
    assert!(
        scrape.contains("chv_architecture_apply_total"),
        "metrics scrape missing chv_architecture_apply_total. Output:\n{scrape}"
    );
    assert!(
        scrape.contains("chv_architecture_apply_total{status=\"enqueued\"}")
            || scrape.contains("status=\"enqueued\""),
        "metrics scrape missing enqueued status label. Output:\n{scrape}"
    );
    assert!(
        scrape.contains("chv_architecture_apply_duration_seconds"),
        "metrics scrape missing duration histogram. Output:\n{scrape}"
    );
}
