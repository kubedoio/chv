//! Architecture repository tests. Uses the in-memory SQLite test pool.

use crate::architectures::*;
use crate::test_util::TestDb;
use crate::StoreError;
use chrono::{Duration, Utc};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitecturePlanId, ArchitectureStatus, ArchitectureVersionId,
    DriftStatus, FleetCheckStatus, InventorySnapshotId, PlanMode, PlanStatus, RunStatus,
    ValidationStatus,
};

fn aid(s: &str) -> ArchitectureId {
    ArchitectureId::new(s).unwrap()
}

fn vid(s: &str) -> ArchitectureVersionId {
    ArchitectureVersionId::new(s).unwrap()
}

fn pid(s: &str) -> ArchitecturePlanId {
    ArchitecturePlanId::new(s).unwrap()
}

fn sid(s: &str) -> InventorySnapshotId {
    InventorySnapshotId::new(s).unwrap()
}

fn make_topology_input(id: &str, name: &str) -> TopologyCreateInput {
    TopologyCreateInput {
        id: aid(id),
        name: name.to_string(),
        display_name: Some(format!("{name} display")),
        description: None,
        environment: Some("test".to_string()),
        status: ArchitectureStatus::Draft,
        owner_user_id: Some("user-1".to_string()),
        design_graph_json: None,
        latest_yaml: None,
    }
}

// ── TopologyRepository ─────────────────────────────────────────────────────

#[tokio::test]
async fn topology_create_get_roundtrip() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    let created = repo
        .create(make_topology_input("topo-1", "alpha"))
        .await
        .expect("create");
    assert_eq!(created.name, "alpha");
    assert_eq!(created.version_number, 1);
    assert_eq!(created.status, ArchitectureStatus::Draft);

    let fetched = repo.get(&aid("topo-1")).await.expect("get");
    assert_eq!(fetched, created);
}

#[tokio::test]
async fn topology_get_not_found() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    let err = repo.get(&aid("missing")).await.unwrap_err();
    assert!(
        matches!(&err, StoreError::NotFound { entity, id } if *entity == "architecture_topology" && id == "missing"),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn topology_list_excludes_archived_by_default() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    repo.create(make_topology_input("topo-a", "alpha"))
        .await
        .unwrap();
    repo.create(make_topology_input("topo-b", "beta"))
        .await
        .unwrap();
    repo.archive(&aid("topo-a")).await.unwrap();

    let active = repo
        .list(TopologyListFilter::default())
        .await
        .expect("list active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id.as_str(), "topo-b");

    let all = repo
        .list(TopologyListFilter {
            include_archived: true,
        })
        .await
        .expect("list all");
    assert_eq!(all.len(), 2);
    let ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"topo-a"));
    assert!(ids.contains(&"topo-b"));
}

#[tokio::test]
async fn topology_archive_then_archive_again_is_not_found() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    repo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();
    repo.archive(&aid("topo-1")).await.expect("first archive");

    let err = repo.archive(&aid("topo-1")).await.unwrap_err();
    assert!(matches!(err, StoreError::NotFound { .. }));
}

#[tokio::test]
async fn topology_update_happy_path_bumps_version() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    let created = repo
        .create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();
    assert_eq!(created.version_number, 1);

    let updated = repo
        .update(TopologyUpdateInput {
            id: aid("topo-1"),
            expected_version: 1,
            display_name: Some("alpha v2".to_string()),
            description: None,
            environment: None,
            status: Some(ArchitectureStatus::Valid),
            design_graph_json: Some("{\"nodes\":[]}".to_string()),
            latest_yaml: None,
            latest_version_id: None,
            last_validation_status: Some(ValidationStatus::Passed),
            last_fleet_check_status: Some(FleetCheckStatus::Unknown),
        })
        .await
        .expect("update");
    assert_eq!(updated.version_number, 2);
    assert_eq!(updated.display_name.as_deref(), Some("alpha v2"));
    assert_eq!(updated.status, ArchitectureStatus::Valid);
}

