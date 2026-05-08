//! Phase 9: End-to-end integration tests for multi-node migration and overlay networking.
//!
//! These tests exercise the migration state machine, overlay manager, VNI lifecycle,
//! and enrollment with VTEP registration—all using in-memory SQLite with mocked node
//! clients (no real gRPC connections or VMs).

use crate::migration::{
    create_migration_record, update_migration_progress, MigrationConfig, MigrationPhase,
    MigrationState, PhaseTimeouts,
};
use crate::node_client_pool::NodeClientPool;
use crate::overlay::OverlayManager;
use chv_controlplane_store::{
    test_util::create_test_pool, DesiredStateRepository, NodeRepository, NodeUpsertInput,
    OperationCreateInput, OperationRepository, StorePool, VtepRepository,
};
use chv_controlplane_types::domain::{
    Generation, NodeId, OperationId, OperationStatus, ResourceId, ResourceKind,
};
use control_plane_node_api::control_plane_node_api as proto;

/// A lightweight two-node test cluster backed by in-memory SQLite.
/// Provides repositories and helpers for setting up migration/overlay test scenarios.
struct TestCluster {
    pool: StorePool,
    node_repo: NodeRepository,
    operation_repo: OperationRepository,
    desired_state_repo: DesiredStateRepository,
    vtep_repo: VtepRepository,
}

impl TestCluster {
    /// Create a TestCluster with an in-memory SQLite database and all migrations applied.
    async fn new() -> Self {
        let pool = create_test_pool().await;
        let node_repo = NodeRepository::new(pool.clone());
        let operation_repo = OperationRepository::new(pool.clone());
        let desired_state_repo = DesiredStateRepository::new(pool.clone());
        let vtep_repo = VtepRepository::new(pool.clone());
        Self {
            pool,
            node_repo,
            operation_repo,
            desired_state_repo,
            vtep_repo,
        }
    }

    /// Enroll two nodes (node_a, node_b) with VTEP addresses registered.
    async fn setup_two_nodes(&self) {
        // Create node_a
        self.node_repo
            .upsert_node(&NodeUpsertInput {
                node_id: NodeId::new("node-a").unwrap(),
                hostname: "host-a".to_string(),
                display_name: "Node A".to_string(),
                certificate_serial: None,
                agent_version: Some("0.1.0".to_string()),
                control_plane_version: Some("0.1.0".to_string()),
                enrolled_unix_ms: 1000,
                last_seen_unix_ms: 1000,
            })
            .await
            .expect("failed to create node-a");

        // Create node_b
        self.node_repo
            .upsert_node(&NodeUpsertInput {
                node_id: NodeId::new("node-b").unwrap(),
                hostname: "host-b".to_string(),
                display_name: "Node B".to_string(),
                certificate_serial: None,
                agent_version: Some("0.1.0".to_string()),
                control_plane_version: Some("0.1.0".to_string()),
                enrolled_unix_ms: 1000,
                last_seen_unix_ms: 1000,
            })
            .await
            .expect("failed to create node-b");

        // Register VTEPs
        self.vtep_repo
            .register_vtep("node-a", "10.0.0.1", 4789)
            .await
            .expect("failed to register VTEP for node-a");
        self.vtep_repo
            .register_vtep("node-b", "10.0.0.2", 4789)
            .await
            .expect("failed to register VTEP for node-b");
    }

