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

fn test_app_state(pool: StorePool) -> chv_webui_bff::AppState {
    let pool_for_mutations = pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let operation_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let alert_repo = AlertRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());
    let observed_state_repo = ObservedStateRepository::new(pool.clone());
    let lifecycle_service = Arc::new(crate::lifecycle::LifecycleServiceImplementation::new(
        node_repo.clone(),
        operation_repo.clone(),
        event_repo.clone(),
        desired_state_repo.clone(),
    ));
    chv_webui_bff::AppState {
        pool,
        node_repo,
        operation_repo,
        event_repo,
        alert_repo,
        desired_state_repo,
        observed_state_repo,
        mutations: Arc::new(crate::ControlPlaneMutationService::new(
            pool_for_mutations,
            lifecycle_service,
        )),
        jwt_secret: "test-secret".to_string(),
    }
}

#[tokio::test]
async fn test_health_endpoint() {
    use axum::http::StatusCode;
    use tower::ServiceExt;

    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let app = crate::api::router::admin_router(test_app_state(test_db.pool.clone()));

    let response = app
        .oneshot(
            axum::http::Request::get("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_ready_endpoint() {
    use axum::http::StatusCode;
    use tower::ServiceExt;

    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let app = crate::api::router::admin_router(test_app_state(test_db.pool.clone()));

    let response = app
        .oneshot(
            axum::http::Request::get("/ready")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_admin_nodes_endpoint() {
    use axum::http::StatusCode;
    use tower::ServiceExt;

    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-http', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();


    let app = crate::api::router::admin_router(test_app_state(pool));

    let response = app
        .oneshot(
            axum::http::Request::get("/admin/nodes")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!json["nodes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_admin_node_not_found() {
    use axum::http::StatusCode;
    use tower::ServiceExt;

    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let app = crate::api::router::admin_router(test_app_state(test_db.pool.clone()));

    let response = app
        .oneshot(
            axum::http::Request::get("/admin/nodes/missing-node")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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

    sqlx::query("INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_at) VALUES (?, ?, 'node', 'node-1', 'Test', 'Pending', strftime('%Y-%m-%dT%H:%M:%SZ','now'))")
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
    let alert = sqlx::query("SELECT alert_type, operation_id FROM alerts WHERE node_id = ?")
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
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, Some(cert_issuer));

    // Seed a bootstrap token for enrollment (sha256("123"))
    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use) VALUES (?, false)")
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
    let node = sqlx::query("SELECT hostname FROM nodes WHERE node_id = ?")
        .bind("node-new-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let hostname: String = sqlx::Row::get(&node, "hostname");
    assert_eq!(hostname, "host-1");

    let inv = sqlx::query("SELECT storage_classes, labels FROM node_inventory WHERE node_id = ?")
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
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, Some(cert_issuer));

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
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, Some(cert_issuer));

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
        sqlx::query("SELECT success, operation_id FROM node_bootstrap_results WHERE node_id = ?")
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
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, Some(cert_issuer));

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
        other => panic!(
            "Expected Unauthorized error for invalid token, got {:?}",
            other
        ),
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
    let event_repo = EventRepository::new(pool.clone());

    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_state_repo,
        event_repo,
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
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
    let row =
        sqlx::query("SELECT vm_id, desired_generation FROM vm_desired_state WHERE vm_id = ?")
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
    let event_repo = EventRepository::new(pool.clone());

    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_state_repo,
        event_repo,
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
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
    let row = sqlx::query(
        "SELECT network_id, service_name, listen_port FROM network_exposures WHERE network_id = ?",
    )
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
    let event_repo = EventRepository::new(pool.clone());

    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_state_repo,
        event_repo,
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
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
            assert!(
                msg.contains("generation must be numeric"),
                "Unexpected message: {}",
                msg
            );
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
    sqlx::query("INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_at) VALUES ('op-node', 'idem-node', 'node', 'node-reconcile', 'Test', 'Pending', strftime('%Y-%m-%dT%H:%M:%SZ','now'))")
        .execute(&pool).await.unwrap();
    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
    );

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
    let event_repo = EventRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
    );

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
        other => panic!(
            "Expected InvalidArgument for unknown field, got {:?}",
            other
        ),
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
        sqlx::query("SELECT operation_id, status FROM operations WHERE operation_type = 'CreateVm'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    assert_eq!(status, "Accepted");
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

    let row = sqlx::query(
        "SELECT desired_state FROM node_desired_state WHERE node_id = ?",
    )
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
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-maint', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

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
    assert_eq!(resp.result.unwrap().status, "OK");

    let row = sqlx::query(
        "SELECT desired_state FROM node_desired_state WHERE node_id = ?",
    )
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
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-vm', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

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
    assert_eq!(resp.result.unwrap().status, "OK");

    let row = sqlx::query("SELECT desired_power_state FROM vm_desired_state WHERE vm_id = ?")
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

#[test]
fn test_error_to_status_mapping() {
    use tonic::Status;
    let err = ControlPlaneServiceError::NotFound("node-x".into());
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::NotFound);

    let err = ControlPlaneServiceError::InvalidArgument("bad arg".into());
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);

    let err = ControlPlaneServiceError::Unauthorized("no".into());
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::Unauthenticated);

    let err = ControlPlaneServiceError::Conflict("dup".into());
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::AlreadyExists);

    let err = ControlPlaneServiceError::StaleGeneration {
        expected: "5".into(),
        received: "3".into(),
    };
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn test_rotate_certificate_returns_not_found_status() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    let node_repo = NodeRepository::new(pool.clone());
    let token_repo = BootstrapTokenRepository::new(pool);
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, token_repo, Some(cert_issuer));
    let server = crate::server::EnrollmentServer::new(Arc::new(service));

    let request = tonic::Request::new(proto::RotateNodeCertificateRequest {
        node_id: "non-existent-node".into(),
        meta: Some(proto::RequestMeta {
            operation_id: "op-1".into(),
            requested_by: "test".into(),
            target_node_id: "non-existent-node".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
    });

    let result = proto::enrollment_service_server::EnrollmentService::rotate_node_certificate(
        &server, request,
    )
    .await;
    match result {
        Err(status) => assert_eq!(status.code(), tonic::Code::NotFound),
        Ok(_) => panic!("Expected NotFound status"),
    }
}

// ============================================================
// Reconcile contract validation tests
// ============================================================

#[tokio::test]
async fn test_apply_vm_rejects_wrong_target_node() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-a', 'host', 'host'), ('node-b', 'host', 'host')")
        .execute(&pool).await.unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let observed_repo = ObservedStateRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        observed_repo,
        op_repo,
    );

    let req = proto::ApplyVmDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-b".into(), // wrong node
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-a".into(),
        vm_id: "vm-1".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "vm-1".into(),
            kind: "Vm".into(),
            generation: "1".into(),
            spec_json: br#"{"cpu_count": 2}"#.to_vec(),
            policy_json: vec![],
            updated_at: "".into(),
            updated_by: "".into(),
        }),
    };

    match service.apply_vm_desired_state(req).await {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("target_node_id mismatch"), "msg: {}", msg);
        }
        other => panic!(
            "Expected InvalidArgument for wrong target_node_id, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_apply_vm_rejects_wrong_fragment_id() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-a', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let observed_repo = ObservedStateRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        observed_repo,
        op_repo,
    );

    let req = proto::ApplyVmDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-a".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-a".into(),
        vm_id: "vm-1".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "vm-WRONG".into(), // wrong id
            kind: "Vm".into(),
            generation: "1".into(),
            spec_json: br#"{"cpu_count": 2}"#.to_vec(),
            policy_json: vec![],
            updated_at: "".into(),
            updated_by: "".into(),
        }),
    };

    match service.apply_vm_desired_state(req).await {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("fragment.id mismatch"), "msg: {}", msg);
        }
        other => panic!(
            "Expected InvalidArgument for wrong fragment id, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_apply_vm_rejects_wrong_fragment_kind() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-a', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let observed_repo = ObservedStateRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        observed_repo,
        op_repo,
    );

    let req = proto::ApplyVmDesiredStateRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-a".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-a".into(),
        vm_id: "vm-1".into(),
        fragment: Some(proto::DesiredStateFragment {
            id: "vm-1".into(),
            kind: "Volume".into(), // wrong kind
            generation: "1".into(),
            spec_json: br#"{"cpu_count": 2}"#.to_vec(),
            policy_json: vec![],
            updated_at: "".into(),
            updated_by: "".into(),
        }),
    };

    match service.apply_vm_desired_state(req).await {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("fragment.kind mismatch"), "msg: {}", msg);
        }
        other => panic!(
            "Expected InvalidArgument for wrong fragment kind, got {:?}",
            other
        ),
    }
}

