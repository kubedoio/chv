use crate::*;
use chv_controlplane_store::*;
use chv_controlplane_types::domain::NodeId;
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;

use crate::lifecycle::LifecycleService;

// Mock CertificateIssuer for enrollment tests
struct MockCertIssuer;
#[async_trait::async_trait]
impl CertificateIssuer for MockCertIssuer {
    async fn issue_node_certificate(
        &self,
        _node_id: &NodeId,
    ) -> Result<IssuedCertificate, ControlPlaneServiceError> {
        Ok(IssuedCertificate {
            certificate_pem: vec![],
            private_key_pem: vec![],
            ca_pem: vec![],
            serial: "mock-serial".into(),
        })
    }
}

#[tokio::test]
async fn test_publish_alert_persistence() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let observed_state_repo = ObservedStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let alert_repo = AlertRepository::new(pool.clone());

    let service = TelemetryServiceImplementation::new(observed_state_repo, event_repo, alert_repo);

    let op_id = "op-123-custom-string";
    let request_ok = proto::PublishAlertRequest {
        meta: Some(proto::RequestMeta {
            operation_id: op_id.into(),
            requested_by: "test-user".into(),
            target_node_id: "node-1".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-1".into(),
        severity: "Critical".into(),
        alert_type: "disk_full".into(),
        summary: "disk is full".into(),
        details_json: b"{\"usage\": 99}".to_vec(),
    };

    // Before we publish, we need the node and operation to exist due to FK constraints
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-1', 'host-1', 'host-1')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_at) VALUES ($1, $2, 'node', 'node-1', 'Test', 'Pending', now())")
        .bind(op_id)
        .bind("idem-123")
        .execute(&pool)
        .await
        .unwrap();

    let result = service
        .publish_alert(request_ok)
        .await
        .expect("Publish failed");
    assert_eq!(result.result.unwrap().status, "ok");

    // VERIFY PERSISTENCE - use runtime query to avoid needing DATABASE_URL at compile time
    let alert = sqlx::query("SELECT alert_type, operation_id FROM alerts WHERE node_id = $1")
        .bind("node-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let alert_type: String = sqlx::Row::get(&alert, "alert_type");
    let operation_id: Option<String> = sqlx::Row::get(&alert, "operation_id");
    assert_eq!(alert_type, "disk_full");
    assert_eq!(operation_id, Some(op_id.to_string()));

    // Case 2: Whitespace-only operation_id should return InvalidArgument
    let request_whitespace = proto::PublishAlertRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "   ".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-1".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-1".into(),
        severity: "Critical".into(),
        alert_type: "disk_full".into(),
        summary: "disk is full".into(),
        details_json: b"{}".to_vec(),
    };
    let result_err = service.publish_alert(request_whitespace).await;
    match result_err {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("operation_id cannot be empty"));
        }
        other => panic!(
            "Expected InvalidArgument for whitespace op_id, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_enrollment_extended_inventory_persistence() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let token_repo = BootstrapTokenRepository::new(pool.clone());
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, cert_issuer);

    // Seed a bootstrap token for enrollment (sha256("123"))
    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use) VALUES ($1, false)")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    let mut labels = std::collections::HashMap::new();
    labels.insert("env".to_string(), "prod".to_string());

    let request = proto::EnrollmentRequest {
        bootstrap_token: "123".into(),
        inventory: Some(proto::NodeInventory {
            node_id: "node-new-1".into(),
            hostname: "host-1".into(),
            architecture: "x86_64".into(),
            cpu_threads: 16,
            memory_bytes: 32 * 1024 * 1024 * 1024,
            storage_classes: vec!["ssd".into()],
            network_capabilities: vec!["vxlan".into()],
            labels,
        }),
        versions: Some(proto::ServiceVersions {
            node_id: "node-new-1".into(),
            chv_agent_version: "1.0.0".into(),
            chv_stord_version: "1.0.0".into(),
            chv_nwd_version: "1.0.0".into(),
            cloud_hypervisor_version: "40.0.0".into(),
            host_bundle_version: "1.2.3".into(),
        }),
    };

    service
        .enroll_node(request)
        .await
        .expect("Enrollment failed");

    // VERIFY PERSISTENCE
    let node = sqlx::query("SELECT hostname FROM nodes WHERE node_id = $1")
        .bind("node-new-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let hostname: String = sqlx::Row::get(&node, "hostname");
    assert_eq!(hostname, "host-1");

    let inv = sqlx::query("SELECT storage_classes, labels FROM node_inventory WHERE node_id = $1")
        .bind("node-new-1")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Check JSONB columns
    let storage_classes_val: serde_json::Value = sqlx::Row::get(&inv, "storage_classes");
    let storage_classes: Vec<String> = serde_json::from_value(storage_classes_val).unwrap();
    assert_eq!(storage_classes, vec!["ssd"]);

    let labels_val: serde_json::Value = sqlx::Row::get(&inv, "labels");
    let labels: std::collections::HashMap<String, String> =
        serde_json::from_value(labels_val).unwrap();
    assert_eq!(labels.get("env").unwrap(), "prod");
}

