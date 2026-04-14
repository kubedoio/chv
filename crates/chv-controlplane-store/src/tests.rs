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

    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::Valid);
    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::AlreadyUsed);
    assert_eq!(repo.validate_and_consume("999").await.unwrap(), BootstrapTokenValidation::Invalid);
}

#[tokio::test]
async fn test_expired_bootstrap_token() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = BootstrapTokenRepository::new(pool.clone());

    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"; // sha256("123")
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use, expires_at) VALUES ($1, true, now() - interval '1 hour')")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::Expired);
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

    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::Valid);
    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::Valid);
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
    let vm_id = ResourceId::new("test-vm-missing-node").unwrap();
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
    let volume_id = ResourceId::new("test-vol-missing-vm").unwrap();
    let vm_id = ResourceId::new("non-existent-attached-vm").unwrap();

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