// ============================================================
// Acknowledge desired state tests
// ============================================================

#[tokio::test]
async fn test_acknowledge_desired_state_version_persists_observed_generation() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-ack', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ('vm-ack', 'vm-ack')")
        .execute(&pool)
        .await
        .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let observed_repo = ObservedStateRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        observed_repo,
        op_repo,
    );

    let req = proto::AcknowledgeDesiredStateVersionRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-ack".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-ack".into(),
        fragment_kind: "Vm".into(),
        fragment_id: "vm-ack".into(),
        observed_generation: "5".into(),
        apply_status: "ok".into(),
    };

    let resp = service
        .acknowledge_desired_state_version(req)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap().node_observed_generation, "5");

    let row = sqlx::query("SELECT observed_generation FROM vm_observed_state WHERE vm_id = ?")
        .bind("vm-ack")
        .fetch_one(&pool)
        .await
        .unwrap();
    let gen: i64 = sqlx::Row::get(&row, "observed_generation");
    assert_eq!(gen, 5);
}

#[tokio::test]
async fn test_acknowledge_advances_operation_to_succeeded() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-ack2', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ('vm-ack2', 'vm-ack2')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_at) VALUES ('op-ack2', 'idem-ack2', 'vm', 'vm-ack2', 'Test', 'Pending', strftime('%Y-%m-%dT%H:%M:%SZ','now'))")
        .execute(&pool).await.unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let observed_repo = ObservedStateRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        observed_repo,
        op_repo,
    );

    let req = proto::AcknowledgeDesiredStateVersionRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-ack2".into(),
            requested_by: "test".into(),
            target_node_id: "node-ack2".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-ack2".into(),
        fragment_kind: "Vm".into(),
        fragment_id: "vm-ack2".into(),
        observed_generation: "3".into(),
        apply_status: "".into(), // empty defaults to ok/Succeeded
    };

    service
        .acknowledge_desired_state_version(req)
        .await
        .unwrap();

    let row = sqlx::query("SELECT status, observed_generation FROM operations WHERE operation_id = ?")
        .bind("op-ack2")
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    let gen: i64 = sqlx::Row::get(&row, "observed_generation");
    assert_eq!(status, "Succeeded");
    assert_eq!(gen, 3);
}

