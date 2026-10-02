//! Loopback proof that the storage-migration receiver's TLS TCP listener
//! actually enforces mTLS (issue #390).
//!
//! This is the negative the listener exists for: a stord that accepts inbound
//! migrations must never accept a peer without a valid client certificate.
//! The test drives the *production* serving path
//! ([`chv_stord_core::server::serve_migration_tls`] with a real tonic
//! `ServerTlsConfig`) and asserts at the transport level that:
//!
//! (a) a client WITH a valid client identity (issued by the trusted CA)
//!     completes the handshake and gets a gRPC-level response;
//! (b) a client WITHOUT any client certificate is rejected;
//! (c) a client with an identity from an UNTRUSTED CA is rejected.
//!
//! Rejections must surface as TLS errors (fatal alert), not as gRPC statuses.

use chv_stord_api::chv_stord_api::storage_migration_service_client::StorageMigrationServiceClient;
use chv_stord_api::chv_stord_api::MigrationMessage;
use chv_stord_backends::LocalFileBackend;
use chv_stord_core::migration::service::StorageMigrationServiceImpl;
use chv_stord_core::migration::tls_config::load_migration_server_tls;
use chv_stord_core::server::serve_migration_tls;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

/// Install the rustls ring crypto provider (same as `cmd/chv-stord` startup).
/// Idempotent: a second call fails harmlessly once a provider is installed.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// A test CA that can issue leaf certificates.
struct TestCa {
    params: rcgen::CertificateParams,
    key: rcgen::KeyPair,
    cert_pem: Vec<u8>,
}

fn test_ca(cn: &str) -> TestCa {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, cn);
    let cert = params.self_signed(&key).unwrap();
    TestCa {
        params,
        key,
        cert_pem: cert.pem().into_bytes(),
    }
}

/// Issue a leaf certificate signed by `ca`. Server leaves carry the
/// `localhost`/127.0.0.1 identities and serverAuth EKU; client leaves carry
/// clientAuth EKU.
fn issue_leaf(ca: &TestCa, cn: &str, server: bool) -> (Vec<u8>, Vec<u8>) {
    use rcgen::string::Ia5String;

    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, cn);
    params.is_ca = rcgen::IsCa::NoCa;
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![if server {
        rcgen::ExtendedKeyUsagePurpose::ServerAuth
    } else {
        rcgen::ExtendedKeyUsagePurpose::ClientAuth
    }];
    if server {
        params.subject_alt_names.push(rcgen::SanType::DnsName(
            Ia5String::try_from("localhost").unwrap(),
        ));
        params.subject_alt_names.push(rcgen::SanType::IpAddress(
            std::net::Ipv4Addr::LOCALHOST.into(),
        ));
    }
    let issuer = rcgen::Issuer::from_params(&ca.params, &ca.key);
    let cert = params.signed_by(&key, &issuer).unwrap();
    (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
}

fn tmp_file(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut f, bytes).unwrap();
    f
}

/// The observable outcome of one probe against the listener.
#[derive(Debug)]
enum Outcome {
    /// A gRPC-level response was received (proves the full mTLS + HTTP/2
    /// round trip works for this client identity).
    Grpc(tonic::Code, String),
    /// The server killed the connection below gRPC with a TLS fatal alert
    /// (the mTLS rejection this listener exists to enforce).
    TlsAlert(String),
}

fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut source = e.source();
    while let Some(err) = source {
        msg.push_str("; ");
        msg.push_str(&err.to_string());
        source = err.source();
    }
    msg
}

