use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct StordConfig {
    pub socket_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_level: String,
    #[serde(default)]
    pub backend_allowlist: Vec<String>,
    pub metrics_bind: Option<String>,
}

impl Default for StordConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/stord/api.sock"),
            runtime_dir: PathBuf::from("/run/chv/stord"),
            log_level: "info".to_string(),
            backend_allowlist: vec![],
            metrics_bind: None,
        }
    }
}

pub fn load_stord_config(path: Option<&Path>) -> Result<StordConfig, ConfigError> {
    let mut cfg = StordConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    Ok(cfg)
}

#[derive(Debug, Clone, Deserialize)]
pub struct NwdConfig {
    pub socket_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_level: String,
    pub metrics_bind: Option<String>,
}

impl Default for NwdConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/nwd/api.sock"),
            runtime_dir: PathBuf::from("/run/chv/nwd"),
            log_level: "info".to_string(),
            metrics_bind: None,
        }
    }
}

pub fn load_nwd_config(path: Option<&Path>) -> Result<NwdConfig, ConfigError> {
    let mut cfg = NwdConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    Ok(cfg)
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub socket_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_level: String,
    pub control_plane_addr: String,
    pub stord_socket: PathBuf,
    pub nwd_socket: PathBuf,
    pub chv_binary_path: PathBuf,
    pub stord_binary_path: PathBuf,
    pub nwd_binary_path: PathBuf,
    pub cache_path: PathBuf,
    pub node_id: String,
    pub metrics_bind: Option<String>,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    pub ca_cert_path: Option<PathBuf>,
    pub bootstrap_token_path: Option<PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/agent/api.sock"),
            runtime_dir: PathBuf::from("/run/chv/agent"),
            log_level: "info".to_string(),
            control_plane_addr: "https://localhost:8443".to_string(),
            stord_socket: PathBuf::from("/run/chv/stord/api.sock"),
            nwd_socket: PathBuf::from("/run/chv/nwd/api.sock"),
            chv_binary_path: PathBuf::from("/usr/bin/cloud-hypervisor"),
            stord_binary_path: PathBuf::from("/usr/bin/chv-stord"),
            nwd_binary_path: PathBuf::from("/usr/bin/chv-nwd"),
            cache_path: PathBuf::from("/var/lib/chv/cache/agent-cache.json"),
            node_id: String::new(),
            metrics_bind: None,
            tls_cert_path: None,
            tls_key_path: None,
            ca_cert_path: None,
            bootstrap_token_path: None,
        }
    }
}

pub fn load_agent_config(path: Option<&Path>) -> Result<AgentConfig, ConfigError> {
    let mut cfg = AgentConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    Ok(cfg)
}

const DEFAULT_CONTROLPLANE_GRPC_BIND: &str = "127.0.0.1:8443";
const DEFAULT_CONTROLPLANE_HTTP_BIND: &str = "127.0.0.1:8080";
const DEFAULT_CONTROLPLANE_LOG_LEVEL: &str = "info";
const DEFAULT_CONTROLPLANE_RUNTIME_DIR: &str = "/run/chv/controlplane";
const DEFAULT_CONTROLPLANE_DATABASE_URL: &str =
    "postgres://postgres:postgres@127.0.0.1:5432/chv_controlplane";
const DEFAULT_CONTROLPLANE_MIGRATIONS_DIR: &str = "cmd/chv-controlplane/migrations";
const DEFAULT_CONTROLPLANE_DB_MAX_CONNECTIONS: u32 = 16;
const DEFAULT_CONTROLPLANE_DB_MIN_CONNECTIONS: u32 = 1;
const DEFAULT_CONTROLPLANE_DB_ACQUIRE_TIMEOUT_SECS: u64 = 5;
const DEFAULT_CONTROLPLANE_DB_IDLE_TIMEOUT_SECS: u64 = 300;
const DEFAULT_CONTROLPLANE_DB_MAX_LIFETIME_SECS: u64 = 1800;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ControlPlaneTlsConfig {
    #[serde(default)]
    pub server_cert_path: Option<PathBuf>,
    #[serde(default)]
    pub server_key_path: Option<PathBuf>,
    #[serde(default)]
    pub client_ca_path: Option<PathBuf>,
    #[serde(default)]
    pub ca_cert_path: Option<PathBuf>,
    #[serde(default)]
    pub ca_key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlPlaneConfig {
    pub grpc_bind: SocketAddr,
    pub http_bind: SocketAddr,
    pub log_level: String,
    pub runtime_dir: PathBuf,
    #[serde(default)]
    pub database: ControlPlaneDatabaseConfig,
    #[serde(default)]
    pub tls: ControlPlaneTlsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlPlaneDatabaseConfig {
    pub url: String,
    pub migrations_dir: PathBuf,
    #[serde(default = "default_controlplane_db_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_controlplane_db_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_controlplane_db_acquire_timeout_secs")]
    pub acquire_timeout_secs: u64,
    #[serde(default = "default_controlplane_db_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_controlplane_db_max_lifetime_secs")]
    pub max_lifetime_secs: u64,
}

impl Default for ControlPlaneDatabaseConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_CONTROLPLANE_DATABASE_URL.to_string(),
            migrations_dir: PathBuf::from(DEFAULT_CONTROLPLANE_MIGRATIONS_DIR),
            max_connections: default_controlplane_db_max_connections(),
            min_connections: default_controlplane_db_min_connections(),
            acquire_timeout_secs: default_controlplane_db_acquire_timeout_secs(),
            idle_timeout_secs: default_controlplane_db_idle_timeout_secs(),
            max_lifetime_secs: default_controlplane_db_max_lifetime_secs(),
        }
    }
}