#[tokio::test]
async fn test_acknowledge_preserves_existing_vm_runtime_status() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-ack-runtime', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, display_name) VALUES ('vm-ack-runtime', 'vm-ack-runtime')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vm_observed_state (vm_id, observed_generation, runtime_status, observed_at, updated_at) VALUES ('vm-ack-runtime', 1, 'Running', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = ReconcileServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
    );

    let req = proto::AcknowledgeDesiredStateVersionRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-ack-runtime".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-ack-runtime".into(),
        fragment_kind: "Vm".into(),
        fragment_id: "vm-ack-runtime".into(),
        observed_generation: "7".into(),
        apply_status: "conflict".into(),
    };

    service
        .acknowledge_desired_state_version(req)
        .await
        .unwrap();

    let row = sqlx::query(
        "SELECT observed_generation, runtime_status FROM vm_observed_state WHERE vm_id = ?",
    )
    .bind("vm-ack-runtime")
    .fetch_one(&pool)
    .await
    .unwrap();
    let generation: i64 = sqlx::Row::get(&row, "observed_generation");
    let runtime_status: String = sqlx::Row::get(&row, "runtime_status");
    assert_eq!(generation, 7);
    assert_eq!(runtime_status, "Running");
}

#[tokio::test]
async fn test_acknowledge_rejects_unknown_fragment_kind() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-ack3', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let observed_repo = ObservedStateRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let service = ReconcileServiceImplementation::new(
        node_repo,
        desired_repo,
        event_repo,
        observed_repo,
        op_repo,
    );

    let req = proto::AcknowledgeDesiredStateVersionRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-ack3".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-ack3".into(),
        fragment_kind: "UnknownKind".into(),
        fragment_id: "x".into(),
        observed_generation: "1".into(),
        apply_status: "".into(),
    };

    match service.acknowledge_desired_state_version(req).await {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("invalid fragment_kind"), "msg: {}", msg);
        }
        other => panic!(
            "Expected InvalidArgument for unknown fragment kind, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_acknowledge_rejects_unknown_apply_status() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-ack4', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ('vm-ack4', 'vm-ack4')")
        .execute(&pool)
        .await
        .unwrap();

    let service = ReconcileServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        ObservedStateRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
    );

    let req = proto::AcknowledgeDesiredStateVersionRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-ack4".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-ack4".into(),
        fragment_kind: "Vm".into(),
        fragment_id: "vm-ack4".into(),
        observed_generation: "2".into(),
        apply_status: "mystery-status".into(),
    };

    match service.acknowledge_desired_state_version(req).await {
        Err(ControlPlaneServiceError::InvalidArgument(msg)) => {
            assert!(msg.contains("invalid apply_status"), "msg: {}", msg);
        }
        other => panic!(
            "Expected InvalidArgument for unknown apply_status, got {:?}",
            other
        ),
    }
}

