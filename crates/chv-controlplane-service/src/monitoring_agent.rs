//! Optional guest monitoring agent service — claim redemption,
//! credential rotation, and agent-authenticated HTTPS ingestion
//! (ADR-026, campaign #602 prompt 03, gate G3).
//!
//! Trust model (agent security contract v1):
//!
//! - The guest agent is **untrusted software in a customer-controlled
//!   VM**. A `target_id` in a payload is a claim, never an identity.
//!   Identity comes from a manager-issued certificate presented over
//!   the agent-authenticated TLS connection; the registry binds that
//!   credential to exactly one `(vm, install_id, epoch)`.
//! - Enrollment claims are hashed, single-use, short-lived bearer
//!   secrets. Redeeming one requires no client certificate — the
//!   claim *is* the enrollment credential, consumed atomically.
//! - The agent generates its keypair locally and submits a CSR; the
//!   manager signs it with the dedicated agent CA and stores only
//!   public material (serial, fingerprint, not_after). It never
//!   generates, transmits, or holds a guest private key.
//! - Batches are deduplicated by `(agent_id, boot_id, sequence)`
//!   through the same durable store machinery as node batches
//!   (sender key `agent:<agent_id>`), with the same high-water-mark
//!   fail-closed behavior.
//! - Nothing here can mutate VM state. Every outcome is a typed
//!   result the HTTP layer maps to the ingestion contract's status
//!   codes; monitoring degradation never backpressenses the control
//!   plane.

use crate::error::ControlPlaneServiceError;
use crate::monitoring_validate::{
    validate_sample, RawSample, RawValue, SampleRejection, MAX_BOOT_ID_BYTES,
    MAX_SAMPLES_PER_BATCH, OUTCOME_BATCH_TOO_LARGE,
};
use chv_common::sha256_hex_bytes;
use chv_controlplane_store::{
    AgentAuthError, AgentCredential, AgentOsMetadata, ClaimConsumeError, EventAppendInput,
    EventRepository, MonitoringAgentRepository, MonitoringAgentRow, RecordBatchOutcome,
};
use chv_controlplane_types::domain::{ActorId, EventSeverity, EventType, ResourceId, ResourceKind};
use chv_monitoring_core::model::Sample;
use chv_monitoring_store::{IngestOutcome, MonitoringHealth, MonitoringStore, NodeBatch};
use dashmap::DashMap;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Maximum accepted request body on the guest routes (ingestion
/// contract v1: 256 KiB uncompressed; compressed uploads disabled).
pub const MAX_GUEST_BODY_BYTES: usize = 256 * 1024;
/// Maximum CSR PEM length accepted at redemption/rotation.
pub const MAX_CSR_PEM_BYTES: usize = 8 * 1024;
/// Maximum lengths for the OS identity allowlist fields.
pub const MAX_OS_FIELD_BYTES: usize = 64;
/// The OU that marks a certificate as a guest monitoring agent
/// credential. Node certificates do not set an OU; an agent cert
/// presented to the node API fails the node registry lookup, and a
/// node cert presented here fails this check.
pub const AGENT_CERT_OU: &str = "chv-monitor-agent";
/// Rate-limit window.
const RATE_WINDOW_MS: u64 = 60 * 1000;

/// The credential the TLS layer extracted from the connection: the
/// agent id (certificate CN) and the certificate fingerprint. Auth
/// is the registry's decision, never the payload's.
#[derive(Debug, Clone)]
pub struct AgentPeerCredential {
    pub agent_id: String,
    pub fingerprint: String,
}

/// Limits and policy knobs for guest ingestion. Defaults come from
/// `[monitoring.guest_ingestion]` config; the contract ceilings are
/// not raised here.
#[derive(Debug, Clone)]
pub struct GuestIngestionLimits {
    pub claim_ttl_ms: i64,
    pub enroll_attempts_per_minute: u32,
    pub agent_batches_per_minute: u32,
    pub rotation_grace_ms: i64,
    pub credential_ttl_ms: i64,
    pub renewal_window_ms: i64,
}

impl Default for GuestIngestionLimits {
    fn default() -> Self {
        Self {
            claim_ttl_ms: 600_000,
            enroll_attempts_per_minute: 10,
            agent_batches_per_minute: 20,
            rotation_grace_ms: 30 * 60 * 1000,
            credential_ttl_ms: 30 * 24 * 3600 * 1000,
            renewal_window_ms: 7 * 24 * 3600 * 1000,
        }
    }
}

// ---------------------------------------------------------------------------
// Certificate issuance
// ---------------------------------------------------------------------------

/// A signed agent credential. Public material only.
#[derive(Debug, Clone)]
pub struct IssuedAgentCertificate {
    pub certificate_pem: String,
    pub serial: String,
    pub fingerprint: String,
    pub not_after_ms: i64,
}

/// Signs agent CSRs with the dedicated agent CA. The manager never
/// holds an agent private key (security contract "Key provisioning").
pub struct AgentCertificateIssuer {
    ca_issuer: rcgen::Issuer<'static, rcgen::KeyPair>,
    ca_pem: String,
    ca_fingerprint: String,
}

impl AgentCertificateIssuer {
    pub fn new(ca_cert_pem: &str, ca_key_pem: &str) -> Result<Self, ControlPlaneServiceError> {
        let ca_key_pair = rcgen::KeyPair::from_pem(ca_key_pem).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to parse agent CA key: {e}"))
        })?;
        let ca_issuer = rcgen::Issuer::from_ca_cert_pem(ca_cert_pem, ca_key_pair).map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to parse agent CA cert: {e}"))
        })?;
        let ca_fingerprint = pem_der_sha256(ca_cert_pem).ok_or_else(|| {
            ControlPlaneServiceError::Internal(
                "failed to digest agent CA certificate PEM".to_string(),
            )
        })?;
        Ok(Self {
            ca_issuer,
            ca_pem: ca_cert_pem.to_string(),
            ca_fingerprint,
        })
    }

    pub fn ca_pem(&self) -> &str {
        &self.ca_pem
    }

    /// SHA-256 of the CA certificate DER — the pin shown in
    /// enrollment instructions.
    pub fn ca_fingerprint(&self) -> &str {
        &self.ca_fingerprint
    }

    /// Verify and sign a CSR for `agent_id`. The CSR's self-signature
    /// is verified during parsing; subject identity fields are
    /// overridden by the manager (the CSR's own subject is never
    /// trusted), and the certificate carries the agent OU marker, a
    /// random serial, clientAuth EKU and the configured lifetime.
    pub fn issue(
        &self,
        agent_id: &str,
        csr_pem: &str,
        ttl_ms: i64,
        now_ms: i64,
    ) -> Result<IssuedAgentCertificate, ControlPlaneServiceError> {
        let mut csr = rcgen::CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| ControlPlaneServiceError::InvalidArgument(format!("invalid CSR: {e}")))?;

        let params = &mut csr.params;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, agent_id);
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationalUnitName, AGENT_CERT_OU);
        params.is_ca = rcgen::IsCa::NoCa;
        params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
        let serial = chv_common::gen_short_id() + &chv_common::gen_short_id();
        params.serial_number = Some(rcgen::SerialNumber::from(serial.as_bytes().to_vec()));
        let not_after_ms = now_ms + ttl_ms;
        params.not_before = rcgen::date_time_ymd(1975, 1, 1);
        params.not_after = unix_ms_to_time(not_after_ms).ok_or_else(|| {
            ControlPlaneServiceError::InvalidArgument(
                "credential lifetime out of range".to_string(),
            )
        })?;

        let signed = csr
            .signed_by(&self.ca_issuer)
            .map_err(|e| ControlPlaneServiceError::Internal(format!("signing failed: {e}")))?;
        let der = signed.der();
        Ok(IssuedAgentCertificate {
            certificate_pem: signed.pem(),
            serial,
            fingerprint: sha256_hex_bytes(der.as_ref()),
            not_after_ms,
        })
    }
}

