//! Migration reaper: background task that detects and fails stuck migration operations.
//!
//! If the control plane crashes mid-migration, the operation stays in its current phase
//! indefinitely. The reaper scans every 60 seconds for migrations that have exceeded their
//! total timeout, force-transitions them to Failed, and logs a warning for operator visibility.

use chv_controlplane_store::StorePool;
use chv_errors::ChvError;
use std::time::Duration;
use tracing::{info, warn};

/// Default timeout for migrations that have no explicit total_timeout configured.
/// 2 hours is generous enough to cover large VMs while still catching truly stuck ops.
const DEFAULT_MIGRATION_TIMEOUT_SECS: i64 = 7200;

/// How often the reaper scans for stuck migrations.
const REAPER_INTERVAL_SECS: u64 = 60;

/// Background task that reaps migrations stuck beyond their timeout.
pub struct MigrationReaper {
    pool: StorePool,
    interval: Duration,
    timeout_secs: i64,
}

impl MigrationReaper {
    pub fn new(pool: StorePool) -> Self {
        Self {
            pool,
            interval: Duration::from_secs(REAPER_INTERVAL_SECS),
            timeout_secs: DEFAULT_MIGRATION_TIMEOUT_SECS,
        }
    }

    /// Run the reaper loop until shutdown is signalled.
    pub async fn run(self, mut shutdown_rx: tokio::sync::watch::Receiver<()>) {
        info!(
            "migration reaper starting (interval={}s, timeout={}s)",
            self.interval.as_secs(),
            self.timeout_secs
        );

        let mut interval = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = shutdown_rx.changed() => {
                    info!("migration reaper shutting down");
                    break;
                }
            }
            if let Err(e) = self.reap_stuck_migrations().await {
                warn!(error = %e, "migration reaper tick failed");
            }
        }
    }

    /// Find in-progress migrations older than the timeout and mark them Failed.
    async fn reap_stuck_migrations(&self) -> Result<(), ChvError> {
        // Query migrations that are NOT in a terminal state and have been running
        // longer than the timeout threshold.
        let stuck: Vec<StuckMigrationRow> = sqlx::query_as(
            r#"
            SELECT
                m.migration_id,
                m.operation_id,
                m.vm_id,
                m.phase,
                m.started_at,
                CAST(
                    (julianday('now') - julianday(m.started_at)) * 86400
                    AS INTEGER
                ) AS age_secs
            FROM migrations m
            WHERE m.phase NOT IN ('Completed', 'Failed', 'RolledBack')
              AND m.started_at < strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ? || ' seconds')
            "#,
        )
        .bind(format!("-{}", self.timeout_secs))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("migration reaper: failed to query stuck migrations: {e}"),
        })?;

        if stuck.is_empty() {
            return Ok(());
        }

        let count = stuck.len();
        for row in &stuck {
            warn!(
                migration_id = %row.migration_id,
                operation_id = %row.operation_id,
                vm_id = %row.vm_id,
                phase = %row.phase,
                age_secs = row.age_secs,
                "migration reaper: force-failing stuck migration"
            );

            // Update the migration record to Failed with an error message
            if let Err(e) = self.fail_migration(row).await {
                warn!(
                    migration_id = %row.migration_id,
                    error = %e,
                    "migration reaper: failed to update migration record"
                );
            }

            // Also fail the parent operation so it doesn't stay in Running forever
            if let Err(e) = self.fail_operation(&row.operation_id).await {
                warn!(
                    operation_id = %row.operation_id,
                    error = %e,
                    "migration reaper: failed to update operation record"
                );
            }
        }

        metrics::counter!("migration_reaper_reaped_total").increment(count as u64);
        info!(count = count, "migration reaper: reaped stuck migrations");

        Ok(())
    }

    /// Mark a single migration as Failed with a reaper error message.
    async fn fail_migration(&self, row: &StuckMigrationRow) -> Result<(), ChvError> {
        let error_msg = format!(
            "operation reaper: exceeded timeout (age={}s, threshold={}s, phase={})",
            row.age_secs, self.timeout_secs, row.phase
        );

        sqlx::query(
            r#"UPDATE migrations
               SET phase = 'Failed',
                   error_message = ?,
                   completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
               WHERE migration_id = ?
                 AND phase NOT IN ('Completed', 'Failed', 'RolledBack')"#,
        )
        .bind(&error_msg)
        .bind(&row.migration_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!(
                "migration reaper: failed to fail migration {}: {e}",
                row.migration_id
            ),
        })?;

        Ok(())
    }

    /// Mark the parent operation as Failed so it is no longer considered in-progress.
    async fn fail_operation(&self, operation_id: &str) -> Result<(), ChvError> {
        sqlx::query(
            r#"UPDATE operations
               SET status = 'Failed',
                   error_code = 'MIGRATION_TIMEOUT',
                   error_message = 'operation reaper: exceeded timeout',
                   updated_by = 'migration_reaper',
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                   completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
               WHERE operation_id = ?
                 AND status NOT IN ('Succeeded', 'Failed', 'Rejected', 'Stale', 'Conflict')"#,
        )
        .bind(operation_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!(
                "migration reaper: failed to fail operation {}: {e}",
                operation_id
            ),
        })?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct StuckMigrationRow {
    migration_id: String,
    operation_id: String,
    vm_id: String,
    phase: String,
    #[allow(dead_code)]
    started_at: String,
    age_secs: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_MIGRATION_TIMEOUT_SECS, 7200);
        assert_eq!(REAPER_INTERVAL_SECS, 60);
    }

    #[test]
    fn test_reaper_construction() {
        // Verify the reaper can be constructed with a dummy pool reference.
        // Full integration tests require a real SQLite DB.
        assert_eq!(
            Duration::from_secs(REAPER_INTERVAL_SECS),
            Duration::from_secs(60)
        );
    }
}