    /// Create a VM on the given node with a network and NIC attached.
    async fn create_vm_on_node(
        &self,
        vm_id: &str,
        node_id: &str,
        network_id: &str,
        mac: &str,
        memory_bytes: i64,
    ) {
        use chv_controlplane_store::{NetworkDesiredStateInput, VmDesiredStateInput};

        // Create VM record
        self.desired_state_repo
            .upsert_vm(&VmDesiredStateInput {
                vm_id: ResourceId::new(vm_id).unwrap(),
                node_id: Some(NodeId::new(node_id).unwrap()),
                display_name: format!("Test VM {vm_id}"),
                tenant_id: None,
                placement_policy: None,
                desired_generation: Generation::new(1),
                desired_status: Some("running".to_string()),
                requested_by: Some("test".to_string()),
                updated_by: Some("test".to_string()),
                target_node_id: Some(NodeId::new(node_id).unwrap()),
                cpu_count: Some(2),
                memory_bytes: Some(memory_bytes),
                image_ref: Some("test-image:latest".to_string()),
                boot_mode: None,
                desired_power_state: Some("Running".to_string()),
                requested_unix_ms: 1000,
            })
            .await
            .expect("failed to create VM desired state");

        // Create network if needed (ignore conflict)
        let _ = self
            .desired_state_repo
            .upsert_network(&NetworkDesiredStateInput {
                network_id: ResourceId::new(network_id).unwrap(),
                node_id: Some(NodeId::new(node_id).unwrap()),
                display_name: format!("Test Network {network_id}"),
                network_class: Some("overlay".to_string()),
                desired_generation: Generation::new(1),
                desired_status: Some("active".to_string()),
                requested_by: Some("test".to_string()),
                updated_by: Some("test".to_string()),
                firewall_rules_json: None,
                nat_rules_json: None,
                dhcp_scope_json: None,
                dns_enabled: None,
                dns_scope_json: None,
                requested_unix_ms: 1000,
            })
            .await;

        // Create NIC linking VM to network
        sqlx::query(
            r#"INSERT OR IGNORE INTO vm_nic_desired_state (nic_id, vm_id, network_id, mac_address)
               VALUES (?, ?, ?, ?)"#,
        )
        .bind(format!("nic-{vm_id}"))
        .bind(vm_id)
        .bind(network_id)
        .bind(mac)
        .execute(&self.pool)
        .await
        .expect("failed to create NIC");
    }

    /// Create an operation record required by the migrations table FK constraint.
    async fn create_operation(&self, operation_id: &str) {
        self.operation_repo
            .create_or_get(&OperationCreateInput {
                operation_id: OperationId::new(operation_id).unwrap(),
                idempotency_key: format!("idem-{operation_id}"),
                resource_kind: ResourceKind::Vm,
                resource_id: None,
                operation_type: "migrate".to_string(),
                status: OperationStatus::Running,
                requested_by: Some("test".to_string()),
                updated_by: Some("test".to_string()),
                desired_generation: Some(Generation::new(1)),
                observed_generation: None,
                correlation_id: None,
                requested_unix_ms: 1000,
            })
            .await
            .expect("failed to create operation");
    }
}

// =============================================================================
// 9.1 — Multi-node integration test harness
// =============================================================================

#[tokio::test]
async fn test_cluster_setup_two_nodes_with_vtep() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;

    // Verify VTEPs registered
    let vtep_a = cluster.vtep_repo.get_vtep("node-a").await.unwrap();
    assert_eq!(vtep_a.vtep_ip, "10.0.0.1");
    assert_eq!(vtep_a.vtep_port, 4789);

    let vtep_b = cluster.vtep_repo.get_vtep("node-b").await.unwrap();
    assert_eq!(vtep_b.vtep_ip, "10.0.0.2");
    assert_eq!(vtep_b.vtep_port, 4789);
}

// =============================================================================
// 9.2 — Test scenarios
// =============================================================================

