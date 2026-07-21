pub mod cert_watcher;

use rand::RngExt;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Multi-node configuration: Overlay, eBPF, and Migration
// ---------------------------------------------------------------------------

/// VXLAN overlay network configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct OverlayConfig {
    /// UDP port used for VXLAN encapsulation.
    #[serde(default = "default_vxlan_port")]
    pub vxlan_port: u16,

    /// Interface used as the VTEP endpoint. "auto" selects the default route interface.
    #[serde(default = "default_vtep_interface")]
    pub vtep_interface: String,

    /// Disable MAC learning on VXLAN interfaces (use explicit FDB entries only).
    #[serde(default = "default_nolearning")]
    pub nolearning: bool,

    /// Enable ARP suppression on the VXLAN interface.
    #[serde(default)]
    pub arp_suppress: bool,

    /// Inner MTU for VXLAN traffic. "auto" calculates from outer MTU minus overhead.
    #[serde(default = "default_inner_mtu")]
    pub inner_mtu: String,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            vxlan_port: default_vxlan_port(),
            vtep_interface: default_vtep_interface(),
            nolearning: default_nolearning(),
            arp_suppress: false,
            inner_mtu: default_inner_mtu(),
        }
    }
}

fn default_vxlan_port() -> u16 {
    4789
}
fn default_vtep_interface() -> String {
    "auto".to_string()
}
fn default_nolearning() -> bool {
    true
}
fn default_inner_mtu() -> String {
    "auto".to_string()
}

/// eBPF policy engine configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct EbpfConfig {
    /// Directory containing compiled eBPF object files (.o).
    #[serde(default = "default_ebpf_program_path")]
    pub program_path: PathBuf,

    /// Default action when no rule matches: "deny" or "allow".
    #[serde(default = "default_ebpf_action")]
    pub default_action: String,

    /// Interval in seconds between eBPF stats collection cycles.
    #[serde(default = "default_ebpf_stats_interval_secs")]
    pub stats_interval_secs: u64,
}

impl Default for EbpfConfig {
    fn default() -> Self {
        Self {
            program_path: default_ebpf_program_path(),
            default_action: default_ebpf_action(),
            stats_interval_secs: default_ebpf_stats_interval_secs(),
        }
    }
}

fn default_ebpf_program_path() -> PathBuf {
    PathBuf::from("/usr/lib/chv/ebpf/")
}
fn default_ebpf_action() -> String {
    "deny".to_string()
}
fn default_ebpf_stats_interval_secs() -> u64 {
    10
}

/// Live migration tuning parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct MigrationTuningConfig {
    /// Dirty block threshold below which convergence is considered achieved.
    #[serde(default = "default_dirty_threshold_blocks")]
    pub dirty_threshold_blocks: u32,

    /// Maximum number of convergence rounds before aborting.
    #[serde(default = "default_max_convergence_rounds")]
    pub max_convergence_rounds: u32,

    /// Block size in bytes for disk copy operations.
    #[serde(default = "default_block_size_bytes")]
    pub block_size_bytes: u32,

    /// Port range used for memory migration data transfer (e.g. "49152-49200").
    #[serde(default = "default_memory_migration_port_range")]
    pub memory_migration_port_range: String,

    /// Multiplier applied to calculated timeouts for total migration budget.
    #[serde(default = "default_total_timeout_multiplier")]
    pub total_timeout_multiplier: f64,
}

impl Default for MigrationTuningConfig {
    fn default() -> Self {
        Self {
            dirty_threshold_blocks: default_dirty_threshold_blocks(),
            max_convergence_rounds: default_max_convergence_rounds(),
            block_size_bytes: default_block_size_bytes(),
            memory_migration_port_range: default_memory_migration_port_range(),
            total_timeout_multiplier: default_total_timeout_multiplier(),
        }
    }
}

fn default_dirty_threshold_blocks() -> u32 {
    1024
}
fn default_max_convergence_rounds() -> u32 {
    10
}
fn default_block_size_bytes() -> u32 {
    4_194_304
}
fn default_memory_migration_port_range() -> String {
    "49152-49200".to_string()
}
fn default_total_timeout_multiplier() -> f64 {
    1.5
}

fn generate_secure_secret() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

const SHARED_SECRET_PATH: &str = "/etc/chv/jwt_secret";

