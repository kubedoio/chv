use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::borrow::Cow;
use std::collections::HashSet;
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
const BACKUP_DIR: &str = "/var/lib/chv/backups";
const MAX_BACKUPS: usize = 10;

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
        .pragma("foreign_keys", "ON"))
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

async fn has_pending_migrations(
    pool: &StorePool,
    migrator: &sqlx::migrate::Migrator,
) -> Result<bool, sqlx::Error> {
    let applied: Vec<i64> =
        sqlx::query_scalar::<sqlx::Sqlite, i64>("SELECT version FROM _sqlx_migrations")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let applied_set: HashSet<i64> = applied.into_iter().collect();

    for migration in migrator.iter() {
        if migration.migration_type.is_up_migration() && !applied_set.contains(&migration.version) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn rotate_backups(backup_dir: &Path, db_filename: &str, keep: usize) -> Result<(), std::io::Error> {
    let prefix = format!("{}.", db_filename);
    let suffix = ".bak";

    let mut entries: Vec<_> = std::fs::read_dir(backup_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(suffix)
        })
        .collect();

    if entries.len() <= keep {
        return Ok(());
    }

    entries.sort_by_key(|a| a.file_name());

    for entry in entries.iter().take(entries.len() - keep) {
        if let Err(e) = std::fs::remove_file(entry.path()) {
            tracing::warn!(
                path = %entry.path().display(),
                error = %e,
                "failed to remove old backup during rotation"
            );
        }
    }

    Ok(())
}

pub async fn run_migrations(
    pool: &StorePool,
    config: Option<&ControlPlaneStoreConfig>,
) -> Result<(), StoreError> {
    let migrator = migrator(config).await?;

    if let Some(config) = config {
        if config.database_url.starts_with("sqlite://") && !config.database_url.contains(":memory:")
        {
            let db_path = &config.database_url["sqlite://".len()..];

            let should_backup = match std::fs::metadata(db_path) {
                Ok(meta) => meta.len() > 0,
                Err(_) => false,
            };

            if should_backup {
                let has_pending = match has_pending_migrations(pool, &migrator).await {
                    Ok(pending) => pending,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "failed to check for pending migrations, assuming migrations are pending"
                        );
                        true
                    }
                };

                if has_pending {
                    let backup_dir = Path::new(BACKUP_DIR);
                    if let Err(e) = std::fs::create_dir_all(backup_dir) {
                        tracing::error!(
                            error = %e,
                            backup_dir = %backup_dir.display(),
                            "failed to create backup directory, continuing without backup"
                        );
                    } else {
                        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
                        let db_filename = Path::new(db_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("controlplane.db");
                        let backup_path =
                            backup_dir.join(format!("{}.{}.bak", db_filename, timestamp));

                        match std::fs::copy(db_path, &backup_path) {
                            Ok(_) => {
                                tracing::info!(
                                    backup_path = %backup_path.display(),
                                    "created pre-migration database backup"
                                );

                                if let Err(e) = rotate_backups(backup_dir, db_filename, MAX_BACKUPS)
                                {
                                    tracing::warn!(
                                        error = %e,
                                        "failed to rotate old backups"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "failed to create pre-migration backup of database, continuing"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    migrator.run(pool).await?;
    Ok(())
}
