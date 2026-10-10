//! Wire types mirroring the guest ingestion contract v1. Kept as an
//! independent serde mirror (not a shared crate with the manager) on
//! purpose: the contract is the source of truth, and the guest
//! package must not link manager code.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: i32 = 1;
pub const SOURCE_GUEST_AGENT: &str = "guest_agent";
pub const TARGET_KIND_VM: &str = "vm";
pub const QUALITY_VALID: &str = "valid";

#[derive(Debug, Serialize)]
pub struct EnrollRequest {
    pub schema_version: i32,
    pub claim: String,
    pub install_id: String,
    pub csr_pem: String,
}

#[derive(Debug, Deserialize)]
pub struct EnrollResponse {
    pub agent_id: String,
    pub vm_id: String,
    pub certificate_pem: String,
    #[allow(dead_code)]
    pub ca_pem: String,
    pub credential_epoch: u64,
    pub expires_at_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct RotateRequest {
    pub schema_version: i32,
    pub csr_pem: String,
}

#[derive(Debug, Deserialize)]
pub struct RotateResponse {
    pub certificate_pem: String,
    pub credential_epoch: u64,
    pub expires_at_ms: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvelopeJson {
    pub schema_version: i32,
    pub agent_id: String,
    pub install_id: String,
    pub boot_id: String,
    pub sequence: u64,
    pub sent_at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<OsJson>,
    pub samples: Vec<SampleJson>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OsJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_release: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SampleJson {
    pub schema_version: i32,
    pub target_kind: String,
    pub target_id: String,
    pub metric_id: String,
    pub source: String,
    pub kind: String,
    pub unit: String,
    pub observed_at_ms: i64,
    pub value: serde_json::Value,
    pub quality: String,
    pub dimensions: std::collections::BTreeMap<String, String>,
    pub boot_id: String,
    pub identity_epoch: String,
}

#[derive(Debug, Deserialize)]
pub struct IngestResponse {
    pub status: String,
    #[serde(default)]
    pub credential: CredentialStateJson,
}

#[derive(Debug, Default, Deserialize)]
pub struct CredentialStateJson {
    #[serde(default)]
    pub state: String,
}

impl IngestResponse {
    pub fn is_renewal_due(&self) -> bool {
        self.credential.state == "renewal_due"
    }
}

/// The contract's error body: `{"error": {"code", "message", ...}}`.
#[derive(Debug, Deserialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub code: String,
    #[serde(default)]
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_response_parses_renewal_due() {
        let r: IngestResponse =
            serde_json::from_str(r#"{"status":"accepted","credential":{"state":"renewal_due"}}"#)
                .unwrap();
        assert!(r.is_renewal_due());
        let r: IngestResponse =
            serde_json::from_str(r#"{"status":"duplicate","credential":{"state":"active"}}"#)
                .unwrap();
        assert!(!r.is_renewal_due());
    }

    #[test]
    fn api_error_body_parses_contract_shape() {
        let e: ApiErrorBody = serde_json::from_str(
            r#"{"error":{"code":"rate_limited","message":"slow down","request_id":"x"}}"#,
        )
        .unwrap();
        assert_eq!(e.error.code, "rate_limited");
    }
}