fn resolve_jwt_secret(current: &str, service_name: &str) -> String {
    if current != "chv-dev-secret-change-in-production" && current.len() >= 32 {
        return current.to_string();
    }
    // Check CHV_JWT_SECRET env var first
    if let Ok(env_secret) = std::env::var("CHV_JWT_SECRET") {
        if env_secret.len() >= 32 {
            tracing::info!("loaded jwt_secret from CHV_JWT_SECRET env var");
            return env_secret;
        }
    }
    if current == "chv-dev-secret-change-in-production" {
        tracing::error!(
            service = service_name,
            "SECURITY: jwt_secret is set to the known default value. \
             This is insecure — set a unique jwt_secret (>= 32 chars) in the {} config or CHV_JWT_SECRET env var. \
             Auto-generating a random secret for this session.",
            service_name
        );
    }
    if let Ok(secret) = std::fs::read_to_string(SHARED_SECRET_PATH) {
        let secret = secret.trim().to_string();
        if secret.len() >= 32 {
            tracing::info!("loaded jwt_secret from {}", SHARED_SECRET_PATH);
            return secret;
        }
    }
    let generated = generate_secure_secret();
    if std::fs::write(SHARED_SECRET_PATH, &generated).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                SHARED_SECRET_PATH,
                std::fs::Permissions::from_mode(0o600),
            );
        }
        tracing::warn!(
            "auto-generated jwt_secret and saved to {} (shared by all CHV services). \
             For production, configure an explicit jwt_secret.",
            SHARED_SECRET_PATH
        );
    } else {
        tracing::error!(
            "auto-generated jwt_secret but could not write to {}. \
             Each service will generate its own secret — tokens will NOT be portable between services. \
             Set jwt_secret explicitly in {} config.",
            SHARED_SECRET_PATH, service_name
        );
    }
    generated
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
    #[serde(default)]
    pub path_allowlist: Vec<PathBuf>,
    #[serde(default)]
    pub device_allowlist: Vec<String>,
    pub metrics_bind: Option<String>,
    /// Storage backend type: "local" (default), "iscsi", "ceph", or "lvm".
    #[serde(default)]
    pub backend_type: Option<String>,
    /// Allowed migration destination hosts. Empty = allow all.
    #[serde(default)]
    pub migration_dest_allowlist: Vec<String>,
    /// iSCSI backend configuration (required when backend_type = "iscsi").
    #[serde(default)]
    pub iscsi: Option<StordIscsiConfig>,
    /// Ceph RBD backend configuration (required when backend_type = "ceph").
    #[serde(default)]
    pub ceph: Option<StordCephConfig>,
    /// LVM backend configuration (required when backend_type = "lvm").
    /// The value is the volume group name.
    #[serde(default)]
    pub lvm_volume_group: Option<String>,
}

/// iSCSI backend configuration embedded in StordConfig.
#[derive(Debug, Clone, Deserialize)]
pub struct StordIscsiConfig {
    pub portal: String,
    pub target_iqn: String,
    pub initiator_name: String,
    pub chap_username: Option<String>,
    pub chap_secret: Option<String>,
}

/// Ceph RBD backend configuration embedded in StordConfig.
#[derive(Debug, Clone, Deserialize)]
pub struct StordCephConfig {
    #[serde(default = "default_ceph_cluster_name")]
    pub cluster_name: String,
    pub pool_name: String,
    pub user: String,
    pub keyring_path: String,
    pub monitors: String,
}

fn default_ceph_cluster_name() -> String {
    "ceph".to_string()
}

impl Default for StordConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/stord/api.sock"),
            runtime_dir: PathBuf::from("/var/lib/chv/storage/localdisk"),
            log_level: "info".to_string(),
            backend_allowlist: vec![],
            path_allowlist: vec![
                PathBuf::from("/var/lib/chv/storage/localdisk"),
                PathBuf::from("/var/lib/chv/storage/lvm"),
            ],
            device_allowlist: vec!["/dev/dm-*".to_string(), "/dev/mapper/*".to_string()],
            metrics_bind: None,
            backend_type: None,
            migration_dest_allowlist: vec![],
            iscsi: None,
            ceph: None,
            lvm_volume_group: None,
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
    /// VXLAN overlay network settings.
    #[serde(default)]
    pub overlay: OverlayConfig,
    /// eBPF policy engine settings.
    #[serde(default)]
    pub ebpf: EbpfConfig,
}

