//! Agent configuration (TOML). Every path is explicit so a deployment
//! can confine the agent to exactly the directories the packaging
//! grants it.
//!
//! The G4 sections (`[collectors]`, `[services]`, `[checks]`,
//! `[plugins]`) are additive with defaults: a pre-G4 config file
//! parses unchanged (baseline collectors only). Privacy-sensitive
//! families (processes) and all execution surfaces (local checks,
//! plugins) stay OFF until explicitly enabled.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    /// Base URL of the manager, always `https://`.
    pub server_url: String,
    /// PEM trust anchor for the manager's TLS server certificate
    /// (`[http_tls]` server cert or its CA). The agent validates the
    /// manager's identity before presenting any credential.
    pub manager_ca_path: PathBuf,
    #[serde(default = "default_claim_path")]
    pub claim_path: PathBuf,
    #[serde(default = "default_credential_path")]
    pub credential_path: PathBuf,
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
    #[serde(default = "default_spool_dir")]
    pub spool_dir: PathBuf,
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
    #[serde(default = "default_max_spool_batches")]
    pub max_spool_batches: usize,
    #[serde(default = "default_spool_max_age_seconds")]
    pub spool_max_age_seconds: u64,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Guest collector families (component spec profiles). The
    /// read-only fs/network/services families default ON; the
    /// privacy-sensitive process family defaults OFF.
    #[serde(default)]
    pub collectors: CollectorsConfig,
    /// systemd service checks: an explicit configured unit list plus
    /// opt-in bounded discovery. Inventory stays minimal by default
    /// (security contract: "guest inventory defaults to minimal
    /// fields").
    #[serde(default)]
    pub services: ServicesConfig,
    /// Declarative local HTTP/TCP checks. Local opt-in: empty by
    /// default, and every endpoint must be a loopback address — the
    /// agent never probes remote targets with these checks (that is
    /// plugin territory, under the root-owned allowlist).
    #[serde(default)]
    pub checks: ChecksConfig,
    /// Opt-in local plugins (security/plugins contract v1). Disabled
    /// by default; the manager can never enable or supply plugins.
    #[serde(default)]
    pub plugins: PluginsConfig,
}

/// Collector family toggles (component spec "collection profiles").
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectorsConfig {
    #[serde(default = "default_true")]
    pub filesystems: bool,
    #[serde(default = "default_true")]
    pub network: bool,
    #[serde(default = "default_true")]
    pub services: bool,
    #[serde(default)]
    pub processes: bool,
    /// Stable executable names matched against `/proc/<pid>/status`
    /// `Name:` only — never command lines. Bounded to 8 selectors so
    /// the worst-case batch stays inside the 512-sample ingest cap.
    #[serde(default)]
    pub process_selectors: Vec<String>,
}

impl Default for CollectorsConfig {
    fn default() -> Self {
        Self {
            filesystems: true,
            network: true,
            services: true,
            processes: false,
            process_selectors: Vec::new(),
        }
    }
}

/// systemd checks configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicesConfig {
    /// Explicit systemd unit names to check (e.g. "nginx.service"),
    /// bounded to 32.
    #[serde(default)]
    pub configured: Vec<String>,
    /// Bounded discovery of running services (adds at most 32
    /// discovered units). Off by default.
    #[serde(default)]
    pub discover: bool,
}

/// Declarative local checks (empty by default).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ChecksConfig {
    #[serde(default)]
    pub http: Vec<HttpCheckConfig>,
    #[serde(default)]
    pub tcp: Vec<TcpCheckConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpCheckConfig {
    /// Check label; the check id becomes `http:<label>`.
    pub label: String,
    /// `http://` or `https://` with a LOOPBACK host only (the
    /// runtime re-verifies resolution — DNS rebinding defense).
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpCheckConfig {
    /// Check label; the check id becomes `tcp:<label>`.
    pub label: String,
    /// Loopback host only.
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_plugins_dir")]
    pub directory: PathBuf,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: default_plugins_dir(),
        }
    }
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from("/etc/chv-monitor/plugins.d")
}

fn default_true() -> bool {
    true
}

