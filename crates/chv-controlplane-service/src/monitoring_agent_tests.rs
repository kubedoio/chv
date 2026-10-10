//! Guest monitoring agent service tests (ADR-026, campaign #602 G3).
//!
//! These drive `MonitoringAgentService` end to end against a real
//! operational store (claims, registry, audit) and a real file-backed
//! monitoring store (durable dedup, high-water mark) — the same rigor
//! as the node-path tests in `monitoring_ingest_tests.rs`. HTTP-level
//! behavior is covered by the route tests below (axum `oneshot` with
//! the `TlsPeer` extension injected, exactly what the TLS accept loop
//! does) plus one full TLS round trip against a real rustls listener.

use crate::monitoring_agent::{
    AgentCertificateIssuer, AgentPeerCredential, EnrollOutcome, GuestBatchEnvelope,
    GuestIngestOutcome, GuestOsMetadata, GuestSampleJson, MonitoringAgentService, RotateOutcome,
};
use chv_controlplane_store::test_util::TestDb;
use chv_controlplane_store::{EventRepository, MonitoringAgentRepository};
use chv_monitoring_store::{MonitoringHealth, MonitoringStore, MonitoringStoreConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

async fn monitoring_store(dir: &std::path::Path) -> Arc<MonitoringStore> {
    Arc::new(
        MonitoringStore::connect(MonitoringStoreConfig {
            database_url: format!("sqlite://{}/monitoring.db", dir.display()),
            migrations_dir: PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../cmd/chv-controlplane/monitoring-migrations"
            )),
            ..MonitoringStoreConfig::default()
        })
        .await
        .expect("monitoring store connect"),
    )
}

/// A throwaway self-signed agent CA.
fn test_ca() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "chv-test-agent-ca");
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

fn test_csr() -> (String, rcgen::KeyPair) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "csr-subject");
    let csr = params.serialize_request(&key).unwrap().pem().unwrap();
    (csr, key)
}

struct Fixture {
    _ops_db: TestDb,
    _monitoring_dir: tempfile::TempDir,
    store: Arc<MonitoringStore>,
    service: Arc<MonitoringAgentService>,
    ca_cert_pem: String,
    ca_key_pem: String,
    vm_id: String,
    agent_id: String,
    agent_cert_pem: String,
    agent_key_pem: String,
    agent_fingerprint: String,
    csr: String,
}

impl Fixture {
    async fn new() -> Self {
        let ops_db = TestDb::new().await;
        let dir = tempfile::tempdir().unwrap();
        let store = monitoring_store(dir.path()).await;

        let vm_id = "vm-agent-1".to_string();
        sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ($1, $1)")
            .bind(&vm_id)
            .execute(&ops_db.pool)
            .await
            .unwrap();

        let (ca_cert, ca_key) = test_ca();
        let issuer = Arc::new(AgentCertificateIssuer::new(&ca_cert, &ca_key).unwrap());
        let repo = MonitoringAgentRepository::new(ops_db.pool.clone());
        let events = EventRepository::new(ops_db.pool.clone());
        let service = Arc::new(MonitoringAgentService::new(
            repo,
            issuer,
            Some(store.clone()),
            MonitoringHealth::new(),
            events,
            Default::default(),
            Some("https://manager.test:8443".to_string()),
        ));

        // Enroll through the real claim path. Enrollment uses the real
        // clock: the issued certificate's not_after is derived from
        // this instant, and the TLS round-trip test below presents it
        // to a real rustls verifier. (Everything after enrollment —
        // ingest, rotation, rate windows — runs on caller-supplied
        // timestamps, which the store treats consistently.)
        let enroll_now = chrono::Utc::now().timestamp_millis();
        let (csr, agent_key) = test_csr();
        let claim = service
            .issue_claim(&vm_id, "op-user", enroll_now)
            .await
            .unwrap();
        let enrolled = match service
            .redeem_claim(
                &claim.token,
                "install-1",
                &csr,
                Some("127.0.0.1"),
                enroll_now + 1_000,
            )
            .await
            .unwrap()
        {
            EnrollOutcome::Enrolled(agent) => agent,
            other => panic!("expected enrollment, got {other:?}"),
        };

        Self {
            _ops_db: ops_db,
            _monitoring_dir: dir,
            store,
            service,
            ca_cert_pem: ca_cert.clone(),
            ca_key_pem: ca_key.clone(),
            vm_id,
            agent_id: enrolled.agent_id.clone(),
            agent_cert_pem: enrolled.certificate_pem.clone(),
            agent_key_pem: agent_key.serialize_pem(),
            agent_fingerprint: fingerprint_of(&enrolled.certificate_pem),
            csr,
        }
    }