impl Default for NwdConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/nwd/api.sock"),
            runtime_dir: PathBuf::from("/run/chv/nwd"),
            log_level: "info".to_string(),
            metrics_bind: None,
            overlay: OverlayConfig::default(),
            ebpf: EbpfConfig::default(),
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

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentAuthorityMode {
    #[default]
    Legacy,
    CoreNative,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub authority_mode: AgentAuthorityMode,
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
    #[serde(default = "default_core_store_path")]
    pub core_store_path: PathBuf,
    #[serde(default = "default_core_api_socket_path")]
    pub core_api_socket_path: PathBuf,
    #[serde(default = "default_core_archive_path")]
    pub core_archive_path: PathBuf,
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
            authority_mode: AgentAuthorityMode::Legacy,
            socket_path: PathBuf::from("/run/chv/agent/api.sock"),
            runtime_dir: PathBuf::from("/var/lib/chv/agent"),
            log_level: "info".to_string(),
            control_plane_addr: "https://localhost:8443".to_string(),
            stord_socket: PathBuf::from("/run/chv/stord/api.sock"),
            nwd_socket: PathBuf::from("/run/chv/nwd/api.sock"),
            chv_binary_path: PathBuf::from("/usr/bin/cloud-hypervisor"),
            stord_binary_path: PathBuf::from("/usr/bin/chv-stord"),
            nwd_binary_path: PathBuf::from("/usr/bin/chv-nwd"),
            cache_path: PathBuf::from("/var/lib/chv/cache/agent-cache.json"),
            core_store_path: default_core_store_path(),
            core_api_socket_path: default_core_api_socket_path(),
            core_archive_path: default_core_archive_path(),
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

fn default_core_store_path() -> PathBuf {
    PathBuf::from("/var/lib/chv/agent/core.db")
}

fn default_core_api_socket_path() -> PathBuf {
    PathBuf::from("/run/chv/core/core-v1.sock")
}

fn default_core_archive_path() -> PathBuf {
    PathBuf::from("/var/lib/chv/agent/node-cache-v1.archive")
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
    materialize_agent_jwt_secret(&mut cfg);
    Ok(cfg)
}

fn materialize_agent_jwt_secret(cfg: &mut AgentConfig) {
    if cfg.authority_mode == AgentAuthorityMode::Legacy
        && (cfg.jwt_secret == "chv-dev-secret-change-in-production" || cfg.jwt_secret.len() < 32)
    {
        cfg.jwt_secret = resolve_jwt_secret(&cfg.jwt_secret, "agent");
    }
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
    #[serde(default = "default_agent_runtime_dir")]
    pub agent_runtime_dir: PathBuf,
    #[serde(default = "default_kernel_path")]
    pub kernel_path: String,
    #[serde(default = "default_firmware_path")]
    pub firmware_path: String,
    /// VXLAN overlay network defaults for cluster-wide behavior.
    #[serde(default)]
    pub overlay: OverlayConfig,
    /// Live migration tuning parameters.
    #[serde(default)]
    pub migration: MigrationTuningConfig,
}

fn default_jwt_secret() -> String {
    "chv-dev-secret-change-in-production".to_string()
}

fn default_agent_runtime_dir() -> PathBuf {
    PathBuf::from("/var/lib/chv/agent")
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
            agent_runtime_dir: default_agent_runtime_dir(),
            kernel_path: default_kernel_path(),
            firmware_path: default_firmware_path(),
            overlay: OverlayConfig::default(),
            migration: MigrationTuningConfig::default(),
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
        cfg.jwt_secret = resolve_jwt_secret(&cfg.jwt_secret, "controlplane");
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
        assert!(
            cfg.jwt_secret.len() >= 32,
            "auto-generated secret should be at least 32 chars"
        );
    }

    #[test]
    fn load_agent_config_auto_generates_secret_when_short() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("agent.toml");
        std::fs::write(
            &config_path,
            r#"
socket_path = "/run/chv/agent/api.sock"
runtime_dir = "/var/lib/chv/agent"
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

        let cfg = load_agent_config(Some(&config_path))
            .expect("should succeed with auto-generated secret");
        assert_ne!(cfg.jwt_secret, "tooshort");
        assert!(cfg.jwt_secret.len() >= 32);
        assert_eq!(
            cfg.core_store_path,
            PathBuf::from("/var/lib/chv/agent/core.db")
        );
        assert_eq!(
            cfg.core_api_socket_path,
            PathBuf::from("/run/chv/core/core-v1.sock")
        );
    }

    #[test]
    fn load_controlplane_config_auto_generates_secret_when_default() {
        let cfg =
            load_controlplane_config(None).expect("should succeed with auto-generated secret");
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

    #[test]
    fn agent_authority_mode_is_strict_and_defaults_legacy() {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(default)]
            authority_mode: AgentAuthorityMode,
        }
        let defaulted: Wrapper = toml::from_str("").unwrap();
        assert_eq!(defaulted.authority_mode, AgentAuthorityMode::Legacy);
        let native: Wrapper = toml::from_str("authority_mode = 'core-native'").unwrap();
        assert_eq!(native.authority_mode, AgentAuthorityMode::CoreNative);
        assert!(toml::from_str::<Wrapper>("authority_mode = 'core'").is_err());
    }

    #[test]
    fn core_native_does_not_materialize_unused_jwt_secret() {
        let mut config = AgentConfig {
            authority_mode: AgentAuthorityMode::CoreNative,
            jwt_secret: "short".to_owned(),
            ..AgentConfig::default()
        };
        materialize_agent_jwt_secret(&mut config);
        assert_eq!(config.authority_mode, AgentAuthorityMode::CoreNative);
        assert_eq!(config.jwt_secret, "short");
    }
}