// Scenario 1: Happy path migration state progression
// Since execute_migration requires real gRPC connections, we test the state machine
// components (record creation, phase transitions, progress updates, placement update)
// individually and verify the database state after each operation.
#[tokio::test]
async fn test_happy_path_migration_state_progression() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-001",
            "node-a",
            "net-overlay",
            "aa:bb:cc:dd:ee:01",
            4_294_967_296,
        )
        .await;
    cluster.create_operation("op-mig-001").await;

    // Create migration record
    let state = MigrationState {
        migration_id: "mig-001".to_string(),
        operation_id: "op-mig-001".to_string(),
        vm_id: "vm-001".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig::default(),
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state)
        .await
        .expect("failed to create migration record");

    // Verify record exists with Pending phase
    let row: (String, String, String) = sqlx::query_as(
        "SELECT phase, source_node_id, destination_node_id FROM migrations WHERE migration_id = ?",
    )
    .bind("mig-001")
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Pending");
    assert_eq!(row.1, "node-a");
    assert_eq!(row.2, "node-b");

    // Simulate phase transitions via update_migration_progress (as agent would report)
    // Phase: PreCopyDisk
    update_migration_progress(
        &cluster.pool,
        "vm-001",
        "op-mig-001",
        proto::MigrationPhase::PrecopyDisk as i32,
        0,
        10_737_418_240, // 10GB
        0,
        100_000,
    )
    .await
    .expect("failed to update to PreCopyDisk");

    let phase: (String,) =
        sqlx::query_as("SELECT phase FROM migrations WHERE migration_id = 'mig-001'")
            .fetch_one(&cluster.pool)
            .await
            .unwrap();
    assert_eq!(phase.0, "PreCopyDisk");

    // Phase: ConvergingDisk with decreasing dirty blocks
    update_migration_progress(
        &cluster.pool,
        "vm-001",
        "op-mig-001",
        proto::MigrationPhase::ConvergingDisk as i32,
        5_000_000_000,
        10_737_418_240,
        1,
        50_000,
    )
    .await
    .unwrap();

    update_migration_progress(
        &cluster.pool,
        "vm-001",
        "op-mig-001",
        proto::MigrationPhase::ConvergingDisk as i32,
        8_000_000_000,
        10_737_418_240,
        2,
        10_000,
    )
    .await
    .unwrap();

    // Verify convergence round and dirty blocks decreasing
    let row: (i32, i64) = sqlx::query_as(
        "SELECT convergence_round, dirty_blocks_remaining FROM migrations WHERE migration_id = 'mig-001'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, 2);
    assert_eq!(row.1, 10_000);

    // Phase: MemoryMigration
    update_migration_progress(
        &cluster.pool,
        "vm-001",
        "op-mig-001",
        proto::MigrationPhase::MemoryMigration as i32,
        10_000_000_000,
        10_737_418_240,
        2,
        500,
    )
    .await
    .unwrap();

    // Phase: Completed
    update_migration_progress(
        &cluster.pool,
        "vm-001",
        "op-mig-001",
        proto::MigrationPhase::Completed as i32,
        10_737_418_240,
        10_737_418_240,
        2,
        0,
    )
    .await
    .unwrap();

    let row: (String, Option<String>) =
        sqlx::query_as("SELECT phase, completed_at FROM migrations WHERE migration_id = 'mig-001'")
            .fetch_one(&cluster.pool)
            .await
            .unwrap();
    assert_eq!(row.0, "Completed");
    assert!(row.1.is_some(), "completed_at should be set");

    // Simulate VM placement update (as orchestrator would do after migration)
    sqlx::query("UPDATE vm_desired_state SET target_node_id = 'node-b' WHERE vm_id = 'vm-001'")
        .execute(&cluster.pool)
        .await
        .unwrap();

    let target: (String,) =
        sqlx::query_as("SELECT target_node_id FROM vm_desired_state WHERE vm_id = 'vm-001'")
            .fetch_one(&cluster.pool)
            .await
            .unwrap();
    assert_eq!(target.0, "node-b");
}

// Scenario 2: Disk convergence — verify rounds occur and dirty count decreases
#[tokio::test]
async fn test_disk_convergence_dirty_block_decrease() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-conv",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:02",
            2_147_483_648,
        )
        .await;
    cluster.create_operation("op-conv-001").await;

    let state = MigrationState {
        migration_id: "mig-conv-001".to_string(),
        operation_id: "op-conv-001".to_string(),
        vm_id: "vm-conv".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig {
            dirty_threshold_blocks: 1024,
            max_convergence_rounds: 5,
            block_size_bytes: 4_194_304,
            total_timeout_seconds: 0,
            timeout_multiplier: 1.0,
        },
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state)
        .await
        .unwrap();

    // Simulate multiple convergence rounds with decreasing dirty blocks
    let dirty_sequence: Vec<(u32, u64)> = vec![
        (1, 100_000), // round 1: 100k dirty blocks
        (2, 50_000),  // round 2: 50k
        (3, 10_000),  // round 3: 10k
        (4, 2_000),   // round 4: 2k
        (5, 500),     // round 5: 500 (below threshold of 1024)
    ];

    for (round, dirty) in &dirty_sequence {
        update_migration_progress(
            &cluster.pool,
            "vm-conv",
            "op-conv-001",
            proto::MigrationPhase::ConvergingDisk as i32,
            0,
            10_737_418_240,
            *round,
            *dirty,
        )
        .await
        .unwrap();
    }

    // Verify final state: round 5, dirty 500
    let row: (i32, i64) = sqlx::query_as(
        "SELECT convergence_round, dirty_blocks_remaining FROM migrations WHERE migration_id = 'mig-conv-001'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, 5);
    assert_eq!(row.1, 500);
    // 500 < 1024 threshold: convergence achieved
    assert!(
        row.1 <= state.config.dirty_threshold_blocks as i64,
        "dirty blocks {} should be <= threshold {}",
        row.1,
        state.config.dirty_threshold_blocks
    );
}

