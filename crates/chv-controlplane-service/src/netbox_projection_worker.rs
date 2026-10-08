//! NetBox projection worker — the composition root of the projection
//! (PR 4 of the #239 plan).
//!
//! Mirrors [`crate::backup_worker::BackupWorker`]'s loop shape: a
//! clone-able struct, a `run(shutdown_rx)` tokio loop on a 30s tick,
//! `warn!`-and-continue on tick errors, never a panic. Each tick:
//!
//! 1. **Reclamation** — runs stuck in `running` for more than
//!    [`RUN_LEASE_TIMEOUT`] (worker crash / stall) are failed by the
//!    store's CAS-guarded `reclaim_stale_running`; a reclaimed run is
//!    retried through the normal requeue path and re-enters
//!    idempotently via the external-id match.
//! 2. **Claim** — one queued run per architecture per tick
//!    (`list_architecture_ids_with_queued` + `claim_next_queued`; the
//!    claim is a single atomic UPDATE, so concurrent workers cannot
//!    double-claim).
//! 3. **Execute** — load the config and decrypt the token in-memory
//!    (fail-closed), resolve the applied version, parse the
//!    `CHVArchitecture` model, and hand plain data to
//!    [`chv_netbox_adapter::NetboxProjectionRunner`]. The adapter stays
//!    store-free; this worker is the only place the two worlds meet.
//! 4. **Persist** — `mark_succeeded` with the serialized outcome and
//!    plan summary, or `mark_failed` with a redacted, secret-free
//!    error message, plus an `EventType::Audit` event carrying the
//!    run/architecture ids and summary (never the token).
//!
//! A NetBox outage never kills the worker: every error path marks the
//! run failed, emits an event, and continues to the next tick.

use std::sync::Arc;
use std::time::Duration;

use chv_architecture_validate::model::CHVArchitecture;
use chv_controlplane_store::{
    ApplyRunRepository, EventAppendInput, EventRepository, NetboxProjectionConfigRepository,
    NetboxProjectionRunRepository, VersionRepository,
};
use chv_controlplane_types::architecture::{
    NetboxProjectionMode, NetboxProjectionRun, NetboxRetentionPolicy, RunStatus,
};
use chv_controlplane_types::domain::{EventSeverity, EventType};
use chv_errors::ChvError;
use chv_netbox_adapter::{
    ownership::CustomFieldNames, plan::RetentionPolicy, ClientError, NetBoxClient, NetBoxToken,
    NetboxProjectionInput, NetboxProjectionRunner,
};
use tracing::{info, warn};

/// How long a `running` run may execute before reclamation (the
/// component spec's lease timeout; the client's 30s request timeout
/// bounds a single HTTP call, so 15 minutes covers a full plan with
/// generous headroom).
const RUN_LEASE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Upper bound on architectures processed per tick — keeps one tick
/// bounded even on fleets with many queued architectures.
const MAX_ARCHITECTURES_PER_TICK: usize = 32;

/// Audit event names (component spec "Security, RBAC and secrets"):
/// emitted as `EventType::Audit` with the event name as the message
/// and structured details (run id, architecture id, summary — never
/// the token).
pub const EVENT_NETBOX_EXPORT_SUCCEEDED: &str = "architecture_netbox_export_succeeded";
pub const EVENT_NETBOX_EXPORT_FAILED: &str = "architecture_netbox_export_failed";
pub const EVENT_NETBOX_DRY_RUN: &str = "architecture_netbox_dry_run";

/// Seam for tests: how the worker builds its NetBox client from the
/// stored config. Production uses [`NetBoxClient::new`] (HTTPS-only);
/// the wiremock suites inject the test-only unchecked constructor.
type ClientFactory =
    Arc<dyn Fn(&str, NetBoxToken) -> Result<NetBoxClient, ClientError> + Send + Sync>;

/// Background worker that claims queued NetBox projection runs and
/// executes them against the configured NetBox instance.
#[derive(Clone)]
pub struct NetboxProjectionWorker {
    run_repo: NetboxProjectionRunRepository,
    config_repo: NetboxProjectionConfigRepository,
    event_repo: EventRepository,
    apply_run_repo: ApplyRunRepository,
    version_repo: VersionRepository,
    tick_interval: Duration,
    client_factory: ClientFactory,
}

