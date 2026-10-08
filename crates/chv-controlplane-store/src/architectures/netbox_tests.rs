//! NetBox projection repository tests (config + runs). Uses the in-memory
//! SQLite test pool, mirroring `architectures/tests.rs`.
//!
//! Token-redaction note: [`NetboxProjectionConfig`] has no token field by
//! construction (a compile-level property — there is nothing to assert at
//! runtime), so `upsert`/`get` cannot leak token material; only
//! `read_token` ever decrypts.

use crate::architectures::*;
use crate::test_util::TestDb;
use crate::StoreError;
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitectureVersionId, NetboxProjectionMode, NetboxProjectionRunId,
    NetboxProjectionRunStatus, NetboxProjectionTrigger, NetboxRetentionPolicy,
};

fn aid(s: &str) -> ArchitectureId {
    ArchitectureId::new(s).unwrap()
}

fn vid(s: &str) -> ArchitectureVersionId {
    ArchitectureVersionId::new(s).unwrap()
}

fn nid(s: &str) -> NetboxProjectionRunId {
    NetboxProjectionRunId::new(s).unwrap()
}

fn make_topology_input(id: &str, name: &str) -> TopologyCreateInput {
    TopologyCreateInput {
        id: aid(id),
        name: name.to_string(),
        display_name: Some(format!("{name} display")),
        description: None,
        environment: Some("test".to_string()),
        status: chv_controlplane_types::architecture::ArchitectureStatus::Draft,
        owner_user_id: Some("user-1".to_string()),
        design_graph_json: None,
        latest_yaml: None,
    }
}

/// Creates a topology + version pair so config/run rows have valid FKs.
async fn setup_architecture(db: &TestDb, topo_id: &str, version_id: &str) {
    TopologyRepository::new(db.pool.clone())
        .create(make_topology_input(topo_id, &format!("{topo_id}-name")))
        .await
        .unwrap();
    VersionRepository::new(db.pool.clone())
        .create(VersionCreateInput {
            id: vid(version_id),
            architecture_id: aid(topo_id),
            version_number: 1,
            yaml_content: "x".to_string(),
            design_graph_json: None,
            normalized_model_json: None,
            change_summary: None,
            created_by: None,
        })
        .await
        .unwrap();
}

fn make_config_input(topo_id: &str) -> NetboxProjectionConfigUpsertInput {
    NetboxProjectionConfigUpsertInput {
        architecture_id: aid(topo_id),
        endpoint: "https://netbox.example.internal".to_string(),
        token: "netbox-api-token-plaintext".to_string(),
        token_secret_ref: format!("netbox-{topo_id}"),
        retention_policy: NetboxRetentionPolicy::MarkStale,
        enable_post_apply: false,
        custom_field_prefix: "chv_".to_string(),
        site_name: Some("dc1".to_string()),
    }
}

fn make_run_input(run_id: &str, topo_id: &str, version_id: &str) -> NetboxProjectionRunCreateInput {
    NetboxProjectionRunCreateInput {
        id: nid(run_id),
        architecture_id: aid(topo_id),
        architecture_version_id: vid(version_id),
        trigger_kind: NetboxProjectionTrigger::Manual,
        mode: NetboxProjectionMode::Export,
        requested_by: Some("senol".to_string()),
    }
}

// ── NetboxProjectionConfigRepository ───────────────────────────────────────

#[tokio::test]
async fn netbox_config_upsert_get_roundtrip() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    // The upsert input always carries explicit values (SQL defaults like
    // retention 'mark_stale' only apply to raw inserts), so the roundtrip
    // must persist exactly what was provided.
    let created = repo.upsert(make_config_input("topo-1")).await.unwrap();
    assert_eq!(created.architecture_id, aid("topo-1"));
    assert_eq!(created.endpoint, "https://netbox.example.internal");
    assert_eq!(created.token_secret_ref, "netbox-topo-1");
    assert_eq!(created.retention_policy, NetboxRetentionPolicy::MarkStale);
    assert!(!created.enable_post_apply);
    assert_eq!(created.custom_field_prefix, "chv_");
    assert_eq!(created.site_name.as_deref(), Some("dc1"));
    assert!(created.created_at <= created.updated_at);

    let fetched = repo.get(&aid("topo-1")).await.unwrap().expect("config row");
    assert_eq!(fetched, created);
}

