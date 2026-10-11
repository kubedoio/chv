//! The export worker: a cadence pass over the bounded native
//! monitoring store that renders and pushes the configured metric
//! allowlist over remote-write v1.
//!
//! Degrade, never block (ADR-027 decision 11): the worker is its own
//! task; a destination outage fails the pass at the FIRST failed push
//! (one request per pass against a struggling receiver, never one per
//! target), retries with the notification dispatcher's capped
//! exponential backoff, and clamps its read window to
//! `max_backfill` — samples older than the window are skipped with a
//! time-range warning (monitoring.db is disposable telemetry; the
//! exact count of skipped samples is deliberately NOT reported
//! because counting them would require reading the data being
//! discarded). Ingest, queries, alerting and VM lifecycle never wait
//! on any of this.
//!
//! Lag accounting: the cursor is the newest exported timestamp per
//! (target kind, id); targets without data advance to `now` on every
//! pass, so `now - min(cursor)` is the export's true backlog. Beyond
//! `lag_degraded` (or after a failed pass) the worker warns and sets
//! `chv_export_degraded` — visible on the admin `/metrics` alongside
//! `chv_export_lag_seconds` and `chv_export_samples_total`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chv_monitoring_core::model::{Source, TargetKind};
use chv_monitoring_store::{MonitoringStore, Resolution};
use tokio::sync::watch;

use super::remote_write::{
    render_push, ExportPoint, ExportPushError, ExportSeries, RemoteWriteBody,
};
use crate::notification_dispatcher::backoff_delay;

/// Push body limit per target per pass: the store's own
/// `MAX_POINTS_PER_SERIES`.
const MAX_POINTS_PER_SERIES: usize = 1000;
/// Metric ids per `query_history` call: the store's own
/// `MAX_METRIC_IDS_PER_QUERY`. The config allows up to 64 metric ids
/// — the worker batches reads in chunks of this size and merges the
/// series, so a large allowlist exports instead of being rejected by
/// the store's query cap (pinned by test).
const MAX_METRIC_IDS_PER_CALL: usize = chv_monitoring_store::query::MAX_METRIC_IDS_PER_QUERY;
/// How many targets per kind one pass will walk (the store's
/// `list_targets` LIMIT — ordered by target id). Targets beyond the
/// first `MAX_TARGETS_PER_KIND` of a kind are NOT walked by v1: the
/// same first page returns every pass, so alphabetical tail targets
/// would never export. The pass warns loudly at the cap (pinned by
/// the runbook's scale boundary) instead of starving silently.
const MAX_TARGETS_PER_KIND: usize = 1000;

/// Metric names emitted on the admin `/metrics` surface (preserved
/// instrumentation — additive, never renamed). `pub(crate)` for the
/// preservation pin in the test suite.
pub(crate) const METRIC_EXPORT_LAG_SECONDS: &str = "chv_export_lag_seconds";
pub(crate) const METRIC_EXPORT_DEGRADED: &str = "chv_export_degraded";
pub(crate) const METRIC_EXPORT_SAMPLES_TOTAL: &str = "chv_export_samples_total";

/// Seam for tests (the dispatcher's `NotificationTransport` pattern):
/// one authenticated remote-write POST of a rendered body.
#[async_trait::async_trait]
pub trait ExportTransport: Send + Sync {
    async fn push(&self, body: &RemoteWriteBody) -> Result<(), ExportPushError>;
}

/// Production transport: the dispatcher's hardened reqwest client
/// (HTTPS-only, no redirects, bounded timeouts, optional operator
/// CA) plus the remote-write v1 headers and optional bearer token.
pub struct ReqwestExportTransport {
    client: reqwest::Client,
    url: String,
    bearer_token: Option<String>,
}

impl ReqwestExportTransport {
    pub fn new(
        url: &str,
        bearer_token: Option<String>,
        ca_pem: Option<&str>,
    ) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5));
        if let Some(pem) = ca_pem {
            let certificate = reqwest::tls::Certificate::from_pem(pem.as_bytes()).map_err(|e| {
                format!("export.ca_cert_path is not a valid PEM certificate bundle: {e}")
            })?;
            builder = builder.add_root_certificate(certificate);
        }
        let client = builder
            .build()
            .map_err(|e| format!("failed to build export client: {e}"))?;
        Ok(Self {
            client,
            url: url.to_string(),
            bearer_token,
        })
    }
}