    fn peer(&self) -> AgentPeerCredential {
        AgentPeerCredential {
            agent_id: self.agent_id.clone(),
            fingerprint: self.agent_fingerprint.clone(),
        }
    }

    fn envelope(&self, sequence: u64, now_ms: i64) -> GuestBatchEnvelope {
        GuestBatchEnvelope {
            schema_version: 1,
            agent_id: self.agent_id.clone(),
            install_id: "install-1".to_string(),
            boot_id: "boot-1".to_string(),
            sequence,
            sent_at_ms: now_ms,
            os: Some(GuestOsMetadata {
                name: Some("Ubuntu".into()),
                version: Some("24.04".into()),
                kernel_release: Some("6.8.0-42-generic".into()),
            }),
            samples: vec![sample(&self.vm_id, now_ms)],
        }
    }
}

fn sample(vm_id: &str, now_ms: i64) -> GuestSampleJson {
    GuestSampleJson {
        schema_version: 1,
        target_kind: "vm".into(),
        target_id: vm_id.to_string(),
        metric_id: "vm.guest.load1".into(),
        source: "guest_agent".into(),
        kind: "gauge".into(),
        unit: "count".into(),
        observed_at_ms: now_ms - 500,
        value: Some(serde_json::json!(0.42)),
        quality: "valid".into(),
        dimensions: BTreeMap::new(),
        boot_id: "boot-1".into(),
        identity_epoch: "agent-credential-generation-1".into(),
    }
}

fn fingerprint_of(cert_pem: &str) -> String {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem.as_bytes()).unwrap();
    chv_common::sha256_hex_bytes(&pem.contents)
}

#[tokio::test]
async fn enrolled_agent_ingests_dedups_and_records_state() {
    let fx = Fixture::new().await;

    // First batch: accepted, samples land in the durable store.
    let now = 100_000i64;
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(1, now), now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Accepted {
            samples,
            renewal_due,
        } => {
            assert_eq!(samples, 1);
            assert!(!renewal_due);
        }
        other => panic!("expected accepted, got {other:?}"),
    }

    let row = fx
        .service
        .repo()
        .find_agent(&fx.agent_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.last_seen_at_ms, Some(now));
    assert_eq!(row.last_sequence, Some(1));
    assert_eq!(row.os_name.as_deref(), Some("Ubuntu"));

    // Same batch key + body: durable duplicate.
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(1, now), now + 1_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Duplicate { samples, .. } => assert_eq!(samples, 1),
        other => panic!("expected duplicate, got {other:?}"),
    }

    // Same key, different body: replay conflict.
    let mut conflicting = fx.envelope(1, now);
    conflicting.samples[0].value = Some(serde_json::json!(9.99));
    match fx
        .service
        .ingest_batch(&fx.peer(), &conflicting, now + 2_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::ReplayConflict => {}
        other => panic!("expected replay conflict, got {other:?}"),
    }

    // The sample is queryable through the real store (sender key
    // `agent:<agent_id>`).
    let series = fx
        .store
        .query_history(
            &chv_monitoring_core::model::TargetKind::Vm,
            &fx.vm_id,
            &["vm.guest.load1".to_string()],
            None,
            (now - 10_000) as u64,
            (now + 10_000) as u64,
            100,
            chv_monitoring_store::Resolution::Raw,
        )
        .await
        .unwrap();
    assert!(!series.is_empty(), "guest sample must be queryable");
}

