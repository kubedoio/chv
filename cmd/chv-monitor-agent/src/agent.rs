//! The agent run loop: enroll, collect, deliver, rotate.
//!
//! Invariants (ADR-026 / ingestion contract v1):
//! - Identity is the TLS credential; the claim is consumed exactly
//!   once and removed from disk on success.
//! - Sequence numbers are allocated durably before use and never
//!   reused within a boot.
//! - Undeliverable batches live in a bounded, age-pruned disk spool
//!   and are replayed oldest-first; the agent never fabricates or
//!   zeroes data, and never blocks shutdown on the network.
//! - Rotation happens when the manager says `renewal_due` or expiry
//!   is imminent; the old credential keeps working through the
//!   manager's grace window while rotation is retried.

use crate::checks::{CheckOutcome, LocalChecks, ServiceChecks};
use crate::client::{ClientError, ManagerClient};
use crate::config::AgentConfig;
use crate::credential::StoredCredential;
use crate::plugins::PluginEngine;
use crate::spool::{DrainResult, DrainSummary, Spool, SpoolEntry};
use crate::state::AgentState;
use crate::wire::{
    CheckJson, EnvelopeJson, OsJson, SampleJson, QUALITY_VALID, SCHEMA_VERSION, SOURCE_GUEST_AGENT,
    TARGET_KIND_VM,
};
use chv_monitor_collectors::{CollectedSample, GuestCollectors, SampleValue};
use std::collections::{BTreeMap, HashMap};