fn unix_ms_to_time(ms: i64) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000)).ok()
}

fn pem_der_sha256(pem: &str) -> Option<String> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(pem.as_bytes()).ok()?;
    Some(sha256_hex_bytes(&pem.contents))
}

// ---------------------------------------------------------------------------
// Wire types (guest JSON, ingestion contract v1)
// ---------------------------------------------------------------------------

/// The guest batch envelope. `agent_id` and `install_id` are
/// authenticated metadata: the identity is the TLS credential, and
/// `install_id` is compared against the enrollment record
/// (cloned-image detection) — neither is an authorization input.
#[derive(Debug, Deserialize)]
pub struct GuestBatchEnvelope {
    pub schema_version: i32,
    pub agent_id: String,
    pub install_id: String,
    pub boot_id: String,
    pub sequence: u64,
    #[serde(default)]
    pub sent_at_ms: i64,
    #[serde(default)]
    pub os: Option<GuestOsMetadata>,
    #[serde(default)]
    pub samples: Vec<GuestSampleJson>,
}

#[derive(Debug, Deserialize)]
pub struct GuestOsMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub kernel_release: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuestSampleJson {
    #[serde(default)]
    pub schema_version: i32,
    pub target_kind: String,
    pub target_id: String,
    pub metric_id: String,
    pub source: String,
    pub kind: String,
    pub unit: String,
    pub observed_at_ms: i64,
    /// A JSON number, or a decimal string for integers above the
    /// 2^53-1 JavaScript safe range (contract). Non-numeric strings
    /// are rejected.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub quality: String,
    #[serde(default)]
    pub dimensions: BTreeMap<String, String>,
    #[serde(default)]
    pub boot_id: String,
    #[serde(default)]
    pub identity_epoch: String,
}