/// Connect to the listener and drive one migration RPC whose request stream
/// is closed immediately (the service answers `InvalidArgument` — "stream
/// closed without sending a message" — which is the expected gRPC-level
/// proof for a validly authenticated peer). A peer rejected at the TLS layer
/// instead sees the connection die with a rustls fatal alert.
async fn probe(
    addr: SocketAddr,
    server_ca_pem: &[u8],
    client_identity: Option<(&[u8], &[u8])>,
) -> Outcome {
    let mut tls = ClientTlsConfig::new()
        .domain_name("localhost")
        .ca_certificate(Certificate::from_pem(server_ca_pem));
    if let Some((cert, key)) = client_identity {
        tls = tls.identity(Identity::from_pem(cert, key));
    }
    let endpoint = Endpoint::from_shared(format!("https://{addr}"))
        .unwrap()
        .tls_config(tls)
        .unwrap()
        .connect_timeout(Duration::from_secs(10));

    let channel: Channel = match endpoint.connect().await {
        Ok(channel) => channel,
        Err(e) => {
            let chain = error_chain(&e);
            if chain.contains("fatal alert") {
                return Outcome::TlsAlert(chain);
            }
            // The listener is up (the positive probe ran first); a non-TLS
            // connect failure means the test infrastructure broke.
            panic!("unexpected non-TLS connect failure: {chain}");
        }
    };

    let mut client = StorageMigrationServiceClient::new(channel);
    let (tx, rx) = mpsc::channel::<MigrationMessage>(1);
    drop(tx); // end the request stream without sending InitMigration
    match client.stream_blocks(ReceiverStream::new(rx)).await {
        Ok(_) => Outcome::Grpc(tonic::Code::Ok, "unexpected success".into()),
        Err(status) => {
            // Transport-level failures surface as a tonic `Status` (code
            // Unknown, "transport error") whose source chain carries the
            // real cause, including the rustls fatal alert sent by the
            // server when client-certificate verification fails.
            let mut chain = format!("code={:?} message={:?}", status.code(), status.message());
            let mut source = std::error::Error::source(&status);
            while let Some(err) = source {
                chain.push_str("; ");
                chain.push_str(&err.to_string());
                source = err.source();
            }
            if chain.contains("fatal alert") {
                Outcome::TlsAlert(chain)
            } else {
                Outcome::Grpc(status.code(), status.message().to_string())
            }
        }
    }
}

#[tokio::test]
async fn migration_tls_listener_requires_client_certificates() {
    install_crypto_provider();

    let dir = tempfile::tempdir().unwrap();

    // Trusted CA: issues the server leaf and the legitimate client identity.
    let ca = test_ca("chv-migration-test-ca");
    let (server_cert, server_key) = issue_leaf(&ca, "stord-receiver", true);
    let (client_cert, client_key) = issue_leaf(&ca, "stord-peer", false);

    // Rogue CA: issues a client identity the receiver must not trust.
    let rogue_ca = test_ca("chv-migration-rogue-ca");
    let (rogue_cert, rogue_key) = issue_leaf(&rogue_ca, "rogue-peer", false);

    // Bind an ephemeral loopback port, then load the validated material
    // through the real startup loader pointing at it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let cert_file = tmp_file(&server_cert);
    let key_file = tmp_file(&server_key);
    let ca_file = tmp_file(&ca.cert_pem);
    let tls = load_migration_server_tls(
        true,
        Some(&addr.to_string()),
        Some(cert_file.path()),
        Some(key_file.path()),
        Some(ca_file.path()),
    )
    .expect("full receiver set must load")
    .expect("configured receiver must produce server TLS material");
    assert_eq!(tls.listen_addr, addr);

    // Serve with the production path: a real tonic server with
    // ServerTlsConfig (client_ca_root, no client_auth_optional).
    let backend = Arc::new(LocalFileBackend::new(dir.path().to_path_buf()));
    let service = StorageMigrationServiceImpl::new(backend, dir.path().to_path_buf());
    tokio::spawn(async move {
        if let Err(e) = serve_migration_tls(listener, tls, service).await {
            panic!("migration TLS listener failed: {e:?}");
        }
    });

    // (a) A client with a valid client identity completes the mTLS handshake
    // and receives a gRPC-level response.
    match probe(addr, &ca.cert_pem, Some((&client_cert, &client_key))).await {
        Outcome::Grpc(tonic::Code::InvalidArgument, msg) => {
            assert!(
                msg.contains("stream closed"),
                "expected the empty-stream InvalidArgument, got: {msg}"
            );
        }
        other => panic!(
            "valid client identity must complete mTLS and get a gRPC response, got: {other:?}"
        ),
    }

    // (b) A client WITHOUT a client certificate is rejected at the TLS layer.
    // `CertificateRequired` is rustls's alert for a mandatory client-cert
    // verifier seeing an empty client Certificate message — this is the
    // exact behavior the listener must enforce.
    match probe(addr, &ca.cert_pem, None).await {
        Outcome::TlsAlert(chain) => {
            assert!(
                chain.contains("CertificateRequired"),
                "expected the CertificateRequired fatal alert, got: {chain}"
            );
        }
        other => {
            panic!("client without a certificate must be rejected at the TLS layer, got: {other:?}")
        }
    }

    // (c) A client with an identity from an untrusted CA is rejected at the
    // TLS layer (the server sends a fatal alert instead of a gRPC response).
    match probe(addr, &ca.cert_pem, Some((&rogue_cert, &rogue_key))).await {
        Outcome::TlsAlert(chain) => {
            assert!(
                !chain.contains("CertificateRequired"),
                "a presented (but untrusted) certificate must fail verification, not the \
                 missing-certificate path; got: {chain}"
            );
        }
        other => panic!(
            "client with an untrusted-CA identity must be rejected at the TLS layer, got: {other:?}"
        ),
    }
}