impl NetboxProjectionWorker {
    pub fn new(
        run_repo: NetboxProjectionRunRepository,
        config_repo: NetboxProjectionConfigRepository,
        event_repo: EventRepository,
        apply_run_repo: ApplyRunRepository,
        version_repo: VersionRepository,
    ) -> Self {
        Self {
            run_repo,
            config_repo,
            event_repo,
            apply_run_repo,
            version_repo,
            tick_interval: Duration::from_secs(30),
            client_factory: Arc::new(NetBoxClient::new),
        }
    }

    /// Replace the client-construction seam (tests only).
    pub fn with_client_factory(mut self, factory: ClientFactory) -> Self {
        self.client_factory = factory;
        self
    }

    pub async fn run(self, mut shutdown_rx: tokio::sync::watch::Receiver<()>) {
        info!("netbox projection worker starting");
        let mut interval = tokio::time::interval(self.tick_interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = shutdown_rx.changed() => {
                    info!("netbox projection worker shutting down");
                    break;
                }
            }
            if let Err(e) = self.tick().await {
                warn!(error = %e, "netbox projection worker tick failed");
            }
        }
    }

    /// One worker iteration. Store-level failures bubble to `run`
    /// (warn + continue); per-run failures are handled inside
    /// `process_run` (mark failed + event) and never bubble.
    pub async fn tick(&self) -> Result<(), ChvError> {
        self.reclaim_stale_runs().await?;
        self.claim_and_process().await?;
        Ok(())
    }

    async fn reclaim_stale_runs(&self) -> Result<(), ChvError> {
        let before =
            chrono::Utc::now() - chrono::Duration::seconds(RUN_LEASE_TIMEOUT.as_secs() as i64);
        let reclaimed = self
            .run_repo
            .reclaim_stale_running(before)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to reclaim stale netbox projection runs: {e}"),
            })?;
        for run in reclaimed {
            warn!(
                run_id = %run.id,
                architecture_id = %run.architecture_id,
                "reclaimed stale netbox projection run (execution lease expired)"
            );
            self.emit_event(
                &run,
                EVENT_NETBOX_EXPORT_FAILED,
                EventSeverity::Error,
                &serde_json::json!({
                    "run_id": run.id.to_string(),
                    "architecture_id": run.architecture_id.to_string(),
                    "reason": "reclaimed",
                }),
            )
            .await;
        }
        Ok(())
    }

    async fn claim_and_process(&self) -> Result<(), ChvError> {
        let architecture_ids = self
            .run_repo
            .list_architecture_ids_with_queued()
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to list architectures with queued netbox runs: {e}"),
            })?;

        for architecture_id in architecture_ids
            .into_iter()
            .take(MAX_ARCHITECTURES_PER_TICK)
        {
            // One claim per architecture per tick; the claim is an
            // atomic queued→running UPDATE, so two workers (or two
            // ticks) can never double-claim the same run.
            let Some(run) = self
                .run_repo
                .claim_next_queued(&architecture_id)
                .await
                .map_err(|e| ChvError::Internal {
                    reason: format!("failed to claim netbox projection run: {e}"),
                })?
            else {
                continue;
            };
            if let Err(e) = self.process_run(&run).await {
                warn!(
                    run_id = %run.id,
                    architecture_id = %run.architecture_id,
                    error = %e,
                    "failed to process netbox projection run"
                );
            }
        }
        Ok(())
    }

    /// Execute one claimed run end to end. Every failure path marks the
    /// run failed and emits an audit event; nothing panics and nothing
    /// propagates to the worker loop.
    async fn process_run(&self, run: &NetboxProjectionRun) -> Result<(), ChvError> {
        // Config: absent → NETBOX_NOT_CONFIGURED (the post-apply trigger
        // coalesces instead of enqueueing, but a config deleted between
        // enqueue and execution still lands here).
        let Some(config) = self
            .config_repo
            .get(&run.architecture_id)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to load netbox projection config: {e}"),
            })?
        else {
            return self
                .fail_run(
                    run,
                    "netbox not configured for this architecture (NETBOX_NOT_CONFIGURED)",
                )
                .await;
        };

        // Token: decrypt in-memory only, fail-closed. The plaintext
        // never leaves this function.
        let token = match self.config_repo.read_token(&run.architecture_id).await {
            Ok(Some(token)) => token,
            Ok(None) => {
                return self
                    .fail_run(
                        run,
                        "netbox token missing for this architecture (NETBOX_TOKEN_MISSING)",
                    )
                    .await;
            }
            Err(e) => {
                // Fail-closed: the store never returns the ciphertext,
                // and neither does this message.
                warn!(
                    architecture_id = %run.architecture_id,
                    error = %e,
                    "netbox token decrypt failed (fail-closed)"
                );
                return self
                    .fail_run(
                        run,
                        "netbox token unreadable for this architecture (NETBOX_TOKEN_MISSING)",
                    )
                    .await;
            }
        };

        // Applied version gate: the projection source is the most
        // recent succeeded apply run's lineage, never the editable
        // draft. `list_for_architecture` does not filter by status, so
        // filter here; the list is newest-first.
        let apply_runs = self
            .apply_run_repo
            .list_for_architecture(&run.architecture_id, None)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to list apply runs: {e}"),
            })?;
        if !apply_runs
            .iter()
            .any(|apply| apply.status == RunStatus::Succeeded)
        {
            return self
                .fail_run(
                    run,
                    "no succeeded apply run for this architecture (NETBOX_NOT_APPLIED)",
                )
                .await;
        }

        // Version: the run row's `architecture_version_id` is authoritative
        // (set at enqueue time to the then-applied version); the succeeded
        // apply run above is the gate proving the architecture was applied.
        let version = self
            .version_repo
            .get(&run.architecture_version_id, None)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to load architecture version: {e}"),
            })?;
        let Some(model_json) = version.normalized_model_json.as_deref() else {
            return self
                .fail_run(
                    run,
                    "applied architecture version carries no normalized model",
                )
                .await;
        };
        let architecture: CHVArchitecture = match serde_json::from_str(model_json) {
            Ok(model) => model,
            Err(e) => {
                return self
                    .fail_run(
                        run,
                        &format!("applied architecture model could not be parsed: {e}"),
                    )
                    .await;
            }
        };

        // Client: HTTPS enforced by the constructor (fail-closed).
        let client = match (self.client_factory)(&config.endpoint, NetBoxToken::new(token.clone()))
        {
            Ok(client) => client,
            Err(e) => {
                return self.fail_run(run, &redact(&e.to_string(), &token)).await;
            }
        };
        let runner = NetboxProjectionRunner::new(client);

        let input = NetboxProjectionInput {
            architecture: &architecture,
            architecture_id: run.architecture_id.as_str(),
            architecture_version: version.version_number as u64,
            snapshot: None,
            site_name: config.site_name.as_deref(),
            retention: match config.retention_policy {
                NetboxRetentionPolicy::MarkStale => RetentionPolicy::MarkStale,
                NetboxRetentionPolicy::Delete => RetentionPolicy::Delete,
            },
            names: CustomFieldNames::new(&config.custom_field_prefix),
        };

        match run.mode {
            NetboxProjectionMode::DryRun => match runner.dry_run(&input).await {
                Ok(plan) => {
                    let result_json =
                        Some(
                            serde_json::to_string(&plan).map_err(|e| ChvError::Internal {
                                reason: format!("failed to serialize dry-run plan: {e}"),
                            })?,
                        );
                    let summary_json = serde_json::to_string(&plan.summary).ok();
                    self.finish_run(run, &plan.summary, result_json, summary_json)
                        .await
                }
                Err(e) => self.fail_run(run, &redact(&e.to_string(), &token)).await,
            },
            NetboxProjectionMode::Export => match runner.run(&input).await {
                Ok(outcome) => {
                    if let Some(error) = &outcome.error {
                        // Partial failure: the executed entries are
                        // resumable via the external-id match on retry.
                        return self.fail_run(run, &redact(&error.message, &token)).await;
                    }
                    let result_json =
                        Some(
                            serde_json::to_string(&outcome).map_err(|e| ChvError::Internal {
                                reason: format!("failed to serialize projection outcome: {e}"),
                            })?,
                        );
                    let summary_json = serde_json::to_string(&outcome.plan.summary).ok();
                    self.finish_run(run, &outcome.plan.summary, result_json, summary_json)
                        .await
                }
                Err(e) => self.fail_run(run, &redact(&e.to_string(), &token)).await,
            },
        }
    }

    /// Terminal `running → succeeded` + success event.
    async fn finish_run(
        &self,
        run: &NetboxProjectionRun,
        summary: &chv_netbox_adapter::PlanSummary,
        result_json: Option<String>,
        summary_json: Option<String>,
    ) -> Result<(), ChvError> {
        self.run_repo
            .mark_succeeded(&run.id, result_json, summary_json)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to mark netbox projection run succeeded: {e}"),
            })?;
        info!(
            run_id = %run.id,
            architecture_id = %run.architecture_id,
            create = summary.create,
            update = summary.update,
            no_op = summary.no_op,
            conflict = summary.conflict,
            stale = summary.stale,
            "netbox projection run succeeded"
        );
        let (event_name, severity) = match run.mode {
            NetboxProjectionMode::Export => (EVENT_NETBOX_EXPORT_SUCCEEDED, EventSeverity::Info),
            NetboxProjectionMode::DryRun => (EVENT_NETBOX_DRY_RUN, EventSeverity::Info),
        };
        self.emit_event(
            run,
            event_name,
            severity,
            &event_details(run, &serde_json::to_value(summary).unwrap_or_default()),
        )
        .await;
        Ok(())
    }

    /// Terminal `running → failed` + failure event. `message` must
    /// already be redacted by the caller.
    async fn fail_run(&self, run: &NetboxProjectionRun, message: &str) -> Result<(), ChvError> {
        self.run_repo
            .mark_failed(&run.id, Some(message.to_string()))
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to mark netbox projection run failed: {e}"),
            })?;
        warn!(
            run_id = %run.id,
            architecture_id = %run.architecture_id,
            error = %message,
            "netbox projection run failed"
        );
        let (event_name, severity) = match run.mode {
            NetboxProjectionMode::Export => (EVENT_NETBOX_EXPORT_FAILED, EventSeverity::Error),
            // A dry run that cannot reach NetBox reports under the
            // dry-run event with error severity — it never wrote.
            NetboxProjectionMode::DryRun => (EVENT_NETBOX_DRY_RUN, EventSeverity::Error),
        };
        self.emit_event(
            run,
            event_name,
            severity,
            &serde_json::json!({
                "run_id": run.id.to_string(),
                "architecture_id": run.architecture_id.to_string(),
                "error": message,
            }),
        )
        .await;
        Ok(())
    }

    /// Append the audit event. Best-effort: an event-store failure is
    /// logged and swallowed — the run's terminal state is the source of
    /// truth, and a broken events table must not fail the run twice.
    async fn emit_event(
        &self,
        run: &NetboxProjectionRun,
        event_name: &str,
        severity: EventSeverity,
        details: &serde_json::Value,
    ) {
        let input = EventAppendInput {
            occurred_unix_ms: chrono::Utc::now().timestamp_millis(),
            event_type: EventType::Audit,
            severity,
            resource_kind: None,
            resource_id: None,
            node_id: None,
            operation_id: None,
            actor_id: None,
            requested_by: run.requested_by.clone(),
            correlation_id: Some(run.id.to_string()),
            message: event_name.to_string(),
            details: Some(details.to_string()),
        };
        if let Err(e) = self.event_repo.append(&input).await {
            warn!(
                run_id = %run.id,
                error = %e,
                "failed to append netbox projection audit event"
            );
        }
    }
}

/// Structured, BTree-ordered event details (secret-free by
/// construction: summary counts only).
fn event_details(run: &NetboxProjectionRun, summary: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "run_id": run.id.to_string(),
        "architecture_id": run.architecture_id.to_string(),
        "mode": run.mode.as_str(),
        "trigger": run.trigger_kind.as_str(),
        "summary": summary,
    })
}

/// Belt-and-braces redaction: scrub any occurrence of the plaintext
/// token from a message before it is persisted or logged. Error
/// displays are token-free by construction; this catches regressions.
fn redact(message: &str, token: &str) -> String {
    if token.is_empty() {
        message.to_string()
    } else {
        message.replace(token, "<redacted>")
    }
}