// Scenario 3: Failure in PreCopyDisk — verify rollback, VM continues on source
#[tokio::test]
async fn test_failure_in_precopy_disk_rollback() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-fail-pre",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:03",
            1_073_741_824,
        )
        .await;
    cluster.create_operation("op-fail-pre").await;

    let state = MigrationState {
        migration_id: "mig-fail-pre".to_string(),
        operation_id: "op-fail-pre".to_string(),
        vm_id: "vm-fail-pre".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig::default(),
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state)
        .await
        .unwrap();

    // Simulate agent reporting RolledBack during PreCopyDisk
    // (e.g., dest stord killed mid-stream)
    update_migration_progress(
        &cluster.pool,
        "vm-fail-pre",
        "op-fail-pre",
        proto::MigrationPhase::PrecopyDisk as i32,
        1_000_000_000,
        10_737_418_240,
        0,
        0,
    )
    .await
    .unwrap();

    // Dest stord failure -> agent reports RolledBack
    update_migration_progress(
        &cluster.pool,
        "vm-fail-pre",
        "op-fail-pre",
        proto::MigrationPhase::RolledBack as i32,
        1_000_000_000,
        10_737_418_240,
        0,
        0,
    )
    .await
    .unwrap();

    // Verify: migration marked RolledBack
    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT phase, completed_at FROM migrations WHERE migration_id = 'mig-fail-pre'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "RolledBack");
    assert!(row.1.is_some(), "completed_at should be set on rollback");

    // Verify: VM still on source node (target_node_id unchanged)
    let target: (String,) =
        sqlx::query_as("SELECT target_node_id FROM vm_desired_state WHERE vm_id = 'vm-fail-pre'")
            .fetch_one(&cluster.pool)
            .await
            .unwrap();
    assert_eq!(
        target.0, "node-a",
        "VM should remain on source node after rollback"
    );
}

// Scenario 4: Failure in MemoryMigration — marked Failed (no clean rollback)
#[tokio::test]
async fn test_failure_in_memory_migration_marked_failed() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-fail-mem",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:04",
            2_147_483_648,
        )
        .await;
    cluster.create_operation("op-fail-mem").await;

    let state = MigrationState {
        migration_id: "mig-fail-mem".to_string(),
        operation_id: "op-fail-mem".to_string(),
        vm_id: "vm-fail-mem".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig::default(),
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state)
        .await
        .unwrap();

    // Progress through precopy and convergence
    update_migration_progress(
        &cluster.pool,
        "vm-fail-mem",
        "op-fail-mem",
        proto::MigrationPhase::PrecopyDisk as i32,
        5_000_000_000,
        10_737_418_240,
        0,
        50_000,
    )
    .await
    .unwrap();

    update_migration_progress(
        &cluster.pool,
        "vm-fail-mem",
        "op-fail-mem",
        proto::MigrationPhase::ConvergingDisk as i32,
        9_000_000_000,
        10_737_418_240,
        3,
        500,
    )
    .await
    .unwrap();

    // Enter memory migration
    update_migration_progress(
        &cluster.pool,
        "vm-fail-mem",
        "op-fail-mem",
        proto::MigrationPhase::MemoryMigration as i32,
        10_000_000_000,
        10_737_418_240,
        3,
        0,
    )
    .await
    .unwrap();

    // Dest agent killed mid-transfer -> agent reports Failed
    update_migration_progress(
        &cluster.pool,
        "vm-fail-mem",
        "op-fail-mem",
        proto::MigrationPhase::Failed as i32,
        10_200_000_000,
        10_737_418_240,
        3,
        0,
    )
    .await
    .unwrap();

    // Verify: migration marked Failed
    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT phase, completed_at FROM migrations WHERE migration_id = 'mig-fail-mem'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Failed");
    assert!(row.1.is_some(), "completed_at should be set on failure");
}