impl GuestSampleJson {
    fn to_raw(&self) -> Result<RawSample, &'static str> {
        let value = match &self.value {
            None => None,
            Some(serde_json::Value::Number(n)) => {
                if let Some(i) = n.as_u64() {
                    Some(RawValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Some(RawValue::Float(f))
                } else {
                    return Err("value is not a representable number");
                }
            }
            Some(serde_json::Value::String(s)) => {
                // Exact-integer decimal string (big counters).
                if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return Err("string value must be a non-negative decimal integer");
                }
                let i: u64 = s.parse().map_err(|_| "decimal string value exceeds u64")?;
                Some(RawValue::Integer(i))
            }
            Some(_) => return Err("value must be a number or a decimal string"),
        };
        Ok(RawSample {
            target_kind: self.target_kind.clone(),
            target_id: self.target_id.clone(),
            metric_id: self.metric_id.clone(),
            source: self.source.clone(),
            kind: self.kind.clone(),
            unit: self.unit.clone(),
            observed_at_ms: self.observed_at_ms,
            quality: self.quality.clone(),
            value,
            dimensions: self.dimensions.clone(),
            boot_id: self.boot_id.clone(),
            identity_epoch: self.identity_epoch.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Service outcomes
// ---------------------------------------------------------------------------

/// Claim redemption outcome. Mapped to HTTP by the route layer.
#[derive(Debug)]
pub enum EnrollOutcome {
    Enrolled(Box<EnrolledAgent>),
    /// More redemption attempts than the per-IP budget allows.
    RateLimited,
    /// No claim row matches the token.
    UnknownClaim,
    ExpiredClaim,
    AlreadyUsedClaim,
    /// An active agent already exists for the VM (explicit
    /// revoke-then-re-enroll required).
    AlreadyEnrolled {
        agent_id: String,
    },
    /// Malformed request (bad CSR, bad lengths).
    InvalidRequest(String),
}

#[derive(Debug)]
pub struct EnrolledAgent {
    pub agent_id: String,
    /// The VM this agent is bound to — the agent must target exactly
    /// this id in every sample (the manager enforces it).
    pub vm_id: String,
    pub certificate_pem: String,
    pub ca_pem: String,
    pub credential_epoch: u64,
    pub expires_at_ms: i64,
}

/// Guest ingestion outcome (ingestion contract v1 response table).
#[derive(Debug)]
pub enum GuestIngestOutcome {
    Accepted {
        samples: u32,
        /// The agent should rotate (operator-forced or the renewal
        /// window opened).
        renewal_due: bool,
    },
    Duplicate {
        samples: u32,
        renewal_due: bool,
    },
    ReplayConflict,
    StaleSequence,
    /// HTTP 401 — no/invalid credential, revoked, expired, conflict.
    Unauthenticated(&'static str),
    /// HTTP 403 — credential is fine, target binding is not.
    ForbiddenTarget,
    /// HTTP 400 — schema/shape/timestamp violations.
    InvalidBatch(String),
    /// HTTP 413 — too many samples.
    BatchTooLarge(String),
    /// HTTP 422 — unknown metric or disallowed source.
    UnsupportedMetric(String),
    /// HTTP 429 — rate or concurrency cap.
    RateLimited {
        retry_after_seconds: u32,
    },
    /// HTTP 503 — no durable write.
    IngestionUnavailable,
}

/// Credential rotation outcome.
#[derive(Debug)]
pub enum RotateOutcome {
    /// New credential plus the credential epoch it belongs to.
    Rotated {
        cert: Box<IssuedAgentCertificate>,
        credential_epoch: u64,
    },
    /// The presented credential no longer authenticates.
    Unauthenticated(&'static str),
    /// Bad CSR.
    InvalidRequest(String),
}

#[derive(Debug, Default, Clone)]
struct RateWindow {
    window_start_ms: u64,
    count: u32,
}

// ---------------------------------------------------------------------------
// The service
// ---------------------------------------------------------------------------

/// The guest monitoring agent service: claim redemption, ingestion
/// and rotation over the agent registry and the monitoring store.
pub struct MonitoringAgentService {
    repo: MonitoringAgentRepository,
    issuer: Arc<AgentCertificateIssuer>,
    store: Option<Arc<MonitoringStore>>,
    health: MonitoringHealth,
    events: EventRepository,
    limits: GuestIngestionLimits,
    /// The base URL guests use to reach this manager (from
    /// `[monitoring.guest_ingestion] public_base_url`), surfaced in
    /// enrollment instructions.
    public_base_url: Option<String>,
    enroll_rate: DashMap<String, RateWindow>,
    agent_rate: DashMap<String, RateWindow>,
    /// Per-agent in-flight guard: the contract allows one accepted
    /// request in flight per agent.
    in_flight: DashMap<String, ()>,
}

impl MonitoringAgentService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: MonitoringAgentRepository,
        issuer: Arc<AgentCertificateIssuer>,
        store: Option<Arc<MonitoringStore>>,
        health: MonitoringHealth,
        events: EventRepository,
        limits: GuestIngestionLimits,
        public_base_url: Option<String>,
    ) -> Self {
        Self {
            repo,
            issuer,
            store,
            health,
            events,
            limits,
            public_base_url,
            enroll_rate: DashMap::new(),
            agent_rate: DashMap::new(),
            in_flight: DashMap::new(),
        }
    }

    pub fn issuer(&self) -> &Arc<AgentCertificateIssuer> {
        &self.issuer
    }

    pub fn repo(&self) -> &MonitoringAgentRepository {
        &self.repo
    }

    pub fn limits(&self) -> &GuestIngestionLimits {
        &self.limits
    }

    /// The manager address guests ingest to (deployment-configured),
    /// when set.
    pub fn public_base_url(&self) -> Option<&str> {
        self.public_base_url.as_deref()
    }

    /// Issue an enrollment claim (operator action, called from the
    /// BFF). Audit-logged.
    pub async fn issue_claim(
        &self,
        vm_id: &str,
        issued_by: &str,
        now_ms: i64,
    ) -> Result<chv_controlplane_store::IssuedClaim, ControlPlaneServiceError> {
        let claim = self
            .repo
            .issue_claim(vm_id, issued_by, self.limits.claim_ttl_ms, now_ms)
            .await?;
        self.audit(
            now_ms,
            Some(vm_id),
            issued_by,
            "monitoring_agent.claim_issued",
            format!("guest monitoring enrollment claim issued for vm {vm_id}"),
        )
        .await;
        Ok(claim)
    }

    /// Redeem a claim: atomically consume it, enroll a fresh agent
    /// identity, and sign the presented CSR. The claim is the only
    /// credential on this path — no client certificate exists yet.
    pub async fn redeem_claim(
        &self,
        claim_token: &str,
        install_id: &str,
        csr_pem: &str,
        remote_ip: Option<&str>,
        now_ms: i64,
    ) -> Result<EnrollOutcome, ControlPlaneServiceError> {
        // Rate limit per source IP before touching the claim (an
        // unknown-claim flood must not become a store scan flood).
        let ip_key = remote_ip.unwrap_or("unknown").to_string();
        {
            let mut entry = self.enroll_rate.entry(ip_key).or_default();
            let window = entry.value_mut();
            if now_ms as u64 >= window.window_start_ms + RATE_WINDOW_MS {
                window.window_start_ms = now_ms as u64;
                window.count = 0;
            }
            window.count += 1;
            if window.count > self.limits.enroll_attempts_per_minute {
                return Ok(EnrollOutcome::RateLimited);
            }
        }

        if install_id.is_empty() || install_id.len() > MAX_BOOT_ID_BYTES {
            return Ok(EnrollOutcome::InvalidRequest(
                "install_id must be 1..=128 bytes".to_string(),
            ));
        }
        if csr_pem.len() > MAX_CSR_PEM_BYTES {
            return Ok(EnrollOutcome::InvalidRequest("CSR too large".to_string()));
        }

        let consumed = match self
            .repo
            .consume_claim(claim_token, install_id, remote_ip, now_ms)
            .await?
        {
            Ok(consumed) => consumed,
            Err(ClaimConsumeError::Unknown) => return Ok(EnrollOutcome::UnknownClaim),
            Err(ClaimConsumeError::Expired) => return Ok(EnrollOutcome::ExpiredClaim),
            Err(ClaimConsumeError::AlreadyUsed) => return Ok(EnrollOutcome::AlreadyUsedClaim),
        };

        let agent_id = uuid::Uuid::new_v4().to_string();
        let issued =
            match self
                .issuer
                .issue(&agent_id, csr_pem, self.limits.credential_ttl_ms, now_ms)
            {
                Ok(issued) => issued,
                Err(e) => {
                    // The claim is consumed; redemption with this claim
                    // cannot be retried. The operator issues a new one.
                    // (Consumed-but-unenrolled is the fail-safe direction:
                    // a partially-executed redemption never yields two
                    // live identities.)
                    self.audit(
                        now_ms,
                        Some(&consumed.vm_id),
                        "system",
                        "monitoring_agent.enroll_failed",
                        format!("claim consumed but CSR rejected: {e}"),
                    )
                    .await;
                    return Ok(EnrollOutcome::InvalidRequest(format!(
                        "claim consumed, but the CSR was rejected: {e}"
                    )));
                }
            };

        let credential = AgentCredential {
            cert_serial: issued.serial.clone(),
            cert_fingerprint: issued.fingerprint.clone(),
            cert_not_after_ms: issued.not_after_ms,
        };
        let row = match self
            .repo
            .enroll_agent(
                &agent_id,
                &consumed.vm_id,
                install_id,
                &credential,
                "claim",
                now_ms,
            )
            .await
        {
            Ok(row) => row,
            Err(chv_controlplane_store::StoreError::Conflict { id, .. }) => {
                return Ok(EnrollOutcome::AlreadyEnrolled { agent_id: id });
            }
            Err(e) => return Err(e.into()),
        };

        self.audit(
            now_ms,
            Some(&consumed.vm_id),
            "claim",
            "monitoring_agent.enrolled",
            format!(
                "guest monitoring agent {agent_id} enrolled for vm {} (epoch 1)",
                consumed.vm_id
            ),
        )
        .await;

        Ok(EnrollOutcome::Enrolled(Box::new(EnrolledAgent {
            agent_id,
            vm_id: consumed.vm_id,
            certificate_pem: issued.certificate_pem,
            ca_pem: self.issuer.ca_pem().to_string(),
            credential_epoch: row.credential_epoch as u64,
            expires_at_ms: issued.not_after_ms,
        })))
    }

    /// Ingest one authenticated guest batch. Identity is the TLS
    /// credential; the payload's `agent_id`/`install_id`/`target_id`
    /// are validated against the registry binding, never trusted.
    pub async fn ingest_batch(
        &self,
        peer: &AgentPeerCredential,
        envelope: &GuestBatchEnvelope,
        now_ms: i64,
    ) -> Result<GuestIngestOutcome, ControlPlaneServiceError> {
        // 1. Registry authentication of the presented credential.
        let agent = match self
            .repo
            .authenticate_agent(
                &peer.agent_id,
                &peer.fingerprint,
                now_ms,
                self.limits.rotation_grace_ms,
            )
            .await?
        {
            Ok(agent) => agent,
            Err(AgentAuthError::Unknown) => {
                return Ok(GuestIngestOutcome::Unauthenticated(
                    "unknown agent credential",
                ))
            }
            Err(AgentAuthError::Revoked) => {
                return Ok(GuestIngestOutcome::Unauthenticated(
                    "agent credential revoked",
                ))
            }
            Err(AgentAuthError::CredentialExpired) => {
                return Ok(GuestIngestOutcome::Unauthenticated(
                    "agent credential expired",
                ))
            }
            Err(AgentAuthError::StaleCredential { .. }) => {
                return Ok(GuestIngestOutcome::Unauthenticated(
                    "agent credential superseded by rotation",
                ))
            }
            Err(AgentAuthError::IdentityConflict) => {
                return Ok(GuestIngestOutcome::Unauthenticated(
                    "agent identity conflict; operator reset required",
                ))
            }
        };

        // 2. Envelope identity consistency: the envelope's agent_id
        //    must match the credential's. (Mismatch is a client bug
        //    or an impersonation attempt — reject, never repair.)
        if envelope.agent_id != agent.agent_id {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(GuestIngestOutcome::InvalidBatch(
                "envelope agent_id does not match the authenticated credential".to_string(),
            ));
        }

        // 3. Envelope shape.
        if envelope.schema_version != 1 {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(GuestIngestOutcome::InvalidBatch(
                "schema_version must be 1".to_string(),
            ));
        }
        if envelope.boot_id.is_empty() || envelope.boot_id.len() > MAX_BOOT_ID_BYTES {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(GuestIngestOutcome::InvalidBatch(
                "boot_id must be 1..=128 bytes".to_string(),
            ));
        }
        if envelope.sequence == 0 || envelope.sequence > i64::MAX as u64 {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(GuestIngestOutcome::InvalidBatch(
                "sequence must be 1..=i64::MAX".to_string(),
            ));
        }
        if envelope.samples.len() > MAX_SAMPLES_PER_BATCH {
            self.health.update(|s| s.rejected_batches += 1);
            return Ok(GuestIngestOutcome::BatchTooLarge(format!(
                "{} samples exceeds the {} per-batch cap",
                envelope.samples.len(),
                MAX_SAMPLES_PER_BATCH
            )));
        }

        // 4. Per-agent rate + single-in-flight concurrency.
        {
            let mut entry = self.agent_rate.entry(peer.agent_id.clone()).or_default();
            let window = entry.value_mut();
            if now_ms as u64 >= window.window_start_ms + RATE_WINDOW_MS {
                window.window_start_ms = now_ms as u64;
                window.count = 0;
            }
            window.count += 1;
            if window.count > self.limits.agent_batches_per_minute {
                self.health.update(|s| s.rejected_batches += 1);
                return Ok(GuestIngestOutcome::RateLimited {
                    retry_after_seconds: 10,
                });
            }
        }
        if self.in_flight.insert(peer.agent_id.clone(), ()).is_some() {
            return Ok(GuestIngestOutcome::RateLimited {
                retry_after_seconds: 1,
            });
        }

        let outcome = self.ingest_authenticated(&agent, envelope, now_ms).await;
        self.in_flight.remove(&peer.agent_id);
        outcome
    }

    async fn ingest_authenticated(
        &self,
        agent: &MonitoringAgentRow,
        envelope: &GuestBatchEnvelope,
        now_ms: i64,
    ) -> Result<GuestIngestOutcome, ControlPlaneServiceError> {
        // 5. Store availability (typed degradation).
        let Some(store) = &self.store else {
            self.health.update(|s| s.unavailable_batches += 1);
            return Ok(GuestIngestOutcome::IngestionUnavailable);
        };

        // 6. Target binding: every sample must target exactly the
        //    authenticated agent's VM. The payload never grants scope.
        let mut samples: Vec<Sample> = Vec::with_capacity(envelope.samples.len());
        for s in &envelope.samples {
            if s.target_kind != "vm" || s.target_id != agent.vm_id {
                self.health.update(|s| s.rejected_batches += 1);
                return Ok(GuestIngestOutcome::ForbiddenTarget);
            }
            // The guest transport carries guest_agent samples only in
            // v1 (node sources arrive on the node transport).
            if s.source != "guest_agent" {
                self.health.update(|s| s.rejected_batches += 1);
                return Ok(GuestIngestOutcome::UnsupportedMetric(format!(
                    "source {} is not accepted on the guest transport",
                    s.source
                )));
            }
            let raw = match s.to_raw() {
                Ok(raw) => raw,
                Err(detail) => {
                    self.health.update(|s| s.rejected_batches += 1);
                    return Ok(GuestIngestOutcome::InvalidBatch(detail.to_string()));
                }
            };
            let _ = ResourceId::new(&agent.vm_id).map_err(|e| {
                ControlPlaneServiceError::Internal(format!("enrolled vm id invalid: {e}"))
            })?;
            match validate_sample(&raw, now_ms) {
                Ok(sample) => samples.push(sample),
                Err(SampleRejection { outcome, detail }) => {
                    self.health.update(|s| s.rejected_batches += 1);
                    return Ok(match outcome {
                        OUTCOME_BATCH_TOO_LARGE => GuestIngestOutcome::BatchTooLarge(detail),
                        crate::monitoring_validate::OUTCOME_UNSUPPORTED_METRIC => {
                            GuestIngestOutcome::UnsupportedMetric(detail)
                        }
                        _ => GuestIngestOutcome::InvalidBatch(detail),
                    });
                }
            }
        }

        // 7. Durable ingestion through the shared store machinery
        //    (dedup by sender+boot_id+sequence, high-water mark,
        //    series caps, transactional commit).
        let batch = NodeBatch {
            boot_id: envelope.boot_id.clone(),
            sequence: envelope.sequence,
            sent_at_ms: envelope.sent_at_ms.unsigned_abs(),
            samples,
        };
        let sender_key = format!("agent:{}", agent.agent_id);
        let outcome = match store
            .ingest_node_batch(&sender_key, &batch, now_ms as u64)
            .await
        {
            Ok(outcome) => outcome,
            Err(e) => {
                tracing::warn!(error = %e, "guest monitoring ingest failed");
                self.health.degrade(format!("guest ingest failure: {e}"));
                self.health.update(|h| h.unavailable_batches += 1);
                return Ok(GuestIngestOutcome::IngestionUnavailable);
            }
        };

        match outcome {
            IngestOutcome::Accepted { samples } => {
                // 8. Registry bookkeeping: last-seen, boot/sequence,
                //    OS metadata, cloned-image install check.
                let os = AgentOsMetadata {
                    name: sanitize_os_field(envelope.os.as_ref().and_then(|o| o.name.as_deref())),
                    version: sanitize_os_field(
                        envelope.os.as_ref().and_then(|o| o.version.as_deref()),
                    ),
                    kernel_release: sanitize_os_field(
                        envelope
                            .os
                            .as_ref()
                            .and_then(|o| o.kernel_release.as_deref()),
                    ),
                };
                match self
                    .repo
                    .record_batch(
                        &agent.agent_id,
                        &envelope.install_id,
                        &envelope.boot_id,
                        envelope.sequence,
                        &os,
                        now_ms,
                    )
                    .await?
                {
                    RecordBatchOutcome::Accepted => {
                        self.health.update(|s| s.accepted_batches += 1);
                        Ok(GuestIngestOutcome::Accepted {
                            samples,
                            renewal_due: self.renewal_due(agent, now_ms),
                        })
                    }
                    RecordBatchOutcome::InstallMismatch => {
                        // Cloned-image signal. The store already
                        // committed the samples under the agent's
                        // authenticated credential (the credential,
                        // not the install id, is the boundary); the
                        // conflict flag now blocks this credential
                        // until an operator reset.
                        tracing::warn!(
                            agent_id = %agent.agent_id,
                            "guest batch accepted under credential but install_id mismatch flagged"
                        );
                        Ok(GuestIngestOutcome::Unauthenticated(
                            "agent identity conflict; operator reset required",
                        ))
                    }
                }
            }
            IngestOutcome::Duplicate { samples } => Ok(GuestIngestOutcome::Duplicate {
                samples,
                renewal_due: self.renewal_due(agent, now_ms),
            }),
            IngestOutcome::ReplayConflict => Ok(GuestIngestOutcome::ReplayConflict),
            IngestOutcome::StaleSequence => Ok(GuestIngestOutcome::StaleSequence),
            other => {
                // Series cap / other store rejections.
                self.health.update(|s| s.rejected_batches += 1);
                Ok(GuestIngestOutcome::InvalidBatch(format!(
                    "store rejected the batch: {other:?}"
                )))
            }
        }
    }

    /// Rotate an agent's credential: sign a fresh CSR, advance the
    /// epoch, keep the previous credential valid for the grace
    /// window. The current credential must authenticate.
    pub async fn rotate_credential(
        &self,
        peer: &AgentPeerCredential,
        csr_pem: &str,
        now_ms: i64,
    ) -> Result<RotateOutcome, ControlPlaneServiceError> {
        let agent = match self
            .repo
            .authenticate_agent(
                &peer.agent_id,
                &peer.fingerprint,
                now_ms,
                self.limits.rotation_grace_ms,
            )
            .await?
        {
            Ok(agent) => agent,
            Err(AgentAuthError::IdentityConflict) => {
                return Ok(RotateOutcome::Unauthenticated(
                    "agent identity conflict; operator reset required",
                ))
            }
            Err(_) => {
                return Ok(RotateOutcome::Unauthenticated(
                    "agent credential not accepted",
                ))
            }
        };
        if csr_pem.len() > MAX_CSR_PEM_BYTES {
            return Ok(RotateOutcome::InvalidRequest("CSR too large".to_string()));
        }
        let issued = match self.issuer.issue(
            &agent.agent_id,
            csr_pem,
            self.limits.credential_ttl_ms,
            now_ms,
        ) {
            Ok(issued) => issued,
            Err(e) => return Ok(RotateOutcome::InvalidRequest(format!("invalid CSR: {e}"))),
        };
        let credential = AgentCredential {
            cert_serial: issued.serial.clone(),
            cert_fingerprint: issued.fingerprint.clone(),
            cert_not_after_ms: issued.not_after_ms,
        };
        self.repo
            .rotate_agent(&agent.agent_id, &credential, now_ms)
            .await?;
        self.audit(
            now_ms,
            Some(&agent.vm_id),
            &agent.agent_id,
            "monitoring_agent.rotated",
            format!(
                "guest monitoring agent {} rotated to epoch {}",
                agent.agent_id,
                agent.credential_epoch + 1
            ),
        )
        .await;
        Ok(RotateOutcome::Rotated {
            cert: Box::new(issued),
            credential_epoch: (agent.credential_epoch + 1) as u64,
        })
    }

    /// Whether the agent should rotate: the operator forced it, or
    /// the credential entered the renewal window.
    pub fn renewal_due(&self, agent: &MonitoringAgentRow, now_ms: i64) -> bool {
        agent.rotation_pending
            || (agent.cert_not_after_ms - now_ms) <= self.limits.renewal_window_ms
    }

    /// Append an audit event for a guest-agent lifecycle action.
    /// Shared with the BFF-tier admin routes (same crate).
    pub async fn audit(
        &self,
        now_ms: i64,
        vm_id: Option<&str>,
        actor: &str,
        event: &str,
        message: String,
    ) {
        let input = EventAppendInput {
            occurred_unix_ms: now_ms,
            event_type: EventType::Audit,
            severity: EventSeverity::Info,
            resource_kind: vm_id.map(|_| ResourceKind::Vm),
            resource_id: vm_id.and_then(|id| ResourceId::new(id).ok()),
            node_id: None,
            operation_id: None,
            actor_id: ActorId::new(actor).ok(),
            requested_by: Some(actor.to_string()),
            correlation_id: None,
            message,
            details: Some(serde_json::json!({ "event": event }).to_string()),
        };
        if let Err(e) = self.events.append(&input).await {
            tracing::warn!(error = %e, %event, "monitoring agent audit event append failed");
        }
    }
}

/// OS identity allowlist: strip control characters, bound the length.
/// Oversized or control-bearing values are dropped (None), never
/// truncated into something misleading.
fn sanitize_os_field(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() || value.len() > MAX_OS_FIELD_BYTES {
        return None;
    }
    if value.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return None;
    }
    Some(value.to_string())
}

/// Extract the agent identity from a presented client certificate
/// DER: the CN must be the agent id and the OU must be the agent
/// marker. Anything else (including node certificates) is rejected.
pub fn peer_credential_from_der(der: &[u8]) -> Result<AgentPeerCredential, &'static str> {
    let (_, cert) =
        x509_parser::parse_x509_certificate(der).map_err(|_| "unparseable peer certificate")?;
    let subject = &cert.tbs_certificate.subject;
    let cn = subject
        .iter_common_name()
        .next()
        .and_then(|c| c.as_str().ok())
        .ok_or("peer certificate has no CN")?;
    let ou = subject
        .iter_organizational_unit()
        .next()
        .and_then(|o| o.as_str().ok())
        .unwrap_or("");
    if ou != AGENT_CERT_OU {
        return Err("peer certificate is not a guest monitoring agent credential");
    }
    if cn.is_empty() || cn.len() > MAX_BOOT_ID_BYTES {
        return Err("peer certificate CN is not a valid agent id");
    }
    Ok(AgentPeerCredential {
        agent_id: cn.to_string(),
        fingerprint: sha256_hex_bytes(der),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_fields_are_sanitized() {
        assert_eq!(sanitize_os_field(Some("Ubuntu")), Some("Ubuntu".into()));
        assert_eq!(sanitize_os_field(None), None);
        assert_eq!(sanitize_os_field(Some("")), None);
        assert_eq!(sanitize_os_field(Some(&"x".repeat(65))), None);
        assert_eq!(sanitize_os_field(Some("bad\nname")), None);
        assert_eq!(sanitize_os_field(Some("bad\u{1}name")), None);
    }

    #[test]
    fn json_values_map_to_raw_values() {
        let mk = |v: serde_json::Value| GuestSampleJson {
            schema_version: 1,
            target_kind: "vm".into(),
            target_id: "vm-abcdefghijkl".into(),
            metric_id: "vm.guest.load1".into(),
            source: "guest_agent".into(),
            kind: "gauge".into(),
            unit: "count".into(),
            observed_at_ms: 1_000,
            value: Some(v),
            quality: "valid".into(),
            dimensions: BTreeMap::new(),
            boot_id: "boot".into(),
            identity_epoch: "agent-credential-generation-1".into(),
        };

        let raw = mk(serde_json::json!(3)).to_raw().unwrap();
        assert_eq!(raw.value, Some(RawValue::Integer(3)));

        let raw = mk(serde_json::json!(1.5)).to_raw().unwrap();
        assert_eq!(raw.value, Some(RawValue::Float(1.5)));

        // Exact-integer decimal strings (2^53-1 and beyond).
        let raw = mk(serde_json::json!("9007199254740993")).to_raw().unwrap();
        assert_eq!(raw.value, Some(RawValue::Integer(9_007_199_254_740_993)));

        // Malformed strings are rejected, not coerced.
        assert!(mk(serde_json::json!("-5")).to_raw().is_err());
        assert!(mk(serde_json::json!("12.5")).to_raw().is_err());
        assert!(mk(serde_json::json!("0x10")).to_raw().is_err());
        assert!(mk(serde_json::json!(true)).to_raw().is_err());
    }

    #[test]
    fn peer_credential_requires_agent_ou() {
        // An agent-shaped certificate parses.
        let ca = test_ca();
        let issuer = AgentCertificateIssuer::new(&ca.cert_pem, &ca.key_pem).unwrap();
        let (csr_pem, _) = test_csr();
        let issued = issuer
            .issue("agent-test-1", &csr_pem, 86_400_000, 1_000)
            .unwrap();
        let der = pem_to_der(&issued.certificate_pem);
        let peer = peer_credential_from_der(&der).unwrap();
        assert_eq!(peer.agent_id, "agent-test-1");
        assert_eq!(peer.fingerprint, issued.fingerprint);

        // A certificate without the agent OU is rejected (node certs
        // set no OU).
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "node-1");
        let ca_key = rcgen::KeyPair::from_pem(&ca.key_pem).unwrap();
        let ca_issuer = rcgen::Issuer::from_ca_cert_pem(&ca.cert_pem, ca_key).unwrap();
        let cert = params.signed_by(&key, &ca_issuer).unwrap();
        let der = pem_to_der(&cert.pem());
        assert!(peer_credential_from_der(&der).is_err());
    }

    // -- shared test CA/CSR helpers --------------------------------

    pub(crate) struct TestCa {
        pub cert_pem: String,
        pub key_pem: String,
    }

    /// A self-signed throwaway CA for unit tests.
    pub(crate) fn test_ca() -> TestCa {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::default();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "chv-test-agent-ca");
        let cert = params.self_signed(&key).unwrap();
        TestCa {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
        }
    }

    pub(crate) fn test_csr() -> (String, rcgen::KeyPair) {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "csr-subject");
        let csr = params.serialize_request(&key).unwrap().pem().unwrap();
        (csr, key)
    }

    fn pem_to_der(pem: &str) -> Vec<u8> {
        let (_, parsed) = x509_parser::pem::parse_x509_pem(pem.as_bytes()).unwrap();
        parsed.contents
    }
}
