use rand::RngExt;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Multi-node configuration: Overlay and eBPF
// ---------------------------------------------------------------------------

/// VXLAN overlay network configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct OverlayConfig {
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
            nolearning: default_nolearning(),
            arp_suppress: false,
            inner_mtu: default_inner_mtu(),
        }
    }
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
}

impl Default for EbpfConfig {
    fn default() -> Self {
        Self {
            program_path: default_ebpf_program_path(),
            default_action: default_ebpf_action(),
        }
    }
}

fn default_ebpf_program_path() -> PathBuf {
    PathBuf::from("/usr/lib/chv/ebpf/")
}
fn default_ebpf_action() -> String {
    "deny".to_string()
}

fn generate_secure_secret() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

const SHARED_SECRET_PATH: &str = "/etc/chv/jwt_secret";

/// Creates/rewrites the shared secret file with owner-only permissions from
/// creation: a plain `fs::write` followed by a chmod leaves a umask-dependent
/// window where the fresh secret is group/world-readable.
#[cfg(unix)]
fn write_secret_private(path: &str, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_secret_private(path: &str, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

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
    if write_secret_private(SHARED_SECRET_PATH, &generated).is_ok() {
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

/// Fabric overlay (ADR-021) configuration for the network daemon.
///
/// When `enabled` is false (the default), `chv-nwd` runs bridge-only and every
/// fabric RPC fails closed. `state_dir` MUST be persistent (not under `/run`,
/// which is tmpfs): it holds the fabric ownership journal and the host's
/// WireGuard private key, which must survive reboots and fabric teardown.
#[derive(Debug, Clone, Deserialize)]
pub struct FabricNwdConfig {
    /// Master switch for the stretched-L2 fabric provider.
    #[serde(default)]
    pub enabled: bool,

    /// Durable state root (ownership journal, plan journal, WireGuard key).
    #[serde(default = "default_fabric_state_dir")]
    pub state_dir: PathBuf,

    /// Interface-name prefix for generated fabric objects (netns, WireGuard,
    /// veths). Bounded to 4 characters by the shared provider (IFNAMSIZ).
    #[serde(default = "default_fabric_name_prefix")]
    pub name_prefix: String,

    /// WireGuard listen port (shared Kubedo fabric convention).
    #[serde(default = "default_fabric_wireguard_port")]
    pub wireguard_port: u16,

    /// VXLAN destination port inside the tunnel.
    #[serde(default = "default_fabric_vxlan_port")]
    pub vxlan_port: u16,

    /// Tenant MTU used when a fabric plan omits it (underlay 1500 − 110 − 10).
    #[serde(default = "default_fabric_tenant_mtu")]
    pub default_tenant_mtu: u32,

    /// Fabric (WireGuard) MTU used when a fabric plan omits it.
    #[serde(default = "default_fabric_fabric_mtu")]
    pub default_fabric_mtu: u32,
}

impl Default for FabricNwdConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            state_dir: default_fabric_state_dir(),
            name_prefix: default_fabric_name_prefix(),
            wireguard_port: default_fabric_wireguard_port(),
            vxlan_port: default_fabric_vxlan_port(),
            default_tenant_mtu: default_fabric_tenant_mtu(),
            default_fabric_mtu: default_fabric_fabric_mtu(),
        }
    }
}

fn default_fabric_state_dir() -> PathBuf {
    PathBuf::from("/var/lib/chv/nwd/fabric")
}
fn default_fabric_name_prefix() -> String {
    "chv".to_string()
}
fn default_fabric_wireguard_port() -> u16 {
    65_001
}
fn default_fabric_vxlan_port() -> u16 {
    4789
}
fn default_fabric_tenant_mtu() -> u32 {
    1380
}
fn default_fabric_fabric_mtu() -> u32 {
    1440
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
    /// Storage migration configuration. `migration.enabled = true` requires
    /// mTLS identity material and validates it at startup (fail-closed).
    /// Disabled (default) means migration actions are unavailable rather than
    /// downgraded. See `docs/specs` migration ADR and issue #232.
    #[serde(default)]
    pub migration: StordMigrationConfig,
}

