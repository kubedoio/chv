use crate::*;
use chv_controlplane_store::*;
use chv_controlplane_types::domain::NodeId;
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;

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
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, cert_issuer);

    let mut labels = std::collections::HashMap::new();
    labels.insert("env".to_string(), "prod".to_string());

    let request = proto::EnrollmentRequest {
        bootstrap_token: "token-123".into(),
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
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, cert_issuer);

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
    let cert_issuer = Arc::new(MockCertIssuer);
    let service = EnrollmentServiceImplementation::new(node_repo, cert_issuer);

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
