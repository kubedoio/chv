//! Agent-side check engines (prompt 04, G4 part 1): systemd service
//! checks with bounded discovery, declarative local HTTP/TCP checks,
//! and the opt-in plugin sandbox.
//!
//! This module owns the shared [`CheckOutcome`] contract every engine
//! produces; the engines live alongside (systemd + local checks) or in
//! their own module (`plugins`). The agent — never the check — is the
//! authoritative timekeeper and the only writer of `check.*` samples:
//! engines hand over the wire record plus a measured runtime, and the
//! envelope assembly maps outcomes to the `checks` array and the
//! `check.status` / `check.duration_seconds` samples (dimensioned by
//! `check_id`) on the 60-second cadence.
//!
//! Check semantics follow
//! `docs/specs/contracts/chv-monitor-agent-security-plugins-v1.md`
//! (status vocabulary, honest failure handling, local-only endpoints,
//! DNS rebinding defense) and
//! `docs/specs/component/chv-monitor-agent-spec.md` (stable `check_id`,
//! bounded discovery, `unknown` ≠ healthy). Numeric outputs follow
//! `docs/specs/contracts/chv-monitoring-metrics-v1.md`: `unknown` and
//! unobserved states emit an ABSENT sample, never a guessed zero.
//! Every engine failure is a check status — this is a VM-guest agent,
//! a poisoned systemctl or a dead endpoint must never crash it.

use crate::config::{ChecksConfig, HttpCheckConfig};
use crate::wire::{CheckJson, SCHEMA_VERSION};
use chv_monitor_collectors::CollectedSample;
use chv_monitoring_core::model::CheckStatus;
use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::Command;

/// The result of one executed check: the wire record for the
/// envelope's `checks` array plus the engine-measured runtime.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    /// The check record (status as the contract's string vocabulary;
    /// engines construct it from validated `CheckStatus` values).
    pub check: CheckJson,
    /// Measured wall-clock runtime in milliseconds. Emitted as the
    /// `check.duration_seconds` sample by the agent; a plugin's own
    /// timing claim is never trusted.
    pub duration_ms: u64,
}

impl CheckOutcome {
    /// The typed status. Engines only construct records from parsed
    /// `CheckStatus` values, so this never actually falls back — but
    /// a malformed record degrades to `unknown` (never healthy)
    /// rather than panicking.
    pub fn status(&self) -> chv_monitoring_core::model::CheckStatus {
        chv_monitoring_core::model::CheckStatus::parse(&self.check.status)
            .unwrap_or(chv_monitoring_core::model::CheckStatus::Unknown)
    }
}