// ============================================================
// Lifecycle durable intent tests
// ============================================================

#[tokio::test]
async fn test_start_vm_persists_desired_power_state_running() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-start', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, node_id, display_name) VALUES ('vm-start', 'node-start', 'vm-start')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::StartVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-start".into(),
            requested_by: "test".into(),
            target_node_id: "node-start".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-start".into(),
        vm_id: "vm-start".into(),
    };

    service.start_vm(req).await.unwrap();

    let row = sqlx::query(
        "SELECT desired_power_state, desired_status FROM vm_desired_state WHERE vm_id = ?",
    )
    .bind("vm-start")
    .fetch_one(&pool)
    .await
    .unwrap();
    let power: Option<String> = sqlx::Row::get(&row, "desired_power_state");
    let status: Option<String> = sqlx::Row::get(&row, "desired_status");
    assert_eq!(power, Some("Running".to_string()));
    assert!(status.is_none());
}

#[tokio::test]
async fn test_start_vm_preserves_existing_vm_shape() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-vm-preserve', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, node_id, display_name, tenant_id, placement_policy) VALUES ('vm-preserve', 'node-vm-preserve', 'vm-preserve', 'tenant-a', 'balanced')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vm_desired_state (vm_id, desired_generation, desired_status, requested_at, updated_at, target_node_id, cpu_count, memory_bytes, image_ref, boot_mode, desired_power_state) VALUES ('vm-preserve', 1, 'seeded', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'node-vm-preserve', 4, 8192, 'image-a', 'uefi', 'Stopped')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::StartVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-vm-preserve".into(),
            desired_state_version: "2".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-vm-preserve".into(),
        vm_id: "vm-preserve".into(),
    };

    service.start_vm(req).await.unwrap();

    let row = sqlx::query(
        "SELECT cpu_count, memory_bytes, image_ref, boot_mode, desired_power_state FROM vm_desired_state WHERE vm_id = ?",
    )
    .bind("vm-preserve")
    .fetch_one(&pool)
    .await
    .unwrap();
    let cpu_count: Option<i32> = sqlx::Row::get(&row, "cpu_count");
    let memory_bytes: Option<i64> = sqlx::Row::get(&row, "memory_bytes");
    let image_ref: Option<String> = sqlx::Row::get(&row, "image_ref");
    let boot_mode: Option<String> = sqlx::Row::get(&row, "boot_mode");
    let power: Option<String> = sqlx::Row::get(&row, "desired_power_state");
    assert_eq!(cpu_count, Some(4));
    assert_eq!(memory_bytes, Some(8192));
    assert_eq!(image_ref, Some("image-a".to_string()));
    assert_eq!(boot_mode, Some("uefi".to_string()));
    assert_eq!(power, Some("Running".to_string()));
}

#[tokio::test]
async fn test_stop_vm_persists_desired_power_state_stopped() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-stop', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, node_id, display_name) VALUES ('vm-stop', 'node-stop', 'vm-stop')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::StopVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-stop".into(),
            requested_by: "test".into(),
            target_node_id: "node-stop".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-stop".into(),
        vm_id: "vm-stop".into(),
        force: false,
    };

    service.stop_vm(req).await.unwrap();

    let row = sqlx::query("SELECT desired_power_state FROM vm_desired_state WHERE vm_id = ?")
        .bind("vm-stop")
        .fetch_one(&pool)
        .await
        .unwrap();
    let power: Option<String> = sqlx::Row::get(&row, "desired_power_state");
    assert_eq!(power, Some("Stopped".to_string()));
}

#[tokio::test]
async fn test_reboot_vm_persists_desired_power_state_rebooting() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-reboot', 'host', 'host')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO vms (vm_id, node_id, display_name) VALUES ('vm-reboot', 'node-reboot', 'vm-reboot')")
        .execute(&pool)
        .await
        .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::RebootVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-reboot".into(),
            requested_by: "test".into(),
            target_node_id: "node-reboot".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-reboot".into(),
        vm_id: "vm-reboot".into(),
        force: false,
    };

    service.reboot_vm(req).await.unwrap();

    let row = sqlx::query("SELECT desired_power_state FROM vm_desired_state WHERE vm_id = ?")
        .bind("vm-reboot")
        .fetch_one(&pool)
        .await
        .unwrap();
    let power: Option<String> = sqlx::Row::get(&row, "desired_power_state");
    assert_eq!(power, Some("Rebooting".to_string()));
}