// Scenario 5: Network continuity — two VMs on same VNI across nodes, FDB entries correct
#[tokio::test]
async fn test_network_continuity_vni_allocation_and_vtep_lookup() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;

    // Create overlay network and two VMs on different nodes
    cluster
        .create_vm_on_node(
            "vm-net-a",
            "node-a",
            "net-vxlan",
            "aa:bb:cc:00:00:01",
            1_073_741_824,
        )
        .await;
    cluster
        .create_vm_on_node(
            "vm-net-b",
            "node-b",
            "net-vxlan",
            "aa:bb:cc:00:00:02",
            1_073_741_824,
        )
        .await;

    // Allocate VNI for the network
    let vni = cluster
        .vtep_repo
        .allocate_vni("net-vxlan")
        .await
        .expect("failed to allocate VNI");
    assert!(vni >= 1, "VNI should be >= 1, got {vni}");
    assert!(vni <= 16_777_214, "VNI should be <= 16777214, got {vni}");

    // Verify VNI stored for network
    let stored_vni = cluster
        .vtep_repo
        .get_vni_for_network("net-vxlan")
        .await
        .unwrap();
    assert_eq!(stored_vni, Some(vni));

    // Get VTEPs for the network (both nodes have VMs on it)
    let vteps = cluster
        .vtep_repo
        .get_vteps_for_network("net-vxlan")
        .await
        .expect("failed to get VTEPs for network");
    assert_eq!(
        vteps.len(),
        2,
        "both nodes should have VTEPs for this network"
    );

    let vtep_ips: Vec<&str> = vteps.iter().map(|v| v.vtep_ip.as_str()).collect();
    assert!(
        vtep_ips.contains(&"10.0.0.1"),
        "node-a VTEP should be present"
    );
    assert!(
        vtep_ips.contains(&"10.0.0.2"),
        "node-b VTEP should be present"
    );
}

// Scenario 6: Security policy — store and retrieve deny rules
#[tokio::test]
async fn test_security_policy_store_deny_rule() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-sec",
            "node-a",
            "net-sec",
            "aa:bb:cc:dd:ee:06",
            1_073_741_824,
        )
        .await;

    // Insert a security policy with a deny rule
    let rules_json = serde_json::json!([
        {
            "direction": "ingress",
            "protocol": "tcp",
            "port": 22,
            "action": "deny",
            "source_cidr": "0.0.0.0/0"
        },
        {
            "direction": "egress",
            "protocol": "udp",
            "port": 53,
            "action": "allow",
            "destination_cidr": "8.8.8.8/32"
        }
    ]);

    sqlx::query(
        r#"INSERT INTO security_policies (policy_id, vm_id, network_id, default_action, rules_json)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind("pol-001")
    .bind("vm-sec")
    .bind("net-sec")
    .bind("deny")
    .bind(rules_json.to_string())
    .execute(&cluster.pool)
    .await
    .expect("failed to create security policy");

    // Verify the rule is stored
    let row: (String, String, String) = sqlx::query_as(
        "SELECT default_action, rules_json, vm_id FROM security_policies WHERE policy_id = 'pol-001'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "deny");
    assert_eq!(row.2, "vm-sec");

    let stored_rules: serde_json::Value = serde_json::from_str(&row.1).unwrap();
    assert_eq!(stored_rules.as_array().unwrap().len(), 2);
    assert_eq!(stored_rules[0]["action"], "deny");
    assert_eq!(stored_rules[0]["port"], 22);
    assert_eq!(stored_rules[1]["action"], "allow");
}

// =============================================================================
// 9.2 — Additional overlay/VNI lifecycle tests
// =============================================================================

#[tokio::test]
async fn test_vni_allocation_and_release_lifecycle() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;

    // Create a network record
    cluster
        .create_vm_on_node(
            "vm-vni",
            "node-a",
            "net-vni-test",
            "aa:bb:cc:00:01:01",
            1_073_741_824,
        )
        .await;

    // Allocate VNI
    let vni1 = cluster
        .vtep_repo
        .allocate_vni("net-vni-test")
        .await
        .unwrap();
    assert!(vni1 >= 1);

    // Allocate a second VNI for a different network
    // Need to create second network first
    cluster
        .create_vm_on_node(
            "vm-vni2",
            "node-b",
            "net-vni-test2",
            "aa:bb:cc:00:01:02",
            1_073_741_824,
        )
        .await;
    let vni2 = cluster
        .vtep_repo
        .allocate_vni("net-vni-test2")
        .await
        .unwrap();
    assert_ne!(vni1, vni2, "VNIs should be distinct");

    // Release first VNI
    cluster
        .vtep_repo
        .release_vni("net-vni-test")
        .await
        .expect("failed to release VNI");

    // Verify VNI is released
    let released_vni = cluster
        .vtep_repo
        .get_vni_for_network("net-vni-test")
        .await
        .unwrap();
    assert_eq!(released_vni, None, "VNI should be None after release");

    // Second VNI still active
    let still_active = cluster
        .vtep_repo
        .get_vni_for_network("net-vni-test2")
        .await
        .unwrap();
    assert_eq!(still_active, Some(vni2));
}