#[async_trait::async_trait]
impl ExportTransport for ReqwestExportTransport {
    async fn push(&self, body: &RemoteWriteBody) -> Result<(), ExportPushError> {
        let mut request = self
            .client
            .post(&self.url)
            .header("content-type", super::remote_write::CONTENT_TYPE)
            .header("content-encoding", super::remote_write::CONTENT_ENCODING)
            .header(
                "x-prometheus-remote-write-version",
                super::remote_write::REMOTE_WRITE_VERSION,
            )
            .body(body.bytes.clone());
        if let Some(token) = &self.bearer_token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(|e| {
            // reqwest's Display embeds the URL (and a URL can carry
            // credentials in its path); classify without it.
            ExportPushError(redact_transport_error(&e))
        })?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            // Every failure is retried with the pass-level backoff;
            // permanent 4xx (bad token, wrong path) surface their
            // status class here for the operator while the bounded
            // backfill caps data loss.
            Err(ExportPushError(format!(
                "destination responded {class}xx",
                class = status.as_u16() / 100
            )))
        }
    }
}

/// Classify a reqwest error WITHOUT the URL (same contract as the
/// dispatcher's `redact_transport_error`). `pub(crate)` for the
/// redaction test.
pub(crate) fn redact_transport_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "network timeout".to_string()
    } else if error.is_connect() {
        "connect failed".to_string()
    } else if error.is_request() {
        "request failed".to_string()
    } else if error.is_body() || error.is_decode() {
        "invalid response body".to_string()
    } else {
        "network error".to_string()
    }
}

/// Flattened from `[monitoring.export]` (the dispatcher's settings
/// pattern); the destination itself lives in the transport.
#[derive(Clone, Debug)]
pub struct ExporterSettings {
    pub interval: Duration,
    pub max_backfill_ms: u64,
    pub lag_degraded_ms: u64,
}

/// The outcome of one pass — drives the degraded signal and the
/// backoff schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassOutcome {
    /// Every target walked; all pushes accepted.
    Complete,
    /// A push failed (destination-level): the pass stopped at the
    /// first failure; cursors for the remaining targets stay put.
    DestinationFailed,
    /// The monitoring store could not be read (store-level, NOT the
    /// destination): the pass stopped, nothing was pushed, and the
    /// next pass retries at the normal cadence — no destination
    /// backoff and no destination-degraded marking for a local
    /// store problem.
    StoreFailed,
}

/// The export worker. Constructed fail-loud: an invalid allowlist,
/// target-kind, or source filter is an operator error at boot, never
/// a silently-empty export.
pub struct MonitoringExporter {
    store: Arc<MonitoringStore>,
    transport: Arc<dyn ExportTransport>,
    settings: ExporterSettings,
    /// Resolved allowlist (registry-validated at construction).
    metric_ids: Vec<String>,
    target_kinds: Vec<TargetKind>,
    sources: Option<Vec<Source>>,
    /// Newest exported timestamp per (kind, target_id), epoch ms.
    cursor: HashMap<(TargetKind, String), u64>,
    exported_samples: u64,
    consecutive_failures: u32,
    degraded: bool,
    /// Test/diagnostics view of the last pass.
    last_error: Option<String>,
}

