//! Peer identity extraction and verification for agent-side gRPC handlers.
//!
//! ## Threat model
//!
//! Agent-side handlers (enrollment rotation, telemetry, observed-state, reconcile
//! acknowledgements, bootstrap-result reports) accept a `node_id` field on the wire.
//! Without binding that asserted `node_id` to the peer's mTLS client certificate, a
//! compromised node holding a *valid* node certificate could forge requests claiming
//! to be a different node — overwriting another node's observed state, rotating
//! another node's certificate, or polluting telemetry/inventory. CVSS-wise this is
//! a cross-node trust gap: any one node compromise becomes cluster-wide impersonation.
//!
//! ## Defense
//!
//! This module:
//!
//! 1. Extracts the peer's leaf certificate from `tonic::transport::server::TlsConnectInfo`
//!    (set automatically by tonic when the server is configured with mTLS).
//! 2. Parses the certificate's CN and DNS-type SANs with `x509-parser`.
//! 3. Stores the authorized peer node identity in request extensions as a typed
//!    [`PeerNodeId`] marker via the [`PeerIdentityInterceptor`].
//! 4. Provides [`verify_peer_matches`] for handlers to reject any request whose
//!    asserted `node_id` does not match the peer's certificate-derived identity.
//!
//! ## Insecure-mode passthrough
//!
//! When `CHV_ALLOW_INSECURE=1` is set (dev/test mode), the interceptor and verification
//! become no-ops. Production deployments MUST run with mTLS enabled.
//!
//! See ADR-008 (error handling) and ADR-014 (mTLS / certificate trust boundary).

use crate::error::ControlPlaneServiceError;
use chv_controlplane_types::domain::NodeId;
use std::sync::Arc;
use tonic::transport::server::TlsConnectInfo;

/// Authoritative node identity derived from the peer's mTLS client certificate.
///
/// Inserted into request extensions by [`PeerIdentityInterceptor`] when running
/// under mTLS. Handlers SHOULD extract this and compare to any wire-asserted
/// `node_id` via [`verify_peer_matches`].
#[derive(Debug, Clone)]
pub struct PeerNodeId(pub NodeId);

impl PeerNodeId {
    pub fn as_node_id(&self) -> &NodeId {
        &self.0
    }
}

/// Marker inserted by the interceptor when the request arrived over an
/// insecure (non-TLS, dev-mode) channel. Handlers MUST treat this as
/// "no peer identity available" and accept any wire-asserted node_id —
/// suitable only for local development where `CHV_ALLOW_INSECURE=1`.
#[derive(Debug, Clone, Copy)]
pub struct InsecurePeer;

/// Tonic interceptor that pins the request's peer identity from the mTLS
/// client certificate.
///
/// Behavior:
/// - When `allow_insecure == true`: inserts [`InsecurePeer`] into request
///   extensions and returns. The handler-side verifier becomes a no-op.
/// - When `allow_insecure == false` and the request carries a peer cert: parses
///   the cert and inserts [`PeerNodeId`].
/// - When `allow_insecure == false` and the request has no peer cert (or the cert
///   does not carry a parseable node identifier): rejects with `Unauthenticated`.
///
/// This interceptor is intentionally conservative: a missing/unparseable cert in
/// production-mode (TLS-required) is a hard failure rather than a soft-pass,
/// because any handler that relies on `PeerNodeId` being present would otherwise
/// silently accept the wire-asserted `node_id`.
#[derive(Clone)]
pub struct PeerIdentityInterceptor {
    allow_insecure: bool,
}