/// Storage migration configuration.
///
/// Explicitly distinguishes "migration disabled" (daemon may start without
/// credentials; migration actions fail unavailable) from "migration enabled but
/// broken" (missing/invalid TLS material is a startup error).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StordMigrationConfig {
    /// Master switch. When `true`, all identity fields below are required and
    /// validated at startup. When `false`, the daemon starts without migration
    /// credentials.
    #[serde(default)]
    pub enabled: bool,
    /// PEM client/node certificate path (issued by the CHV CA).
    #[serde(default)]
    pub client_cert_path: Option<PathBuf>,
    /// PEM client private key path.
    #[serde(default)]
    pub client_key_path: Option<PathBuf>,
    /// PEM CA bundle used to validate the migration destination.
    #[serde(default)]
    pub ca_cert_path: Option<PathBuf>,
    /// Expected destination server name used for certificate validation.
    /// Must match the destination certificate's DNS SAN / identity.
    #[serde(default)]
    pub dest_server_name: Option<String>,
    /// TCP address for the migration receiver mTLS listener (server half,
    /// issue #390), e.g. `"127.0.0.1:50052"`. Kept as a raw String here and
    /// parsed/validated by the fail-closed startup loader
    /// (`load_migration_server_tls`); no cross-field validation in this crate.
    /// Unset (default) means this stord is migration-source-only and never
    /// accepts inbound migrations.
    #[serde(default)]
    pub listen_addr: Option<String>,
    /// PEM server certificate path for the migration receiver listener
    /// (validated at startup, all-or-nothing with the other receiver fields).
    #[serde(default)]
    pub server_cert_path: Option<PathBuf>,
    /// PEM server private key path for the migration receiver listener.
    #[serde(default)]
    pub server_key_path: Option<PathBuf>,
    /// PEM CA bundle used to authenticate migration peers (client
    /// certificates) on the receiver listener.
    #[serde(default)]
    pub client_ca_path: Option<PathBuf>,
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
                PathBuf::from("/var/lib/chv/agent"),
            ],
            device_allowlist: vec!["/dev/dm-*".to_string(), "/dev/mapper/*".to_string()],
            metrics_bind: None,
            backend_type: None,
            migration_dest_allowlist: vec![],
            iscsi: None,
            ceph: None,
            lvm_volume_group: None,
            migration: StordMigrationConfig::default(),
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
    /// Stretched-L2 fabric (ADR-021) settings.
    #[serde(default)]
    pub fabric: FabricNwdConfig,
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
            fabric: FabricNwdConfig::default(),
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
    CoreManaged,
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
    /// Paths preserved in the supervisor-generated chv-stord.toml's
    /// `path_allowlist` when the agent respawns stord (#376). Empty omits
    /// the key — stord then allows all locator paths (the pre-#376
    /// behavior, kept as the default). Deployments relying on stord's
    /// path confinement set this to mirror their stord config (it must
    /// cover the volume locator dir, e.g. storage_base_dir, and any
    /// image/seed dirs the control plane references).
    #[serde(default)]
    pub stord_path_allowlist: Vec<PathBuf>,
    #[serde(default = "default_console_bind")]
    pub console_bind: String,
    #[serde(default = "default_agent_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default)]
    pub watchdog: BootWatchdogAgentConfig,
}

/// Node-level guest-liveness (boot) watchdog settings, from the agent
/// config's `[watchdog]` section. The watchdog detects the frozen-guest
/// condition CH v43's serial-manager defect produces (§ evidence:
/// a mid-burst serial-client death silently kills the manager; the guest
/// freezes mid-boot while `vm.info` keeps reporting Running) and recovers
/// it with `vm.reboot`, which also re-creates the serial manager and
/// restores console capture.
///
/// Detection is marker-based: a boot is complete when `boot_marker`
/// (default `systemd-logind`) appears in the VM's console capture after
/// the current boot's kernel banner (a re-spawned VMM or an agent-driven
/// reboot starts a fresh boot at a recorded byte offset — earlier
/// boots' markers never count). A console that has stalled (no new
/// bytes) with no marker on a Running VM triggers the reboot; a console
/// that wraps or truncates (the 10 MiB capture cap, a graceful stop)
/// never looks frozen on its own.
///
/// OPT-IN by design: a marker-based detector cannot distinguish a frozen
/// boot from a legitimately quiet guest whose image never prints the
/// marker (non-systemd/minimal images — set `boot_marker` accordingly or
/// leave the watchdog disabled), nor from an adopted long-running guest
/// whose console wrapped past its banner. Reboots are bounded by
/// `max_reboots` per unhealthy episode, and a declined or failed reboot
/// never consumes budget it did not earn.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BootWatchdogAgentConfig {
    /// Master switch. Default: disabled.
    #[serde(default)]
    pub enabled: bool,
    /// The boot-complete marker string searched for in the console
    /// capture after the most recent kernel banner. Default
    /// `systemd-logind` (present exactly twice per systemd boot:
    /// Starting + Started lines).
    #[serde(default = "default_watchdog_boot_marker")]
    pub boot_marker: String,
    /// Seconds of no new console bytes (with the boot-complete marker
    /// absent and the VMM alive) before the reboot fires. Default 120 —
    /// far above a healthy boot's quiet gaps on the reference stack.
    #[serde(default = "default_watchdog_stall_secs")]
    pub stall_secs: u64,
    /// Maximum watchdog reboots per unhealthy episode before standing
    /// down (operator territory). Default 2.
    #[serde(default = "default_watchdog_max_reboots")]
    pub max_reboots: u32,
    /// Seconds of continuous marker-healthy state after which the
    /// reboot budget resets. Default 900 (15 minutes).
    #[serde(default = "default_watchdog_healthy_reset_secs")]
    pub healthy_reset_secs: u64,
}