/// Assemble one wire check record from a typed status. Engines never
/// build `CheckJson` by hand with raw strings; the status always
/// comes from the typed enum, so the manager's vocabulary check can
/// only ever see valid values.
fn record(
    check_id: String,
    service_key: Option<String>,
    status: CheckStatus,
    summary: String,
    now_ms: i64,
) -> CheckJson {
    CheckJson {
        schema_version: SCHEMA_VERSION,
        check_id,
        service_key,
        status: status.as_str().to_string(),
        summary: Some(summary),
        observed_at_ms: now_ms,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

// ---------------------------------------------------------------------------
// systemd service checks
// ---------------------------------------------------------------------------

/// `vm.guest.service.up` (metrics contract v1: "Configured/discovered
/// service known running"), dimensioned by `service_key` — the
/// normalized systemd unit name.
const SERVICE_UP_METRIC: &str = "vm.guest.service.up";
/// Hard per-unit `systemctl show` ceiling (component spec execution
/// limits: 5-second default).
const UNIT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
/// Hard `systemctl list-units` discovery ceiling.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
/// At most this many discovered units per collection (component spec:
/// discovery has a bounded item count).
const MAX_DISCOVERED_UNITS: usize = 32;

/// The systemd engine's output: the check records plus the
/// `vm.guest.service.up` samples they justify. Samples are pushed in
/// outcome order — one per outcome that actually observed a unit
/// state; `not installed` / `not queried` units contribute none
/// (honest absence, metrics contract quality rules).
#[derive(Debug, Clone)]
pub struct ServiceCheckResult {
    /// One outcome per configured unit (plus discovered units when
    /// enabled), `check_id = service:<unit>`.
    pub outcomes: Vec<CheckOutcome>,
    /// `vm.guest.service.up` samples for the outcomes above,
    /// dimensioned by `service_key`; the agent maps them onto the
    /// envelope's samples array.
    pub service_up_samples: Vec<CollectedSample>,
}

/// Systemd service check engine. Units are queried with a direct
/// `systemctl` exec — no shell, no user-controlled interpolation
/// (security/plugins contract v1) — sequentially and with a hard
/// per-unit timeout, so a wedged systemctl can never stall the agent
/// beyond its bounds.
pub struct ServiceChecks {
    /// Injectable for tests; production uses `/usr/bin/systemctl`.
    /// A missing binary degrades every unit to `unknown` /
    /// "not queried" — never a crash.
    systemctl: PathBuf,
}

impl Default for ServiceChecks {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceChecks {
    /// Production engine against the distribution's systemctl.
    pub fn new() -> Self {
        Self {
            systemctl: PathBuf::from("/usr/bin/systemctl"),
        }
    }

    /// Engine against an explicit systemctl path (tests inject a
    /// fake script here).
    pub fn with_systemctl(path: PathBuf) -> Self {
        Self { systemctl: path }
    }

    /// Check every configured unit and, when `discover` is set, run
    /// one bounded discovery pass. Units are queried sequentially
    /// (bounded, quiet — the worst case is 32 units × 5 s of local
    /// exec, no concurrency, no log spam). `now_ms` is the agent's
    /// authoritative observation time; engines never read the clock
    /// themselves.
    pub async fn run(
        &self,
        configured: &[String],
        discover: bool,
        now_ms: i64,
    ) -> ServiceCheckResult {
        let mut outcomes = Vec::with_capacity(configured.len());
        let mut service_up_samples = Vec::with_capacity(configured.len());
        // Config already bounds and dedupes the unit list; the seen
        // set re-asserts both at runtime (defense in depth) and
        // carries over to discovery dedup below.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for unit in configured {
            if !seen.insert(unit.clone()) {
                tracing::warn!(unit = %unit, "duplicate configured unit skipped");
                continue;
            }
            let (outcome, sample) = self.check_unit(unit, now_ms).await;
            outcomes.push(outcome);
            if let Some(sample) = sample {
                service_up_samples.push(sample);
            }
        }
        if discover {
            let (units, duration_ms) = self.discover().await;
            let mut discovered = 0usize;
            for unit in units {
                if discovered >= MAX_DISCOVERED_UNITS {
                    break;
                }
                // Dedupe against configured units and within the
                // discovery itself: one check record per unit.
                if !seen.insert(unit.clone()) {
                    continue;
                }
                discovered += 1;
                // Discovered units were observed running by the
                // `list-units` query itself; each outcome carries
                // that query's measured runtime.
                let (outcome, sample) = unit_result(
                    &unit,
                    CheckStatus::Ok,
                    "discovered (running)",
                    Some(1),
                    now_ms,
                    duration_ms,
                );
                outcomes.push(outcome);
                // Discovered units always observed running: up = 1.
                if let Some(sample) = sample {
                    service_up_samples.push(sample);
                }
            }
        }
        ServiceCheckResult {
            outcomes,
            service_up_samples,
        }
    }

    /// Query one unit via `systemctl show` and map its states to a
    /// check outcome. Any failure — invalid name, spawn error,
    /// timeout, non-zero exit, unparsable output — is an `unknown`
    /// record with no `service.up` sample: the unit was not observed,
    /// and `unknown` must never be laundered into `ok` or `critical`.
    async fn check_unit(&self, unit: &str, now_ms: i64) -> (CheckOutcome, Option<CollectedSample>) {
        let started = Instant::now();
        // Config already validated the charset; re-assert it before
        // handing anything to an exec (a reload race or hand-edited
        // file must never reach the argv).
        if !valid_unit_name(unit) {
            tracing::warn!(unit = %unit, "refusing to query a unit with an invalid name");
            return unit_result(
                unit,
                CheckStatus::Unknown,
                "not queried",
                None,
                now_ms,
                elapsed_ms(started),
            );
        }
        let output = run_systemctl(
            &self.systemctl,
            &["show", unit, "--property=ActiveState,LoadState", "--value"],
            UNIT_QUERY_TIMEOUT,
        )
        .await;
        let duration_ms = elapsed_ms(started);
        let Some(stdout) = output else {
            // Spawn failure, timeout or non-zero exit: honest
            // absence — the unit is neither up nor down.
            return unit_result(
                unit,
                CheckStatus::Unknown,
                "not queried",
                None,
                now_ms,
                duration_ms,
            );
        };
        let Some((load_state, active_state)) = parse_unit_state(&stdout) else {
            return unit_result(
                unit,
                CheckStatus::Unknown,
                "unexpected unit state",
                None,
                now_ms,
                duration_ms,
            );
        };
        let (status, summary, up) = unit_state_status(&load_state, &active_state);
        unit_result(unit, status, summary, up, now_ms, duration_ms)
    }

    /// Bounded discovery of running services. Returns the candidate
    /// unit names (charset-filtered, deduped, sorted for determinism)
    /// plus the query's runtime. Any query or parse failure degrades
    /// to an empty discovery — never a crash, never a false `ok`.
    async fn discover(&self) -> (Vec<String>, u64) {
        let started = Instant::now();
        let output = run_systemctl(
            &self.systemctl,
            &[
                "list-units",
                "--type=service",
                "--state=running",
                "--no-legend",
                "--plain",
            ],
            DISCOVERY_TIMEOUT,
        )
        .await;
        let duration_ms = elapsed_ms(started);
        let Some(text) = output else {
            return (Vec::new(), duration_ms);
        };
        // Column 1 of `--no-legend --plain` output is the unit name;
        // anything that does not survive the unit-name filter (empty
        // line, garbage, hostile label) is dropped.
        let mut units: Vec<String> = text
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|unit| valid_unit_name(unit))
            .map(str::to_owned)
            .collect();
        units.sort();
        units.dedup();
        (units, duration_ms)
    }
}

/// `ETXTBSY` (exec of a binary the kernel still counts a writer
/// for). A freshly-installed — or in tests, freshly-written —
/// executable can transiently fail to exec with this; retry briefly
/// instead of degrading a whole collection to "not queried".
const ETXTBSY: i32 = 26;
/// Exec attempts for the `ETXTBSY` window above (bounded, with a
/// short backoff well inside the per-query budget).
const EXEC_ATTEMPTS: usize = 3;
const EXEC_RETRY_DELAY: Duration = Duration::from_millis(25);

/// Exec systemctl directly (no shell) with a hard timeout, capturing
/// stdout. Any failure — spawn error, timeout, non-zero exit — is
/// `None`; callers degrade to a check status, never a panic.
async fn run_systemctl(systemctl: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(systemctl);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        // The child runs in its own process group: a timeout or
        // kill_on_drop signal can never reach the agent's own group,
        // and a descendant can never hold the agent hostage.
        .process_group(0)
        // A timed-out child must not outlive the attempt.
        .kill_on_drop(true);
    for attempt in 0..EXEC_ATTEMPTS {
        match tokio::time::timeout(timeout, cmd.output()).await {
            Ok(Ok(output)) if output.status.success() => {
                return Some(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            Ok(Ok(output)) => {
                tracing::warn!(code = ?output.status.code(), "systemctl exited non-zero");
                return None;
            }
            Ok(Err(e)) if e.raw_os_error() == Some(ETXTBSY) && attempt + 1 < EXEC_ATTEMPTS => {
                tracing::debug!(attempt, "systemctl executable busy; retrying");
                tokio::time::sleep(EXEC_RETRY_DELAY).await;
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "failed to exec systemctl");
                return None;
            }
            Err(_) => {
                tracing::warn!("systemctl query timed out");
                return None;
            }
        }
    }
    tracing::warn!("systemctl executable stayed busy; giving up");
    None
}

/// The config-time unit charset (`config.rs` validation), re-asserted
/// at runtime before any exec and applied to discovery output:
/// `[A-Za-z0-9._-@:]`, 1..=128 bytes.
fn valid_unit_name(unit: &str) -> bool {
    !unit.is_empty()
        && unit.len() <= 128
        && unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'@' | b':'))
}

