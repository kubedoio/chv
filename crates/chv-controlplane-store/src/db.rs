use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;

pub type StorePool = SqlitePool;

const DEFAULT_MAX_CONNECTIONS: u32 = 16;
const DEFAULT_ACQUIRE_TIMEOUT_SECS: u64 = 5;
const DEFAULT_MIGRATIONS_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../cmd/chv-controlplane/migrations"
);

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database connection error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("resource not found: {entity} (id: {id})")]
    NotFound { entity: &'static str, id: String },
    #[error("invalid store configuration: {reason}")]
    InvalidConfiguration { reason: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlPlaneStoreConfig {
    pub database_url: String,
    #[serde(default = "default_migrations_dir")]
    pub migrations_dir: PathBuf,

    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    #[serde(default = "default_acquire_timeout_secs")]
    pub acquire_timeout_secs: u64,
}

fn default_max_connections() -> u32 {
    DEFAULT_MAX_CONNECTIONS
}

fn default_acquire_timeout_secs() -> u64 {
    DEFAULT_ACQUIRE_TIMEOUT_SECS
}

fn default_migrations_dir() -> PathBuf {
    PathBuf::from(DEFAULT_MIGRATIONS_DIR)
}

pub fn build_connect_options(
    config: &ControlPlaneStoreConfig,
) -> Result<SqliteConnectOptions, StoreError> {
    Ok(SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true)
        .pragma("foreign_keys", "OFF"))
}

pub fn build_pool_options(config: &ControlPlaneStoreConfig) -> SqlitePoolOptions {
    SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
}

pub async fn connect_pool(config: &ControlPlaneStoreConfig) -> Result<StorePool, StoreError> {
    let connect_options = build_connect_options(config)?;
    let pool = build_pool_options(config)
        .connect_with(connect_options)
        .await?;
    Ok(pool)
}

pub fn migrations_path(config: Option<&ControlPlaneStoreConfig>) -> Cow<'_, Path> {
    match config {
        Some(config) => Cow::Borrowed(config.migrations_dir.as_path()),
        None => Cow::Borrowed(Path::new(DEFAULT_MIGRATIONS_DIR)),
    }
}

pub async fn migrator(
    config: Option<&ControlPlaneStoreConfig>,
) -> Result<sqlx::migrate::Migrator, StoreError> {
    Ok(sqlx::migrate::Migrator::new(migrations_path(config).as_ref()).await?)
}

pub async fn run_migrations(
    pool: &StorePool,
    config: Option<&ControlPlaneStoreConfig>,
) -> Result<(), StoreError> {
    let migrator = migrator(config).await?;
    migrator.run(pool).await?;
    Ok(())
}