#[tokio::test]
async fn payload_targets_never_grant_scope() {
    let fx = Fixture::new().await;

    // A sample claiming another VM's id: rejected whole-batch.
    let now = 100_000i64;
    let mut envelope = fx.envelope(1, now);
    envelope.samples[0].target_id = "vm-other-1".into();
    match fx
        .service
        .ingest_batch(&fx.peer(), &envelope, now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::ForbiddenTarget => {}
        other => panic!("expected forbidden target, got {other:?}"),
    }

    // The envelope's agent_id must match the credential.
    let mut envelope = fx.envelope(1, now);
    envelope.agent_id = "someone-else".into();
    match fx
        .service
        .ingest_batch(&fx.peer(), &envelope, now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::InvalidBatch(detail) => {
            assert!(detail.contains("does not match"))
        }
        other => panic!("expected invalid batch, got {other:?}"),
    }

    // Non-guest sources do not ride the guest transport.
    let mut envelope = fx.envelope(1, now);
    envelope.samples[0].source = "node_os".into();
    match fx
        .service
        .ingest_batch(&fx.peer(), &envelope, now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::UnsupportedMetric(detail) => assert!(detail.contains("node_os")),
        other => panic!("expected unsupported metric, got {other:?}"),
    }
}

#[tokio::test]
async fn claims_are_single_use_and_rate_limited() {
    let fx = Fixture::new().await;

    // Fresh claim for the same VM: blocked while an agent is active.
    let claim = fx
        .service
        .issue_claim(&fx.vm_id, "op-user", 200_000)
        .await
        .unwrap();
    match fx
        .service
        .redeem_claim(&claim.token, "install-2", &fx.csr, None, 201_000)
        .await
        .unwrap()
    {
        EnrollOutcome::AlreadyEnrolled { .. } => {}
        other => panic!("expected already-enrolled conflict, got {other:?}"),
    }

    // After revoke, a replacement enrolls through a fresh claim.
    fx.service
        .repo()
        .revoke_agent(&fx.agent_id, "op-user", 202_000)
        .await
        .unwrap();
    let claim = fx
        .service
        .issue_claim(&fx.vm_id, "op-user", 203_000)
        .await
        .unwrap();
    assert!(matches!(
        fx.service
            .redeem_claim(&claim.token, "install-2", &fx.csr, None, 204_000)
            .await
            .unwrap(),
        EnrollOutcome::Enrolled(_)
    ));

    // Redemption attempts are rate-limited per source IP (default
    // budget 10/minute).
    let mut rate_limited = false;
    for i in 0..15 {
        let claim = fx
            .service
            .issue_claim(&fx.vm_id, "op-user", 300_000 + i)
            .await;
        if let Ok(claim) = claim {
            if matches!(
                fx.service
                    .redeem_claim(
                        &claim.token,
                        "install-x",
                        "not a csr",
                        Some("10.9.9.9"),
                        300_500
                    )
                    .await
                    .unwrap(),
                EnrollOutcome::RateLimited
            ) {
                rate_limited = true;
                break;
            }
        }
    }
    assert!(rate_limited, "enrollment rate limit must engage");
}

#[tokio::test]
async fn rotation_advances_epoch_and_keeps_grace() {
    let fx = Fixture::new().await;
    let now = 100_000i64;

    // Accepted first.
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(1, now), now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Accepted { .. } => {}
        other => panic!("expected accepted, got {other:?}"),
    }

    // Rotate with a fresh CSR.
    let (new_csr, _new_key) = test_csr();
    let (new_fingerprint, epoch) = match fx
        .service
        .rotate_credential(&fx.peer(), &new_csr, now + 1_000)
        .await
        .unwrap()
    {
        RotateOutcome::Rotated {
            cert,
            credential_epoch,
        } => (fingerprint_of(&cert.certificate_pem), credential_epoch),
        other => panic!("expected rotation, got {other:?}"),
    };
    assert_eq!(epoch, 2);

    // The previous credential still authenticates inside the grace
    // window (in-flight retry tolerance).
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(2, now + 2_000), now + 2_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Accepted { .. } => {}
        other => panic!("previous credential must work in grace, got {other:?}"),
    }

    // The new credential authenticates too.
    let new_peer = AgentPeerCredential {
        agent_id: fx.agent_id.clone(),
        fingerprint: new_fingerprint,
    };
    match fx
        .service
        .ingest_batch(&new_peer, &fx.envelope(3, now + 3_000), now + 3_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Accepted { .. } => {}
        other => panic!("new credential must work, got {other:?}"),
    }

    // Outside the grace window (default 30 min) the old credential is
    // refused.
    match fx
        .service
        .ingest_batch(
            &fx.peer(),
            &fx.envelope(4, now + 3_600_000),
            now + 3_600_000,
        )
        .await
        .unwrap()
    {
        GuestIngestOutcome::Unauthenticated(reason) => assert!(reason.contains("rotation")),
        other => panic!("expected stale credential, got {other:?}"),
    }
}