/// systemctl's `LoadState` vocabulary.
const LOAD_STATES: [&str; 5] = ["loaded", "not-found", "error", "masked", "bad-setting"];
/// systemctl's `ActiveState` vocabulary; disjoint from `LoadState`.
const ACTIVE_STATES: [&str; 6] = [
    "active",
    "reloading",
    "inactive",
    "failed",
    "activating",
    "deactivating",
];

/// Parse `systemctl show --property=ActiveState,LoadState --value`
/// output into `(load_state, active_state)`.
///
/// The two value vocabularies are disjoint, so each line is assigned
/// by the value it carries — the parser does not depend on
/// systemctl's property ordering. It also tolerates labeled
/// `Key=value` output (an older systemctl without `--value` support).
/// Anything missing, duplicated or unassignable is a parse failure;
/// the caller turns that into `unknown`, never a guess.
fn parse_unit_state(output: &str) -> Option<(String, String)> {
    let mut load_state: Option<String> = None;
    let mut active_state: Option<String> = None;
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Labeled output ("ActiveState=active"): the key names the
        // property directly. An unknown property label is a parse
        // failure, not something to guess at.
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            match key.to_ascii_lowercase().as_str() {
                "loadstate" => {
                    if load_state.replace(value.to_owned()).is_some() {
                        return None;
                    }
                    continue;
                }
                "activestate" => {
                    if active_state.replace(value.to_owned()).is_some() {
                        return None;
                    }
                    continue;
                }
                _ => return None,
            }
        }
        // `--value` output: assign by vocabulary membership.
        if LOAD_STATES.contains(&line) {
            if load_state.replace(line.to_owned()).is_some() {
                return None;
            }
        } else if ACTIVE_STATES.contains(&line) {
            if active_state.replace(line.to_owned()).is_some() {
                return None;
            }
        } else {
            return None;
        }
    }
    Some((load_state?, active_state?))
}

/// Map parsed unit states to a check status, summary and
/// `vm.guest.service.up` value (metrics contract v1: a `state`
/// boolean is a measured 0/1 — an UNOBSERVED state is an absent
/// sample, never a zero). Distinct honest states:
///
/// - `not-found` → `unknown` "not installed" (absent sample);
/// - `active` → `ok`, up = 1;
/// - `activating`/`deactivating`/`reloading` → `warning`, up = 0;
/// - `failed` → `critical`, up = 0;
/// - `inactive` → `critical` "inactive (stopped)", up = 0;
/// - anything else → `unknown` "unexpected unit state" (absent
///   sample).
fn unit_state_status(
    load_state: &str,
    active_state: &str,
) -> (CheckStatus, &'static str, Option<u64>) {
    if load_state == "not-found" {
        return (CheckStatus::Unknown, "not installed", None);
    }
    match active_state {
        "active" => (CheckStatus::Ok, "active (running)", Some(1)),
        "activating" => (CheckStatus::Warning, "activating", Some(0)),
        "deactivating" => (CheckStatus::Warning, "deactivating", Some(0)),
        "reloading" => (CheckStatus::Warning, "reloading", Some(0)),
        "failed" => (CheckStatus::Critical, "failed", Some(0)),
        "inactive" => (CheckStatus::Critical, "inactive (stopped)", Some(0)),
        _ => (CheckStatus::Unknown, "unexpected unit state", None),
    }
}

/// One service check outcome plus its (possibly absent)
/// `vm.guest.service.up` sample. `check_id = service:<unit>`,
/// `service_key` = the unit name (dimension allowlist).
fn unit_result(
    unit: &str,
    status: CheckStatus,
    summary: &str,
    up: Option<u64>,
    now_ms: i64,
    duration_ms: u64,
) -> (CheckOutcome, Option<CollectedSample>) {
    let outcome = CheckOutcome {
        check: record(
            format!("service:{unit}"),
            Some(unit.to_string()),
            status,
            summary.to_string(),
            now_ms,
        ),
        duration_ms,
    };
    let sample = up.map(|value| {
        CollectedSample::integer_with_dimension(
            SERVICE_UP_METRIC,
            value,
            "service_key",
            unit.to_string(),
        )
    });
    (outcome, sample)
}

// ---------------------------------------------------------------------------
// declarative local HTTP/TCP checks
// ---------------------------------------------------------------------------

/// Hard per-attempt ceiling for every declarative local check
/// (component spec execution limits: 5-second default). The engine
/// measures the whole attempt and enforces this bound around it.
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// A local check target after name resolution, awaiting layer-2
/// verification (see [`CheckTarget::verified`]). These are LOCAL
/// checks by explicit opt-in — the agent never probes remote targets
/// with them (remote endpoints are root-approved plugin territory).
enum CheckTarget {
    /// A literal loopback IP from the configuration; no name
    /// resolution is involved.
    Literal(SocketAddr),
    /// A `localhost` resolution: the RAW addresses from the system
    /// resolver, which must still be verified loopback before
    /// anything is dialed.
    Named {
        host: String,
        addrs: Vec<SocketAddr>,
    },
}

