use crate::config::MonitoringStoreConfig;
use crate::error::MonitoringStoreError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use std::time::Duration;

/// The monitoring store's own pool. Deliberately separate from the
/// control-plane store pool (ADR-027): monitoring work can never hold
/// operational DB locks, and this file's failure is monitoring
/// degradation, never lifecycle failure.
#[derive(Debug, Clone)]
pub struct MonitoringStore {
    pub(crate) pool: SqlitePool,
    pub(crate) config: MonitoringStoreConfig,
}

impl MonitoringStore {
    /// Connect, verify integrity, and run migrations. A corrupt or
    /// unwritable monitoring database is reported as an error the
    /// caller turns into a degraded signal — never a startup abort of
    /// the control plane itself.
    pub async fn connect(config: MonitoringStoreConfig) -> Result<Self, MonitoringStoreError> {
        let connect_options = SqliteConnectOptions::from_str(&config.database_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .pragma("foreign_keys", "ON")
            .pragma("mmap_size", "0")
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(connect_options)
            .await?;

        let store = MonitoringStore {
            pool,
            config: config.clone(),
        };

        // Quick corruption probe. Unlike the operational store, a failed
        // probe does NOT brick startup: telemetry is disposable, so the
        // caller may drop and recreate the file (see `reset_corrupt`).
        let result: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
            .fetch_one(&store.pool)
            .await?;
        if result != "ok" {
            return Err(MonitoringStoreError::Degraded {
                reason: format!("monitoring db integrity check failed: {result}"),
            });
        }

        let migrator = sqlx::migrate::Migrator::new(config.migrations_dir.as_path()).await?;
        migrator.run(&store.pool).await?;
        // Truncate the WAL after migrations so a fresh file starts small.
        let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&store.pool)
            .await;
        Ok(store)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn config(&self) -> &MonitoringStoreConfig {
        &self.config
    }

    /// Close the pool (used before a reset attempt).
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// The database file path derived from the configured URL, when the
    /// URL is a plain file path (the only supported shape).
    pub fn db_path(&self) -> Option<std::path::PathBuf> {
        self.config
            .database_url
            .strip_prefix("sqlite://")
            .map(std::path::PathBuf::from)
    }

    /// Current size of the main database file in bytes (WAL excluded —
    /// it is checkpointed by maintenance and bounded separately).
    pub async fn db_size_bytes(&self) -> Result<u64, MonitoringStoreError> {
        let (page_count, page_size): (i64, i64) = sqlx::query(
            "SELECT (SELECT page_count FROM pragma_page_count), (SELECT page_size FROM pragma_page_size)",
        )
        .map(|row: sqlx::sqlite::SqliteRow| (row.get::<i64, _>(0), row.get::<i64, _>(1)))
        .fetch_one(&self.pool)
        .await?;
        Ok(page_count.unsigned_abs() * page_size.unsigned_abs())
    }
}