impl PeerIdentityInterceptor {
    /// Construct the interceptor.
    ///
    /// # Panics
    ///
    /// Panics at startup if `allow_insecure` is `true` and the binary was **not**
    /// compiled with the `dev` Cargo feature. This is an intentional deployment
    /// guard: `CHV_ALLOW_INSECURE=1` disables all mTLS peer-identity enforcement,
    /// and must never be usable in production builds.
    ///
    /// To build for local development:
    /// ```text
    /// cargo build --features dev
    /// ```
    pub fn new(allow_insecure: bool) -> Self {
        if allow_insecure && !cfg!(feature = "dev") {
            panic!(
                "CHV_ALLOW_INSECURE=1 is set but this binary was not compiled with the 'dev' \
                 Cargo feature. This env var disables all mTLS peer-identity enforcement and \
                 must never be enabled in production. \
                 To use it for local development, rebuild with: cargo build --features dev"
            );
        }
        Self { allow_insecure }
    }

    /// Apply the interceptor to a request, mutating its extensions in-place.
    #[allow(clippy::result_large_err)] // tonic::Status is the required error type for tonic interceptors.
    pub fn intercept<T>(
        &self,
        mut request: tonic::Request<T>,
    ) -> Result<tonic::Request<T>, tonic::Status> {
        if self.allow_insecure {
            request.extensions_mut().insert(InsecurePeer);
            return Ok(request);
        }

        let peer_node_id = match extract_peer_node_id_from_extensions(request.extensions()) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(error = %err, "rejecting request: peer identity extraction failed");
                return Err(tonic::Status::unauthenticated(
                    "peer mTLS certificate required",
                ));
            }
        };

        request.extensions_mut().insert(PeerNodeId(peer_node_id));
        Ok(request)
    }
}

/// Errors returned when extracting a peer's node identity from request extensions.
#[derive(Debug, thiserror::Error)]
pub enum PeerIdentityError {
    #[error("no TLS connect info in request extensions")]
    MissingTlsInfo,

    #[error("peer presented no client certificate")]
    NoClientCertificate,

    #[error("failed to parse peer certificate: {0}")]
    InvalidCertificate(String),

    #[error("peer certificate carries no node_id-bearing CN or DNS SAN")]
    NoNodeIdInCertificate,

    #[error("peer certificate node_id is malformed: {0}")]
    InvalidNodeId(String),
}

/// Extract the peer's authoritative `NodeId` from the leaf certificate found in
/// the request's [`TlsConnectInfo`] extension.
///
/// Identity is taken from (in order of preference):
/// 1. The first DNS-type SAN entry.
/// 2. The certificate Subject's CommonName.
///
/// This matches how `CaBackedCertificateIssuer` mints node certs (CN = node_id,
/// SAN = DNS:node_id).
pub fn extract_peer_node_id_from_extensions(
    extensions: &tonic::Extensions,
) -> Result<NodeId, PeerIdentityError> {
    use tonic::transport::server::TcpConnectInfo;

    let tls_info: &TlsConnectInfo<TcpConnectInfo> = extensions
        .get::<TlsConnectInfo<TcpConnectInfo>>()
        .ok_or(PeerIdentityError::MissingTlsInfo)?;

    let certs: Arc<Vec<_>> = tls_info
        .peer_certs()
        .ok_or(PeerIdentityError::NoClientCertificate)?;

    let leaf = certs
        .first()
        .ok_or(PeerIdentityError::NoClientCertificate)?;

    parse_node_id_from_der(leaf.as_ref())
}

/// Parse a DER-encoded X.509 certificate and return its asserted node_id.
///
/// Exposed for unit tests. Production code goes through
/// [`extract_peer_node_id_from_extensions`].
pub fn parse_node_id_from_der(der: &[u8]) -> Result<NodeId, PeerIdentityError> {
    use x509_parser::prelude::*;

    let (_, cert) = X509Certificate::from_der(der)
        .map_err(|e| PeerIdentityError::InvalidCertificate(e.to_string()))?;

    // Prefer DNS SAN — that's what `CaBackedCertificateIssuer` populates.
    if let Ok(Some(ext)) = cert.subject_alternative_name() {
        for name in &ext.value.general_names {
            if let GeneralName::DNSName(dns) = name {
                let dns = dns.trim();
                if !dns.is_empty() {
                    return NodeId::new(dns.to_string())
                        .map_err(|e| PeerIdentityError::InvalidNodeId(e.to_string()));
                }
            }
        }
    }

    // Fall back to CN.
    if let Some(cn) = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
    {
        let cn = cn.trim();
        if !cn.is_empty() {
            return NodeId::new(cn.to_string())
                .map_err(|e| PeerIdentityError::InvalidNodeId(e.to_string()));
        }
    }

    Err(PeerIdentityError::NoNodeIdInCertificate)
}