fn default_claim_path() -> PathBuf {
    // Inside the agent's state dir, not /etc: the shipped systemd unit
    // mounts /etc read-only (ProtectSystem=strict), so a claim placed
    // there could never be deleted after consumption. The operator
    // places it with: install -o chv-monitor -g chv-monitor -m 0600
    // <claim> /var/lib/chv-monitor/claim
    PathBuf::from("/var/lib/chv-monitor/claim")
}
fn default_credential_path() -> PathBuf {
    PathBuf::from("/var/lib/chv-monitor/credential.json")
}
fn default_state_dir() -> PathBuf {
    PathBuf::from("/var/lib/chv-monitor")
}
fn default_spool_dir() -> PathBuf {
    PathBuf::from("/var/lib/chv-monitor/spool")
}
fn default_interval_seconds() -> u64 {
    // The component spec's `resources` profile default (15 s), within
    // the ingestion contract's one-batch-per-5-seconds ceiling.
    15
}
fn default_max_spool_batches() -> usize {
    500
}
fn default_spool_max_age_seconds() -> u64 {
    7 * 24 * 3600
}
fn default_log_level() -> String {
    "info".to_string()
}

impl AgentConfig {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config: AgentConfig = toml::from_str(text).map_err(ConfigError::Parse)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_toml(&text)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if !self.server_url.starts_with("https://") {
            return Err(ConfigError::Invalid(
                "server_url must be https:// — the agent never speaks monitoring over plain TLS-less HTTP"
                    .into(),
            ));
        }
        if self.interval_seconds == 0 {
            return Err(ConfigError::Invalid("interval_seconds must be >= 1".into()));
        }
        if self.max_spool_batches == 0 {
            return Err(ConfigError::Invalid(
                "max_spool_batches must be >= 1 (0 would discard all data on outage)".into(),
            ));
        }
        self.validate_collectors()?;
        self.validate_services()?;
        self.validate_checks()?;
        self.validate_plugins()?;
        Ok(())
    }

    fn validate_collectors(&self) -> Result<(), ConfigError> {
        let selectors = &self.collectors.process_selectors;
        if selectors.len() > 8 {
            return Err(ConfigError::Invalid(
                "collectors.process_selectors is bounded to 8 entries".into(),
            ));
        }
        if !self.collectors.processes && !selectors.is_empty() {
            return Err(ConfigError::Invalid(
                "collectors.process_selectors requires collectors.processes = true".into(),
            ));
        }
        for s in selectors {
            let t = s.trim();
            if t.is_empty() || t.len() > 128 || t.bytes().any(|b| b < 0x20 || b == 0x7f) {
                return Err(ConfigError::Invalid(format!(
                    "process selector {s:?} must be 1..=128 printable bytes"
                )));
            }
        }
        if selectors
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != selectors.len()
        {
            return Err(ConfigError::Invalid(
                "collectors.process_selectors contains duplicates".into(),
            ));
        }
        Ok(())
    }

    fn validate_services(&self) -> Result<(), ConfigError> {
        let units = &self.services.configured;
        if units.len() > 32 {
            return Err(ConfigError::Invalid(
                "services.configured is bounded to 32 units".into(),
            ));
        }
        for u in units {
            let t = u.trim();
            if t.is_empty() || t.len() > 128 {
                return Err(ConfigError::Invalid(format!(
                    "service unit {u:?} must be 1..=128 bytes"
                )));
            }
            if !t
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'@' | b':'))
            {
                return Err(ConfigError::Invalid(format!(
                    "service unit {u:?} may only contain [A-Za-z0-9._-@:]"
                )));
            }
        }
        if units
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != units.len()
        {
            return Err(ConfigError::Invalid(
                "services.configured contains duplicates".into(),
            ));
        }
        Ok(())
    }

    fn validate_checks(&self) -> Result<(), ConfigError> {
        let total = self.checks.http.len() + self.checks.tcp.len();
        if total > 16 {
            return Err(ConfigError::Invalid(
                "checks are bounded to 16 entries total (http + tcp)".into(),
            ));
        }
        let mut labels = std::collections::BTreeSet::new();
        for c in &self.checks.http {
            validate_check_label(&c.label)?;
            if !labels.insert(format!("http:{}", c.label)) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate check label {:?}",
                    c.label
                )));
            }
            validate_local_http_url(&c.url)
                .map_err(|e| ConfigError::Invalid(format!("checks.http {:?}: {e}", c.label)))?;
        }
        for c in &self.checks.tcp {
            validate_check_label(&c.label)?;
            if !labels.insert(format!("tcp:{}", c.label)) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate check label {:?}",
                    c.label
                )));
            }
            if !is_loopback_host(&c.host) {
                return Err(ConfigError::Invalid(format!(
                    "checks.tcp {:?}: host must be a loopback address (these are local checks; \
                     remote endpoints belong to root-approved plugins)",
                    c.label
                )));
            }
            if c.port == 0 {
                return Err(ConfigError::Invalid(format!(
                    "checks.tcp {:?}: port must be >= 1",
                    c.label
                )));
            }
        }
        Ok(())
    }

    fn validate_plugins(&self) -> Result<(), ConfigError> {
        if self.plugins.enabled && !self.plugins.directory.is_absolute() {
            return Err(ConfigError::Invalid(
                "plugins.directory must be an absolute path".into(),
            ));
        }
        Ok(())
    }
}

