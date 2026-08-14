use crate::*;
use chv_controlplane_types::domain::{Generation, NodeId, ResourceId};

// NOTE: These tests use an ephemeral Postgres instance via testcontainers.
// No manual setup is required.

use crate::test_util::TestDb;

#[tokio::test]
async fn test_bootstrap_token_validation() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = BootstrapTokenRepository::new(pool.clone());

    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"; // sha256("123")
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use) VALUES ($1, true)")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        repo.validate_and_consume("123").await.unwrap(),
        BootstrapTokenValidation::Valid
    );
    assert_eq!(
        repo.validate_and_consume("123").await.unwrap(),
        BootstrapTokenValidation::AlreadyUsed
    );
    assert_eq!(
        repo.validate_and_consume("999").await.unwrap(),
        BootstrapTokenValidation::Invalid
    );
}

#[tokio::test]
async fn test_expired_bootstrap_token() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = BootstrapTokenRepository::new(pool.clone());

    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"; // sha256("123")
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use, expires_at) VALUES ($1, true, strftime('%Y-%m-%dT%H:%M:%SZ', strftime('%s','now') - 3600, 'unixepoch'))")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        repo.validate_and_consume("123").await.unwrap(),
        BootstrapTokenValidation::Expired
    );
}

#[tokio::test]
async fn test_reusable_bootstrap_token() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = BootstrapTokenRepository::new(pool.clone());

    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"; // sha256("123")
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use) VALUES ($1, false)")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        repo.validate_and_consume("123").await.unwrap(),
        BootstrapTokenValidation::Valid
    );
    assert_eq!(
        repo.validate_and_consume("123").await.unwrap(),
        BootstrapTokenValidation::Valid
    );
}

#[tokio::test]
async fn test_bootstrap_result_repeatable_upsert() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = NodeRepository::new(pool);
    let node_id = NodeId::new("test-node-bootstrap").unwrap();

    // Ensure node exists
    repo.upsert_node(&NodeUpsertInput {
        node_id: node_id.clone(),
        hostname: "test-host".into(),
        display_name: "test-host".into(),
        certificate_serial: None,
        agent_version: None,
        control_plane_version: None,
        enrolled_unix_ms: 0,
        last_seen_unix_ms: 0,
    })
    .await
    .unwrap();

    let input = NodeBootstrapResultInput {
        node_id: node_id.clone(),
        operation_id: Some("op-1".into()),
        success: true,
        error_message: None,
        details: None,
        started_unix_ms: Some(1000),
        completed_unix_ms: 2000,
    };

    // First write
    repo.upsert_bootstrap_result(&input)
        .await
        .expect("First write failed");

    // Second write (updates existing row via ON CONFLICT)
    repo.upsert_bootstrap_result(&input).await.expect(
        "Second write failed - should have succeeded with ON CONFLICT and updated_at now()",
    );
}