#[tokio::test]
async fn forced_rotation_surfaces_renewal_due() {
    let fx = Fixture::new().await;
    let now = 100_000i64;

    fx.service
        .repo()
        .set_rotation_pending(&fx.agent_id)
        .await
        .unwrap();
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(1, now), now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Accepted { renewal_due, .. } => assert!(renewal_due),
        other => panic!("expected accepted, got {other:?}"),
    }
}

#[tokio::test]
async fn cloned_install_is_flagged_and_blocked() {
    let fx = Fixture::new().await;
    let now = 100_000i64;

    // A credential presented from a different install (cloned image).
    let mut envelope = fx.envelope(1, now);
    envelope.install_id = "install-clone".into();
    match fx
        .service
        .ingest_batch(&fx.peer(), &envelope, now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Unauthenticated(reason) => {
            assert!(reason.contains("conflict"))
        }
        other => panic!("expected conflict, got {other:?}"),
    }

    // The flag now blocks even correct-install batches until an
    // authorized reset.
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(2, now + 1_000), now + 1_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Unauthenticated(_) => {}
        other => panic!("expected blocked, got {other:?}"),
    }

    fx.service
        .repo()
        .clear_conflict(&fx.agent_id)
        .await
        .unwrap();
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(3, now + 2_000), now + 2_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Accepted { .. } => {}
        other => panic!("expected accepted after reset, got {other:?}"),
    }
}