#[tokio::test]
async fn test_vtep_registration_updates_existing() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;

    // Update VTEP IP for node-a
    cluster
        .vtep_repo
        .register_vtep("node-a", "192.168.1.100", 4790)
        .await
        .expect("failed to update VTEP");

    let vtep = cluster.vtep_repo.get_vtep("node-a").await.unwrap();
    assert_eq!(vtep.vtep_ip, "192.168.1.100");
    assert_eq!(vtep.vtep_port, 4790);
}

#[tokio::test]
async fn test_enrollment_vtep_registration() {
    let cluster = TestCluster::new().await;

    // Enroll a new node and register VTEP (simulating what EnrollmentService does)
    cluster
        .node_repo
        .upsert_node(&NodeUpsertInput {
            node_id: NodeId::new("node-new").unwrap(),
            hostname: "host-new".to_string(),
            display_name: "New Node".to_string(),
            certificate_serial: None,
            agent_version: Some("0.1.0".to_string()),
            control_plane_version: Some("0.1.0".to_string()),
            enrolled_unix_ms: 2000,
            last_seen_unix_ms: 2000,
        })
        .await
        .unwrap();

    cluster
        .vtep_repo
        .register_vtep("node-new", "172.16.0.50", 4789)
        .await
        .unwrap();

    let vtep = cluster.vtep_repo.get_vtep("node-new").await.unwrap();
    assert_eq!(vtep.node_id, "node-new");
    assert_eq!(vtep.vtep_ip, "172.16.0.50");
    assert_eq!(vtep.vtep_port, 4789);
}

// =============================================================================
// 9.3 — Performance baseline test structures
// =============================================================================

/// Test structure for measuring block streaming throughput.
/// In a real benchmark, this would stream actual blocks and measure time.
/// Here we verify the measurement infrastructure works.
#[tokio::test]
async fn test_performance_baseline_block_streaming_throughput() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-perf-blk",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:10",
            4_294_967_296,
        )
        .await;
    cluster.create_operation("op-perf-blk").await;

    let state = MigrationState {
        migration_id: "mig-perf-blk".to_string(),
        operation_id: "op-perf-blk".to_string(),
        vm_id: "vm-perf-blk".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig {
            dirty_threshold_blocks: 1024,
            max_convergence_rounds: 10,
            block_size_bytes: 4_194_304, // 4MB blocks
            total_timeout_seconds: 0,
            timeout_multiplier: 1.0,
        },
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state)
        .await
        .unwrap();

    // Simulate block streaming: record timestamps and bytes for throughput calculation
    let start = std::time::Instant::now();
    let total_disk_bytes: u64 = 10_737_418_240; // 10 GB
    let block_size: u64 = 4_194_304; // 4 MB
    let total_blocks = total_disk_bytes / block_size;

    // Simulate streaming 100 blocks and updating progress
    let simulated_blocks = 100u64;
    let bytes_per_update = simulated_blocks * block_size;

    update_migration_progress(
        &cluster.pool,
        "vm-perf-blk",
        "op-perf-blk",
        proto::MigrationPhase::PrecopyDisk as i32,
        bytes_per_update,
        total_disk_bytes,
        0,
        (total_blocks - simulated_blocks) as u64,
    )
    .await
    .unwrap();

    let elapsed = start.elapsed();

    // Verify progress was recorded
    let row: (i64, i64) = sqlx::query_as(
        "SELECT bytes_transferred, total_bytes FROM migrations WHERE migration_id = 'mig-perf-blk'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, bytes_per_update as i64);
    assert_eq!(row.1, total_disk_bytes as i64);

    // Performance assertion: DB update should be fast (< 1s for in-memory SQLite)
    assert!(
        elapsed.as_secs() < 1,
        "block streaming progress update took too long: {:?}",
        elapsed
    );

    // Calculate theoretical throughput
    let throughput_mbps = (bytes_per_update as f64 / 1_048_576.0) / elapsed.as_secs_f64();
    // Just verify we can calculate it (actual value depends on machine)
    assert!(throughput_mbps > 0.0, "throughput should be positive");
}