#[tokio::test]
async fn test_rotate_certificate_missing_node() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool);
    let token_repo = BootstrapTokenRepository::new(test_db.pool.clone());
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, cert_issuer);

    let request = proto::RotateNodeCertificateRequest {
        node_id: "non-existent-node".into(),
        meta: Some(proto::RequestMeta {
            operation_id: "op-1".into(),
            requested_by: "test".into(),
            target_node_id: "non-existent-node".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
    };

    let result = service.rotate_node_certificate(request).await;
    match result {
        Err(ControlPlaneServiceError::NotFound(_)) => { /* Correct */ }
        other => panic!("Expected NotFound error for missing node, got {:?}", other),
    }
}

#[tokio::test]
async fn test_report_bootstrap_result_persistence() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let token_repo = BootstrapTokenRepository::new(pool.clone());
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, cert_issuer);

    // Node must exist
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-b1', 'host-b1', 'host-b1')")
        .execute(&pool)
        .await
        .unwrap();

    let request = proto::ReportBootstrapResultRequest {
        node_id: "node-b1".into(),
        bootstrap_status: "SUCCESS".into(),
        message: "".into(),
        meta: Some(proto::RequestMeta {
            operation_id: "op-b1".into(),
            requested_by: "test".into(),
            target_node_id: "node-b1".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
    };

    service
        .report_bootstrap_result(request)
        .await
        .expect("Report failed");

    // Verify persistence
    let row =
        sqlx::query("SELECT success, operation_id FROM node_bootstrap_results WHERE node_id = $1")
            .bind("node-b1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let success: bool = sqlx::Row::get(&row, "success");
    let operation_id: Option<String> = sqlx::Row::get(&row, "operation_id");
    assert!(success);
    assert_eq!(operation_id, Some("op-b1".to_string()));
}

#[tokio::test]
async fn test_ca_backed_issuer_issuance() {
    // Generate a temporary CA
    use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};
    let mut ca_params = CertificateParams::default();
    ca_params.distinguished_name = DistinguishedName::new();
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "Test CA");
    ca_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);

    let ca_key_pair = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key_pair).unwrap();
    let ca_cert_pem = ca_cert.pem();
    let ca_key_pem = ca_key_pair.serialize_pem();

    let issuer =
        CaBackedCertificateIssuer::new(&ca_cert_pem, &ca_key_pem).expect("Failed to create issuer");
    let node_id = NodeId::new("test-node-1").unwrap();

    let issued = issuer
        .issue_node_certificate(&node_id)
        .await
        .expect("Failed to issue cert");

    // Basic format checks
    assert!(!issued.certificate_pem.is_empty());
    assert!(!issued.private_key_pem.is_empty());
    assert_eq!(issued.ca_pem, ca_cert_pem.as_bytes());

    // Cryptographic chain verification
    use x509_parser::prelude::*;

    // 1. Parse leaf cert from PEM
    let (_, leaf_pem) = parse_x509_pem(&issued.certificate_pem).expect("Failed to parse leaf PEM");
    let (_, leaf) =
        X509Certificate::from_der(&leaf_pem.contents).expect("Failed to parse leaf DER");

    // 2. Verify Subject Common Name
    let cn = leaf
        .subject()
        .iter_common_name()
        .next()
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(cn, node_id.as_str());

    // 3. Verify Signature using CA public key
    let (_, ca_pem_parsed) =
        parse_x509_pem(ca_cert_pem.as_bytes()).expect("Failed to parse CA PEM");
    let (_, ca) =
        X509Certificate::from_der(&ca_pem_parsed.contents).expect("Failed to parse CA DER");

    // Verify signature
    leaf.verify_signature(Some(ca.public_key()))
        .expect("Certificate signature verification failed");
}