impl Default for ControlPlaneConfig {
    fn default() -> Self {
        Self {
            grpc_bind: DEFAULT_CONTROLPLANE_GRPC_BIND
                .parse()
                .expect("valid default grpc bind"),
            http_bind: DEFAULT_CONTROLPLANE_HTTP_BIND
                .parse()
                .expect("valid default http bind"),
            log_level: DEFAULT_CONTROLPLANE_LOG_LEVEL.to_string(),
            runtime_dir: PathBuf::from(DEFAULT_CONTROLPLANE_RUNTIME_DIR),
            database: ControlPlaneDatabaseConfig::default(),
            tls: ControlPlaneTlsConfig::default(),
        }
    }
}

fn default_controlplane_db_max_connections() -> u32 {
    DEFAULT_CONTROLPLANE_DB_MAX_CONNECTIONS
}

fn default_controlplane_db_min_connections() -> u32 {
    DEFAULT_CONTROLPLANE_DB_MIN_CONNECTIONS
}

fn default_controlplane_db_acquire_timeout_secs() -> u64 {
    DEFAULT_CONTROLPLANE_DB_ACQUIRE_TIMEOUT_SECS
}

fn default_controlplane_db_idle_timeout_secs() -> u64 {
    DEFAULT_CONTROLPLANE_DB_IDLE_TIMEOUT_SECS
}

fn default_controlplane_db_max_lifetime_secs() -> u64 {
    DEFAULT_CONTROLPLANE_DB_MAX_LIFETIME_SECS
}

pub fn load_controlplane_config(path: Option<&Path>) -> Result<ControlPlaneConfig, ConfigError> {
    let mut cfg = ControlPlaneConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_controlplane_config_uses_defaults_without_file() {
        let config = load_controlplane_config(None).expect("config should load");
        assert_eq!(
            config.grpc_bind,
            DEFAULT_CONTROLPLANE_GRPC_BIND
                .parse::<SocketAddr>()
                .unwrap()
        );
        assert_eq!(
            config.http_bind,
            DEFAULT_CONTROLPLANE_HTTP_BIND
                .parse::<SocketAddr>()
                .unwrap()
        );
        assert_eq!(config.log_level, DEFAULT_CONTROLPLANE_LOG_LEVEL);
        assert_eq!(
            config.database.migrations_dir,
            PathBuf::from(DEFAULT_CONTROLPLANE_MIGRATIONS_DIR)
        );
        assert_eq!(
            config.database.max_connections,
            DEFAULT_CONTROLPLANE_DB_MAX_CONNECTIONS
        );
    }

    #[test]
    fn load_controlplane_config_reads_explicit_values() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("controlplane.toml");
        std::fs::write(
            &config_path,
            r#"
grpc_bind = "0.0.0.0:9443"
http_bind = "0.0.0.0:9080"
log_level = "debug"
runtime_dir = "/tmp/chv-controlplane"

[tls]
server_cert_path = "/tmp/server.crt"
server_key_path = "/tmp/server.key"
client_ca_path = "/tmp/ca.crt"

[database]
url = "postgres://example/chv"
migrations_dir = "custom/migrations"
max_connections = 32
min_connections = 2
acquire_timeout_secs = 7
idle_timeout_secs = 90
max_lifetime_secs = 1200
"#,
        )
        .expect("write config");

        let config = load_controlplane_config(Some(&config_path)).expect("config should load");
        assert_eq!(
            config.grpc_bind,
            "0.0.0.0:9443".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            config.http_bind,
            "0.0.0.0:9080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.runtime_dir, PathBuf::from("/tmp/chv-controlplane"));
        assert_eq!(config.database.url, "postgres://example/chv");
        assert_eq!(
            config.database.migrations_dir,
            PathBuf::from("custom/migrations")
        );
        assert_eq!(config.database.max_connections, 32);
        assert_eq!(config.database.min_connections, 2);
        assert_eq!(config.database.acquire_timeout_secs, 7);
        assert_eq!(config.database.idle_timeout_secs, 90);
        assert_eq!(config.database.max_lifetime_secs, 1200);
        assert_eq!(
            config.tls.server_cert_path,
            Some(PathBuf::from("/tmp/server.crt"))
        );
        assert_eq!(
            config.tls.server_key_path,
            Some(PathBuf::from("/tmp/server.key"))
        );
        assert_eq!(
            config.tls.client_ca_path,
            Some(PathBuf::from("/tmp/ca.crt"))
        );
    }
}
