use rand::Rng;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

fn generate_secure_secret() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid config: {0}")]
    Invalid(String),
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
            runtime_dir: PathBuf::from("/var/lib/chv/storage/localdisk"),
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
    #[serde(default = "default_storage_base_dir")]
    pub storage_base_dir: PathBuf,
    #[serde(default = "default_console_bind")]
    pub console_bind: String,
    #[serde(default = "default_agent_jwt_secret")]
    pub jwt_secret: String,
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
            storage_base_dir: PathBuf::from("/var/lib/chv/storage"),
            console_bind: default_console_bind(),
            jwt_secret: default_agent_jwt_secret(),
        }
    }
}

fn default_storage_base_dir() -> PathBuf {
    PathBuf::from("/var/lib/chv/storage")
}

fn default_console_bind() -> String {
    "127.0.0.1:8444".to_string()
}

fn default_agent_jwt_secret() -> String {
    "chv-dev-secret-change-in-production".to_string()
}

pub fn load_agent_config(path: Option<&Path>) -> Result<AgentConfig, ConfigError> {
    let mut cfg = AgentConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    if cfg.jwt_secret == "chv-dev-secret-change-in-production" || cfg.jwt_secret.len() < 32 {
        let generated = generate_secure_secret();
        eprintln!(
            "WARNING: jwt_secret not configured or too short. Auto-generated a secure secret for this session. \
             To persist, add to your agent config: jwt_secret = \"{}\"",
            generated
        );
        cfg.jwt_secret = generated;
    }
    Ok(cfg)
}

const DEFAULT_CONTROLPLANE_GRPC_BIND: &str = "127.0.0.1:8443";
const DEFAULT_CONTROLPLANE_HTTP_BIND: &str = "127.0.0.1:8080";
const DEFAULT_CONTROLPLANE_LOG_LEVEL: &str = "info";
const DEFAULT_CONTROLPLANE_RUNTIME_DIR: &str = "/run/chv/controlplane";
const DEFAULT_CONTROLPLANE_DATABASE_URL: &str = "sqlite:///var/lib/chv/controlplane.db";
const DEFAULT_CONTROLPLANE_MIGRATIONS_DIR: &str = "cmd/chv-controlplane/migrations";
const DEFAULT_CONTROLPLANE_DB_MAX_CONNECTIONS: u32 = 16;
const DEFAULT_CONTROLPLANE_DB_MIN_CONNECTIONS: u32 = 1;
const DEFAULT_CONTROLPLANE_DB_ACQUIRE_TIMEOUT_SECS: u64 = 5;
const DEFAULT_CONTROLPLANE_DB_IDLE_TIMEOUT_SECS: u64 = 300;
const DEFAULT_CONTROLPLANE_DB_MAX_LIFETIME_SECS: u64 = 1800;
const DEFAULT_CONTROLPLANE_AGENT_SOCKET_PATTERN: &str = "/run/chv/agent/api.sock";
const DEFAULT_CONTROLPLANE_KERNEL_PATH: &str = "/var/lib/chv/vmlinux";
const DEFAULT_CONTROLPLANE_FIRMWARE_PATH: &str = "/var/lib/chv/hypervisor-fw";

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
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default)]
    pub database: ControlPlaneDatabaseConfig,
    #[serde(default)]
    pub tls: ControlPlaneTlsConfig,
    #[serde(default = "default_agent_socket_pattern")]
    pub agent_socket_pattern: String,
    #[serde(default = "default_kernel_path")]
    pub kernel_path: String,
    #[serde(default = "default_firmware_path")]
    pub firmware_path: String,
}

fn default_jwt_secret() -> String {
    "chv-dev-secret-change-in-production".to_string()
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
            jwt_secret: default_jwt_secret(),
            database: ControlPlaneDatabaseConfig::default(),
            tls: ControlPlaneTlsConfig::default(),
            agent_socket_pattern: default_agent_socket_pattern(),
            kernel_path: default_kernel_path(),
            firmware_path: default_firmware_path(),
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

fn default_agent_socket_pattern() -> String {
    DEFAULT_CONTROLPLANE_AGENT_SOCKET_PATTERN.to_string()
}

fn default_kernel_path() -> String {
    DEFAULT_CONTROLPLANE_KERNEL_PATH.to_string()
}

fn default_firmware_path() -> String {
    DEFAULT_CONTROLPLANE_FIRMWARE_PATH.to_string()
}

pub fn load_controlplane_config(path: Option<&Path>) -> Result<ControlPlaneConfig, ConfigError> {
    let mut cfg = ControlPlaneConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    if cfg.jwt_secret == "chv-dev-secret-change-in-production" || cfg.jwt_secret.len() < 32 {
        let generated = generate_secure_secret();
        eprintln!(
            "WARNING: jwt_secret not configured or too short. Auto-generated a secure secret for this session. \
             To persist, add to your controlplane config: jwt_secret = \"{}\"",
            generated
        );
        cfg.jwt_secret = generated;
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_agent_config_auto_generates_secret_when_default() {
        let cfg = load_agent_config(None).expect("should succeed with auto-generated secret");
        assert_ne!(cfg.jwt_secret, "chv-dev-secret-change-in-production");
        assert!(cfg.jwt_secret.len() >= 32, "auto-generated secret should be at least 32 chars");
    }

    #[test]
    fn load_agent_config_auto_generates_secret_when_short() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("agent.toml");
        std::fs::write(
            &config_path,
            r#"
socket_path = "/run/chv/agent/api.sock"
runtime_dir = "/run/chv/agent"
log_level = "info"
control_plane_addr = "https://localhost:8443"
stord_socket = "/run/chv/stord/api.sock"
nwd_socket = "/run/chv/nwd/api.sock"
chv_binary_path = "/usr/bin/cloud-hypervisor"
stord_binary_path = "/usr/bin/chv-stord"
nwd_binary_path = "/usr/bin/chv-nwd"
cache_path = "/var/lib/chv/cache/agent-cache.json"
node_id = "test-node"
jwt_secret = "tooshort"
"#,
        )
        .expect("write config");

        let cfg = load_agent_config(Some(&config_path)).expect("should succeed with auto-generated secret");
        assert_ne!(cfg.jwt_secret, "tooshort");
        assert!(cfg.jwt_secret.len() >= 32);
    }

    #[test]
    fn load_controlplane_config_auto_generates_secret_when_default() {
        let cfg = load_controlplane_config(None).expect("should succeed with auto-generated secret");
        assert_ne!(cfg.jwt_secret, "chv-dev-secret-change-in-production");
        assert!(cfg.jwt_secret.len() >= 32);
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
jwt_secret = "a]Kx8v2mN!pR7qYsW3dF6gH9jL0nBcTe"

[tls]
server_cert_path = "/tmp/server.crt"
server_key_path = "/tmp/server.key"
client_ca_path = "/tmp/ca.crt"

[database]
url = "sqlite:///tmp/test.db"
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
        assert_eq!(config.database.url, "sqlite:///tmp/test.db");
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