#[tokio::test]
async fn test_telemetry_no_fabrication() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = ObservedStateRepository::new(pool);
    let vm_id = ResourceId::new("non-existent-vm").unwrap();

    let input = VmObservedStateInput {
        vm_id: vm_id.clone(),
        observed_generation: Generation::new(1),
        runtime_status: "running".into(),
        health_status: None,
        node_id: None,
        cloud_hypervisor_pid: None,
        api_socket_path: None,
        last_error: None,
        last_transition_unix_ms: None,
        observed_unix_ms: 1000,
    };

    let result = repo.upsert_vm(&input).await;

    // Should fail with NotFound, not create a skeleton row
    match result {
        Err(StoreError::NotFound { entity, id }) => {
            assert_eq!(entity, "vm");
            assert_eq!(id, vm_id.to_string());
        }
        other => panic!("Expected NotFound error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_update_certificate_serial_missing_node() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = NodeRepository::new(pool);
    let node_id = NodeId::new("non-existent-node").unwrap();

    let result = repo.update_certificate_serial(&node_id, "serial-123").await;

    match result {
        Err(StoreError::NotFound { entity, id }) => {
            assert_eq!(entity, "node");
            assert_eq!(id, node_id.to_string());
        }
        other => panic!("Expected NotFound error for missing node, got {:?}", other),
    }
}
#[tokio::test]
async fn test_telemetry_missing_parent_node() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = ObservedStateRepository::new(pool.clone());
    let vm_id = ResourceId::new("vm-missing-node").unwrap();
    let node_id = NodeId::new("non-existent-parent-node").unwrap();

    // Ensure VM exists (or try to create it, but wait, if VM has a node_id FK we can test that)
    // Actually, vms(node_id) REFERENCES nodes(node_id).
    // But vm_observed_state(node_id) ALSO REFERENCES nodes(node_id).

    // First, create the VM record properly (without a node)
    let _node_repo = NodeRepository::new(pool.clone());
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(vm_id.as_str())
        .bind("test-vm")
        .execute(&pool)
        .await
        .unwrap();

    let input = VmObservedStateInput {
        vm_id: vm_id.clone(),
        observed_generation: Generation::new(1),
        runtime_status: "running".into(),
        health_status: None,
        node_id: Some(node_id.clone()), // NON-EXISTENT
        cloud_hypervisor_pid: None,
        api_socket_path: None,
        last_error: None,
        last_transition_unix_ms: None,
        observed_unix_ms: 1000,
    };

    let result = repo.upsert_vm(&input).await;

    match result {
        Err(StoreError::NotFound { entity, id }) => {
            assert_eq!(entity, "node");
            assert_eq!(id, node_id.to_string());
        }
        other => panic!("Expected NotFound(node) error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_telemetry_missing_attached_vm() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = ObservedStateRepository::new(pool.clone());
    let volume_id = ResourceId::new("vol-missing-vm").unwrap();
    let vm_id = ResourceId::new("vm-not-attached").unwrap();

    // Ensure Volume exists
    sqlx::query("INSERT INTO volumes (volume_id, display_name, capacity_bytes) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(volume_id.as_str())
        .bind("test-vol")
        .bind(1024 * 1024 * 1024i64)
        .execute(&pool)
        .await
        .unwrap();

    let input = VolumeObservedStateInput {
        volume_id: volume_id.clone(),
        observed_generation: Generation::new(1),
        runtime_status: "available".into(),
        health_status: None,
        attached_vm_id: Some(vm_id.clone()), // NON-EXISTENT
        device_path: None,
        export_path: None,
        last_transition_unix_ms: None,
        observed_unix_ms: 1000,
    };

    let result = repo.upsert_volume(&input).await;

    match result {
        Err(StoreError::NotFound { entity, id }) => {
            assert_eq!(entity, "vm");
            assert_eq!(id, vm_id.to_string());
        }
        other => panic!(
            "Expected NotFound(vm) error for attached-vm, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_ack_node_generation_preserves_observed_state() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = ObservedStateRepository::new(pool.clone());
    let node_id = NodeId::new("test-node-ack-preserve").unwrap();

    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ($1, 'host', 'host')")
        .bind(node_id.as_str())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO node_observed_state (node_id, observed_generation, observed_state, observed_at, updated_at) VALUES ($1, 1, 'Discovered', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
    )
    .bind(node_id.as_str())
    .execute(&pool)
    .await
    .unwrap();

    repo.acknowledge_node_generation(&node_id, Generation::new(5), 2000)
        .await
        .expect("ack should succeed when observed row exists");

    let row = sqlx::query(
        "SELECT observed_generation, observed_state FROM node_observed_state WHERE node_id = $1",
    )
    .bind(node_id.as_str())
    .fetch_one(&pool)
    .await
    .unwrap();

    let generation: i64 = sqlx::Row::get(&row, "observed_generation");
    let state: String = sqlx::Row::get(&row, "observed_state");
    assert_eq!(generation, 5);
    assert_eq!(state, "Discovered");
}

#[tokio::test]
async fn test_ack_node_generation_rejects_missing_observed_row() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = ObservedStateRepository::new(pool.clone());
    let node_id = NodeId::new("test-node-ack-missing").unwrap();

    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ($1, 'host', 'host')")
        .bind(node_id.as_str())
        .execute(&pool)
        .await
        .unwrap();

    // Seed desired state but no observed state
    sqlx::query(
        "INSERT INTO node_desired_state (node_id, desired_generation, desired_state, requested_at, updated_at, scheduling_paused) VALUES ($1, 1, 'TenantReady', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), false)",
    )
    .bind(node_id.as_str())
    .execute(&pool)
    .await
    .unwrap();

    let result = repo
        .acknowledge_node_generation(&node_id, Generation::new(5), 2000)
        .await;

    match result {
        Err(StoreError::NotFound { entity, id }) => {
            assert_eq!(entity, "node_observed_state");
            assert_eq!(id, node_id.to_string());
        }
        other => panic!(
            "Expected NotFound for missing observed row, got {:?}",
            other
        ),
    }

    // Verify no observed row was fabricated from desired state
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM node_observed_state WHERE node_id = $1")
            .bind(node_id.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_upsert_vm_rejects_stale_generation() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = DesiredStateRepository::new(pool.clone());

    let input = VmDesiredStateInput {
        vm_id: ResourceId::new("vm-gen-test").unwrap(),
        node_id: None,
        display_name: "gen-test".to_string(),
        tenant_id: None,
        placement_policy: None,
        desired_generation: Generation::new(5),
        desired_status: Some("Running".to_string()),
        requested_by: Some("test".to_string()),
        updated_by: Some("test".to_string()),
        target_node_id: None,
        cpu_count: Some(2),
        memory_bytes: Some(1024),
        image_ref: Some("img".to_string()),
        boot_mode: None,
        desired_power_state: Some("on".to_string()),
        requested_unix_ms: 1000,
    };

    repo.upsert_vm(&input).await.unwrap();

    let mut stale_input = input.clone();
    stale_input.desired_generation = Generation::new(3);
    stale_input.requested_unix_ms = 2000;

    match repo.upsert_vm(&stale_input).await {
        Err(StoreError::StaleGeneration {
            entity,
            id,
            incoming,
        }) => {
            assert_eq!(entity, "vm");
            assert_eq!(id, "vm-gen-test");
            assert_eq!(incoming, 3);
        }
        other => panic!("Expected StaleGeneration, got {:?}", other),
    }
}

#[tokio::test]
async fn test_upsert_vm_accepts_newer_generation() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = DesiredStateRepository::new(pool.clone());

    let input = VmDesiredStateInput {
        vm_id: ResourceId::new("vm-gen-newer").unwrap(),
        node_id: None,
        display_name: "gen-newer".to_string(),
        tenant_id: None,
        placement_policy: None,
        desired_generation: Generation::new(3),
        desired_status: Some("Running".to_string()),
        requested_by: Some("test".to_string()),
        updated_by: Some("test".to_string()),
        target_node_id: None,
        cpu_count: Some(2),
        memory_bytes: Some(1024),
        image_ref: Some("img".to_string()),
        boot_mode: None,
        desired_power_state: Some("on".to_string()),
        requested_unix_ms: 1000,
    };

    repo.upsert_vm(&input).await.unwrap();

    let mut newer_input = input.clone();
    newer_input.desired_generation = Generation::new(5);
    newer_input.requested_unix_ms = 2000;

    repo.upsert_vm(&newer_input).await.unwrap();

    let gen: i64 = sqlx::query_scalar(
        "SELECT desired_generation FROM vm_desired_state WHERE vm_id = 'vm-gen-newer'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(gen, 5);
}

#[tokio::test]
async fn test_upsert_vm_accepts_same_generation_idempotent() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = DesiredStateRepository::new(pool.clone());

    let input = VmDesiredStateInput {
        vm_id: ResourceId::new("vm-gen-idem").unwrap(),
        node_id: None,
        display_name: "gen-idem".to_string(),
        tenant_id: None,
        placement_policy: None,
        desired_generation: Generation::new(5),
        desired_status: Some("Running".to_string()),
        requested_by: Some("test".to_string()),
        updated_by: Some("test".to_string()),
        target_node_id: None,
        cpu_count: Some(2),
        memory_bytes: Some(1024),
        image_ref: Some("img".to_string()),
        boot_mode: None,
        desired_power_state: Some("on".to_string()),
        requested_unix_ms: 1000,
    };

    repo.upsert_vm(&input).await.unwrap();
    repo.upsert_vm(&input).await.unwrap();
}

#[tokio::test]
async fn test_set_vm_resources_stale_vs_not_found() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = DesiredStateRepository::new(pool.clone());

    // First, create a VM with generation 5
    let vm_input = VmDesiredStateInput {
        vm_id: ResourceId::new("vm-res-test").unwrap(),
        node_id: None,
        display_name: "res-test".to_string(),
        tenant_id: None,
        placement_policy: None,
        desired_generation: Generation::new(5),
        desired_status: Some("Running".to_string()),
        requested_by: Some("test".to_string()),
        updated_by: Some("test".to_string()),
        target_node_id: None,
        cpu_count: Some(2),
        memory_bytes: Some(1024),
        image_ref: Some("img".to_string()),
        boot_mode: None,
        desired_power_state: Some("on".to_string()),
        requested_unix_ms: 1000,
    };
    repo.upsert_vm(&vm_input).await.unwrap();

    // Stale generation → StaleGeneration error
    let stale_patch = VmResourcesPatchInput {
        vm_id: ResourceId::new("vm-res-test").unwrap(),
        cpu_count: Some(4),
        memory_bytes: None,
        desired_generation: Generation::new(3),
        requested_by: Some("test".to_string()),
        target_node_id: None,
        requested_unix_ms: 2000,
    };
    match repo.set_vm_resources(&stale_patch).await {
        Err(StoreError::StaleGeneration { entity, .. }) => {
            assert_eq!(entity, "vm");
        }
        other => panic!("Expected StaleGeneration, got {:?}", other),
    }

    // Non-existent VM → NotFound error
    let missing_patch = VmResourcesPatchInput {
        vm_id: ResourceId::new("vm-nonexistent").unwrap(),
        cpu_count: Some(4),
        memory_bytes: None,
        desired_generation: Generation::new(10),
        requested_by: Some("test".to_string()),
        target_node_id: None,
        requested_unix_ms: 2000,
    };
    match repo.set_vm_resources(&missing_patch).await {
        Err(StoreError::NotFound { entity, .. }) => {
            assert_eq!(entity, "vm");
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

/// Fail-closed regression for C4: a row with an undecryptable `enc:`-prefixed
/// credential MUST surface as `None` (with an error log), not as the ciphertext
/// literal. The previous fail-soft behavior wrote `enc:hex...` back into
/// `s3_access_key`/`s3_secret_key`, which the worker passed to the S3 client
/// as a credential — manifesting as opaque AWS auth errors.
#[tokio::test]
async fn decrypt_schedule_row_nulls_undecryptable_credential() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();

    // Insert a schedule row with deliberately-bad ciphertext directly. Bypass
    // the repository's encrypt path so we can simulate a row written by a
    // process running under a different (now-lost) key.
    let bad_ciphertext = "enc:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    sqlx::query(
        "INSERT INTO backup_schedules ( \
             schedule_id, vm_id, name, cron_expression, retention_count, \
             enabled, s3_access_key, s3_secret_key \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sched-c4-regression")
    .bind("vm-x")
    .bind("c4-test")
    .bind("0 0 * * *")
    .bind(1_i64)
    .bind(true)
    .bind(bad_ciphertext)
    .bind(bad_ciphertext)
    .execute(&pool)
    .await
    .expect("insert schedule row");

    // BackupRepository::new picks up the *current* CHV_ENCRYPTION_KEY env.
    // Whatever the test process has, the bad ciphertext will not authenticate
    // under it (or KeyUnavailable / Malformed), so decrypt MUST fail.
    let repo = BackupRepository::new(pool);
    let row = repo
        .get_schedule("sched-c4-regression")
        .await
        .expect("query succeeds")
        .expect("row exists");

    assert!(
        row.s3_access_key.is_none(),
        "fail-closed: undecryptable s3_access_key MUST be None, got {:?}",
        row.s3_access_key
    );
    assert!(
        row.s3_secret_key.is_none(),
        "fail-closed: undecryptable s3_secret_key MUST be None, got {:?}",
        row.s3_secret_key
    );
    // Critically, neither field may equal the ciphertext literal.
    assert_ne!(row.s3_access_key.as_deref(), Some(bad_ciphertext));
    assert_ne!(row.s3_secret_key.as_deref(), Some(bad_ciphertext));
}