#[tokio::test]
async fn topology_update_with_stale_version_returns_stale_version() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    repo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();

    // First update succeeds and bumps to version 2.
    repo.update(TopologyUpdateInput {
        id: aid("topo-1"),
        expected_version: 1,
        display_name: Some("v2".to_string()),
        description: None,
        environment: None,
        status: None,
        design_graph_json: None,
        latest_yaml: None,
        latest_version_id: None,
        last_validation_status: None,
        last_fleet_check_status: None,
    })
    .await
    .unwrap();

    // Second update with stale expected_version=1 must fail.
    let err = repo
        .update(TopologyUpdateInput {
            id: aid("topo-1"),
            expected_version: 1,
            display_name: Some("v3".to_string()),
            description: None,
            environment: None,
            status: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
            last_validation_status: None,
            last_fleet_check_status: None,
        })
        .await
        .unwrap_err();

    match err {
        StoreError::StaleVersion {
            current, expected, ..
        } => {
            assert_eq!(current, 2);
            assert_eq!(expected, 1);
        }
        other => panic!("expected StaleVersion, got {other:?}"),
    }
}

#[tokio::test]
async fn topology_update_missing_returns_not_found() {
    let db = TestDb::new().await;
    let repo = TopologyRepository::new(db.pool.clone());

    let err = repo
        .update(TopologyUpdateInput {
            id: aid("missing"),
            expected_version: 1,
            display_name: None,
            description: None,
            environment: None,
            status: None,
            design_graph_json: None,
            latest_yaml: None,
            latest_version_id: None,
            last_validation_status: None,
            last_fleet_check_status: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::NotFound { .. }), "got {err:?}");
}

// ── VersionRepository ──────────────────────────────────────────────────────