impl MonitoringExporter {
    /// Build and validate. Fails loud on anything outside the
    /// registry vocabulary (the `[monitoring.export]` twin of the
    /// alert-rule validation contract).
    pub fn new(
        config: &chv_config::MonitoringExportConfig,
        store: Arc<MonitoringStore>,
        transport: Arc<dyn ExportTransport>,
    ) -> Result<Self, String> {
        let mut metric_ids = Vec::with_capacity(config.metric_ids.len());
        for id in &config.metric_ids {
            let def = chv_monitoring_core::registry::lookup(id).ok_or_else(|| {
                format!("monitoring.export.metric_ids entry {id:?} is not in the v1 registry")
            })?;
            // A source filter that excludes every allowed source of a
            // metric is a misconfiguration: that metric would export
            // nothing forever.
            if !config.sources.is_empty() {
                let parsed: Vec<Source> = config
                    .sources
                    .iter()
                    .map(|s| {
                        s.parse()
                            .map_err(|e| format!("monitoring.export.sources entry {s:?}: {e}"))
                    })
                    .collect::<Result<_, _>>()?;
                if !def.allowed_sources.iter().any(|a| parsed.contains(a)) {
                    return Err(format!(
                        "monitoring.export: the sources filter excludes every allowed \
                         source of metric {id:?}"
                    ));
                }
            }
            metric_ids.push(id.clone());
        }
        // Registry lookups succeeded, so every id is in the closed
        // vocabulary; parse the filters with the same discipline.
        let mut target_kinds = Vec::new();
        for kind in &config.target_kinds {
            let parsed: TargetKind = kind
                .parse()
                .map_err(|e| format!("monitoring.export.target_kinds entry {kind:?}: {e}"))?;
            if !matches!(parsed, TargetKind::Node | TargetKind::Vm) {
                return Err(format!(
                    "monitoring.export.target_kinds entry {kind:?}: only node and vm carry \
                     monitoring series"
                ));
            }
            if !target_kinds.contains(&parsed) {
                target_kinds.push(parsed);
            }
        }
        if target_kinds.is_empty() {
            target_kinds = vec![TargetKind::Node, TargetKind::Vm];
        }
        let sources = if config.sources.is_empty() {
            None
        } else {
            let mut parsed = Vec::new();
            for s in &config.sources {
                let source: Source = s
                    .parse()
                    .map_err(|e| format!("monitoring.export.sources entry {s:?}: {e}"))?;
                if !parsed.contains(&source) {
                    parsed.push(source);
                }
            }
            Some(parsed)
        };

        Ok(Self {
            store,
            transport,
            settings: ExporterSettings {
                interval: Duration::from_secs(config.interval_secs),
                max_backfill_ms: config.max_backfill_secs * 1000,
                lag_degraded_ms: config.lag_degraded_secs * 1000,
            },
            metric_ids,
            target_kinds,
            sources,
            cursor: HashMap::new(),
            exported_samples: 0,
            consecutive_failures: 0,
            degraded: false,
            last_error: None,
        })
    }

