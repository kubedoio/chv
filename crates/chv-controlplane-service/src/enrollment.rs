use crate::error::ControlPlaneServiceError;
use async_trait::async_trait;
use chv_controlplane_store::{
    BootstrapTokenRepository, BootstrapTokenValidation, NodeBootstrapResultInput, NodeInventoryInput,
    NodeRepository, NodeUpsertInput, NodeVersionInput,
};
use chv_controlplane_types::domain::NodeId;
use control_plane_node_api::control_plane_node_api as proto;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[async_trait]
pub trait CertificateIssuer: Send + Sync {
    async fn issue_node_certificate(
        &self,
        node_id: &NodeId,
    ) -> Result<IssuedCertificate, ControlPlaneServiceError>;
}

pub struct IssuedCertificate {
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub ca_pem: Vec<u8>,
    pub serial: String,
}

#[async_trait]
pub trait EnrollmentService: Send + Sync {
    async fn enroll_node(
        &self,
        request: proto::EnrollmentRequest,
    ) -> Result<proto::EnrollmentResponse, ControlPlaneServiceError>;

    async fn rotate_node_certificate(
        &self,
        request: proto::RotateNodeCertificateRequest,
    ) -> Result<proto::RotateNodeCertificateResponse, ControlPlaneServiceError>;

    async fn report_bootstrap_result(
        &self,
        request: proto::ReportBootstrapResultRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError>;
}

#[derive(Clone)]
pub struct EnrollmentServiceImplementation {
    node_repo: NodeRepository,
    token_repo: BootstrapTokenRepository,
    cert_issuer: Arc<dyn CertificateIssuer>,
}

impl EnrollmentServiceImplementation {
    pub fn new(
        node_repo: NodeRepository,
        token_repo: BootstrapTokenRepository,
        cert_issuer: Arc<dyn CertificateIssuer>,
    ) -> Self {
        Self {
            node_repo,
            token_repo,
            cert_issuer,
        }
    }

    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

use chv_controlplane_types::constants::{
    COMPONENT_AGENT, SOURCE_ENROLLMENT, STATUS_OK, SUMMARY_BOOTSTRAP_REPORTED,
    SUMMARY_CERT_ROTATED, SUMMARY_NODE_ENROLLED,
};

#[async_trait]
impl EnrollmentService for EnrollmentServiceImplementation {
    async fn enroll_node(
        &self,
        request: proto::EnrollmentRequest,
    ) -> Result<proto::EnrollmentResponse, ControlPlaneServiceError> {
        match self
            .token_repo
            .validate_and_consume(&request.bootstrap_token)
            .await?
        {
            BootstrapTokenValidation::Valid => {}
            _ => {
                return Err(ControlPlaneServiceError::Unauthorized(
                    "invalid bootstrap token".into(),
                ))
            }
        }

        let inventory = request
            .inventory
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing inventory".into()))?;
        let versions = request
            .versions
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing versions".into()))?;

        let node_id = NodeId::new(inventory.node_id.clone()).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;

        // Issue certificates
        let cert = self.cert_issuer.issue_node_certificate(&node_id).await?;

        let now = self.now_ms();

        // Persist node
        self.node_repo
            .upsert_node(&NodeUpsertInput {
                node_id: node_id.clone(),
                hostname: inventory.hostname.clone(),
                display_name: inventory.hostname.clone(),
                certificate_serial: Some(cert.serial.clone()),
                agent_version: Some(versions.chv_agent_version.clone()),
                control_plane_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                enrolled_unix_ms: now,
                last_seen_unix_ms: now,
            })
            .await?;

        // Persist inventory
        self.node_repo
            .upsert_inventory(&NodeInventoryInput {
                node_id: node_id.clone(),
                architecture: inventory.architecture.clone(),
                kernel_version: None,
                os_release: None,
                cpu_count: inventory.cpu_threads as i32,
                memory_bytes: inventory.memory_bytes as i64,
                disk_bytes: None,
                cloud_hypervisor_version: Some(versions.cloud_hypervisor_version.clone()),
                chv_agent_version: Some(versions.chv_agent_version.clone()),
                chv_stord_version: Some(versions.chv_stord_version.clone()),
                chv_nwd_version: Some(versions.chv_nwd_version.clone()),
                host_bundle_version: Some(versions.host_bundle_version.clone()),
                inventory_status: Some(SOURCE_ENROLLMENT.into()),
                storage_classes: if inventory.storage_classes.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_value(&inventory.storage_classes).map_err(|e| {
                            ControlPlaneServiceError::Internal(format!(
                                "failed to serialize storage_classes: {}",
                                e
                            ))
                        })?,
                    )
                },
                network_capabilities: if inventory.network_capabilities.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_value(&inventory.network_capabilities).map_err(|e| {
                            ControlPlaneServiceError::Internal(format!(
                                "failed to serialize network_capabilities: {}",
                                e
                            ))
                        })?,
                    )
                },
                labels: if inventory.labels.is_empty() {
                    None
                } else {
                    Some(serde_json::to_value(&inventory.labels).map_err(|e| {
                        ControlPlaneServiceError::Internal(format!(
                            "failed to serialize labels: {}",
                            e
                        ))
                    })?)
                },
                reported_unix_ms: now,
            })
            .await?;

        // Persist versions as history
        self.node_repo
            .append_version(&NodeVersionInput {
                node_id: node_id.clone(),
                component_name: COMPONENT_AGENT.into(),
                version: versions.chv_agent_version,
                source: Some(SOURCE_ENROLLMENT.into()),
                reported_unix_ms: now,
            })
            .await?;

        Ok(proto::EnrollmentResponse {
            result: Some(proto::ResultMeta {
                operation_id: "".into(),
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: SUMMARY_NODE_ENROLLED.into(),
            }),
            issued_node_id: node_id.to_string(),
            certificate_pem: cert.certificate_pem,
            private_key_pem: cert.private_key_pem,
            ca_pem: cert.ca_pem,
        })
    }

    async fn rotate_node_certificate(
        &self,
        request: proto::RotateNodeCertificateRequest,
    ) -> Result<proto::RotateNodeCertificateResponse, ControlPlaneServiceError> {
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;

        let meta = request
            .meta
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing meta".into()))?;

        // Issue new certificates
        let cert = self.cert_issuer.issue_node_certificate(&node_id).await?;

        // UPDATE STORE FIRST
        self.node_repo
            .update_certificate_serial(&node_id, &cert.serial)
            .await?;

        Ok(proto::RotateNodeCertificateResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: SUMMARY_CERT_ROTATED.into(),
            }),
            certificate_pem: cert.certificate_pem,
            private_key_pem: cert.private_key_pem,
            ca_pem: cert.ca_pem,
        })
    }

    async fn report_bootstrap_result(
        &self,
        request: proto::ReportBootstrapResultRequest,
    ) -> Result<proto::AckResponse, ControlPlaneServiceError> {
        let node_id = NodeId::new(request.node_id).map_err(|e| {
            ControlPlaneServiceError::InvalidArgument(format!("invalid node_id: {}", e))
        })?;

        let meta = request
            .meta
            .ok_or_else(|| ControlPlaneServiceError::InvalidArgument("missing meta".into()))?;

        let success = request.bootstrap_status.to_lowercase().contains("success")
            || request.bootstrap_status.to_lowercase() == "ok";

        // PERSIST BOOTSTRAP RESULT
        self.node_repo
            .upsert_bootstrap_result(&NodeBootstrapResultInput {
                node_id,
                operation_id: if meta.operation_id.is_empty() {
                    None
                } else {
                    Some(meta.operation_id.clone())
                },
                success,
                error_message: if success { None } else { Some(request.message) },
                details: None, // bootstrap report doesn't have details_json yet
                started_unix_ms: None,
                completed_unix_ms: self.now_ms(),
            })
            .await?;

        Ok(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id,
                status: STATUS_OK.into(),
                node_observed_generation: "".into(),
                error_code: "".into(),
                human_summary: SUMMARY_BOOTSTRAP_REPORTED.into(),
            }),
        })
    }
}