#[tokio::test]
async fn test_delete_vm_persists_desired_power_state_deleted() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-del', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, node_id, display_name) VALUES ('vm-del', 'node-del', 'vm-del')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::DeleteVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-del".into(),
            requested_by: "test".into(),
            target_node_id: "node-del".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-del".into(),
        vm_id: "vm-del".into(),
        force: false,
    };

    service.delete_vm(req).await.unwrap();

    let row = sqlx::query("SELECT desired_power_state FROM vm_desired_state WHERE vm_id = ?")
        .bind("vm-del")
        .fetch_one(&pool)
        .await
        .unwrap();
    let power: Option<String> = sqlx::Row::get(&row, "desired_power_state");
    assert_eq!(power, Some("Deleted".to_string()));
}

#[tokio::test]
async fn test_attach_volume_persists_attached_vm_id() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-attach', 'host', 'host')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ('vm-attach', 'vm-attach')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes) VALUES ('vol-attach', 'node-attach', 'vol-attach', 1024)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::AttachVolumeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-attach".into(),
            requested_by: "test".into(),
            target_node_id: "node-attach".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-attach".into(),
        volume: Some(proto::VolumeMutationSpec {
            volume_id: "vol-attach".into(),
            vm_id: "vm-attach".into(),
            volume_spec_json: vec![],
        }),
    };

    service.attach_volume(req).await.unwrap();

    let row = sqlx::query("SELECT attached_vm_id FROM volume_desired_state WHERE volume_id = ?")
        .bind("vol-attach")
        .fetch_one(&pool)
        .await
        .unwrap();
    let vm_id: Option<String> = sqlx::Row::get(&row, "attached_vm_id");
    assert_eq!(vm_id, Some("vm-attach".to_string()));
}

#[tokio::test]
async fn test_detach_volume_clears_attached_vm_id() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-detach', 'host', 'host')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ('vm-detach', 'vm-detach')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes) VALUES ('vol-detach', 'node-detach', 'vol-detach', 1024)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::DetachVolumeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-detach".into(),
            requested_by: "test".into(),
            target_node_id: "node-detach".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-detach".into(),
        vm_id: "vm-detach".into(),
        volume_id: "vol-detach".into(),
        force: false,
    };

    service.detach_volume(req).await.unwrap();

    let row = sqlx::query("SELECT attached_vm_id FROM volume_desired_state WHERE volume_id = ?")
        .bind("vol-detach")
        .fetch_one(&pool)
        .await
        .unwrap();
    let vm_id: Option<String> = sqlx::Row::get(&row, "attached_vm_id");
    assert!(vm_id.is_none());
}

#[tokio::test]
async fn test_resize_volume_persists_resize_to_bytes() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-resize', 'host', 'host')")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes) VALUES ('vol-resize', 'node-resize', 'vol-resize', 1024)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::ResizeVolumeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-resize".into(),
            requested_by: "test".into(),
            target_node_id: "node-resize".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-resize".into(),
        volume_id: "vol-resize".into(),
        new_size_bytes: 21474836480,
    };

    service.resize_volume(req).await.unwrap();

    let row = sqlx::query("SELECT resize_to_bytes FROM volume_desired_state WHERE volume_id = ?")
        .bind("vol-resize")
        .fetch_one(&pool)
        .await
        .unwrap();
    let size: Option<i64> = sqlx::Row::get(&row, "resize_to_bytes");
    assert_eq!(size, Some(21474836480));
}