#[tokio::test]
async fn test_enrollment_rejects_invalid_bootstrap_token() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let token_repo = chv_controlplane_store::BootstrapTokenRepository::new(pool);
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, cert_issuer);

    let request = proto::EnrollmentRequest {
        bootstrap_token: "invalid-token".into(),
        inventory: Some(proto::NodeInventory {
            node_id: "node-invalid".into(),
            hostname: "host".into(),
            architecture: "x86_64".into(),
            cpu_threads: 1,
            memory_bytes: 1024,
            storage_classes: vec![],
            network_capabilities: vec![],
            labels: Default::default(),
        }),
        versions: Some(proto::ServiceVersions {
            node_id: "node-invalid".into(),
            chv_agent_version: "1.0.0".into(),
            chv_stord_version: "1.0.0".into(),
            chv_nwd_version: "1.0.0".into(),
            cloud_hypervisor_version: "1.0.0".into(),
            host_bundle_version: "1.0.0".into(),
        }),
    };

    let result = service.enroll_node(request).await;
    match result {
        Err(ControlPlaneServiceError::Unauthorized(_)) => { /* Correct */ }
        other => panic!("Expected Unauthorized error for invalid token, got {:?}", other),
    }
}

#[tokio::test]
async fn test_apply_vm_desired_state_persistence() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();

    // Insert a node to satisfy FK constraints
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-vm-1', 'host-vm-1', 'host-vm-1')")
        .execute(&pool)
        .await
        .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());
    let network_exposure_repo = NetworkExposureRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());

    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_state_repo,
        network_exposure_repo,
        event_repo,
    );

    let spec_json = r#"{"cpu_count": 2, "memory_bytes": 4294967296, "image_ref": "ubuntu-22.04"}"#;
    let request = proto::ApplyVmDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-vm-1".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-vm-1".into(),
        vm_id: "vm-1".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "vm-1".into(),
            kind: "Vm".into(),
            generation: "1".into(),
            spec_json: spec_json.as_bytes().to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".into(),
            updated_by: "test-user".into(),
        }),
    };

    let result = service.apply_vm_desired_state(request).await;
    assert!(result.is_ok(), "Expected success, got {:?}", result);

    // Verify persistence in vm_desired_state
    let row = sqlx::query("SELECT vm_id, desired_generation FROM vm_desired_state WHERE vm_id = $1")
        .bind("vm-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let vm_id: String = sqlx::Row::get(&row, "vm_id");
    let desired_generation: i64 = sqlx::Row::get(&row, "desired_generation");
    assert_eq!(vm_id, "vm-1");
    assert_eq!(desired_generation, 1);
}