impl Default for BootWatchdogAgentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            boot_marker: default_watchdog_boot_marker(),
            stall_secs: default_watchdog_stall_secs(),
            max_reboots: default_watchdog_max_reboots(),
            healthy_reset_secs: default_watchdog_healthy_reset_secs(),
        }
    }
}

fn default_watchdog_boot_marker() -> String {
    "systemd-logind".to_string()
}

fn default_watchdog_stall_secs() -> u64 {
    120
}

fn default_watchdog_max_reboots() -> u32 {
    2
}

fn default_watchdog_healthy_reset_secs() -> u64 {
    900
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
            stord_path_allowlist: vec![],
            console_bind: default_console_bind(),
            jwt_secret: default_agent_jwt_secret(),
            watchdog: BootWatchdogAgentConfig::default(),
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
    // main.rs constructs ConsoleServer (the jwt_secret consumer) in legacy
    // and core-managed modes, so a default or short secret must not survive
    // config load in either. Core-native returns into run_core_native before
    // the console exists: no in-process consumer, so the configured value is
    // left untouched and no secret is minted on disk.
    if cfg.authority_mode != AgentAuthorityMode::CoreNative
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
    fn watchdog_section_parses_with_defaults_and_overrides() {
        let base = r#"
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
jwt_secret = "0123456789abcdef0123456789abcdef"
"#;
        // Without the section: disabled with the documented defaults —
        // existing agent.toml files are unaffected.
        let cfg = load_agent_config_from_str(base).expect("parse without section");
        assert!(!cfg.watchdog.enabled);
        assert_eq!(cfg.watchdog.boot_marker, "systemd-logind");
        assert_eq!(cfg.watchdog.stall_secs, 120);
        assert_eq!(cfg.watchdog.max_reboots, 2);
        assert_eq!(cfg.watchdog.healthy_reset_secs, 900);

        // With the section: every field overridable (a non-systemd
        // guest image needs a custom marker).
        let cfg = load_agent_config_from_str(&format!(
            "{base}\n[watchdog]\nenabled = true\nboot_marker = \"login:\"\nstall_secs = 45\nmax_reboots = 1\nhealthy_reset_secs = 300\n"
        ))
        .expect("parse with section");
        assert!(cfg.watchdog.enabled);
        assert_eq!(cfg.watchdog.boot_marker, "login:");
        assert_eq!(cfg.watchdog.stall_secs, 45);
        assert_eq!(cfg.watchdog.max_reboots, 1);
        assert_eq!(cfg.watchdog.healthy_reset_secs, 300);
    }

    fn load_agent_config_from_str(
        contents: &str,
    ) -> Result<AgentConfig, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("agent.toml");
        std::fs::write(&config_path, contents).expect("write config");
        Ok(load_agent_config(Some(&config_path))?)
    }

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
    fn core_managed_also_materializes_console_jwt_secret() {
        // Core-managed agents run the console server on this secret; a
        // default or short secret must not survive config load.
        let mut config = AgentConfig {
            authority_mode: AgentAuthorityMode::CoreManaged,
            jwt_secret: "short".to_owned(),
            ..AgentConfig::default()
        };
        materialize_agent_jwt_secret(&mut config);
        assert_eq!(config.authority_mode, AgentAuthorityMode::CoreManaged);
        assert!(config.jwt_secret.len() >= 32);
        assert_ne!(config.jwt_secret, "short");
    }

    #[test]
    fn nwd_config_fabric_defaults_and_overrides() {
        let defaulted = load_nwd_config(None).expect("default nwd config");
        assert!(!defaulted.fabric.enabled);
        assert_eq!(
            defaulted.fabric.state_dir,
            PathBuf::from("/var/lib/chv/nwd/fabric")
        );
        assert_eq!(defaulted.fabric.name_prefix, "chv");
        assert_eq!(defaulted.fabric.wireguard_port, 65001);
        assert_eq!(defaulted.fabric.vxlan_port, 4789);
        assert_eq!(defaulted.fabric.default_tenant_mtu, 1380);
        assert_eq!(defaulted.fabric.default_fabric_mtu, 1440);

        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("nwd.toml");
        std::fs::write(
            &config_path,
            r#"
socket_path = "/run/chv/nwd/api.sock"
runtime_dir = "/run/chv/nwd"
log_level = "info"

[fabric]
enabled = true
state_dir = "/var/lib/chv/nwd/fabric"
name_prefix = "chv"
wireguard_port = 65002
vxlan_port = 4790
default_tenant_mtu = 1370
default_fabric_mtu = 1430
"#,
        )
        .expect("write config");

        let cfg = load_nwd_config(Some(&config_path)).expect("nwd config");
        assert!(cfg.fabric.enabled);
        assert_eq!(cfg.fabric.wireguard_port, 65002);
        assert_eq!(cfg.fabric.vxlan_port, 4790);
        assert_eq!(cfg.fabric.default_tenant_mtu, 1370);
        assert_eq!(cfg.fabric.default_fabric_mtu, 1430);
    }

    #[test]
    fn core_native_does_not_materialize_unused_jwt_secret() {
        // Core-native returns into run_core_native before the console server
        // is constructed: there is no in-process consumer, so the configured
        // value is stored untouched and nothing is minted on disk.
        let mut config = AgentConfig {
            authority_mode: AgentAuthorityMode::CoreNative,
            jwt_secret: "short".to_owned(),
            ..AgentConfig::default()
        };
        materialize_agent_jwt_secret(&mut config);
        assert_eq!(config.authority_mode, AgentAuthorityMode::CoreNative);
        assert_eq!(config.jwt_secret, "short");
    }

    #[test]
    fn stord_migration_receiver_fields_parse_with_defaults_and_overrides() {
        let base = r#"
socket_path = "/run/chv/stord/api.sock"
runtime_dir = "/var/lib/chv/storage/localdisk"
log_level = "info"
"#;

        // Without any receiver fields (and even with the client half set):
        // the four receiver fields stay raw Option::None — validation is the
        // startup loader's job, not this crate's.
        let cfg = load_stord_config_from_str(base).expect("parse without receiver fields");
        assert!(!cfg.migration.enabled);
        assert_eq!(cfg.migration.listen_addr, None);
        assert_eq!(cfg.migration.server_cert_path, None);
        assert_eq!(cfg.migration.server_key_path, None);
        assert_eq!(cfg.migration.client_ca_path, None);

        // With receiver fields set: every field round-trips as configured
        // (listen_addr stays a String; it is parsed by the fail-closed
        // startup loader, so even an invalid value must parse here).
        let cfg = load_stord_config_from_str(
            r#"
socket_path = "/run/chv/stord/api.sock"
runtime_dir = "/var/lib/chv/storage/localdisk"
log_level = "info"

[migration]
enabled = true
client_cert_path = "/etc/chv/tls/stord-client.crt"
client_key_path = "/etc/chv/tls/stord-client.key"
ca_cert_path = "/etc/chv/tls/chv-ca.crt"
dest_server_name = "stord-peer.internal"
listen_addr = "127.0.0.1:50052"
server_cert_path = "/etc/chv/tls/stord-server.crt"
server_key_path = "/etc/chv/tls/stord-server.key"
client_ca_path = "/etc/chv/tls/chv-ca.crt"
"#,
        )
        .expect("parse with receiver fields");
        assert!(cfg.migration.enabled);
        assert_eq!(
            cfg.migration.listen_addr.as_deref(),
            Some("127.0.0.1:50052")
        );
        assert_eq!(
            cfg.migration.server_cert_path,
            Some(PathBuf::from("/etc/chv/tls/stord-server.crt"))
        );
        assert_eq!(
            cfg.migration.server_key_path,
            Some(PathBuf::from("/etc/chv/tls/stord-server.key"))
        );
        assert_eq!(
            cfg.migration.client_ca_path,
            Some(PathBuf::from("/etc/chv/tls/chv-ca.crt"))
        );
    }

    fn load_stord_config_from_str(
        contents: &str,
    ) -> Result<StordConfig, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("stord.toml");
        std::fs::write(&config_path, contents).expect("write config");
        Ok(load_stord_config(Some(&config_path))?)
    }
}