/// Check labels: short printable tokens usable inside a check_id.
fn validate_check_label(label: &str) -> Result<(), ConfigError> {
    if label.is_empty() || label.len() > 64 {
        return Err(ConfigError::Invalid(format!(
            "check label {label:?} must be 1..=64 bytes"
        )));
    }
    if !label
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(ConfigError::Invalid(format!(
            "check label {label:?} may only contain [A-Za-z0-9._-]"
        )));
    }
    Ok(())
}

/// Strict loopback test for declarative local checks (SSRF defense,
/// layer 1 of 2: config-time). The runtime re-verifies after any
/// name resolution so a rebinding cannot redirect to a forbidden
/// address. Allowed: `localhost`, IPv4 127.0.0.0/8, IPv6 `::1` (with
/// optional brackets). Everything else — including link-local
/// 169.254.0.0/16 (cloud metadata), 0.0.0.0 and any remote host — is
/// rejected: these checks are LOCAL, by explicit opt-in.
pub(crate) fn is_loopback_host(host: &str) -> bool {
    let h = host.trim();
    if h.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let h = h
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(h);
    if h == "::1" {
        return true;
    }
    // IPv4 127.0.0.0/8: all four decimal octets, first is 127.
    let parts: Vec<&str> = h.split('.').collect();
    if parts.len() == 4 {
        return parts[0] == "127"
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
            && parts[1..].iter().all(|p| p.parse::<u8>().is_ok());
    }
    false
}