#[tokio::test]
async fn test_apply_network_desired_state_with_exposures() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();

    // Insert a node to satisfy FK constraints
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-net-1', 'host-net-1', 'host-net-1')")
        .execute(&pool)
        .await
        .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());
    let network_exposure_repo = NetworkExposureRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());

    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_state_repo,
        network_exposure_repo,
        event_repo,
    );

    let spec_json = r#"{"network_class": "bridge", "exposures": [{"service_name": "web", "protocol": "tcp", "listen_port": 80, "target_port": 8080}]}"#;
    let request = proto::ApplyNetworkDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-net-1".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-net-1".into(),
        network_id: "net-1".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "net-1".into(),
            kind: "Network".into(),
            generation: "1".into(),
            spec_json: spec_json.as_bytes().to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".into(),
            updated_by: "test-user".into(),
        }),
    };

    let result = service.apply_network_desired_state(request).await;
    assert!(result.is_ok(), "Expected success, got {:?}", result);

    // Verify persistence in network_exposures
    let row =
        sqlx::query("SELECT network_id, service_name, listen_port FROM network_exposures WHERE network_id = $1")
            .bind("net-1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let network_id: String = sqlx::Row::get(&row, "network_id");
    let service_name: String = sqlx::Row::get(&row, "service_name");
    let listen_port: i32 = sqlx::Row::get(&row, "listen_port");
    assert_eq!(network_id, "net-1");
    assert_eq!(service_name, "web");
    assert_eq!(listen_port, 80);
}

#[tokio::test]
async fn test_apply_rejects_non_numeric_generation() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());
    let network_exposure_repo = NetworkExposureRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());

    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_state_repo,
        network_exposure_repo,
        event_repo,
    );

    let request = proto::ApplyVmDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-bad-gen".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-bad-gen".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-bad-gen".into(),
        vm_id: "vm-bad-gen".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "vm-bad-gen".into(),
            kind: "Vm".into(),
            generation: "not-a-number".into(),
            spec_json: b"{}".to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".into(),
            updated_by: "test-user".into(),
        }),
    };

    let result = service.apply_vm_desired_state(request).await;
    match result {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("generation must be numeric"), "Unexpected message: {}", msg);
        }
        other => panic!(
            "Expected InvalidArgument for non-numeric generation, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_apply_node_desired_state_persistence() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    // seed node
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-reconcile', 'host', 'host')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_at) VALUES ('op-node', 'idem-node', 'node', 'node-reconcile', 'Test', 'Pending', now())")
        .execute(&pool).await.unwrap();
    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let net_repo = NetworkExposureRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(node_repo, desired_repo, net_repo, event_repo);

    let req = proto::ApplyNodeDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-node".into(),
            requested_by: "test".into(),
            target_node_id: "node-reconcile".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-reconcile".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "node-reconcile".into(),
            kind: "Node".into(),
            generation: "1".into(),
            spec_json: br#"{"desired_state": "TenantReady"}"#.to_vec(),
            policy_json: vec![],
            updated_at: "".into(),
            updated_by: "".into(),
        }),
    };

    let resp = service.apply_node_desired_state(req).await.unwrap();
    assert_eq!(resp.result.unwrap().status, "ok");
}

#[tokio::test]
async fn test_apply_rejects_invalid_spec_json() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let net_repo = NetworkExposureRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(node_repo, desired_repo, net_repo, event_repo);

    let req = proto::ApplyVmDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-bad".into(),
            requested_by: "test".into(),
            target_node_id: "".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "".into(),
        vm_id: "vm-1".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "vm-1".into(),
            kind: "Vm".into(),
            generation: "1".into(),
            spec_json: br#"{"unknown_field": true}"#.to_vec(),
            policy_json: vec![],
            updated_at: "".into(),
            updated_by: "".into(),
        }),
    };

    let result = service.apply_vm_desired_state(req).await;
    match result {
        Err(ControlPlaneServiceError::InvalidArgument(_)) => { /* correct */ }
        other => panic!("Expected InvalidArgument for unknown field, got {:?}", other),
    }
}

#[tokio::test]
async fn test_create_vm_creates_operation() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();

    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-lifecycle-1', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let operation_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());

    let service = LifecycleServiceImplementation::new(
        node_repo,
        operation_repo,
        event_repo,
        desired_state_repo,
    );

    let request = proto::CreateVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-lifecycle-1".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-lifecycle-1".into(),
        vm: Some(proto::VmMutationSpec {
            vm_id: "vm-lifecycle-1".into(),
            vm_spec_json: b"{}".to_vec(),
        }),
    };

    let result = service.create_vm(request).await;
    assert!(result.is_ok(), "Expected success, got {:?}", result);

    let row =
        sqlx::query("SELECT operation_id, status::text as status FROM operations WHERE operation_type = 'CreateVm'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    assert_eq!(status, "Pending");
}