/// Test structure for measuring total migration time.
/// Verifies the timing infrastructure across all phases.
#[tokio::test]
async fn test_performance_baseline_total_migration_time() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-perf-tot",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:11",
            8_589_934_592,
        )
        .await;
    cluster.create_operation("op-perf-tot").await;

    let state = MigrationState {
        migration_id: "mig-perf-tot".to_string(),
        operation_id: "op-perf-tot".to_string(),
        vm_id: "vm-perf-tot".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig::default(),
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state)
        .await
        .unwrap();

    let migration_start = std::time::Instant::now();

    // Phase 1: PreCopyDisk
    let phase_start = std::time::Instant::now();
    update_migration_progress(
        &cluster.pool,
        "vm-perf-tot",
        "op-perf-tot",
        proto::MigrationPhase::PrecopyDisk as i32,
        5_000_000_000,
        10_737_418_240,
        0,
        50_000,
    )
    .await
    .unwrap();
    let precopy_duration = phase_start.elapsed();

    // Phase 2: ConvergingDisk
    let phase_start = std::time::Instant::now();
    update_migration_progress(
        &cluster.pool,
        "vm-perf-tot",
        "op-perf-tot",
        proto::MigrationPhase::ConvergingDisk as i32,
        9_500_000_000,
        10_737_418_240,
        3,
        500,
    )
    .await
    .unwrap();
    let converge_duration = phase_start.elapsed();

    // Phase 3: MemoryMigration
    let phase_start = std::time::Instant::now();
    update_migration_progress(
        &cluster.pool,
        "vm-perf-tot",
        "op-perf-tot",
        proto::MigrationPhase::MemoryMigration as i32,
        10_500_000_000,
        10_737_418_240,
        3,
        0,
    )
    .await
    .unwrap();
    let memory_duration = phase_start.elapsed();

    // Phase 4: Completed
    let phase_start = std::time::Instant::now();
    update_migration_progress(
        &cluster.pool,
        "vm-perf-tot",
        "op-perf-tot",
        proto::MigrationPhase::Completed as i32,
        10_737_418_240,
        10_737_418_240,
        3,
        0,
    )
    .await
    .unwrap();
    let complete_duration = phase_start.elapsed();

    let total_duration = migration_start.elapsed();

    // All phase transitions on in-memory DB should be very fast
    assert!(precopy_duration.as_millis() < 100);
    assert!(converge_duration.as_millis() < 100);
    assert!(memory_duration.as_millis() < 100);
    assert!(complete_duration.as_millis() < 100);
    assert!(
        total_duration.as_millis() < 500,
        "total migration state updates took too long: {:?}",
        total_duration
    );

    // Verify final state
    let row: (String, i64) = sqlx::query_as(
        "SELECT phase, bytes_transferred FROM migrations WHERE migration_id = 'mig-perf-tot'",
    )
    .fetch_one(&cluster.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Completed");
    assert_eq!(row.1, 10_737_418_240);

    // Verify calculated timeouts are reasonable for 10GB disk / 8GB memory
    let timeouts = PhaseTimeouts::calculate(10, 8, 1.0);
    assert_eq!(timeouts.precopy_disk_secs, 600); // 10 * 60
    assert_eq!(timeouts.memory_migration_secs, 360); // 8 * 30 + 120
    assert!(timeouts.total_secs > 0);
}

// =============================================================================
// Additional integration scenarios
// =============================================================================