/// Validate a declarative HTTP check URL: `http://` or `https://`,
/// a loopback host, and a well-formed authority. Query strings and
/// fragments are rejected (a check target is a fixed path, not a
/// user-controlled URL surface).
pub(crate) fn validate_local_http_url(url: &str) -> Result<(), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "must be scheme://host[:port]/path".to_string())?;
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported scheme {scheme:?} (http/https only)"));
    }
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, p),
        None => (rest, ""),
    };
    if authority.is_empty() {
        return Err("empty host".into());
    }
    if path.contains('?') || path.contains('#') {
        return Err("query strings and fragments are not allowed".into());
    }
    // Strip the port, keeping IPv6 brackets intact.
    let host = if authority.starts_with('[') {
        match authority.split_once(']') {
            Some((h, tail)) => {
                if !tail.is_empty() && !tail.starts_with(':') {
                    return Err("malformed IPv6 authority".into());
                }
                format!("{h}]")
            }
            None => return Err("malformed IPv6 authority".into()),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, port)) => {
                if !port.parse::<u16>().is_ok_and(|p| p > 0) {
                    return Err("malformed port".into());
                }
                h.to_string()
            }
            None => authority.to_string(),
        }
    };
    if !is_loopback_host(&host) {
        return Err(
            "host must be a loopback address (local checks only; remote endpoints \
             belong to root-approved plugins)"
                .into(),
        );
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to parse agent config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to read agent config: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid agent config: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_config_takes_defaults() {
        let c = AgentConfig::from_toml(
            "server_url = \"https://manager.example:8443\"\nmanager_ca_path = \"/etc/chv-monitor/ca.pem\"\n",
        )
        .unwrap();
        assert_eq!(c.interval_seconds, 15);
        assert_eq!(c.max_spool_batches, 500);
        assert_eq!(c.state_dir, PathBuf::from("/var/lib/chv-monitor"));
        assert_eq!(c.spool_dir, PathBuf::from("/var/lib/chv-monitor/spool"));
    }

    #[test]
    fn plain_http_is_rejected() {
        let err = AgentConfig::from_toml(
            "server_url = \"http://manager.example\"\nmanager_ca_path = \"/ca.pem\"\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("https://"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = AgentConfig::from_toml(
            "server_url = \"https://m\"\nmanager_ca_path = \"/ca.pem\"\nexec_plugin = true\n",
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn example_config_refuses_to_start_until_configured() {
        // Drift guard for docs/examples/monitor-agent.toml: the
        // shipped example is deliberately incomplete — the agent must
        // refuse it loudly, never guess a manager. Uncommenting the
        // two documented values must make it valid with the packaged
        // defaults.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/examples/monitor-agent.toml"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let err = AgentConfig::from_toml(&text).unwrap_err();
        assert!(
            err.to_string().contains("server_url"),
            "the failure must point at server_url, got: {err}"
        );

        let configured = text
            .replace("# server_url = ", "server_url = ")
            .replace("# manager_ca_path = ", "manager_ca_path = ");
        let config = AgentConfig::from_toml(&configured).unwrap();
        assert_eq!(config.state_dir, PathBuf::from("/var/lib/chv-monitor"));
        assert_eq!(
            config.spool_dir,
            PathBuf::from("/var/lib/chv-monitor/spool")
        );
        assert_eq!(config.interval_seconds, 15);
        // G4 defaults: read-only families on, privacy/execution off.
        assert!(config.collectors.filesystems);
        assert!(config.collectors.network);
        assert!(config.collectors.services);
        assert!(!config.collectors.processes);
        assert!(!config.services.discover);
        assert!(config.checks.http.is_empty());
        assert!(!config.plugins.enabled);
        assert_eq!(
            config.plugins.directory,
            PathBuf::from("/etc/chv-monitor/plugins.d")
        );
    }

    const BASE: &str = "server_url = \"https://m\"\nmanager_ca_path = \"/ca.pem\"\n";

    #[test]
    fn pre_g4_configs_parse_unchanged() {
        // Additive sections: a config written before G4 keeps its
        // exact meaning.
        let c = AgentConfig::from_toml(BASE).unwrap();
        assert!(c.collectors.filesystems);
        assert!(!c.collectors.processes);
        assert!(c.checks.http.is_empty());
        assert!(!c.plugins.enabled);
    }

    #[test]
    fn collector_and_service_bounds_are_enforced() {
        let many: Vec<String> = (0..9).map(|i| format!("proc{i}")).collect();
        let selectors = many
            .iter()
            .map(|s| format!("{s:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        let err = AgentConfig::from_toml(&format!(
            "{BASE}[collectors]\nprocesses = true\nprocess_selectors = [{selectors}]\n"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("bounded to 8"));

        let err = AgentConfig::from_toml(&format!(
            "{BASE}[collectors]\nprocess_selectors = [\"nginx\"]\n"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("requires collectors.processes"));

        let units: Vec<String> = (0..33).map(|i| format!("\"s{i}.service\"")).collect();
        let err = AgentConfig::from_toml(&format!(
            "{BASE}[services]\nconfigured = [{}]\n",
            units.join(", ")
        ))
        .unwrap_err();
        assert!(err.to_string().contains("bounded to 32"));

        let err = AgentConfig::from_toml(&format!(
            "{BASE}[services]\nconfigured = [\"nginx; rm -rf /\"]\n"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("[A-Za-z0-9._-@:]"));
    }

    #[test]
    fn local_checks_reject_remote_and_metadata_endpoints() {
        // Loopback endpoints are the only valid declarative checks.
        let ok = AgentConfig::from_toml(&format!(
            "{BASE}[[checks.http]]\nlabel = \"local\"\nurl = \"http://127.0.0.1:8080/health\"\n\
             [[checks.tcp]]\nlabel = \"ssh\"\nhost = \"localhost\"\nport = 22\n"
        ))
        .unwrap();
        assert_eq!(ok.checks.http.len(), 1);
        assert_eq!(ok.checks.tcp[0].port, 22);

        for bad in [
            "http://10.0.0.5/health",        // remote RFC1918
            "http://169.254.169.254/latest", // cloud metadata (SSRF)
            "ftp://127.0.0.1/x",             // bad scheme
            "http://127.0.0.1:99999/x",      // port beyond u16
            "http://127.0.0.1:0/x",          // zero port
            "http://example.com/health",     // remote name
            "http://0.0.0.0/health",         // unspecified address
            "http://127.0.0.1/health?x=1",   // query string
            "http://127.0.0.1/health#frag",  // fragment
            "http://2130706433/health",      // non-dotted integer host
        ] {
            let err = AgentConfig::from_toml(&format!(
                "{BASE}[[checks.http]]\nlabel = \"x\"\nurl = \"{bad}\"\n"
            ))
            .unwrap_err();
            assert!(!err.to_string().is_empty(), "{bad} must be rejected");
        }

        // IPv6 loopback and no-path URLs are fine.
        AgentConfig::from_toml(&format!(
            "{BASE}[[checks.http]]\nlabel = \"v6\"\nurl = \"http://[::1]:8080\"\n"
        ))
        .unwrap();

        // Bad labels are rejected.
        let err = AgentConfig::from_toml(&format!(
            "{BASE}[[checks.tcp]]\nlabel = \"bad label!\"\nhost = \"127.0.0.1\"\nport = 1\n"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("label"));

        // Duplicate labels within one check type are rejected (the
        // check_id would collide); the same label across http and tcp
        // is fine — the ids are namespaced.
        let err = AgentConfig::from_toml(&format!(
            "{BASE}[[checks.tcp]]\nlabel = \"a\"\nhost = \"127.0.0.1\"\nport = 1\n\
             [[checks.tcp]]\nlabel = \"a\"\nhost = \"127.0.0.1\"\nport = 2\n"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("duplicate"));
        AgentConfig::from_toml(&format!(
            "{BASE}[[checks.tcp]]\nlabel = \"a\"\nhost = \"127.0.0.1\"\nport = 1\n\
             [[checks.http]]\nlabel = \"a\"\nurl = \"http://127.0.0.1/\"\n"
        ))
        .unwrap();

        // More than 16 declarative checks are rejected.
        let mut toml = BASE.to_string();
        for i in 0..17 {
            toml.push_str(&format!(
                "[[checks.tcp]]\nlabel = \"t{i}\"\nhost = \"127.0.0.1\"\nport = {i}\n"
            ));
        }
        let err = AgentConfig::from_toml(&toml).unwrap_err();
        assert!(err.to_string().contains("bounded to 16"));
    }

    #[test]
    fn plugin_directory_must_be_absolute_when_enabled() {
        AgentConfig::from_toml(&format!(
            "{BASE}[plugins]\nenabled = false\ndirectory = \"relative\"\n"
        ))
        .unwrap();
        let err = AgentConfig::from_toml(&format!(
            "{BASE}[plugins]\nenabled = true\ndirectory = \"relative\"\n"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn loopback_host_test_cases() {
        for ok in [
            "localhost",
            "LOCALHOST",
            "127.0.0.1",
            "127.255.0.9",
            "::1",
            "[::1]",
        ] {
            assert!(is_loopback_host(ok), "{ok} must be loopback");
        }
        for bad in [
            "169.254.169.254",
            "0.0.0.0",
            "10.1.2.3",
            "::",
            "fe80::1",
            "127.0.0.256",
            "127.0.0",
            "localhost.example.com",
            "",
        ] {
            assert!(!is_loopback_host(bad), "{bad} must not be loopback");
        }
    }
}