    /// Run until shutdown. One pass per interval (or per backoff
    /// after a destination failure, with the dispatcher's ±20%
    /// jitter so N managers against one recovering receiver do not
    /// retry in lockstep); the pass itself is bounded by the store's
    /// own query caps. Store-level failures retry at the normal
    /// cadence — backing off for a local problem would only delay
    /// recovery.
    pub async fn run(mut self, mut shutdown: watch::Receiver<()>) {
        loop {
            let delay = if self.consecutive_failures == 0 {
                self.settings.interval
            } else {
                let backoff = backoff_delay(self.consecutive_failures);
                let jitter = crate::notification_dispatcher::jitter_ms(backoff);
                if jitter >= 0 {
                    backoff + Duration::from_millis(jitter as u64)
                } else {
                    backoff.saturating_sub(Duration::from_millis(jitter.unsigned_abs()))
                }
            };
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = shutdown.changed() => {
                    tracing::info!(
                        samples_exported = self.exported_samples,
                        "monitoring export worker stopped"
                    );
                    return;
                }
            }
            self.run_pass(now_ms()).await;
        }
    }

    /// One bounded pass. Public visibility is crate-only in effect
    /// (tests drive it directly); production reaches it through
    /// [`MonitoringExporter::run`].
    pub async fn run_pass(&mut self, now: u64) -> PassOutcome {
        let mut outcome = PassOutcome::Complete;
        let mut min_cursor: Option<u64> = None;
        let mut new_cursors = 0usize;
        let backfill_start = now.saturating_sub(self.settings.max_backfill_ms);

        for kind in &self.target_kinds {
            let targets = match self.store.list_targets(kind, MAX_TARGETS_PER_KIND).await {
                Ok(targets) => {
                    if targets.len() >= MAX_TARGETS_PER_KIND {
                        // The store's list_targets caps at the LIMIT
                        // with no pagination: targets beyond the
                        // first page of a kind are never walked, and
                        // the walk is the same first page every pass.
                        // Loud, not silent starvation — the operator
                        // must know the export is partial.
                        tracing::warn!(
                            target_kind = kind.as_str(),
                            limit = MAX_TARGETS_PER_KIND,
                            "monitoring export target walk hit the list_targets cap; \
                             targets beyond the first {MAX_TARGETS_PER_KIND} of this kind \
                             are NOT exported in v1"
                        );
                    }
                    targets
                }
                Err(error) => {
                    // A store that cannot be queried is a typed
                    // degradation, never a panic and never a block on
                    // anyone else — and never a destination signal:
                    // no backoff, no destination-degraded marking.
                    self.last_error = Some(format!("list_targets failed: {error:?}"));
                    tracing::warn!(
                        error = ?error,
                        "monitoring export store read failed; retrying at the normal cadence"
                    );
                    outcome = PassOutcome::StoreFailed;
                    break;
                }
            };
            for target_id in targets {
                let key = (*kind, target_id.clone());
                let cursor = self
                    .cursor
                    .entry(key)
                    // Restart semantics: bounded catch-up, never an
                    // unbounded replay of disposable telemetry. A
                    // first-seen target (fresh process, or a newly
                    // discovered target) starts here too — the same
                    // bound applies, reported once per pass below
                    // rather than as a per-target skip warning (the
                    // clamp warning below fires for a cursor that
                    // has lagged in-session, e.g. after a long
                    // destination outage).
                    .or_insert_with(|| {
                        new_cursors += 1;
                        backfill_start
                    });

                // Clamp to the backfill window; the skipped backlog
                // is reported as a time range (counting the skipped
                // samples would mean reading the data being thrown
                // away).
                if *cursor < backfill_start {
                    tracing::warn!(
                        target_kind = kind.as_str(),
                        target_id = %target_id,
                        skipped_ms = backfill_start - *cursor,
                        "monitoring export backlog beyond max_backfill_seconds skipped"
                    );
                    *cursor = backfill_start;
                }
                if *cursor >= now {
                    min_cursor = Some(min_cursor.unwrap_or(*cursor).min(*cursor));
                    continue;
                }

                // Bounded reads: the store caps metric ids per call
                // (the config allows up to 64), so the allowlist is
                // queried in chunks and the series merged — one
                // logical window per target.
                let mut series = Vec::new();
                let mut query_failed = false;
                for chunk in self.metric_ids.chunks(MAX_METRIC_IDS_PER_CALL) {
                    match self
                        .store
                        .query_history(
                            kind,
                            &target_id,
                            chunk,
                            self.sources.as_deref(),
                            *cursor,
                            now,
                            MAX_POINTS_PER_SERIES,
                            Resolution::Auto,
                        )
                        .await
                    {
                        Ok(mut chunk_series) => series.append(&mut chunk_series),
                        Err(error) => {
                            // One target's query rejection never fails
                            // the pass (the store's own caps are the
                            // authority) and never advances the
                            // cursor: the window is retried next pass,
                            // bounded by the backfill clamp.
                            tracing::warn!(
                                target_kind = kind.as_str(),
                                target_id = %target_id,
                                error = ?error,
                                "monitoring export query rejected"
                            );
                            query_failed = true;
                            break;
                        }
                    }
                }
                if query_failed {
                    continue;
                }

                let export_series: Vec<ExportSeries> = series
                    .iter()
                    .map(|s| ExportSeries {
                        metric_id: s.metric_id.clone(),
                        target_kind: kind.as_str().to_string(),
                        target_id: target_id.clone(),
                        source: s.source.as_str().to_string(),
                        dimensions: s.dimensions.clone(),
                        points: s
                            .points
                            .iter()
                            .filter_map(|p| {
                                let value = p.value.or(p.integer_value.map(|v| v as f64))?;
                                Some(ExportPoint {
                                    timestamp_ms: p.timestamp_ms,
                                    value,
                                })
                            })
                            .collect(),
                    })
                    .collect();

                if export_series.is_empty() {
                    *cursor = now;
                    min_cursor = Some(min_cursor.unwrap_or(*cursor).min(*cursor));
                    continue;
                }
                let body = match render_push(&export_series) {
                    Ok(body) => body,
                    Err(error) => {
                        // Unreachable in practice (rendering a
                        // self-encoded in-memory buffer); a render
                        // failure is our own bug, never a reason to
                        // panic — skip the target, hold its cursor,
                        // keep walking.
                        self.last_error = Some(error.0.clone());
                        tracing::warn!(
                            error = %error.0,
                            "monitoring export render failed; target window held for retry"
                        );
                        continue;
                    }
                };
                if body.skipped_unknown_metric > 0 {
                    // Unreachable through construction (the allowlist
                    // is registry-validated); defensive visibility.
                    tracing::warn!(
                        target_id = %target_id,
                        skipped = body.skipped_unknown_metric,
                        "monitoring export skipped series outside the registry"
                    );
                }
                if body.sample_count == 0 {
                    *cursor = now;
                    min_cursor = Some(min_cursor.unwrap_or(*cursor).min(*cursor));
                    continue;
                }

                match self.transport.push(&body).await {
                    Ok(()) => {
                        // The whole [from, now) window is exported:
                        // park the cursor at `now` (not newest+1) so
                        // the lag signal measures EXPORT backlog, not
                        // data staleness of quiet targets.
                        *cursor = now;
                        self.exported_samples += body.sample_count as u64;
                        metrics::counter!(
                            METRIC_EXPORT_SAMPLES_TOTAL,
                            "outcome" => "exported"
                        )
                        .increment(body.sample_count as u64);
                    }
                    Err(error) => {
                        // Destination-level failure: stop the pass
                        // with ONE request against the struggling
                        // receiver; the cursor stays put so the next
                        // pass retries exactly this window.
                        self.last_error = Some(error.0.clone());
                        tracing::warn!(
                            error = %error.0,
                            "monitoring export push failed; will retry with backoff"
                        );
                        outcome = PassOutcome::DestinationFailed;
                        break;
                    }
                }
                min_cursor = Some(min_cursor.unwrap_or(*cursor).min(*cursor));
            }
            if outcome == PassOutcome::DestinationFailed {
                break;
            }
        }

        // First-seen targets (fresh process, newly discovered):
        // their pre-window history is not exported — the same
        // bounded-backfill rule as everything else, reported once
        // per pass instead of per target.
        if new_cursors > 0 {
            tracing::info!(
                targets = new_cursors,
                max_backfill_ms = self.settings.max_backfill_ms,
                "monitoring export: new targets start at the backfill window; \
                 older history is not exported"
            );
        }

        // Degraded signal: backlog beyond the threshold, or a failed
        // destination push. A store read failure is NOT a destination
        // signal — it leaves the degraded state untouched and retries
        // at the normal cadence. Never blocks anything; visible as
        // the gauge + warn.
        let lag_ms = now.saturating_sub(min_cursor.unwrap_or(now));
        let degraded = match outcome {
            PassOutcome::Complete => lag_ms > self.settings.lag_degraded_ms,
            PassOutcome::DestinationFailed => true,
            PassOutcome::StoreFailed => self.degraded,
        };
        if degraded && !self.degraded {
            tracing::warn!(
                lag_ms,
                outcome = ?outcome,
                "monitoring export DEGRADED (destination unreachable or backlog beyond \
                 lag_degraded_seconds); local monitoring is unaffected"
            );
        } else if !degraded && self.degraded {
            tracing::info!("monitoring export recovered");
        }
        self.degraded = degraded;
        self.consecutive_failures = match outcome {
            PassOutcome::DestinationFailed => self.consecutive_failures + 1,
            PassOutcome::Complete => 0,
            // A store read failure neither extends nor resets the
            // destination backoff ladder.
            PassOutcome::StoreFailed => self.consecutive_failures,
        };
        metrics::gauge!(METRIC_EXPORT_LAG_SECONDS).set(lag_ms as f64 / 1000.0);
        metrics::gauge!(METRIC_EXPORT_DEGRADED).set(if degraded { 1.0 } else { 0.0 });
        outcome
    }

    /// Diagnostics (tests, runbook introspection).
    pub fn exported_samples(&self) -> u64 {
        self.exported_samples
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn cursor(&self, kind: &TargetKind, target_id: &str) -> Option<u64> {
        self.cursor.get(&(*kind, target_id.to_string())).copied()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