#[tokio::test]
async fn netbox_config_upsert_is_idempotent_and_refreshes_updated_at() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    repo.upsert(make_config_input("topo-1")).await.unwrap();

    // Timestamps have second resolution; backdate the row so the refresh
    // is observable without sleeping.
    sqlx::query(
        "UPDATE netbox_projection_config SET updated_at = '2020-01-01T00:00:00Z' \
         WHERE architecture_id = $1",
    )
    .bind("topo-1")
    .execute(&db.pool)
    .await
    .unwrap();

    let mut input = make_config_input("topo-1");
    input.endpoint = "https://netbox2.example.internal".to_string();
    input.retention_policy = NetboxRetentionPolicy::Delete;
    input.enable_post_apply = true;
    input.site_name = None;

    let updated = repo.upsert(input).await.unwrap();
    assert_eq!(updated.endpoint, "https://netbox2.example.internal");
    assert_eq!(updated.retention_policy, NetboxRetentionPolicy::Delete);
    assert!(updated.enable_post_apply);
    assert_eq!(updated.site_name, None);
    assert!(updated.updated_at.timestamp() > 1_577_836_800); // 2020-01-01
                                                             // created_at is not touched by the update path.
    assert_eq!(
        updated.created_at,
        repo.get(&aid("topo-1")).await.unwrap().unwrap().created_at
    );
}

#[tokio::test]
async fn netbox_config_upsert_unknown_topology_returns_not_found() {
    let db = TestDb::new().await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    let err = repo.upsert(make_config_input("missing")).await.unwrap_err();
    assert!(
        matches!(&err, StoreError::NotFound { entity, id }
            if *entity == "architecture_topology_or_version" && id == "missing"),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn netbox_config_get_unknown_returns_none() {
    let db = TestDb::new().await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    assert!(repo.get(&aid("missing")).await.unwrap().is_none());
}

#[tokio::test]
async fn netbox_config_delete_returns_true_then_false() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    repo.upsert(make_config_input("topo-1")).await.unwrap();

    assert!(repo.delete(&aid("topo-1")).await.unwrap());
    assert!(!repo.delete(&aid("topo-1")).await.unwrap());
    assert!(repo.get(&aid("topo-1")).await.unwrap().is_none());
    assert!(repo.read_token(&aid("topo-1")).await.unwrap().is_none());
}

#[tokio::test]
async fn netbox_config_token_roundtrip() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    let mut input = make_config_input("topo-1");
    input.token = "PAbCd-super-secret-token".to_string();
    repo.upsert(input).await.unwrap();

    // The only decrypt path returns exactly the plaintext that was
    // encrypted at write time. (Whether encryption is active depends on
    // the ambient CHV_ENCRYPTION_KEY; the same repo instance performs
    // both directions, so the roundtrip holds either way.)
    let token = repo.read_token(&aid("topo-1")).await.unwrap();
    assert_eq!(token.as_deref(), Some("PAbCd-super-secret-token"));
}

#[tokio::test]
async fn netbox_config_token_decrypt_failure_is_fail_closed() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionConfigRepository::new(db.pool.clone());

    repo.upsert(make_config_input("topo-1")).await.unwrap();

    // Corrupt the ciphertext out-of-band. The `enc:` prefix forces the
    // decrypt path: with a key configured it fails as Malformed/
    // AuthFailed, without one as KeyUnavailable — never a passthrough.
    sqlx::query(
        "UPDATE netbox_projection_config SET token_ciphertext = 'enc:garbage' \
         WHERE architecture_id = $1",
    )
    .bind("topo-1")
    .execute(&db.pool)
    .await
    .unwrap();

    let err = repo.read_token(&aid("topo-1")).await.unwrap_err();
    assert!(
        matches!(err, StoreError::InvalidConfiguration { .. }),
        "expected InvalidConfiguration (fail-closed), got {err:?}"
    );
}

// ── NetboxProjectionRunRepository ──────────────────────────────────────────

#[tokio::test]
async fn netbox_run_create_get_roundtrip() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    let run = repo
        .create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();
    assert_eq!(run.id, nid("netrun-1"));
    assert_eq!(run.architecture_id, aid("topo-1"));
    assert_eq!(run.architecture_version_id, vid("v-1"));
    assert_eq!(run.trigger_kind, NetboxProjectionTrigger::Manual);
    assert_eq!(run.mode, NetboxProjectionMode::Export);
    assert_eq!(run.requested_by.as_deref(), Some("senol"));
    // Create defaults: queued, no attempt yet, no timestamps.
    assert_eq!(run.status, NetboxProjectionRunStatus::Queued);
    assert_eq!(run.attempt_count, 0);
    assert!(run.started_at.is_none());
    assert!(run.finished_at.is_none());
    assert!(run.plan_json.is_none());
    assert!(run.result_json.is_none());
    assert!(run.summary_json.is_none());
    assert!(run.error_message.is_none());

    let fetched = repo.get(&nid("netrun-1")).await.unwrap().expect("run row");
    assert_eq!(fetched, run);
}