#[tokio::test]
async fn version_create_get_list() {
    let db = TestDb::new().await;
    let topo = TopologyRepository::new(db.pool.clone());
    let repo = VersionRepository::new(db.pool.clone());

    topo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();

    let v1 = repo
        .create(VersionCreateInput {
            id: vid("v-1"),
            architecture_id: aid("topo-1"),
            version_number: 1,
            yaml_content: "apiVersion: chv.kubedo.io/v1alpha1\n".to_string(),
            design_graph_json: None,
            normalized_model_json: None,
            change_summary: Some("initial".to_string()),
            created_by: Some("senol".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(v1.version_number, 1);

    let _v2 = repo
        .create(VersionCreateInput {
            id: vid("v-2"),
            architecture_id: aid("topo-1"),
            version_number: 2,
            yaml_content: "apiVersion: chv.kubedo.io/v1alpha1\nfoo: bar\n".to_string(),
            design_graph_json: None,
            normalized_model_json: None,
            change_summary: Some("add foo".to_string()),
            created_by: None,
        })
        .await
        .unwrap();

    let fetched = repo.get(&vid("v-1")).await.unwrap();
    assert_eq!(fetched, v1);

    let list = repo.list_for_architecture(&aid("topo-1")).await.unwrap();
    assert_eq!(list.len(), 2);
    // Ordered DESC by version_number.
    assert_eq!(list[0].version_number, 2);
    assert_eq!(list[1].version_number, 1);
}

#[tokio::test]
async fn version_create_with_unknown_topology_returns_not_found() {
    let db = TestDb::new().await;
    let repo = VersionRepository::new(db.pool.clone());

    let err = repo
        .create(VersionCreateInput {
            id: vid("v-1"),
            architecture_id: aid("nope"),
            version_number: 1,
            yaml_content: "x".to_string(),
            design_graph_json: None,
            normalized_model_json: None,
            change_summary: None,
            created_by: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::NotFound { .. }), "got {err:?}");
}

#[tokio::test]
async fn version_cascades_when_topology_hard_deleted() {
    // Topology's own API is soft-delete-only, so this test exercises the FK
    // cascade declared on the migration by going through raw SQL, mirroring
    // the path a future hard-delete maintenance job would take.
    let db = TestDb::new().await;
    let topo = TopologyRepository::new(db.pool.clone());
    let repo = VersionRepository::new(db.pool.clone());

    topo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();

    repo.create(VersionCreateInput {
        id: vid("v-1"),
        architecture_id: aid("topo-1"),
        version_number: 1,
        yaml_content: "x".to_string(),
        design_graph_json: None,
        normalized_model_json: None,
        change_summary: None,
        created_by: None,
    })
    .await
    .unwrap();

    // SQLx pools start with foreign_keys ON via build_connect_options, but
    // the test harness uses bare SqlitePool::connect — re-enable it
    // explicitly for this test path.
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&db.pool)
        .await
        .unwrap();

    sqlx::query("DELETE FROM architecture_topologies WHERE id = $1")
        .bind("topo-1")
        .execute(&db.pool)
        .await
        .unwrap();

    let err = repo.get(&vid("v-1")).await.unwrap_err();
    assert!(
        matches!(err, StoreError::NotFound { .. }),
        "expected version to cascade, got {err:?}"
    );
}

// ── PlanRepository ─────────────────────────────────────────────────────────

#[tokio::test]
async fn plan_create_get_list() {
    let db = TestDb::new().await;
    let topo = TopologyRepository::new(db.pool.clone());
    let ver = VersionRepository::new(db.pool.clone());
    let repo = PlanRepository::new(db.pool.clone());

    topo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();
    ver.create(VersionCreateInput {
        id: vid("v-1"),
        architecture_id: aid("topo-1"),
        version_number: 1,
        yaml_content: "x".to_string(),
        design_graph_json: None,
        normalized_model_json: None,
        change_summary: None,
        created_by: None,
    })
    .await
    .unwrap();

    let now = Utc::now();
    let plan = repo
        .create(PlanCreateInput {
            id: pid("plan-1"),
            architecture_id: aid("topo-1"),
            architecture_version_id: vid("v-1"),
            inventory_snapshot_id: None,
            mode: PlanMode::DryRun,
            status: PlanStatus::Draft,
            plan_json: Some("{\"changes\":[]}".to_string()),
            summary_json: None,
            created_by: Some("senol".to_string()),
            expires_at: now + Duration::minutes(15),
        })
        .await
        .unwrap();
    assert_eq!(plan.mode, PlanMode::DryRun);

    let fetched = repo.get(&pid("plan-1")).await.unwrap();
    assert_eq!(fetched, plan);

    let list = repo.list_for_architecture(&aid("topo-1")).await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn plan_update_status_marks_confirmed() {
    let db = TestDb::new().await;
    let topo = TopologyRepository::new(db.pool.clone());
    let ver = VersionRepository::new(db.pool.clone());
    let repo = PlanRepository::new(db.pool.clone());

    topo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();
    ver.create(VersionCreateInput {
        id: vid("v-1"),
        architecture_id: aid("topo-1"),
        version_number: 1,
        yaml_content: "x".to_string(),
        design_graph_json: None,
        normalized_model_json: None,
        change_summary: None,
        created_by: None,
    })
    .await
    .unwrap();
    repo.create(PlanCreateInput {
        id: pid("plan-1"),
        architecture_id: aid("topo-1"),
        architecture_version_id: vid("v-1"),
        inventory_snapshot_id: None,
        mode: PlanMode::Confirm,
        status: PlanStatus::RequiresConfirmation,
        plan_json: None,
        summary_json: None,
        created_by: None,
        expires_at: Utc::now() + Duration::minutes(15),
    })
    .await
    .unwrap();

    let updated = repo
        .update_status(PlanStatusUpdateInput {
            id: pid("plan-1"),
            status: PlanStatus::Applying,
            confirmed_by: Some("senol".to_string()),
            mark_confirmed: true,
            mark_discarded: false,
        })
        .await
        .unwrap();
    assert_eq!(updated.status, PlanStatus::Applying);
    assert_eq!(updated.confirmed_by.as_deref(), Some("senol"));
    assert!(updated.confirmed_at.is_some());
}

#[tokio::test]
async fn plan_generate_returns_not_implemented() {
    let db = TestDb::new().await;
    let repo = PlanRepository::new(db.pool.clone());

    let err = repo.generate().await.unwrap_err();
    assert!(matches!(err, StoreError::NotImplemented { .. }));
}

// ── ApplyRunRepository ─────────────────────────────────────────────────────

#[tokio::test]
async fn apply_run_create_get_list_update() {
    let db = TestDb::new().await;
    let topo = TopologyRepository::new(db.pool.clone());
    let ver = VersionRepository::new(db.pool.clone());
    let repo = ApplyRunRepository::new(db.pool.clone());

    topo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();
    ver.create(VersionCreateInput {
        id: vid("v-1"),
        architecture_id: aid("topo-1"),
        version_number: 1,
        yaml_content: "x".to_string(),
        design_graph_json: None,
        normalized_model_json: None,
        change_summary: None,
        created_by: None,
    })
    .await
    .unwrap();

    let run = repo
        .create(ApplyRunCreateInput {
            id: ArchitectureApplyRunIdNew("run-1"),
            architecture_id: aid("topo-1"),
            architecture_version_id: vid("v-1"),
            plan_id: None,
            task_id: None,
            status: RunStatus::Queued,
            requested_by: Some("senol".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Queued);

    let updated = repo
        .update(ApplyRunUpdateInput {
            id: ArchitectureApplyRunIdNew("run-1"),
            status: Some(RunStatus::Running),
            started_at: Some(Utc::now()),
            finished_at: None,
            task_id: Some("task-abc".to_string()),
            result_json: None,
            logs_ref: None,
            error_message: None,
        })
        .await
        .unwrap();
    assert_eq!(updated.status, RunStatus::Running);
    assert_eq!(updated.task_id.as_deref(), Some("task-abc"));

    let fetched = repo.get(&ArchitectureApplyRunIdNew("run-1")).await.unwrap();
    assert_eq!(fetched, updated);

    let list = repo.list_for_architecture(&aid("topo-1")).await.unwrap();
    assert_eq!(list.len(), 1);
}

// Helper because we don't want to repeatedly type the long path; ditto the
// matching helper for drift IDs below.
#[allow(non_snake_case)]
fn ArchitectureApplyRunIdNew(s: &str) -> chv_controlplane_types::architecture::ArchitectureApplyRunId {
    chv_controlplane_types::architecture::ArchitectureApplyRunId::new(s).unwrap()
}

#[allow(non_snake_case)]
fn ArchitectureDriftReportIdNew(s: &str) -> chv_controlplane_types::architecture::ArchitectureDriftReportId {
    chv_controlplane_types::architecture::ArchitectureDriftReportId::new(s).unwrap()
}

// ── DriftReportRepository ──────────────────────────────────────────────────

#[tokio::test]
async fn drift_report_create_get_list() {
    let db = TestDb::new().await;
    let topo = TopologyRepository::new(db.pool.clone());
    let ver = VersionRepository::new(db.pool.clone());
    let repo = DriftReportRepository::new(db.pool.clone());

    topo.create(make_topology_input("topo-1", "alpha"))
        .await
        .unwrap();
    ver.create(VersionCreateInput {
        id: vid("v-1"),
        architecture_id: aid("topo-1"),
        version_number: 1,
        yaml_content: "x".to_string(),
        design_graph_json: None,
        normalized_model_json: None,
        change_summary: None,
        created_by: None,
    })
    .await
    .unwrap();

    let drift = repo
        .create(DriftReportCreateInput {
            id: ArchitectureDriftReportIdNew("drift-1"),
            architecture_id: aid("topo-1"),
            baseline_version_id: vid("v-1"),
            inventory_snapshot_id: None,
            status: DriftStatus::NoDrift,
            summary_json: None,
            findings_json: None,
        })
        .await
        .unwrap();
    assert_eq!(drift.status, DriftStatus::NoDrift);

    let fetched = repo
        .get(&ArchitectureDriftReportIdNew("drift-1"))
        .await
        .unwrap();
    assert_eq!(fetched, drift);

    let list = repo.list_for_architecture(&aid("topo-1")).await.unwrap();
    assert_eq!(list.len(), 1);
}

// ── InventorySnapshotRepository ────────────────────────────────────────────

#[tokio::test]
async fn inventory_snapshot_create_get_list_recent() {
    let db = TestDb::new().await;
    let repo = InventorySnapshotRepository::new(db.pool.clone());

    let snap = repo
        .create(InventorySnapshotCreateInput {
            id: sid("snap-1"),
            source: "node-agent".to_string(),
            snapshot_json: "{\"nodes\":[]}".to_string(),
            summary_json: None,
            captured_by: Some("senol".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(snap.source, "node-agent");

    let fetched = repo.get(&sid("snap-1")).await.unwrap();
    assert_eq!(fetched, snap);

    let list = repo.list_recent(10).await.unwrap();
    assert_eq!(list.len(), 1);

    let err = repo.capture_from_fleet().await.unwrap_err();
    assert!(matches!(err, StoreError::NotImplemented { .. }));
}