pub struct CaBackedCertificateIssuer {
    ca_cert: rcgen::Certificate,
    ca_key_pair: rcgen::KeyPair,
    ca_pem: String,
}

impl CaBackedCertificateIssuer {
    pub fn new(ca_cert_pem: &str, ca_key_pem: &str) -> Result<Self, ControlPlaneServiceError> {
        let ca_key_pair = rcgen::KeyPair::from_pem(ca_key_pem).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to parse CA key: {}", e))
        })?;

        let params = rcgen::CertificateParams::from_ca_cert_pem(ca_cert_pem).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to parse CA cert: {}", e))
        })?;

        let ca_cert = params.self_signed(&ca_key_pair).map_err(|e| {
            ControlPlaneServiceError::Internal(format!(
                "failed to reconstruct CA certificate: {}",
                e
            ))
        })?;

        Ok(Self {
            ca_cert,
            ca_key_pair,
            ca_pem: ca_cert_pem.to_string(),
        })
    }
}

#[async_trait]
impl CertificateIssuer for CaBackedCertificateIssuer {
    async fn issue_node_certificate(
        &self,
        node_id: &NodeId,
    ) -> Result<IssuedCertificate, ControlPlaneServiceError> {
        use rcgen::{
            CertificateParams, DistinguishedName, DnType, Ia5String, IsCa, KeyPair, SanType,
        };

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, node_id.as_str());

        let dns_name = Ia5String::try_from(node_id.as_str()).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("invalid node_id for DNS: {}", e))
        })?;
        params.subject_alt_names.push(SanType::DnsName(dns_name));
        params.is_ca = IsCa::NoCa;

        let key_pair =
            KeyPair::generate().map_err(|e| ControlPlaneServiceError::Internal(e.to_string()))?;

        let cert = params
            .signed_by(&key_pair, &self.ca_cert, &self.ca_key_pair)
            .map_err(|e| ControlPlaneServiceError::Internal(format!("signing failed: {}", e)))?;

        Ok(IssuedCertificate {
            certificate_pem: cert.pem().into_bytes(),
            private_key_pem: key_pair.serialize_pem().into_bytes(),
            ca_pem: self.ca_pem.as_bytes().to_vec(),
            serial: cert
                .params()
                .serial_number
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "0".into()),
        })
    }
}