#[tokio::test]
async fn netbox_run_get_unknown_returns_none() {
    let db = TestDb::new().await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());
    assert!(repo.get(&nid("missing")).await.unwrap().is_none());
}

#[tokio::test]
async fn netbox_run_create_unknown_fk_returns_not_found() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    // Unknown topology.
    let err = repo
        .create(make_run_input("netrun-1", "missing", "v-1"))
        .await
        .unwrap_err();
    assert!(
        matches!(&err, StoreError::NotFound { entity, id }
            if *entity == "architecture_topology_or_version" && id == "missing"),
        "expected NotFound, got {err:?}"
    );

    // Known topology, unknown version.
    let err = repo
        .create(make_run_input("netrun-2", "topo-1", "missing-v"))
        .await
        .unwrap_err();
    assert!(
        matches!(&err, StoreError::NotFound { entity, .. }
            if *entity == "architecture_topology_or_version"),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn netbox_run_one_active_invariant() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    repo.create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();

    // A second queued run for the same architecture must Conflict —
    // surfaced as 409 NETBOX_RUN_ACTIVE at the BFF.
    let err = repo
        .create(make_run_input("netrun-2", "topo-1", "v-1"))
        .await
        .unwrap_err();
    match err {
        StoreError::Conflict { entity, reason, .. } => {
            assert_eq!(entity, "netbox_projection_run");
            assert!(
                reason.contains("active run already exists"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("expected Conflict, got {other:?}"),
    }

    // Drain the active run to terminal state, then a new create succeeds.
    let claimed = repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .unwrap();
    repo.mark_succeeded(&claimed.id, Some("{}".into()), Some("{}".into()))
        .await
        .unwrap();
    repo.create(make_run_input("netrun-3", "topo-1", "v-1"))
        .await
        .unwrap();
}

#[tokio::test]
async fn netbox_run_claim_next_queued() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    // Nothing queued → None.
    assert!(repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .is_none());
    // Other architectures are not affected.
    setup_architecture(&db, "topo-2", "v-2").await;
    repo.create(make_run_input("netrun-2", "topo-2", "v-2"))
        .await
        .unwrap();
    assert!(repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .is_none());

    repo.create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();

    let claimed = repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .expect("queued run claimed");
    assert_eq!(claimed.id, nid("netrun-1"));
    assert_eq!(claimed.status, NetboxProjectionRunStatus::Running);
    assert!(claimed.started_at.is_some());

    // The single-statement claim means a second claim finds nothing
    // queued (sequential simulation of concurrent workers).
    assert!(repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn netbox_run_mark_succeeded_from_running() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    repo.create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();
    let claimed = repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .unwrap();

    let done = repo
        .mark_succeeded(
            &claimed.id,
            Some("{\"outcomes\":[]}".to_string()),
            Some("{\"create\":0}".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(done.status, NetboxProjectionRunStatus::Succeeded);
    assert!(done.finished_at.is_some());
    assert_eq!(done.result_json.as_deref(), Some("{\"outcomes\":[]}"));
    assert_eq!(done.summary_json.as_deref(), Some("{\"create\":0}"));

    // A stale worker (still holding the pre-terminal view) cannot
    // overwrite the terminal state.
    let err = repo
        .mark_succeeded(&claimed.id, Some("{\"evil\":true}".into()), None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::Conflict { entity, .. } if entity == "netbox_projection_run"),
        "expected Conflict, got {err:?}"
    );
    // The stale write did not land.
    let after = repo.get(&claimed.id).await.unwrap().unwrap();
    assert_eq!(after.result_json.as_deref(), Some("{\"outcomes\":[]}"));
}

#[tokio::test]
async fn netbox_run_mark_failed_increments_attempts() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    repo.create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();
    let claimed = repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .unwrap();

    let failed = repo
        .mark_failed(&claimed.id, Some("netbox unreachable".to_string()))
        .await
        .unwrap();
    assert_eq!(failed.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(failed.attempt_count, 1);
    assert!(failed.finished_at.is_some());
    assert_eq!(failed.error_message.as_deref(), Some("netbox unreachable"));

    // CAS guard: a terminal state cannot be re-failed.
    let err = repo
        .mark_failed(&claimed.id, Some("stale".to_string()))
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::Conflict { .. }),
        "expected Conflict, got {err:?}"
    );
}

#[tokio::test]
async fn netbox_run_requeue_from_failed() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    repo.create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();
    let claimed = repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .unwrap();
    let failed = repo
        .mark_failed(&claimed.id, Some("netbox unreachable".to_string()))
        .await
        .unwrap();
    assert_eq!(failed.attempt_count, 1);

    let requeued = repo.requeue(&claimed.id).await.unwrap();
    assert_eq!(requeued.status, NetboxProjectionRunStatus::Queued);
    assert_eq!(requeued.attempt_count, 1, "attempt_count is preserved");
    assert!(requeued.finished_at.is_none(), "finished_at is cleared");

    // Requeue from a non-failed state must Conflict.
    let claimed_again = repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .unwrap()
        .unwrap();
    let err = repo.requeue(&claimed_again.id).await.unwrap_err();
    assert!(
        matches!(err, StoreError::Conflict { .. }),
        "expected Conflict requeueing a running run, got {err:?}"
    );
    repo.mark_succeeded(&claimed_again.id, None, None)
        .await
        .unwrap();
    let err = repo.requeue(&claimed_again.id).await.unwrap_err();
    assert!(
        matches!(err, StoreError::Conflict { .. }),
        "expected Conflict requeueing a succeeded run, got {err:?}"
    );
}

#[tokio::test]
async fn netbox_run_requeue_at_attempt_cap_conflicts() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    repo.create(make_run_input("netrun-1", "topo-1", "v-1"))
        .await
        .unwrap();

    // Burn through all MAX_ATTEMPTS attempts (each mark_failed +1, each
    // requeue keeps the count).
    for expected_attempt in 1..=MAX_ATTEMPTS {
        let claimed = repo
            .claim_next_queued(&aid("topo-1"))
            .await
            .unwrap()
            .unwrap();
        let failed = repo
            .mark_failed(&claimed.id, Some("netbox unreachable".to_string()))
            .await
            .unwrap();
        assert_eq!(failed.attempt_count, expected_attempt);
        if expected_attempt < MAX_ATTEMPTS {
            repo.requeue(&claimed.id).await.unwrap();
        }
    }

    let err = repo.requeue(&nid("netrun-1")).await.unwrap_err();
    match err {
        StoreError::Conflict { reason, .. } => {
            assert!(reason.contains("exhausted"), "unexpected reason: {reason}");
        }
        other => panic!("expected Conflict at the attempt cap, got {other:?}"),
    }

    // The exhausted run stays failed for operator inspection — and no
    // longer blocks new runs.
    let exhausted = repo.get(&nid("netrun-1")).await.unwrap().unwrap();
    assert_eq!(exhausted.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(exhausted.attempt_count, MAX_ATTEMPTS);
    repo.create(make_run_input("netrun-2", "topo-1", "v-1"))
        .await
        .unwrap();
}

#[tokio::test]
async fn netbox_run_list_by_architecture_ordering_and_limit() {
    let db = TestDb::new().await;
    setup_architecture(&db, "topo-1", "v-1").await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    // created_at has second resolution; cycle runs to terminal states and
    // backdate the older rows so the DESC ordering is deterministic.
    for (i, run_id) in ["netrun-1", "netrun-2", "netrun-3"].iter().enumerate() {
        repo.create(make_run_input(run_id, "topo-1", "v-1"))
            .await
            .unwrap();
        let claimed = repo
            .claim_next_queued(&aid("topo-1"))
            .await
            .unwrap()
            .unwrap();
        repo.mark_succeeded(&claimed.id, None, None).await.unwrap();

        sqlx::query("UPDATE netbox_projection_runs SET created_at = $2 WHERE id = $1")
            .bind(run_id)
            .bind(format!("2026-01-0{}T00:00:00Z", i + 1))
            .execute(&db.pool)
            .await
            .unwrap();
    }

    let all = repo.list_by_architecture(&aid("topo-1"), 10).await.unwrap();
    assert_eq!(all.len(), 3);
    // Newest first.
    assert_eq!(all[0].id, nid("netrun-3"));
    assert_eq!(all[1].id, nid("netrun-2"));
    assert_eq!(all[2].id, nid("netrun-1"));

    let limited = repo.list_by_architecture(&aid("topo-1"), 2).await.unwrap();
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].id, nid("netrun-3"));

    // Other architectures see nothing.
    setup_architecture(&db, "topo-2", "v-2").await;
    assert!(repo
        .list_by_architecture(&aid("topo-2"), 10)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn netbox_run_terminal_transition_on_missing_run_is_not_found() {
    let db = TestDb::new().await;
    let repo = NetboxProjectionRunRepository::new(db.pool.clone());

    let err = repo
        .mark_succeeded(&nid("missing"), None, None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::NotFound { entity, .. }
            if entity == "netbox_projection_run"),
        "expected NotFound, got {err:?}"
    );
    let err = repo.requeue(&nid("missing")).await.unwrap_err();
    assert!(matches!(err, StoreError::NotFound { .. }), "got {err:?}");
}
