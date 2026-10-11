//! The manager HTTP client. One reqwest client per credential state:
//! enrollment runs without a client certificate, ingestion and
//! rotation present it.

use crate::credential::StoredCredential;
use crate::wire::{
    ApiErrorBody, EnrollRequest, EnvelopeJson, IngestResponse, RotateRequest, RotateResponse,
    SCHEMA_VERSION,
};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("transport failure: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid server_url: {0}")]
    InvalidServerUrl(String),
    #[error("failed to decode manager response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("manager refused: status {status} code={code} message={message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
}

#[derive(Clone)]
pub struct ManagerClient {
    http: reqwest::Client,
    base: String,
}

impl ManagerClient {
    /// Build a client trusting `manager_ca_pem` for the manager's
    /// server certificate, optionally presenting the agent credential
    /// as the TLS client identity.
    ///
    /// G5: pass a `transport` to redirect the TCP dial onto the VM's
    /// vsock device (loopback forwarder → host relay). Everything
    /// above the dial — the URL, TLS SNI, certificate validation, the
    /// mTLS identity — is IDENTICAL for both transports (design D1:
    /// one TLS/HTTP stack; the transport choice lives strictly below
    /// it). `None` dials the management network as before.
    pub fn new(
        server_url: &str,
        manager_ca_pem: &str,
        credential: Option<&StoredCredential>,
        transport: Option<&crate::transport::VsockTransport>,
    ) -> Result<Self, ClientError> {
        let mut builder = reqwest::Client::builder()
            .tls_built_in_root_certs(false)
            .add_root_certificate(reqwest::Certificate::from_pem(manager_ca_pem.as_bytes())?)
            // Long enough for slow cold TLS handshakes, short enough
            // that an unresponsive manager cannot stall a tick.
            .timeout(std::time::Duration::from_secs(30));
        if let Some(transport) = transport {
            let host = crate::transport::server_host(server_url).ok_or_else(|| {
                ClientError::InvalidServerUrl(format!(
                    "cannot extract the TLS host from {server_url:?}"
                ))
            })?;
            // The pipe IS the network path: a configured proxy would
            // bypass or break it, so vsock mode never uses one.
            builder = builder.resolve(&host, transport.loopback()).no_proxy();
        }
        if let Some(cred) = credential {
            let identity_pem = format!("{}\n{}", cred.certificate_pem, cred.private_key_pem);
            builder = builder.identity(reqwest::Identity::from_pem(identity_pem.as_bytes())?);
        }
        Ok(Self {
            http: builder.build()?,
            base: server_url.trim_end_matches('/').to_string(),
        })
    }

    /// POST /monitoring/v1/enroll (claim is the credential — no
    /// client certificate).
    pub async fn enroll(
        &self,
        claim: &str,
        install_id: &str,
        csr_pem: &str,
    ) -> Result<crate::wire::EnrollResponse, ClientError> {
        let body = EnrollRequest {
            schema_version: SCHEMA_VERSION,
            claim: claim.to_string(),
            install_id: install_id.to_string(),
            csr_pem: csr_pem.to_string(),
        };
        self.post_json("/monitoring/v1/enroll", &body).await
    }

    /// POST /monitoring/v1/ingest with the client certificate.
    pub async fn ingest(&self, envelope: &EnvelopeJson) -> Result<IngestResponse, ClientError> {
        self.post_json("/monitoring/v1/ingest", envelope).await
    }

    /// POST /monitoring/v1/rotate with the current client certificate.
    pub async fn rotate(&self, csr_pem: &str) -> Result<RotateResponse, ClientError> {
        let body = RotateRequest {
            schema_version: SCHEMA_VERSION,
            csr_pem: csr_pem.to_string(),
        };
        self.post_json("/monitoring/v1/rotate", &body).await
    }

    async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, ClientError> {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .json(body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            let detail = serde_json::from_str::<ApiErrorBody>(&text)
                .ok()
                .map(|e| (e.error.code, e.error.message))
                .unwrap_or_else(|| ("unknown".to_string(), text.clone()));
            return Err(ClientError::Api {
                status: status.as_u16(),
                code: detail.0,
                message: detail.1,
            });
        }
        serde_json::from_str(&text).map_err(ClientError::Decode)
    }
}