impl CheckTarget {
    /// SSRF layer 2 of 2 (runtime; layer 1 is config-time validation
    /// in `config.rs`): every address must be loopback. A `localhost`
    /// resolution containing any non-loopback address — a DNS
    /// rebinding — refuses the whole target and NOTHING is dialed.
    /// Literal targets are re-checked here too (defense in depth:
    /// a reload race cannot smuggle a remote endpoint past this).
    fn verified(&self) -> Option<&[SocketAddr]> {
        match self {
            CheckTarget::Literal(addr) if addr.ip().is_loopback() => {
                Some(std::slice::from_ref(addr))
            }
            CheckTarget::Named { addrs, .. }
                if !addrs.is_empty() && addrs.iter().all(|a| a.ip().is_loopback()) =>
            {
                Some(addrs)
            }
            _ => None,
        }
    }
}

/// Declarative local HTTP/TCP check engine. Each configured check is
/// re-validated at runtime (a config reload race cannot smuggle a
/// remote endpoint past the engine), resolved, verified loopback, and
/// only then dialed — with a hard 5-second ceiling per attempt. Every
/// failure is a check status; summaries are class-level only (a raw
/// connection error can embed the URL).
pub struct LocalChecks {
    /// Client for literal loopback-IP targets (no DNS to pin).
    /// `None` only if the TLS backend failed to initialize — HTTP
    /// checks then report `unknown` ("check engine error"), never a
    /// panic.
    client: Option<reqwest::Client>,
    /// Hard per-attempt ceiling (HTTP and TCP alike).
    timeout: Duration,
}

impl Default for LocalChecks {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalChecks {
    /// Production engine with the default 5-second per-check ceiling.
    pub fn new() -> Self {
        let timeout = CHECK_TIMEOUT;
        // A failed build (TLS backend init) must not panic the agent.
        let client = Self::base_builder(timeout)
            .build()
            .map_err(|e| {
                tracing::error!(error = %e, "failed to initialize the local check HTTP client");
                e
            })
            .ok();
        Self { client, timeout }
    }

    /// Run every configured declarative check, sequentially (at most
    /// 16 by config bound, each capped at `timeout`). `now_ms` is the
    /// agent's authoritative observation time.
    pub async fn run(&self, checks: &ChecksConfig, now_ms: i64) -> Vec<CheckOutcome> {
        let mut outcomes = Vec::with_capacity(checks.http.len() + checks.tcp.len());
        for check in &checks.http {
            let target = Self::resolve_for_url(&check.url).await;
            outcomes.push(self.http_check_with(check, target, now_ms).await);
        }
        for check in &checks.tcp {
            let target = Self::resolve_target(&check.host, check.port).await;
            outcomes.push(self.tcp_check_with(&check.label, target, now_ms).await);
        }
        outcomes
    }

    /// The engine's fixed client options: redirects are NEVER
    /// followed (a redirect must never lead to another host) and
    /// every request carries the per-check timeout as an inner bound
    /// (the engine's own timeout wrapper remains authoritative).
    fn base_builder(timeout: Duration) -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
    }

    /// A client with the verified addresses pinned for a name-resolved
    /// target: the client never consults the resolver for that name
    /// again, so post-verification rebinding cannot redirect the
    /// connection. Build failure is `None` — a check-level `unknown`,
    /// never a panic.
    fn pinned_client(
        host: &str,
        addrs: &[SocketAddr],
        timeout: Duration,
    ) -> Option<reqwest::Client> {
        Self::base_builder(timeout)
            .resolve_to_addrs(host, addrs)
            .build()
            .map_err(|e| {
                tracing::warn!(error = %e, "failed to build a pinned local check client");
                e
            })
            .ok()
    }

    /// Resolve an HTTP check's URL to a dial target. Layer 2 of the
    /// SSRF defense: the URL is re-validated (config already did
    /// this at load time — a reload race cannot smuggle a remote
    /// endpoint past the runtime), then the host is resolved.
    async fn resolve_for_url(url: &str) -> Option<CheckTarget> {
        if crate::config::validate_local_http_url(url).is_err() {
            return None;
        }
        let parsed = reqwest::Url::parse(url).ok()?;
        let port = parsed.port_or_known_default()?;
        Self::resolve_target(parsed.host_str()?, port).await
    }