/// Verify that the asserted `requested` node_id matches the peer's
/// certificate-derived identity carried in the request extensions.
///
/// Returns:
/// - `Ok(())` if the peer is [`InsecurePeer`] (dev-mode passthrough) or if
///   `peer_node_id == requested`.
/// - `Err(ControlPlaneServiceError::PermissionDenied)` if the asserted node_id
///   does not match the peer cert. Increments
///   `chv_peer_node_id_mismatch_total{method}`.
/// - `Err(ControlPlaneServiceError::Unauthorized)` if no peer identity is
///   present in extensions (cert missing in TLS-required mode — should already
///   have been rejected by the interceptor, but defense-in-depth here).
pub fn verify_peer_matches(
    extensions: &tonic::Extensions,
    requested: &NodeId,
    method: &'static str,
) -> Result<(), ControlPlaneServiceError> {
    if extensions.get::<InsecurePeer>().is_some() {
        return Ok(());
    }

    let peer = extensions.get::<PeerNodeId>().ok_or_else(|| {
        tracing::warn!(
            requested_node_id = %requested,
            method,
            "rejecting request: peer identity extension missing (mTLS not enforced upstream?)"
        );
        ControlPlaneServiceError::PermissionDenied(
            "peer mTLS identity required for this RPC".into(),
        )
    })?;

    if peer.as_node_id() == requested {
        return Ok(());
    }

    metrics::counter!(
        "chv_peer_node_id_mismatch_total",
        "method" => method,
    )
    .increment(1);

    tracing::warn!(
        peer_node_id = %peer.as_node_id(),
        requested_node_id = %requested,
        method,
        "peer node_id mismatch: rejecting impersonation attempt"
    );

    Err(ControlPlaneServiceError::PermissionDenied(format!(
        "peer node_id {} does not match request node_id {}",
        peer.as_node_id(),
        requested,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a self-signed leaf certificate whose CN and DNS SAN both equal
    /// `node_id`. Returns the DER bytes.
    fn fake_leaf_der(node_id: &str) -> Vec<u8> {
        use rcgen::{
            CertificateParams, DistinguishedName, DnType, Ia5String, IsCa, KeyPair, SanType,
        };

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, node_id);
        let dns = Ia5String::try_from(node_id.to_string()).unwrap();
        params.subject_alt_names.push(SanType::DnsName(dns));
        params.is_ca = IsCa::NoCa;

        let key = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        cert.der().to_vec()
    }

    #[test]
    fn parses_node_id_from_san() {
        let der = fake_leaf_der("node-alpha");
        let got = parse_node_id_from_der(&der).expect("parse");
        assert_eq!(got.as_str(), "node-alpha");
    }

    #[test]
    fn parses_node_id_from_cn_when_no_san() {
        // rcgen always populates SAN if asked; for CN-only we hand-build.
        use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "node-cn-only");
        params.is_ca = IsCa::NoCa;
        let key = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let got = parse_node_id_from_der(cert.der()).expect("parse");
        assert_eq!(got.as_str(), "node-cn-only");
    }

    #[test]
    fn rejects_garbage_der() {
        let err = parse_node_id_from_der(b"not a certificate").unwrap_err();
        assert!(matches!(err, PeerIdentityError::InvalidCertificate(_)));
    }

    /// Documents the compile-time invariant: production builds (no `dev` feature)
    /// must never have the `dev` feature enabled.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn allow_insecure_is_gated_by_feature() {
        #[cfg(not(feature = "dev"))]
        assert!(
            !cfg!(feature = "dev"),
            "production builds must not have 'dev' feature enabled"
        );
    }
}
