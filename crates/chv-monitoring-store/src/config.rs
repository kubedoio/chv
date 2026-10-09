use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_MIGRATIONS_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../cmd/chv-controlplane/monitoring-migrations"
);

/// Raw-sample retention (ADR-027 first-release target).
pub const DEFAULT_RAW_RETENTION_MS: u64 = 48 * 60 * 60 * 1000;
/// Five-minute rollup retention.
pub const DEFAULT_ROLLUP_5M_RETENTION_MS: u64 = 30 * 24 * 60 * 60 * 1000;
/// One-hour rollup retention.
pub const DEFAULT_ROLLUP_1H_RETENTION_MS: u64 = 180 * 24 * 60 * 60 * 1000;
/// Dedup proof retention (must exceed any plausible agent retry window).
pub const DEFAULT_DEDUP_RETENTION_MS: u64 = 24 * 60 * 60 * 1000;

/// Ingestion caps (ingestion contract v1 initial defaults).
pub const MAX_SAMPLES_PER_BATCH: usize = 512;
/// Per-target series cap (contract: 1024 per VM agent; one shared
/// default covers node and VM targets in v1).
pub const DEFAULT_MAX_SERIES_PER_TARGET: i64 = 1024;
/// Maximum accepted age of a sample's timestamp (live raw ingestion).
pub const MAX_SAMPLE_AGE_MS: u64 = 5 * 60 * 1000;
/// Maximum accepted future skew of a sample's timestamp.
pub const MAX_FUTURE_SKEW_MS: u64 = 2 * 60 * 1000;

/// How often the maintenance pass runs (rollups, retention, checkpoint).
pub const DEFAULT_MAINTENANCE_INTERVAL_MS: u64 = 60 * 1000;

/// Filesystem headroom floor: ingestion refuses new batches below this
/// and marks monitoring degraded (ADR-027: a separate file on the same
/// filesystem does NOT isolate disk-full risk).
pub const DEFAULT_MIN_HEADROOM_BYTES: u64 = 256 * 1024 * 1024;
/// Hard budget for the monitoring database file itself; maintenance
/// evicts oldest raw data first when exceeded.
pub const DEFAULT_MAX_DB_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Configuration for the isolated monitoring database. Defaults follow
/// ADR-027's first-release targets; they are adjustable only within the
/// enforced storage and query budgets.
#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringStoreConfig {
    /// SQLite URL, default `sqlite:///var/lib/chv/monitoring/monitoring.db`.
    pub database_url: String,
    #[serde(default = "default_migrations_dir")]
    pub migrations_dir: PathBuf,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_raw_retention_ms")]
    pub raw_retention_ms: u64,
    #[serde(default = "default_rollup_5m_retention_ms")]
    pub rollup_5m_retention_ms: u64,
    #[serde(default = "default_rollup_1h_retention_ms")]
    pub rollup_1h_retention_ms: u64,
    #[serde(default = "default_dedup_retention_ms")]
    pub dedup_retention_ms: u64,
    #[serde(default = "default_max_series_per_target")]
    pub max_series_per_target: i64,
    #[serde(default = "default_min_headroom_bytes")]
    pub min_headroom_bytes: u64,
    #[serde(default = "default_max_db_bytes")]
    pub max_db_bytes: u64,
}

impl Default for MonitoringStoreConfig {
    fn default() -> Self {
        MonitoringStoreConfig {
            database_url: "sqlite:///var/lib/chv/monitoring/monitoring.db".to_string(),
            migrations_dir: default_migrations_dir(),
            max_connections: default_max_connections(),
            raw_retention_ms: DEFAULT_RAW_RETENTION_MS,
            rollup_5m_retention_ms: DEFAULT_ROLLUP_5M_RETENTION_MS,
            rollup_1h_retention_ms: DEFAULT_ROLLUP_1H_RETENTION_MS,
            dedup_retention_ms: DEFAULT_DEDUP_RETENTION_MS,
            max_series_per_target: DEFAULT_MAX_SERIES_PER_TARGET,
            min_headroom_bytes: DEFAULT_MIN_HEADROOM_BYTES,
            max_db_bytes: DEFAULT_MAX_DB_BYTES,
        }
    }
}

fn default_migrations_dir() -> PathBuf {
    PathBuf::from(DEFAULT_MIGRATIONS_DIR)
}

fn default_max_connections() -> u32 {
    4
}

fn default_raw_retention_ms() -> u64 {
    DEFAULT_RAW_RETENTION_MS
}

fn default_rollup_5m_retention_ms() -> u64 {
    DEFAULT_ROLLUP_5M_RETENTION_MS
}

fn default_rollup_1h_retention_ms() -> u64 {
    DEFAULT_ROLLUP_1H_RETENTION_MS
}

fn default_dedup_retention_ms() -> u64 {
    DEFAULT_DEDUP_RETENTION_MS
}

fn default_max_series_per_target() -> i64 {
    DEFAULT_MAX_SERIES_PER_TARGET
}

fn default_min_headroom_bytes() -> u64 {
    DEFAULT_MIN_HEADROOM_BYTES
}

fn default_max_db_bytes() -> u64 {
    DEFAULT_MAX_DB_BYTES
}