    /// Resolve a local check host. Only `localhost` is name-resolved
    /// (config layer 1 allows nothing else); the RAW resolution is
    /// returned for layer-2 verification before any dial. Literal
    /// hosts must be loopback IPs — anything else (a smuggled remote
    /// or metadata address) is refused without a lookup.
    async fn resolve_target(host: &str, port: u16) -> Option<CheckTarget> {
        let host = host.trim();
        if host.eq_ignore_ascii_case("localhost") {
            let addrs: Vec<SocketAddr> = match tokio::net::lookup_host((host, port)).await {
                Ok(addrs) => addrs.collect(),
                Err(e) => {
                    tracing::warn!(error = %e, host = %host, "local check host resolution failed");
                    return None;
                }
            };
            return Some(CheckTarget::Named {
                host: "localhost".to_string(),
                addrs,
            });
        }
        // Strip optional IPv6 brackets (config allows "[::1]").
        let literal = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host);
        match literal.parse::<IpAddr>() {
            Ok(ip) if ip.is_loopback() => Some(CheckTarget::Literal(SocketAddr::new(ip, port))),
            _ => None,
        }
    }

    /// One HTTP check against an already-resolved target (the public
    /// path resolves via [`Self::resolve_for_url`]; tests inject
    /// targets directly). `check_id = http:<label>`, `service_key`
    /// is `None`.
    async fn http_check_with(
        &self,
        check: &HttpCheckConfig,
        target: Option<CheckTarget>,
        now_ms: i64,
    ) -> CheckOutcome {
        let started = Instant::now();
        let (status, summary) = self.http_attempt(check, target.as_ref()).await;
        CheckOutcome {
            check: record(
                format!("http:{}", check.label),
                None,
                status,
                summary,
                now_ms,
            ),
            duration_ms: elapsed_ms(started),
        }
    }

    async fn http_attempt(
        &self,
        check: &HttpCheckConfig,
        target: Option<&CheckTarget>,
    ) -> (CheckStatus, String) {
        // Layer-2 re-validation (defense in depth): even a target
        // injected past the resolution step cannot dial anything the
        // config-time validator would refuse.
        if crate::config::validate_local_http_url(&check.url).is_err() {
            return refused();
        }
        let Some(target) = target else {
            return refused();
        };
        let Some(verified) = target.verified() else {
            return refused();
        };
        let url = match reqwest::Url::parse(&check.url) {
            Ok(url) => url,
            Err(_) => return refused(),
        };
        let client = match target {
            // Pin the verified addresses: post-verification rebinding
            // cannot redirect the connection.
            CheckTarget::Named { host, .. } => Self::pinned_client(host, verified, self.timeout),
            CheckTarget::Literal(_) => self.client.clone(),
        };
        let Some(client) = client else {
            // Engine failure (client construction), honestly unknown
            // — never a panic, never `ok`.
            tracing::error!("local check HTTP client unavailable");
            return (CheckStatus::Unknown, "check engine error".to_string());
        };
        match tokio::time::timeout(self.timeout, client.get(url).send()).await {
            Ok(Ok(response)) => {
                let code = response.status().as_u16();
                let status = if response.status().is_success() {
                    CheckStatus::Ok
                } else {
                    CheckStatus::Critical
                };
                (status, format!("HTTP {code}"))
            }
            // Class-level summary only: the raw error string can
            // embed the request URL.
            Ok(Err(_)) | Err(_) => (CheckStatus::Critical, "connection failed".to_string()),
        }
    }

    /// One TCP check against an already-resolved target. `check_id =
    /// tcp:<label>`, `service_key` is `None`.
    async fn tcp_check_with(
        &self,
        label: &str,
        target: Option<CheckTarget>,
        now_ms: i64,
    ) -> CheckOutcome {
        let started = Instant::now();
        let (status, summary) = self.tcp_attempt(target.as_ref()).await;
        CheckOutcome {
            check: record(format!("tcp:{label}"), None, status, summary, now_ms),
            duration_ms: elapsed_ms(started),
        }
    }

    async fn tcp_attempt(&self, target: Option<&CheckTarget>) -> (CheckStatus, String) {
        let Some(verified) = target.and_then(CheckTarget::verified) else {
            return refused();
        };
        match tokio::time::timeout(self.timeout, tokio::net::TcpStream::connect(verified)).await {
            Ok(Ok(_stream)) => (CheckStatus::Ok, "Connected".to_string()),
            Ok(Err(e)) => match e.kind() {
                std::io::ErrorKind::ConnectionRefused => {
                    (CheckStatus::Critical, "Connection refused".to_string())
                }
                std::io::ErrorKind::TimedOut => {
                    (CheckStatus::Critical, "Connection timed out".to_string())
                }
                _ => (CheckStatus::Critical, "connection failed".to_string()),
            },
            Err(_) => (CheckStatus::Critical, "Connection timed out".to_string()),
        }
    }
}