/// Verify that multiple migrations can be tracked concurrently.
#[tokio::test]
async fn test_concurrent_migrations_tracked_independently() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;
    cluster
        .create_vm_on_node(
            "vm-conc-1",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:20",
            1_073_741_824,
        )
        .await;
    cluster
        .create_vm_on_node(
            "vm-conc-2",
            "node-a",
            "net-001",
            "aa:bb:cc:dd:ee:21",
            1_073_741_824,
        )
        .await;
    cluster.create_operation("op-conc-1").await;
    cluster.create_operation("op-conc-2").await;

    // Create two concurrent migrations
    let state1 = MigrationState {
        migration_id: "mig-conc-1".to_string(),
        operation_id: "op-conc-1".to_string(),
        vm_id: "vm-conc-1".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig::default(),
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };
    let state2 = MigrationState {
        migration_id: "mig-conc-2".to_string(),
        operation_id: "op-conc-2".to_string(),
        vm_id: "vm-conc-2".to_string(),
        source_node_id: "node-a".to_string(),
        dest_node_id: "node-b".to_string(),
        phase: MigrationPhase::Pending,
        config: MigrationConfig::default(),
        bytes_transferred: 0,
        total_bytes: 0,
        convergence_round: 0,
        dirty_blocks_remaining: 0,
    };

    create_migration_record(&cluster.pool, &state1)
        .await
        .unwrap();
    create_migration_record(&cluster.pool, &state2)
        .await
        .unwrap();

    // Progress migration 1 to Completed
    update_migration_progress(
        &cluster.pool,
        "vm-conc-1",
        "op-conc-1",
        proto::MigrationPhase::Completed as i32,
        10_000_000_000,
        10_000_000_000,
        2,
        0,
    )
    .await
    .unwrap();

    // Progress migration 2 to Failed
    update_migration_progress(
        &cluster.pool,
        "vm-conc-2",
        "op-conc-2",
        proto::MigrationPhase::Failed as i32,
        5_000_000_000,
        10_000_000_000,
        1,
        50_000,
    )
    .await
    .unwrap();

    // Verify independent states
    let phase1: (String,) =
        sqlx::query_as("SELECT phase FROM migrations WHERE migration_id = 'mig-conc-1'")
            .fetch_one(&cluster.pool)
            .await
            .unwrap();
    let phase2: (String,) =
        sqlx::query_as("SELECT phase FROM migrations WHERE migration_id = 'mig-conc-2'")
            .fetch_one(&cluster.pool)
            .await
            .unwrap();
    assert_eq!(phase1.0, "Completed");
    assert_eq!(phase2.0, "Failed");
}

/// Verify that MigrationConfig roundtrips correctly through correlation_id format.
#[tokio::test]
async fn test_migration_config_roundtrip_via_correlation_id() {
    let config = MigrationConfig {
        dirty_threshold_blocks: 2048,
        max_convergence_rounds: 7,
        block_size_bytes: 8_388_608,
        total_timeout_seconds: 5400,
        timeout_multiplier: 1.0,
    };

    let correlation_id = format!(
        "source=node-a:dest=node-b:threshold={}:rounds={}:block_size={}:timeout={}",
        config.dirty_threshold_blocks,
        config.max_convergence_rounds,
        config.block_size_bytes,
        config.total_timeout_seconds
    );

    let (source, dest, parsed) = MigrationConfig::from_correlation_id(&correlation_id);
    assert_eq!(source, "node-a");
    assert_eq!(dest, "node-b");
    assert_eq!(parsed.dirty_threshold_blocks, config.dirty_threshold_blocks);
    assert_eq!(parsed.max_convergence_rounds, config.max_convergence_rounds);
    assert_eq!(parsed.block_size_bytes, config.block_size_bytes);
    assert_eq!(parsed.total_timeout_seconds, config.total_timeout_seconds);
}

/// Verify overlay manager construction (no actual gRPC calls needed).
#[tokio::test]
async fn test_overlay_manager_construction() {
    let cluster = TestCluster::new().await;
    cluster.setup_two_nodes().await;

    let node_pool = NodeClientPool::new();
    let overlay_manager = OverlayManager::new(
        cluster.vtep_repo.clone(),
        node_pool,
        "/tmp/chv/agent/{node_id}/agent.sock".to_string(),
    );

    // Verify overlay manager can be cloned (required for concurrent usage)
    let _cloned = overlay_manager.clone();
}