#[tokio::test]
async fn test_duplicate_idempotency_returns_same_operation() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();

    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-lifecycle-2', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let operation_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());

    let service = LifecycleServiceImplementation::new(
        node_repo,
        operation_repo,
        event_repo,
        desired_state_repo,
    );

    let request = proto::CreateVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-lifecycle-2".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-lifecycle-2".into(),
        vm: Some(proto::VmMutationSpec {
            vm_id: "vm-lifecycle-2".into(),
            vm_spec_json: b"{}".to_vec(),
        }),
    };

    let result1 = service.create_vm(request.clone()).await.unwrap();
    let result2 = service.create_vm(request).await.unwrap();

    assert_eq!(
        result1.result.unwrap().operation_id,
        result2.result.unwrap().operation_id
    );

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM operations WHERE operation_type = 'CreateVm'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_drain_node_updates_desired_state() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();

    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-lifecycle-3', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let operation_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());

    let service = LifecycleServiceImplementation::new(
        node_repo,
        operation_repo,
        event_repo,
        desired_state_repo,
    );

    let request = proto::DrainNodeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test-user".into(),
            target_node_id: "node-lifecycle-3".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-lifecycle-3".into(),
        allow_workload_stop: false,
    };

    let result = service.drain_node(request).await;
    assert!(result.is_ok(), "Expected success, got {:?}", result);

    let row = sqlx::query("SELECT desired_state::text as desired_state FROM node_desired_state WHERE node_id = $1")
        .bind("node-lifecycle-3")
        .fetch_one(&pool)
        .await
        .unwrap();
    let desired_state: String = sqlx::Row::get(&row, "desired_state");
    assert_eq!(desired_state, "Draining");
}

#[tokio::test]
async fn test_enter_maintenance_updates_desired_state() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-maint', 'host', 'host')")
        .execute(&pool).await.unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::EnterMaintenanceRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-maint".into(),
            requested_by: "test".into(),
            target_node_id: "node-maint".into(),
            desired_state_version: "2".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-maint".into(),
        reason: "upgrade".into(),
    };

    let resp = service.enter_maintenance(req).await.unwrap();
    assert_eq!(resp.result.unwrap().status, "ok");

    let row = sqlx::query("SELECT desired_state::text as desired_state FROM node_desired_state WHERE node_id = $1")
        .bind("node-maint")
        .fetch_one(&pool)
        .await
        .unwrap();
    let state: String = sqlx::Row::get(&row, "desired_state");
    assert_eq!(state, "Maintenance");
}

#[tokio::test]
async fn test_create_vm_writes_desired_state() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-vm', 'host', 'host')")
        .execute(&pool).await.unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::CreateVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-vm".into(),
            requested_by: "test".into(),
            target_node_id: "node-vm".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-vm".into(),
        vm: Some(proto::VmMutationSpec {
            vm_id: "vm-1".into(),
            vm_spec_json: br#"{"cpu_count": 2}"#.to_vec(),
        }),
    };

    let resp = service.create_vm(req).await.unwrap();
    assert_eq!(resp.result.unwrap().status, "ok");

    let row = sqlx::query("SELECT desired_power_state FROM vm_desired_state WHERE vm_id = $1")
        .bind("vm-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let power: String = sqlx::Row::get(&row, "desired_power_state");
    assert_eq!(power, "Created");
}

#[tokio::test]
async fn test_lifecycle_invalid_node_id() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::DrainNodeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-drain".into(),
            requested_by: "test".into(),
            target_node_id: "non-existent-node".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "non-existent-node".into(),
        allow_workload_stop: false,
    };

    let result = service.drain_node(req).await;
    match result {
        Err(ControlPlaneServiceError::NotFound(_)) => { /* correct */ }
        other => panic!("Expected NotFound for invalid node, got {:?}", other),
    }
}