/// The shared refusal for layer-2 verification failures: the check
/// degrades to `unknown` and NOTHING is dialed.
fn refused() -> (CheckStatus, String) {
    (
        CheckStatus::Unknown,
        "endpoint resolution refused".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TcpCheckConfig;
    use chv_monitor_collectors::SampleValue;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // -----------------------------------------------------------------
    // systemd service engine: fake systemctl
    // -----------------------------------------------------------------

    /// Write a fake systemctl (a `/bin/sh` script) and return its
    /// path. The script body handles the argv shapes the engine uses
    /// (`show <unit> ...` / `list-units ...`).
    fn write_systemctl(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("systemctl");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    /// Run the engine against a fake systemctl with the given
    /// configured units and discovery flag.
    async fn run_services(body: &str, configured: &[&str], discover: bool) -> ServiceCheckResult {
        let dir = tempfile::tempdir().unwrap();
        let engine = ServiceChecks::with_systemctl(write_systemctl(dir.path(), body));
        let configured: Vec<String> = configured.iter().map(|s| s.to_string()).collect();
        engine.run(&configured, discover, 1_000).await
    }

    /// Run one configured unit against a fake systemctl whose `show`
    /// prints `show_output` (printf format; `\n` separators).
    async fn check_unit_output(show_output: &str) -> (CheckOutcome, Option<CollectedSample>) {
        let body = format!(r#"if [ "$1" = "show" ]; then printf '{show_output}'; fi"#);
        let result = run_services(&body, &["unit.service"], false).await;
        assert_eq!(result.outcomes.len(), 1, "exactly one configured unit");
        (
            result.outcomes.into_iter().next().unwrap(),
            result.service_up_samples.into_iter().next(),
        )
    }

    const SHOW_FOR_ANY_UNIT: &str = r#"if [ "$1" = "show" ]; then printf 'active\nloaded\n'; fi"#;

    #[tokio::test]
    async fn active_unit_is_ok_with_up_sample() {
        let (outcome, sample) = check_unit_output("active\\nloaded\\n").await;
        assert_eq!(outcome.check.check_id, "service:unit.service");
        assert_eq!(outcome.check.service_key.as_deref(), Some("unit.service"));
        assert_eq!(outcome.check.observed_at_ms, 1_000);
        assert_eq!(outcome.check.schema_version, SCHEMA_VERSION);
        assert_eq!(outcome.status(), CheckStatus::Ok);
        assert_eq!(outcome.check.summary.as_deref(), Some("active (running)"));
        assert!(outcome.duration_ms <= 5_000);
        let sample = sample.expect("an active unit must carry an up sample");
        assert_eq!(sample.metric_id, "vm.guest.service.up");
        assert_eq!(sample.value, SampleValue::Integer(1));
        assert_eq!(
            sample.dimension,
            Some(("service_key", "unit.service".to_string()))
        );
    }

    #[tokio::test]
    async fn failed_unit_is_critical_with_up_zero() {
        let (outcome, sample) = check_unit_output("failed\\nloaded\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Critical);
        assert_eq!(outcome.check.summary.as_deref(), Some("failed"));
        assert_eq!(
            sample.expect("failed is an observed state").value,
            SampleValue::Integer(0)
        );
    }

    #[tokio::test]
    async fn inactive_unit_is_critical() {
        let (outcome, sample) = check_unit_output("inactive\\nloaded\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Critical);
        assert_eq!(outcome.check.summary.as_deref(), Some("inactive (stopped)"));
        assert_eq!(
            sample.expect("inactive is an observed state").value,
            SampleValue::Integer(0)
        );
    }

    #[tokio::test]
    async fn not_found_unit_is_unknown_without_up_sample() {
        // Honest absence: a unit that is not installed was never
        // observed running or stopped — no sample, never a zero.
        let (outcome, sample) = check_unit_output("inactive\\nnot-found\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Unknown);
        assert_eq!(outcome.check.summary.as_deref(), Some("not installed"));
        assert!(sample.is_none(), "not-installed units carry no up sample");
    }

    #[tokio::test]
    async fn transitional_unit_states_are_warning() {
        for state in ["activating", "deactivating", "reloading"] {
            let (outcome, sample) = check_unit_output(&format!("{state}\\nloaded\\n")).await;
            assert_eq!(outcome.status(), CheckStatus::Warning, "{state}");
            assert_eq!(outcome.check.summary.as_deref(), Some(state));
            assert_eq!(
                sample.expect("a transitional state is observed").value,
                SampleValue::Integer(0)
            );
        }
    }

    #[tokio::test]
    async fn garbage_output_is_unknown_without_up_sample() {
        let (outcome, sample) = check_unit_output("banana\\ncherry\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Unknown);
        assert_eq!(
            outcome.check.summary.as_deref(),
            Some("unexpected unit state")
        );
        assert!(sample.is_none(), "unparsable output carries no up sample");
    }

    #[tokio::test]
    async fn systemctl_failure_is_not_queried() {
        let result = run_services("exit 1", &["unit.service"], false).await;
        assert_eq!(result.outcomes.len(), 1);
        let outcome = &result.outcomes[0];
        assert_eq!(outcome.status(), CheckStatus::Unknown);
        assert_eq!(outcome.check.summary.as_deref(), Some("not queried"));
        assert!(
            result.service_up_samples.is_empty(),
            "a unit that was not queried carries no up sample"
        );
    }

    #[tokio::test]
    async fn unit_state_line_order_and_labels_are_tolerated() {
        // The value vocabularies are disjoint, so the parser must not
        // depend on systemctl's property ordering — and it accepts
        // older labeled `Key=value` output.
        let (outcome, _) = check_unit_output("loaded\\nactive\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Ok);
        let (outcome, _) = check_unit_output("ActiveState=active\\nLoadState=loaded\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Ok);
        // A labeled garbage value is still a parse failure.
        let (outcome, sample) = check_unit_output("ActiveState=banana\\nLoadState=loaded\\n").await;
        assert_eq!(outcome.status(), CheckStatus::Unknown);
        assert!(sample.is_none());
    }

    #[tokio::test]
    async fn invalid_unit_name_is_never_executed() {
        // The fake systemctl reports every unit as active: if the
        // engine had exec'd for the invalid name, the outcome would
        // be `ok` instead of the honest "not queried".
        let result = run_services(SHOW_FOR_ANY_UNIT, &["bad unit"], false).await;
        assert_eq!(result.outcomes.len(), 1);
        let outcome = &result.outcomes[0];
        assert_eq!(outcome.status(), CheckStatus::Unknown);
        assert_eq!(outcome.check.summary.as_deref(), Some("not queried"));
        assert!(result.service_up_samples.is_empty());
    }

    #[tokio::test]
    async fn duplicate_configured_units_are_checked_once() {
        let result = run_services(SHOW_FOR_ANY_UNIT, &["a.service", "a.service"], false).await;
        assert_eq!(result.outcomes.len(), 1);
        assert_eq!(result.service_up_samples.len(), 1);
    }

    #[tokio::test]
    async fn discovery_is_bounded_deduped_filtered_and_sorted() {
        // 35 valid candidates + the configured unit (must dedupe) +
        // one charset-violating line (must be filtered).
        let mut list = String::new();
        for i in 0..35 {
            list.push_str(&format!("c{i:02}.service load active running unit {i}\n"));
        }
        list.push_str("good.service load active running configured\n");
        list.push_str("we;ird.service load active running filtered\n");
        let body = format!(
            r#"if [ "$1" = "show" ]; then printf 'active\nloaded\n';
elif [ "$1" = "list-units" ]; then printf '{list}'
fi"#
        );
        let result = run_services(&body, &["good.service"], true).await;

        // 1 configured + at most 32 discovered, deterministic order.
        assert_eq!(result.outcomes.len(), 1 + 32);
        let discovered: Vec<String> = result
            .outcomes
            .iter()
            .map(|o| o.check.check_id.clone())
            .filter(|id| id.as_str() != "service:good.service")
            .collect();
        let expected: Vec<String> = (0..32)
            .map(|i| format!("service:c{i:02}.service"))
            .collect();
        assert_eq!(discovered, expected);

        // The configured unit keeps its queried outcome; discovered
        // units report the discovery observation.
        let configured = &result.outcomes[0];
        assert_eq!(configured.check.check_id, "service:good.service");
        assert_eq!(configured.status(), CheckStatus::Ok);
        assert_eq!(
            configured.check.summary.as_deref(),
            Some("active (running)")
        );
        for outcome in result.outcomes.iter().skip(1) {
            assert_eq!(outcome.status(), CheckStatus::Ok);
            assert_eq!(
                outcome.check.summary.as_deref(),
                Some("discovered (running)")
            );
            assert!(outcome.duration_ms <= 5_000);
        }

        // One up=1 sample per outcome, none for the filtered line.
        assert_eq!(result.service_up_samples.len(), 33);
        assert!(result
            .service_up_samples
            .iter()
            .all(|s| s.value == SampleValue::Integer(1)));
        assert!(result
            .service_up_samples
            .iter()
            .any(|s| s.dimension == Some(("service_key", "c00.service".to_string()))));
    }

    #[tokio::test]
    async fn discovery_empty_output_adds_nothing() {
        // The script has no `list-units` branch: systemctl exits 0
        // with empty output.
        let result = run_services(SHOW_FOR_ANY_UNIT, &["good.service"], true).await;
        assert_eq!(result.outcomes.len(), 1);
        assert_eq!(result.outcomes[0].check.check_id, "service:good.service");
        assert_eq!(result.service_up_samples.len(), 1);
    }

    #[tokio::test]
    async fn discovery_failure_degrades_to_empty() {
        let body = r#"if [ "$1" = "show" ]; then printf 'active\nloaded\n';
elif [ "$1" = "list-units" ]; then exit 1
fi"#;
        let result = run_services(body, &["good.service"], true).await;
        // The configured unit is unaffected; discovery degrades to
        // empty rather than crashing or inventing units.
        assert_eq!(result.outcomes.len(), 1);
        assert_eq!(result.outcomes[0].status(), CheckStatus::Ok);
        assert_eq!(result.service_up_samples.len(), 1);
    }

    #[tokio::test]
    async fn discovery_disabled_never_lists() {
        let body = r#"if [ "$1" = "show" ]; then printf 'active\nloaded\n';
elif [ "$1" = "list-units" ]; then printf 'sneaky.service load active running nope\n'
fi"#;
        let result = run_services(body, &["good.service"], false).await;
        assert_eq!(result.outcomes.len(), 1);
        assert_eq!(result.outcomes[0].check.check_id, "service:good.service");
        assert!(!result
            .outcomes
            .iter()
            .any(|o| o.check.check_id == "service:sneaky.service"));
    }

    // -----------------------------------------------------------------
    // declarative local HTTP/TCP checks
    // -----------------------------------------------------------------

    fn http_config(label: &str, url: String) -> HttpCheckConfig {
        HttpCheckConfig {
            label: label.to_string(),
            url,
        }
    }

    fn tcp_config(label: &str, host: &str, port: u16) -> TcpCheckConfig {
        TcpCheckConfig {
            label: label.to_string(),
            host: host.to_string(),
            port,
        }
    }

    /// Accept one connection on a raw listener, read the request,
    /// write a canned HTTP response.
    async fn serve_one(listener: tokio::net::TcpListener, status_line: &str, body: &str) {
        let Ok((mut sock, _)) = listener.accept().await else {
            return;
        };
        let mut buf = [0u8; 2048];
        let _ = sock.read(&mut buf).await;
        let response = format!(
            "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = sock.write_all(response.as_bytes()).await;
        let _ = sock.shutdown().await;
    }

    #[tokio::test]
    async fn tcp_check_connects_to_ephemeral_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let checks = ChecksConfig {
            http: vec![],
            tcp: vec![tcp_config("t", "127.0.0.1", port)],
        };
        let outcomes = LocalChecks::new().run(&checks, 1_000).await;
        assert_eq!(outcomes.len(), 1);
        let outcome = &outcomes[0];
        assert_eq!(outcome.check.check_id, "tcp:t");
        assert!(outcome.check.service_key.is_none());
        assert_eq!(outcome.check.observed_at_ms, 1_000);
        assert_eq!(outcome.status(), CheckStatus::Ok);
        assert_eq!(outcome.check.summary.as_deref(), Some("Connected"));
        assert!(outcome.duration_ms <= 5_000);
    }

    #[tokio::test]
    async fn tcp_check_closed_port_is_critical() {
        // Bind, learn the port, drop: a guaranteed-closed loopback
        // port.
        let port = {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            port
        };
        let checks = ChecksConfig {
            http: vec![],
            tcp: vec![tcp_config("closed", "127.0.0.1", port)],
        };
        let outcomes = LocalChecks::new().run(&checks, 1_000).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), CheckStatus::Critical);
        let summary = outcomes[0].check.summary.as_deref().unwrap();
        assert!(
            summary == "Connection refused" || summary == "Connection timed out",
            "unexpected summary {summary}"
        );
    }

    #[tokio::test]
    async fn tcp_check_non_loopback_host_is_refused() {
        // Config drift / reload race: a remote host must never be
        // dialed — the check degrades to unknown without a
        // connection attempt.
        let checks = ChecksConfig {
            http: vec![],
            tcp: vec![tcp_config("remote", "10.0.0.5", 80)],
        };
        let outcomes = LocalChecks::new().run(&checks, 1_000).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), CheckStatus::Unknown);
        assert_eq!(
            outcomes[0].check.summary.as_deref(),
            Some("endpoint resolution refused")
        );
    }

    #[tokio::test]
    async fn http_check_ok_against_raw_http_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(serve_one(listener, "HTTP/1.1 200 OK", "ok"));
        let checks = ChecksConfig {
            http: vec![http_config("h", format!("http://127.0.0.1:{port}/health"))],
            tcp: vec![],
        };
        let outcomes = LocalChecks::new().run(&checks, 1_000).await;
        assert_eq!(outcomes.len(), 1);
        let outcome = &outcomes[0];
        assert_eq!(outcome.check.check_id, "http:h");
        assert!(outcome.check.service_key.is_none());
        assert_eq!(outcome.status(), CheckStatus::Ok);
        assert_eq!(outcome.check.summary.as_deref(), Some("HTTP 200"));
        assert!(outcome.duration_ms <= 5_000);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn http_check_500_is_critical() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(serve_one(
            listener,
            "HTTP/1.1 500 Internal Server Error",
            "",
        ));
        let checks = ChecksConfig {
            http: vec![http_config("h", format!("http://127.0.0.1:{port}/health"))],
            tcp: vec![],
        };
        let outcomes = LocalChecks::new().run(&checks, 1_000).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), CheckStatus::Critical);
        assert_eq!(outcomes[0].check.summary.as_deref(), Some("HTTP 500"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn http_check_invalid_url_is_refused_without_dialing() {
        // A URL that fails the config-time validator (here: a remote
        // host smuggled past config) must degrade to unknown without
        // a dial — a dialed 10.0.0.5 would come back critical
        // "connection failed", never unknown.
        let checks = ChecksConfig {
            http: vec![http_config("remote", "http://10.0.0.5/health".to_string())],
            tcp: vec![],
        };
        let outcomes = LocalChecks::new().run(&checks, 1_000).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), CheckStatus::Unknown);
        assert_eq!(
            outcomes[0].check.summary.as_deref(),
            Some("endpoint resolution refused")
        );
    }

    #[tokio::test]
    async fn rebinding_resolution_refuses_tcp_without_dialing() {
        // Trap listener: if anything connects, the accept below
        // succeeds and the test fails.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        // A "localhost" resolution containing a non-loopback address
        // (DNS rebinding). The whole resolution is refused.
        let poisoned = Some(CheckTarget::Named {
            host: "localhost".to_string(),
            addrs: vec![
                SocketAddr::from(([127, 0, 0, 1], port)),
                SocketAddr::from(([8, 8, 8, 8], 53)),
            ],
        });
        let engine = LocalChecks::new();
        let outcome = engine.tcp_check_with("rebind", poisoned, 1_000).await;
        assert_eq!(
            outcome.status(),
            CheckStatus::Unknown,
            "unknown never becomes ok"
        );
        assert_eq!(
            outcome.check.summary.as_deref(),
            Some("endpoint resolution refused")
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(250), listener.accept())
                .await
                .is_err(),
            "a poisoned resolution must never be dialed"
        );
    }

    #[tokio::test]
    async fn rebinding_resolution_refuses_http_without_dialing() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let poisoned = Some(CheckTarget::Named {
            host: "localhost".to_string(),
            addrs: vec![
                SocketAddr::from(([127, 0, 0, 1], port)),
                SocketAddr::from(([8, 8, 8, 8], 53)),
            ],
        });
        let engine = LocalChecks::new();
        let check = http_config("rebind", format!("http://localhost:{port}/health"));
        let outcome = engine.http_check_with(&check, poisoned, 1_000).await;
        assert_eq!(outcome.status(), CheckStatus::Unknown);
        assert_eq!(
            outcome.check.summary.as_deref(),
            Some("endpoint resolution refused")
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(250), listener.accept())
                .await
                .is_err(),
            "a poisoned resolution must never be dialed"
        );
    }

    #[tokio::test]
    async fn http_check_pins_verified_localhost_resolution() {
        // The URL uses the NAME; the injected verified resolution
        // (pinned into the client via resolve_to_addrs) points it at
        // the raw listener — no system resolver involved.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(serve_one(listener, "HTTP/1.1 200 OK", "ok"));
        let engine = LocalChecks::new();
        let target = Some(CheckTarget::Named {
            host: "localhost".to_string(),
            addrs: vec![SocketAddr::from(([127, 0, 0, 1], port))],
        });
        let check = http_config("lh", format!("http://localhost:{port}/health"));
        let outcome = engine.http_check_with(&check, target, 1_000).await;
        assert_eq!(outcome.status(), CheckStatus::Ok);
        assert_eq!(outcome.check.summary.as_deref(), Some("HTTP 200"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn target_resolution_rules() {
        // Literal loopback addresses pass through; IPv6 brackets are
        // tolerated.
        assert!(matches!(
            LocalChecks::resolve_target("127.0.0.1", 8080).await,
            Some(CheckTarget::Literal(sa)) if sa == SocketAddr::from(([127, 0, 0, 1], 8080))
        ));
        assert!(matches!(
            LocalChecks::resolve_target("[::1]", 9).await,
            Some(CheckTarget::Literal(sa)) if sa.ip().is_loopback()
        ));
        // Non-loopback literals and foreign names are refused
        // without a lookup (SSRF defense: remote and metadata
        // endpoints never reach the resolver or the dialer).
        assert!(LocalChecks::resolve_target("0.0.0.0", 80).await.is_none());
        assert!(LocalChecks::resolve_target("10.0.0.5", 80).await.is_none());
        assert!(LocalChecks::resolve_target("example.com", 80)
            .await
            .is_none());
        // A real `localhost` resolution contains only loopback
        // addresses on a sane host, and layer-2 verification accepts
        // it...
        let target = LocalChecks::resolve_target("localhost", 80)
            .await
            .expect("localhost must resolve on the test host");
        let CheckTarget::Named { addrs, .. } = &target else {
            panic!("localhost must be name-resolved");
        };
        assert!(!addrs.is_empty());
        assert!(
            addrs.iter().all(|a| a.ip().is_loopback()),
            "unexpected resolution {addrs:?}"
        );
        assert!(target.verified().is_some());
        // ...while an empty or rebound resolution is refused, and so
        // is a non-loopback literal smuggled into a Literal target.
        assert!(CheckTarget::Named {
            host: "localhost".to_string(),
            addrs: vec![],
        }
        .verified()
        .is_none());
        assert!(CheckTarget::Literal(SocketAddr::from(([10, 0, 0, 5], 80)))
            .verified()
            .is_none());
    }
}