/// Rotate when the credential expires within this horizon, even if
/// the manager has not flagged renewal yet (covers agents that were
/// offline across the renewal window).
const PROACTIVE_ROTATION_WINDOW_MS: i64 = 48 * 3600 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("configuration: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("state: {0}")]
    State(#[from] crate::state::StateError),
    #[error("spool: {0}")]
    Spool(#[from] crate::spool::SpoolError),
    #[error("credential: {0}")]
    Credential(#[from] crate::credential::CredentialError),
    #[error("manager CA: {0}")]
    ManagerCa(String),
}

#[derive(Debug, PartialEq)]
pub enum TickOutcome {
    /// No credential and no claim to redeem: nothing to do.
    Idle,
    /// Enrolled through a claim during this tick.
    Enrolled,
    /// At least the current batch reached the manager.
    Delivered { samples: usize },
    /// Nothing delivered; data is durably spooled for replay.
    Spooled,
    /// The manager refused the credential; operator action (revoke +
    /// fresh claim) is required.
    Unauthorized,
}

pub struct Agent {
    config: AgentConfig,
    collectors: GuestCollectors,
    /// Per-family collection cadence (component spec profiles):
    /// baseline resources + network ride every tick (15 s default);
    /// filesystems, checks (60 s) and processes (30 s) are time-gated
    /// so one interval setting cannot starve or flood a family.
    cadence: FamilyCadence,
    /// The check engines (G4): systemd services, declarative local
    /// http/tcp, and the opt-in plugin sandbox (constructed only when
    /// `[plugins] enabled` — disabled means no engine, no directory
    /// scan, nothing executed).
    service_checks: ServiceChecks,
    local_checks: LocalChecks,
    plugin_engine: Option<PluginEngine>,
    /// Per-plugin last-run stamps for interval due-ness (caller-owned
    /// by the engine's contract; agent restart simply re-runs).
    plugin_last_run: HashMap<String, i64>,
    state: AgentState,
    spool: Spool,
    credential: Option<StoredCredential>,
    client: Option<ManagerClient>,
    enroll_client: ManagerClient,
    manager_ca_pem: String,
    /// Set when the manager answered 401 — suppresses ingestion until
    /// a fresh claim re-enrolls (or the manager clears the conflict).
    unauthorized: bool,
}

/// Time-gated family scheduling. `due` is true on the first tick and
/// again after the family's interval has elapsed — a pure decision
/// over the last-collection timestamp, unit-tested without a clock.
#[derive(Debug, Default)]
struct FamilyCadence {
    last_filesystems_ms: Option<i64>,
    last_processes_ms: Option<i64>,
    last_checks_ms: Option<i64>,
}

/// Component spec profile intervals.
const FILESYSTEMS_INTERVAL_MS: i64 = 60_000;
const PROCESSES_INTERVAL_MS: i64 = 30_000;
/// The checks family (systemd services, local http/tcp, plugins)
/// rides the same 60 s profile as the service discovery it serves.
const CHECKS_INTERVAL_MS: i64 = 60_000;

impl FamilyCadence {
    fn filesystems_due(&self, now_ms: i64) -> bool {
        self.last_filesystems_ms
            .is_none_or(|last| now_ms - last >= FILESYSTEMS_INTERVAL_MS)
    }
    fn processes_due(&self, now_ms: i64) -> bool {
        self.last_processes_ms
            .is_none_or(|last| now_ms - last >= PROCESSES_INTERVAL_MS)
    }
    fn checks_due(&self, now_ms: i64) -> bool {
        self.last_checks_ms
            .is_none_or(|last| now_ms - last >= CHECKS_INTERVAL_MS)
    }
}

impl Agent {
    pub fn start(config: AgentConfig) -> Result<Self, AgentError> {
        let manager_ca_pem = std::fs::read_to_string(&config.manager_ca_path).map_err(|e| {
            AgentError::ManagerCa(format!(
                "failed to read {}: {e}",
                config.manager_ca_path.display()
            ))
        })?;
        let state = AgentState::load(&config.state_dir)?;
        state.prune_sequences(8).ok();
        let spool = Spool::open(
            &config.spool_dir,
            config.max_spool_batches,
            config.spool_max_age_seconds,
        )?;
        let credential = StoredCredential::load(&config.credential_path)?;
        let client = match &credential {
            Some(cred) => Some(
                ManagerClient::new(&config.server_url, &manager_ca_pem, Some(cred))
                    .map_err(|e| AgentError::ManagerCa(e.to_string()))?,
            ),
            None => None,
        };
        let enroll_client = ManagerClient::new(&config.server_url, &manager_ca_pem, None)
            .map_err(|e| AgentError::ManagerCa(e.to_string()))?;
        // Collector opt-ins come from config (validated at load).
        let mut collectors = GuestCollectors::new();
        if config.collectors.filesystems {
            collectors = collectors.enable_filesystems();
        }
        if config.collectors.network {
            collectors = collectors.enable_network();
        }
        if config.collectors.processes {
            collectors = collectors.enable_processes(config.collectors.process_selectors.clone());
        }
        // The plugin engine exists only when explicitly enabled: a
        // disabled config never touches the allowlist directory.
        let plugin_engine = config
            .plugins
            .enabled
            .then(|| PluginEngine::new(config.plugins.directory.clone()));
        Ok(Self {
            config,
            collectors,
            cadence: FamilyCadence::default(),
            service_checks: ServiceChecks::new(),
            local_checks: LocalChecks::new(),
            plugin_engine,
            plugin_last_run: HashMap::new(),
            state,
            spool,
            credential,
            client,
            enroll_client,
            manager_ca_pem,
            unauthorized: false,
        })
    }

    pub fn spool_len(&self) -> usize {
        self.spool.len()
    }

    pub fn is_enrolled(&self) -> bool {
        self.credential.is_some()
    }

    /// One collection/delivery cycle. Never panics, never blocks
    /// indefinitely; all failures degrade to spooling or idling.
    pub async fn tick(&mut self) -> TickOutcome {
        let enrolled_this_tick = self.ensure_enrolled().await;
        if enrolled_this_tick {
            // Enrollment is a complete, auditable step on its own; the
            // first collection follows on the next tick.
            return TickOutcome::Enrolled;
        }
        let Some(credential) = self.credential.clone() else {
            return TickOutcome::Idle;
        };
        if self.unauthorized {
            return TickOutcome::Unauthorized;
        }

        let now_ms = chrono_like_now_ms();
        let envelope = self.build_envelope(&credential, now_ms).await;
        let samples = envelope.samples.len();
        if let Err(e) = self.spool.push(SpoolEntry {
            agent_id: envelope.agent_id.clone(),
            boot_id: envelope.boot_id.clone(),
            sequence: envelope.sequence,
            envelope,
        }) {
            tracing::warn!(error = %e, "failed to spool batch; data loss this tick");
            return TickOutcome::Spooled;
        }

        let client = self
            .client
            .clone()
            .expect("client exists while a credential exists");
        let drain_summary: DrainSummary = match self
            .spool
            .drain(|entry| {
                let client = client.clone();
                async move {
                    match client.ingest(&entry.envelope).await {
                        Ok(resp) => {
                            tracing::debug!(
                                status = %resp.status,
                                sequence = entry.sequence,
                                "ingest acknowledged"
                            );
                            DrainResult::Delivered {
                                renewal_due: resp.is_renewal_due(),
                            }
                        }
                        Err(e) => classify(&e, &entry),
                    }
                }
            })
            .await
        {
            Ok(summary) => summary,
            Err(e) => {
                tracing::warn!(error = %e, "spool drain failed");
                return TickOutcome::Spooled;
            }
        };

        if drain_summary.unauthorized {
            self.unauthorized = true;
        }

        let outcome = if self.unauthorized {
            TickOutcome::Unauthorized
        } else if drain_summary.delivered_any {
            TickOutcome::Delivered { samples }
        } else {
            TickOutcome::Spooled
        };

        // Rotation: when the manager asked for it, or expiry is close
        // enough that the next outage could strand the credential.
        let expires_within_window =
            credential.expires_at_ms - now_ms <= PROACTIVE_ROTATION_WINDOW_MS;
        if drain_summary.renewal_due || expires_within_window {
            self.rotate_now().await;
        }

        outcome
    }

    /// Redeem a claim when one is available and no usable credential
    /// exists. Returns true when enrollment happened this call.
    async fn ensure_enrolled(&mut self) -> bool {
        let needs_enroll =
            self.credential.is_none() || (self.unauthorized && self.claim_available());
        if !needs_enroll {
            return false;
        }
        let Some(claim) = self.read_claim() else {
            return false;
        };
        let (csr_pem, key_pem) = generate_csr();
        let install_id = self.state.install_id().to_string();
        match self
            .enroll_client
            .enroll(&claim, &install_id, &csr_pem)
            .await
        {
            Ok(resp) => {
                let credential = StoredCredential::from_enroll(&resp, key_pem);
                if let Err(e) = credential.store(&self.config.credential_path) {
                    tracing::error!(error = %e, "enrolled but failed to persist credential");
                    return false;
                }
                // The claim is consumed server-side; remove the local
                // copy so a restart cannot replay it.
                let _ = std::fs::remove_file(&self.config.claim_path);
                // Anything spooled under a revoked prior identity can
                // never be attributed; drop it rather than let it
                // starve the replay queue.
                if let Err(e) = self.spool.purge_other_agents(&resp.agent_id) {
                    tracing::warn!(error = %e, "failed to purge stale spool entries");
                }
                self.credential = Some(credential.clone());
                self.client = Some(
                    ManagerClient::new(
                        &self.config.server_url,
                        &self.manager_ca_pem,
                        Some(&credential),
                    )
                    .expect("client rebuilt from a working config"),
                );
                self.unauthorized = false;
                tracing::info!(
                    agent_id = %resp.agent_id,
                    vm_id = %resp.vm_id,
                    "enrolled with the manager over mutual TLS"
                );
                true
            }
            Err(e) => {
                tracing::warn!(
                    error = %error_chain(&e),
                    "enrollment attempt failed; will retry"
                );
                false
            }
        }
    }

    async fn rotate_now(&mut self) {
        let Some(credential) = self.credential.clone() else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        let (csr_pem, key_pem) = generate_csr();
        match client.rotate(&csr_pem).await {
            Ok(resp) => {
                let mut cred = credential;
                cred.apply_rotation(
                    resp.certificate_pem,
                    resp.credential_epoch,
                    resp.expires_at_ms,
                );
                // The new private key replaces the old one atomically
                // in the stored credential.
                cred.private_key_pem = key_pem;
                if let Err(e) = cred.store(&self.config.credential_path) {
                    tracing::error!(error = %e, "rotated but failed to persist credential");
                    return;
                }
                self.credential = Some(cred.clone());
                self.client = Some(
                    ManagerClient::new(&self.config.server_url, &self.manager_ca_pem, Some(&cred))
                        .expect("client rebuilt from a working config"),
                );
                tracing::info!(epoch = resp.credential_epoch, "credential rotated");
            }
            Err(e) => {
                // The manager's grace window keeps the old credential
                // usable; retry next tick.
                tracing::warn!(error = %e, "credential rotation failed; retrying next tick");
            }
        }
    }

    fn claim_available(&self) -> bool {
        self.config.claim_path.exists()
    }

    fn read_claim(&self) -> Option<String> {
        let claim = std::fs::read_to_string(&self.config.claim_path).ok()?;
        let claim = claim.trim();
        if claim.is_empty() {
            None
        } else {
            Some(claim.to_string())
        }
    }

    async fn build_envelope(&mut self, credential: &StoredCredential, now_ms: i64) -> EnvelopeJson {
        // Baseline (resources profile) rides every tick; the G4
        // families follow their profile cadence. Everything collected
        // in one cycle forms one batch, bounded by the family budgets
        // (worst case stays under the contract's 512 samples).
        let mut collected = self.collectors.collect();
        if self.cadence.filesystems_due(now_ms) {
            collected.extend(self.collectors.collect_filesystems());
            self.cadence.last_filesystems_ms = Some(now_ms);
        }
        // The network profile is 15 s — the default tick — so it
        // rides every tick rather than its own gate.
        collected.extend(self.collectors.collect_network());
        if self.cadence.processes_due(now_ms) {
            collected.extend(self.collectors.collect_processes(now_ms.unsigned_abs()));
            self.cadence.last_processes_ms = Some(now_ms);
        }
        // The checks family (60 s profile): systemd services,
        // declarative local http/tcp and due plugins. The outcomes
        // become the envelope's checks array AND the check.status /
        // check.duration_seconds samples (dimensioned by check_id);
        // service outcomes additionally justify their
        // vm.guest.service.up samples. The agent is the authoritative
        // timekeeper — durations are engine-measured, never trusted
        // from check output. Worst case stays inside the ingest
        // contract's 512-sample batch ceiling (see the budget note on
        // the collectors' bound constants).
        let mut checks_json: Vec<CheckJson> = Vec::new();
        if self.cadence.checks_due(now_ms) {
            self.cadence.last_checks_ms = Some(now_ms);
            let mut outcomes: Vec<CheckOutcome> = Vec::new();
            if self.config.collectors.services {
                let services = self
                    .service_checks
                    .run(
                        &self.config.services.configured,
                        self.config.services.discover,
                        now_ms,
                    )
                    .await;
                collected.extend(services.service_up_samples);
                outcomes.extend(services.outcomes);
            }
            if !self.config.checks.http.is_empty() || !self.config.checks.tcp.is_empty() {
                outcomes.extend(self.local_checks.run(&self.config.checks, now_ms).await);
            }
            if let Some(engine) = &self.plugin_engine {
                outcomes.extend(engine.run_due(now_ms, &mut self.plugin_last_run).await);
            }
            let (records, samples) = outcomes_to_wire(outcomes);
            checks_json = records;
            collected.extend(samples);
        }
        let os = self.collectors.os_identity();
        let boot_id = {
            let b = self.collectors.boot_id();
            if b.is_empty() {
                // No procfs boot id (non-Linux dev run): the install id
                // keeps the dedup key sound (sequences are persisted
                // and never reused).
                self.state.install_id().to_string()
            } else {
                b
            }
        };
        let sequence = self
            .state
            .next_sequence(&boot_id)
            .expect("sequence allocation is local and atomic");
        let identity_epoch = credential.identity_epoch();
        let samples = build_samples(
            &collected,
            &credential.vm_id,
            &boot_id,
            &identity_epoch,
            now_ms,
        );
        EnvelopeJson {
            schema_version: SCHEMA_VERSION,
            agent_id: credential.agent_id.clone(),
            install_id: self.state.install_id().to_string(),
            boot_id,
            sequence,
            sent_at_ms: now_ms,
            os: Some(OsJson {
                name: os.name,
                version: os.version,
                kernel_release: os.kernel_release,
            }),
            samples,
            // The 60 s checks family's records (empty when no checks
            // are configured or the family is between cadence ticks).
            checks: checks_json,
        }
    }
}

/// Map check-engine outcomes to their envelope forms: the records
/// for the `checks` array plus the `check.status` (typed integer
/// state, never a float) and `check.duration_seconds` samples, each
/// dimensioned by `check_id`. Pure — unit-tested without engines.
/// The agent is the authoritative timekeeper: durations here are the
/// engine-measured values, never a check's own claim.
fn outcomes_to_wire(outcomes: Vec<CheckOutcome>) -> (Vec<CheckJson>, Vec<CollectedSample>) {
    let mut records = Vec::with_capacity(outcomes.len());
    let mut samples = Vec::with_capacity(outcomes.len() * 2);
    for outcome in outcomes {
        samples.push(CollectedSample {
            metric_id: "check.status",
            value: SampleValue::Integer(outcome.status().code()),
            dimension: Some(("check_id", outcome.check.check_id.clone())),
        });
        samples.push(CollectedSample {
            metric_id: "check.duration_seconds",
            value: SampleValue::Float(outcome.duration_ms as f64 / 1000.0),
            dimension: Some(("check_id", outcome.check.check_id.clone())),
        });
        records.push(outcome.check);
    }
    (records, samples)
}

/// Map a transport result to the spool's drain decision. Pure
/// classification, extracted for clarity.
fn classify(e: &ClientError, entry: &SpoolEntry) -> DrainResult {
    match e {
        ClientError::Transport(_) | ClientError::Decode(_) => DrainResult::Retry,
        ClientError::Api { status, code, .. } => match (*status, code.as_str()) {
            (409, "replay_conflict") | (409, "resync_required") => {
                // The manager can never accept this batch; dropping it
                // is the only forward path (the durable high-water
                // mark already holds the sequence).
                tracing::warn!(
                    sequence = entry.sequence,
                    code,
                    "manager rejected a spooled batch permanently; discarding"
                );
                DrainResult::Discard
            }
            (401, _) => {
                tracing::error!(
                    "manager refused the credential (revoked or identity conflict); \
                     waiting for operator re-enrollment (revoke + fresh claim)"
                );
                DrainResult::Unauthorized
            }
            (400, "invalid_batch") => {
                // A permanently malformed batch (e.g. spooled under a
                // previous agent identity before re-enrollment):
                // retrying can never succeed, and keeping it at the
                // spool head would starve every later batch.
                tracing::warn!(
                    sequence = entry.sequence,
                    "manager rejected a spooled batch as invalid; discarding"
                );
                DrainResult::Discard
            }
            (429, _) => DrainResult::Retry,
            // 5xx and anything unexpected: retry later.
            _ => DrainResult::Retry,
        },
    }
}

/// Map collected values to contract samples, resolving kind/unit from
/// the shared registry (the agent never hard-codes them).
fn build_samples(
    collected: &[CollectedSample],
    vm_id: &str,
    boot_id: &str,
    identity_epoch: &str,
    now_ms: i64,
) -> Vec<SampleJson> {
    collected
        .iter()
        .filter_map(|s| {
            let def = chv_monitoring_core::registry::lookup(s.metric_id)?;
            let value = match s.value {
                SampleValue::Float(v) => {
                    if !v.is_finite() {
                        return None;
                    }
                    serde_json::json!(v)
                }
                SampleValue::Integer(v) => {
                    // Contract: exact integers above the JavaScript
                    // safe range (2^53 - 1) travel as decimal strings
                    // — the manager parses them back exactly, and a
                    // JSON number would lose precision in any JS hop.
                    if v > (1u64 << 53) - 1 {
                        serde_json::json!(v.to_string())
                    } else {
                        serde_json::json!(v)
                    }
                }
            };
            let mut dimensions = BTreeMap::new();
            if let Some((key, dim)) = &s.dimension {
                dimensions.insert(key.to_string(), dim.clone());
            }
            Some(SampleJson {
                schema_version: SCHEMA_VERSION,
                target_kind: TARGET_KIND_VM.to_string(),
                target_id: vm_id.to_string(),
                metric_id: s.metric_id.to_string(),
                source: SOURCE_GUEST_AGENT.to_string(),
                kind: def.kind.as_str().to_string(),
                unit: def.unit.as_str().to_string(),
                observed_at_ms: now_ms,
                value,
                quality: QUALITY_VALID.to_string(),
                dimensions,
                boot_id: boot_id.to_string(),
                identity_epoch: identity_epoch.to_string(),
            })
        })
        .collect()
}

fn generate_csr() -> (String, String) {
    let key = rcgen::KeyPair::generate().expect("key generation cannot fail");
    let mut params = rcgen::CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    // The manager overrides the subject to CN=agent_id; the CSR
    // subject is decorative.
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "chv-monitor-agent");
    let csr = params
        .serialize_request(&key)
        .expect("csr serialization cannot fail")
        .pem()
        .expect("pem serialization cannot fail");
    (csr, key.serialize_pem())
}

fn chrono_like_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Full `caused by` chain — reqwest's own Display stops at the first
/// layer, which is useless in operations logs.
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        out.push_str("; caused by: ");
        out.push_str(&s.to_string());
        src = s.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_monitor_collectors::SampleValue;

    #[test]
    fn family_cadence_gates_sixty_and_thirty_second_families() {
        let mut c = FamilyCadence::default();
        // First tick: everything due.
        assert!(c.filesystems_due(1_000));
        assert!(c.processes_due(1_000));
        c.last_filesystems_ms = Some(1_000);
        c.last_processes_ms = Some(1_000);
        // +15 s: neither due yet.
        assert!(!c.filesystems_due(16_000));
        assert!(!c.processes_due(16_000));
        // +30 s: processes due, filesystems not.
        assert!(!c.filesystems_due(31_000));
        assert!(c.processes_due(31_000));
        // +60 s: both due.
        assert!(c.filesystems_due(61_000));
        assert!(c.processes_due(61_000));
        // A backwards clock never re-arms a family.
        c.last_filesystems_ms = Some(100_000);
        assert!(!c.filesystems_due(61_000));
    }

    #[test]
    fn family_cadence_gates_the_sixty_second_checks_family() {
        let mut c = FamilyCadence::default();
        assert!(c.checks_due(1_000), "first tick: checks due");
        c.last_checks_ms = Some(1_000);
        assert!(!c.checks_due(16_000), "+15 s: not due");
        assert!(!c.checks_due(60_000), "+59 s: not due");
        assert!(c.checks_due(61_000), "+60 s: due");
        c.last_checks_ms = Some(100_000);
        assert!(!c.checks_due(61_000), "backwards clock never re-arms");
    }

    #[test]
    fn outcomes_to_wire_maps_records_and_typed_state_samples() {
        use crate::checks::CheckOutcome;
        use crate::wire::CheckJson;
        use chv_monitoring_core::model::CheckStatus;

        let outcome = |check_id: &str, status: CheckStatus, duration_ms: u64| CheckOutcome {
            check: CheckJson {
                schema_version: 1,
                check_id: check_id.to_string(),
                service_key: Some("nginx.service".to_string()),
                status: status.as_str().to_string(),
                summary: Some("active (running)".to_string()),
                observed_at_ms: 1_000,
            },
            duration_ms,
        };
        let (records, samples) = outcomes_to_wire(vec![
            outcome("service:nginx.service", CheckStatus::Ok, 42),
            outcome("http:app", CheckStatus::Critical, 5_100),
        ]);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].check_id, "service:nginx.service");
        // One status + one duration sample per record, dimensioned by
        // check_id, statuses as TYPED INTEGER state codes (never
        // floats), durations as engine-measured seconds.
        assert_eq!(samples.len(), 4);
        let status = samples
            .iter()
            .find(|s| {
                s.metric_id == "check.status"
                    && s.dimension.as_ref().map(|d| d.1.as_str()) == Some("http:app")
            })
            .expect("http:app status sample");
        assert_eq!(status.value, SampleValue::Integer(2), "critical = code 2");
        let duration = samples
            .iter()
            .find(|s| {
                s.metric_id == "check.duration_seconds"
                    && s.dimension.as_ref().map(|d| d.1.as_str()) == Some("http:app")
            })
            .expect("http:app duration sample");
        match duration.value {
            SampleValue::Float(v) => assert!((v - 5.1).abs() < 1e-9, "5100 ms -> 5.1 s"),
            other => panic!("duration must be a float, got {other:?}"),
        }
        // Empty outcomes: empty wire forms (no fabricated samples).
        let (records, samples) = outcomes_to_wire(Vec::new());
        assert!(records.is_empty());
        assert!(samples.is_empty());
    }

    #[test]
    fn build_samples_encodes_big_integers_as_decimal_strings() {
        // Contract: integers above the JS-safe range (2^53 - 1)
        // travel as decimal strings; smaller ones stay JSON numbers.
        let collected = vec![
            CollectedSample {
                metric_id: "vm.guest.net.rx_bytes_total",
                value: SampleValue::Integer((1u64 << 53) - 1),
                dimension: Some(("interface_id", "phys:eth0".to_string())),
            },
            CollectedSample {
                metric_id: "vm.guest.net.tx_bytes_total",
                value: SampleValue::Integer(1u64 << 53),
                dimension: Some(("interface_id", "phys:eth0".to_string())),
            },
        ];
        let out = build_samples(&collected, "vm-1", "boot", "epoch-1", 1_000);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].value, serde_json::json!((1u64 << 53) - 1));
        assert_eq!(
            out[1].value,
            serde_json::json!((1u64 << 53).to_string()),
            "2^53 must encode as a decimal string"
        );
        assert_eq!(
            out[0].dimensions.get("interface_id").map(String::as_str),
            Some("phys:eth0")
        );
        // Kind/unit resolve from the registry, never hard-coded.
        assert_eq!(out[0].kind, "counter");
        assert_eq!(out[0].unit, "bytes");
    }
}