#[tokio::test]
async fn test_resize_volume_preserves_existing_volume_shape() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-vol-preserve', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, display_name) VALUES ('vm-preserve-attach', 'vm-preserve-attach')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes, volume_kind, storage_class) VALUES ('vol-preserve', 'node-vol-preserve', 'vol-preserve', 4096, 'Block', 'ssd')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO volume_desired_state (volume_id, desired_generation, desired_status, requested_at, updated_at, attached_vm_id, attachment_mode, device_name, read_only, resize_to_bytes) VALUES ('vol-preserve', 1, 'seeded', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'vm-preserve-attach', 'rw', '/dev/vdb', true, NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::ResizeVolumeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-vol-preserve".into(),
            desired_state_version: "2".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-vol-preserve".into(),
        volume_id: "vol-preserve".into(),
        new_size_bytes: 16384,
    };

    service.resize_volume(req).await.unwrap();

    let volume = sqlx::query(
        "SELECT capacity_bytes, volume_kind, storage_class FROM volumes WHERE volume_id = ?",
    )
    .bind("vol-preserve")
    .fetch_one(&pool)
    .await
    .unwrap();
    let capacity_bytes: i64 = sqlx::Row::get(&volume, "capacity_bytes");
    let volume_kind: Option<String> = sqlx::Row::get(&volume, "volume_kind");
    let storage_class: Option<String> = sqlx::Row::get(&volume, "storage_class");
    assert_eq!(capacity_bytes, 4096);
    assert_eq!(volume_kind, Some("Block".to_string()));
    assert_eq!(storage_class, Some("ssd".to_string()));

    let desired = sqlx::query(
        "SELECT attached_vm_id, attachment_mode, device_name, read_only, resize_to_bytes FROM volume_desired_state WHERE volume_id = ?",
    )
    .bind("vol-preserve")
    .fetch_one(&pool)
    .await
    .unwrap();
    let attached_vm_id: Option<String> = sqlx::Row::get(&desired, "attached_vm_id");
    let attachment_mode: Option<String> = sqlx::Row::get(&desired, "attachment_mode");
    let device_name: Option<String> = sqlx::Row::get(&desired, "device_name");
    let read_only: bool = sqlx::Row::get(&desired, "read_only");
    let resize_to_bytes: Option<i64> = sqlx::Row::get(&desired, "resize_to_bytes");
    assert_eq!(attached_vm_id, Some("vm-preserve-attach".to_string()));
    assert_eq!(attachment_mode, Some("rw".to_string()));
    assert_eq!(device_name, Some("/dev/vdb".to_string()));
    assert!(read_only);
    assert_eq!(resize_to_bytes, Some(16384));
}

#[tokio::test]
async fn test_pause_node_scheduling_persists_scheduling_paused() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-pause', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO node_desired_state (node_id, desired_generation, desired_state, requested_at, updated_at, scheduling_paused) VALUES ('node-pause', 1, 'TenantReady', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), false)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::PauseNodeSchedulingRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-pause".into(),
            requested_by: "test".into(),
            target_node_id: "node-pause".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-pause".into(),
    };

    service.pause_node_scheduling(req).await.unwrap();

    let row = sqlx::query("SELECT scheduling_paused FROM node_desired_state WHERE node_id = ?")
        .bind("node-pause")
        .fetch_one(&pool)
        .await
        .unwrap();
    let paused: bool = sqlx::Row::get(&row, "scheduling_paused");
    assert!(paused);
}

#[tokio::test]
async fn test_pause_node_scheduling_preserves_existing_desired_state() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-pause-preserve', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO node_desired_state (node_id, desired_generation, desired_state, requested_at, updated_at, scheduling_paused, allow_workload_stop) VALUES ('node-pause-preserve', 1, 'Draining', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), false, true)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::PauseNodeSchedulingRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-pause-preserve".into(),
            desired_state_version: "2".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-pause-preserve".into(),
    };

    service.pause_node_scheduling(req).await.unwrap();

    let row = sqlx::query(
        "SELECT desired_state, scheduling_paused, allow_workload_stop FROM node_desired_state WHERE node_id = ?",
    )
    .bind("node-pause-preserve")
    .fetch_one(&pool)
    .await
    .unwrap();
    let desired_state: String = sqlx::Row::get(&row, "desired_state");
    let scheduling_paused: bool = sqlx::Row::get(&row, "scheduling_paused");
    let allow_workload_stop: Option<bool> = sqlx::Row::get(&row, "allow_workload_stop");
    assert_eq!(desired_state, "Draining");
    assert!(scheduling_paused);
    assert_eq!(allow_workload_stop, Some(true));
}

#[tokio::test]
async fn test_resume_node_scheduling_clears_scheduling_paused() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-resume', 'host', 'host')")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO node_desired_state (node_id, desired_generation, desired_state, requested_at, updated_at, scheduling_paused) VALUES ('node-resume', 1, 'TenantReady', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), true)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::ResumeNodeSchedulingRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-resume".into(),
            requested_by: "test".into(),
            target_node_id: "node-resume".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-resume".into(),
    };

    service.resume_node_scheduling(req).await.unwrap();

    let row = sqlx::query("SELECT scheduling_paused FROM node_desired_state WHERE node_id = ?")
        .bind("node-resume")
        .fetch_one(&pool)
        .await
        .unwrap();
    let paused: bool = sqlx::Row::get(&row, "scheduling_paused");
    assert!(!paused);
}