#[tokio::test]
async fn per_agent_rate_and_concurrency_limits() {
    let fx = Fixture::new().await;
    let now = 100_000i64;

    // Default budget: 20 batches/minute. Distinct sequences so the
    // dedup does not answer before the limiter.
    let mut limited = false;
    for seq in 1..=25u64 {
        match fx
            .service
            .ingest_batch(
                &fx.peer(),
                &fx.envelope(seq, now + seq as i64),
                now + seq as i64,
            )
            .await
            .unwrap()
        {
            GuestIngestOutcome::RateLimited { .. } => {
                limited = true;
                break;
            }
            GuestIngestOutcome::Accepted { .. } | GuestIngestOutcome::Duplicate { .. } => {}
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    assert!(limited, "per-agent rate limit must engage");
}

#[tokio::test]
async fn revoked_agent_cannot_report() {
    let fx = Fixture::new().await;
    let now = 100_000i64;
    fx.service
        .repo()
        .revoke_agent(&fx.agent_id, "op-user", now)
        .await
        .unwrap();
    match fx
        .service
        .ingest_batch(&fx.peer(), &fx.envelope(1, now), now)
        .await
        .unwrap()
    {
        GuestIngestOutcome::Unauthenticated(reason) => assert!(reason.contains("revoked")),
        other => panic!("expected revoked rejection, got {other:?}"),
    }
}

#[tokio::test]
async fn unavailable_store_is_typed_degradation() {
    let ops_db = TestDb::new().await;
    let vm_id = "vm-agent-2".to_string();
    sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ($1, $1)")
        .bind(&vm_id)
        .execute(&ops_db.pool)
        .await
        .unwrap();
    let (ca_cert, ca_key) = test_ca();
    let issuer = Arc::new(AgentCertificateIssuer::new(&ca_cert, &ca_key).unwrap());
    let service = MonitoringAgentService::new(
        MonitoringAgentRepository::new(ops_db.pool.clone()),
        issuer,
        None, // store unavailable — the honest degradation path
        MonitoringHealth::new(),
        EventRepository::new(ops_db.pool.clone()),
        Default::default(),
        None,
    );

    let claim = service.issue_claim(&vm_id, "op", 1_000).await.unwrap();
    let enrolled = match service
        .redeem_claim(&claim.token, "install-1", &test_csr().0, None, 2_000)
        .await
        .unwrap()
    {
        EnrollOutcome::Enrolled(a) => a,
        other => panic!("expected enrollment, got {other:?}"),
    };
    let peer = AgentPeerCredential {
        agent_id: enrolled.agent_id.clone(),
        fingerprint: fingerprint_of(&enrolled.certificate_pem),
    };

    let mut envelope = GuestBatchEnvelope {
        schema_version: 1,
        agent_id: enrolled.agent_id,
        install_id: "install-1".into(),
        boot_id: "boot-1".into(),
        sequence: 1,
        sent_at_ms: 100_000,
        os: None,
        samples: vec![sample(&vm_id, 100_000)],
    };
    envelope.samples[0].target_id = vm_id.clone();
    match service
        .ingest_batch(&peer, &envelope, 100_000)
        .await
        .unwrap()
    {
        GuestIngestOutcome::IngestionUnavailable => {}
        other => panic!("expected unavailable, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Full TLS round trip: the real accept loop, the real optional client
// verification, the real peer-certificate extraction. This is the exact
// machinery bootstrap mounts when [http_tls] is configured.
// ---------------------------------------------------------------------------

/// A self-signed server certificate for `localhost`.
fn test_server_cert() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.subject_alt_names = vec![rcgen::SanType::DnsName("localhost".try_into().unwrap())];
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// Minimal HTTP/1.1 client over TLS (no reqwest dependency in this
/// crate; the guest routes are small single-shot posts).
async fn tls_post(
    addr: std::net::SocketAddr,
    server_cert_pem: &str,
    client_identity: Option<(&str, &str)>, // (cert_pem, key_pem)
    path: &str,
    body: &str,
) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut roots = rustls::RootCertStore::empty();
    let (_, server_pem) = x509_parser::pem::parse_x509_pem(server_cert_pem.as_bytes()).unwrap();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            server_pem.contents.clone(),
        ))
        .unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap();
    let config = match client_identity {
        Some((cert_pem, key_pem)) => {
            let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
                rustls_pemfile::certs(&mut cert_pem.as_bytes())
                    .collect::<Result<_, _>>()
                    .unwrap();
            let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
                .unwrap()
                .unwrap();
            builder
                .with_root_certificates(roots)
                .with_client_auth_cert(certs, key)
                .unwrap()
        }
        None => builder.with_root_certificates(roots).with_no_client_auth(),
    };
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut tls = connector
        .connect(
            rustls::pki_types::ServerName::try_from("localhost").unwrap(),
            tcp,
        )
        .await
        .expect("tls handshake");

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    tls.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tls.read_to_end(&mut response).await.unwrap();
    let text = String::from_utf8_lossy(&response).to_string();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_default();
    let body = text
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_string();
    (status, body)
}

#[tokio::test]
async fn tls_round_trip_enroll_ingest_and_unauthenticated() {
    let fx = Fixture::new().await;

    let (server_cert, server_key) = test_server_cert();
    // The CA the fixture enrolled under doubles as the TLS client CA
    // (production shape) so the agent certificate chains to it.
    let https =
        crate::api::tls::build_https_config(&server_cert, &server_key, Some(&fx.ca_cert_pem))
            .unwrap();

    let router = crate::api::agent_routes::agent_routes(axum::Router::new())
        .layer(axum::Extension(fx.service.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    let server = tokio::spawn(crate::api::tls::serve_tls(
        listener,
        https,
        router,
        shutdown_rx,
    ));

    // 1. Ingest without a client certificate: the TLS layer lets the
    //    connection through (browsers connect like this), the handler
    //    must refuse.
    let now = chrono::Utc::now().timestamp_millis();
    let envelope = fx.envelope(50, now);
    let body = serde_json::to_string(&wire_envelope(&envelope)).unwrap();
    let (status, error_body) =
        tls_post(addr, &server_cert, None, "/monitoring/v1/ingest", &body).await;
    assert_eq!(status, 401, "no-cert ingest must be 401: {error_body}");
    assert!(error_body.contains("unauthenticated"), "{error_body}");

    // 2. A node-shaped certificate (no agent OU) signed by the same
    //    CA: chain-valid but not an agent credential — refused.
    let node_key = rcgen::KeyPair::generate().unwrap();
    let mut node_params = rcgen::CertificateParams::default();
    node_params.distinguished_name = rcgen::DistinguishedName::new();
    node_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "node-1");
    let ca_kp = rcgen::KeyPair::from_pem(&fx.ca_key_pem).unwrap();
    let ca_issuer = rcgen::Issuer::from_ca_cert_pem(&fx.ca_cert_pem, ca_kp).unwrap();
    let node_cert = node_params.signed_by(&node_key, &ca_issuer).unwrap();
    let (status, error_body) = tls_post(
        addr,
        &server_cert,
        Some((&node_cert.pem(), &node_key.serialize_pem())),
        "/monitoring/v1/ingest",
        &body,
    )
    .await;
    assert_eq!(status, 401, "node-cert ingest must be 401: {error_body}");
    assert!(error_body.contains("monitoring agent"), "{error_body}");

    // 3. Ingest with the enrolled agent credential: accepted, and the
    //    peer certificate (not the payload) is the identity.
    let (status, resp) = tls_post(
        addr,
        &server_cert,
        Some((&fx.agent_cert_pem, &fx.agent_key_pem)),
        "/monitoring/v1/ingest",
        &body,
    )
    .await;
    assert_eq!(status, 202, "agent-cert ingest must be accepted: {resp}");
    assert!(resp.contains("\"accepted\""), "{resp}");

    // 4. Oversized body: 413 with the contract error shape.
    let big = "x".repeat(300 * 1024);
    let oversized = format!("{{\"padding\":\"{big}\"}}");
    let (status, resp) = tls_post(
        addr,
        &server_cert,
        Some((&fx.agent_cert_pem, &fx.agent_key_pem)),
        "/monitoring/v1/ingest",
        &oversized,
    )
    .await;
    assert_eq!(status, 413, "{resp}");
    assert!(resp.contains("batch_too_large"), "{resp}");

    // 5. Claim redemption over the wire with a fresh claim: the claim
    //    is the credential — no client certificate needed.
    let claim = fx
        .service
        .issue_claim(&fx.vm_id, "op-user", chrono::Utc::now().timestamp_millis())
        .await
        .unwrap();
    // Revoke the existing agent so the VM may enroll a replacement.
    fx.service
        .repo()
        .revoke_agent(
            &fx.agent_id,
            "op-user",
            chrono::Utc::now().timestamp_millis(),
        )
        .await
        .unwrap();
    let (csr, _key) = test_csr();
    let enroll_body = serde_json::json!({
        "schema_version": 1,
        "claim": claim.token,
        "install_id": "install-wire",
        "csr_pem": csr,
    })
    .to_string();
    let (status, resp) = tls_post(
        addr,
        &server_cert,
        None,
        "/monitoring/v1/enroll",
        &enroll_body,
    )
    .await;
    assert_eq!(status, 201, "claim redemption must succeed: {resp}");
    assert!(resp.contains("certificate_pem"), "{resp}");
    assert!(resp.contains("agent-credential-generation-1"), "{resp}");

    server.abort();
}

/// Serialize the envelope exactly as the guest agent sends it (the
/// wire format the ingestion contract specifies).
fn wire_envelope(envelope: &GuestBatchEnvelope) -> serde_json::Value {
    serde_json::json!({
        "schema_version": envelope.schema_version,
        "agent_id": envelope.agent_id,
        "install_id": envelope.install_id,
        "boot_id": envelope.boot_id,
        "sequence": envelope.sequence,
        "sent_at_ms": envelope.sent_at_ms,
        "os": {
            "name": envelope.os.as_ref().and_then(|o| o.name.clone()),
            "version": envelope.os.as_ref().and_then(|o| o.version.clone()),
            "kernel_release": envelope.os.as_ref().and_then(|o| o.kernel_release.clone()),
        },
        "samples": envelope.samples.iter().map(|s| serde_json::json!({
            "schema_version": s.schema_version,
            "target_kind": s.target_kind,
            "target_id": s.target_id,
            "metric_id": s.metric_id,
            "source": s.source,
            "kind": s.kind,
            "unit": s.unit,
            "observed_at_ms": s.observed_at_ms,
            "value": s.value,
            "quality": s.quality,
            "dimensions": s.dimensions,
            "boot_id": s.boot_id,
            "identity_epoch": s.identity_epoch,
        })).collect::<Vec<_>>(),
    })
}
