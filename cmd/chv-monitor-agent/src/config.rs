//! Agent configuration (TOML). Every path is explicit so a deployment
//! can confine the agent to exactly the directories the packaging
//! grants it.

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
        Ok(())
    }
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
    }
}