#[tokio::test]
async fn test_pause_node_scheduling_rejects_missing_desired_state() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-pause-missing', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::PauseNodeSchedulingRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-pause-missing".into(),
            requested_by: "test".into(),
            target_node_id: "node-pause-missing".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-pause-missing".into(),
    };

    let result = service.pause_node_scheduling(req).await;
    match result {
        Err(ControlPlaneServiceError::NotFound(msg)) => {
            assert!(msg.contains("node_desired_state"), "msg: {}", msg);
        }
        other => panic!(
            "Expected NotFound for missing desired state, got {:?}",
            other
        ),
    }

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM node_desired_state WHERE node_id = ?")
            .bind("node-pause-missing")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "no fabricated row should be created");
}

#[tokio::test]
async fn test_resume_node_scheduling_rejects_missing_desired_state() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-resume-missing', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::ResumeNodeSchedulingRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-resume-missing".into(),
            requested_by: "test".into(),
            target_node_id: "node-resume-missing".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-resume-missing".into(),
    };

    let result = service.resume_node_scheduling(req).await;
    match result {
        Err(ControlPlaneServiceError::NotFound(msg)) => {
            assert!(msg.contains("node_desired_state"), "msg: {}", msg);
        }
        other => panic!(
            "Expected NotFound for missing desired state, got {:?}",
            other
        ),
    }

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM node_desired_state WHERE node_id = ?")
            .bind("node-resume-missing")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "no fabricated row should be created");
}

#[tokio::test]
async fn test_exit_maintenance_persists_tenant_ready() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-exit', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::ExitMaintenanceRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-exit".into(),
            requested_by: "test".into(),
            target_node_id: "node-exit".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-exit".into(),
    };

    service.exit_maintenance(req).await.unwrap();

    let row = sqlx::query("SELECT desired_state, scheduling_paused FROM node_desired_state WHERE node_id = ?")
        .bind("node-exit")
        .fetch_one(&pool)
        .await
        .unwrap();
    let state: String = sqlx::Row::get(&row, "desired_state");
    let paused: bool = sqlx::Row::get(&row, "scheduling_paused");
    assert_eq!(state, "TenantReady");
    assert!(!paused);
}

#[tokio::test]
async fn test_drain_node_persists_allow_workload_stop() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query("INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-drain2', 'host', 'host')")
        .execute(&pool).await.unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::DrainNodeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-drain2".into(),
            requested_by: "test".into(),
            target_node_id: "node-drain2".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-drain2".into(),
        allow_workload_stop: true,
    };

    service.drain_node(req).await.unwrap();

    let row = sqlx::query("SELECT allow_workload_stop FROM node_desired_state WHERE node_id = ?")
        .bind("node-drain2")
        .fetch_one(&pool)
        .await
        .unwrap();
    let allow: Option<bool> = sqlx::Row::get(&row, "allow_workload_stop");
    assert_eq!(allow, Some(true));
}

#[tokio::test]
async fn test_lifecycle_operation_accepted_after_intent_persisted() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-op', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO vms (vm_id, node_id, display_name) VALUES ('vm-op', 'node-op', 'vm-op')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let node_repo = NodeRepository::new(pool.clone());
    let op_repo = OperationRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let desired_repo = DesiredStateRepository::new(pool.clone());
    let service = LifecycleServiceImplementation::new(node_repo, op_repo, event_repo, desired_repo);

    let req = proto::StartVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-op".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-op".into(),
        vm_id: "vm-op".into(),
    };

    let resp = service.start_vm(req).await.unwrap();
    let op_id = resp.result.unwrap().operation_id;

    let row = sqlx::query("SELECT status FROM operations WHERE operation_id = ?")
        .bind(&op_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    assert_eq!(status, "Accepted");
}

#[tokio::test]
async fn test_resize_volume_payload_changes_affect_idempotency() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-idem-resize', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes) VALUES ('vol-idem-resize', 'node-idem-resize', 'vol-idem-resize', 1024)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req_one = proto::ResizeVolumeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-idem-resize".into(),
            desired_state_version: "9".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-idem-resize".into(),
        volume_id: "vol-idem-resize".into(),
        new_size_bytes: 2048,
    };

    let req_two = proto::ResizeVolumeRequest {
        new_size_bytes: 4096,
        ..req_one.clone()
    };

    let op_one = service.resize_volume(req_one).await.unwrap();
    let op_two = service.resize_volume(req_two).await.unwrap();

    let op_id_one = op_one.result.unwrap().operation_id;
    let op_id_two = op_two.result.unwrap().operation_id;
    assert_ne!(op_id_one, op_id_two);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM operations WHERE operation_type = 'ResizeVolume'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_start_vm_rejects_missing_vm() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-missing-vm', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::StartVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-missing-vm".into(),
            desired_state_version: "2".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-missing-vm".into(),
        vm_id: "vm-missing".into(),
    };

    match service.start_vm(req).await {
        Err(ControlPlaneServiceError::NotFound(msg)) => {
            assert!(msg.contains("vm"), "msg: {}", msg);
        }
        other => panic!("Expected NotFound for missing vm, got {:?}", other),
    }
}

#[tokio::test]
async fn test_resize_volume_rejects_missing_volume() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-missing-vol', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::ResizeVolumeRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "".into(),
            requested_by: "test".into(),
            target_node_id: "node-missing-vol".into(),
            desired_state_version: "2".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-missing-vol".into(),
        volume_id: "vol-missing".into(),
        new_size_bytes: 4096,
    };

    match service.resize_volume(req).await {
        Err(ControlPlaneServiceError::NotFound(msg)) => {
            assert!(msg.contains("volume"), "msg: {}", msg);
        }
        other => panic!("Expected NotFound for missing volume, got {:?}", other),
    }
}

#[tokio::test]
async fn test_start_vm_fails_operation_when_vm_missing() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-op-fail', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::StartVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-fail-test".into(),
            requested_by: "test".into(),
            target_node_id: "node-op-fail".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-op-fail".into(),
        vm_id: "vm-missing-for-op".into(),
    };

    let result = service.start_vm(req).await;
    match result {
        Err(ControlPlaneServiceError::NotFound(msg)) => {
            assert!(msg.contains("vm"), "msg: {}", msg);
        }
        other => panic!("Expected NotFound for missing vm, got {:?}", other),
    }

    // Look up the operation by idempotency key to get the actual operation_id
    let operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM operations WHERE idempotency_key = ?")
            .bind("request:op-fail-test")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Operation should be Failed, not Pending or Accepted
    let row = sqlx::query("SELECT status FROM operations WHERE operation_id = ?")
        .bind(&operation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    assert_eq!(status, "Failed");

    // OperationFailed event should exist
    let event = sqlx::query(
        "SELECT event_type FROM events WHERE operation_id = ? AND event_type = 'OperationFailed'",
    )
    .bind(&operation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let event_type: String = sqlx::Row::get(&event, "event_type");
    assert_eq!(event_type, "OperationFailed");
}

#[tokio::test]
async fn test_failed_operation_can_be_retried_idempotently() {
    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let pool = test_db.pool.clone();
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('node-op-retry', 'host', 'host')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = LifecycleServiceImplementation::new(
        NodeRepository::new(pool.clone()),
        OperationRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        DesiredStateRepository::new(pool.clone()),
    );

    let req = proto::StartVmRequest {
        meta: Some(proto::RequestMeta {
            operation_id: "op-retry-test".into(),
            requested_by: "test".into(),
            target_node_id: "node-op-retry".into(),
            desired_state_version: "1".into(),
            request_unix_ms: 1000,
        }),
        node_id: "node-op-retry".into(),
        vm_id: "vm-retry-later".into(),
    };

    // First attempt fails because VM does not exist
    let result1 = service.start_vm(req.clone()).await;
    assert!(result1.is_err(), "first attempt should fail");

    // Retrieve the actual generated operation_id
    let operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM operations WHERE idempotency_key = ?")
            .bind("request:op-retry-test")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Create the VM row so the retry can succeed
    sqlx::query(
        "INSERT INTO vms (vm_id, node_id, display_name) VALUES ('vm-retry-later', 'node-op-retry', 'vm-retry-later')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Retry with the exact same request
    let resp2 = service.start_vm(req).await.unwrap();
    let op_id2 = resp2.result.unwrap().operation_id;

    // Should get the same operation idempotently
    assert_eq!(op_id2, operation_id);

    let row = sqlx::query("SELECT status FROM operations WHERE operation_id = ?")
        .bind(&operation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    assert_eq!(status, "Accepted");
}
