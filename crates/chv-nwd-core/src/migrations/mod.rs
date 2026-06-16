//! Versioned schema migrations for the `chv-nwd` topology store.
//!
//! Replaces the previous `let _ = conn.execute("ALTER TABLE …")` pattern that
//! silently swallowed every error (including racing concurrent boots and any
//! genuine schema failure). Migrations are embedded at compile time, applied
//! in order inside a transaction, and tracked in a dedicated
//! `_chv_nwd_schema_version` table so future renames or rebuilds have a real
//! history to roll forward or back against (per ADR-007).
//!
//! ## In-place upgrade contract
//!
//! Real production databases were created by the prior unversioned code, which
//! means the `vni` and `peer_vteps` columns may already be present. The
//! `ALTER TABLE … ADD COLUMN` in migrations 0002 and 0003 would fail on those
//! databases because SQLite has no `ADD COLUMN IF NOT EXISTS`. Each migration
//! therefore exposes a `precondition` hook that inspects `PRAGMA table_info`
//! and skips the SQL body when the column already exists, while still
//! recording the version so subsequent boots are idempotent.
//!
//! ## Concurrency
//!
//! Two processes booting at once will both attempt to create the version
//! table and acquire a write transaction. SQLite serializes those, and the
//! version check inside the transaction ensures the loser becomes a no-op
//! rather than swallowing a "duplicate column" error.

use chv_errors::ChvError;
use rusqlite::{Connection, OptionalExtension};
use tracing::{debug, info};

/// Embedded migration SQL. Keep entries append-only and in version order.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_topologies",
        sql: include_str!("../../migrations/0001_initial_topologies.sql"),
        precondition: None,
    },
    Migration {
        version: 2,
        name: "add_vni_column",
        sql: include_str!("../../migrations/0002_add_vni_column.sql"),
        precondition: Some(column_absent_topologies_vni),
    },
    Migration {
        version: 3,
        name: "add_peer_vteps_column",
        sql: include_str!("../../migrations/0003_add_peer_vteps_column.sql"),
        precondition: Some(column_absent_topologies_peer_vteps),
    },
];

/// Static registry name for the version-tracking table.
const VERSION_TABLE: &str = "_chv_nwd_schema_version";

/// Function pointer signature for migration preconditions.
///
/// Returns `Ok(true)` when the migration body should run, `Ok(false)` when
/// the desired state is already satisfied (in which case the version row is
/// still inserted), or `Err(_)` when the inspection itself failed.
type Precondition = fn(&Connection) -> Result<bool, ChvError>;

pub(crate) struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
    precondition: Option<Precondition>,
}

fn column_absent_topologies_vni(conn: &Connection) -> Result<bool, ChvError> {
    column_absent_runtime(conn, "topologies", "vni")
}

fn column_absent_topologies_peer_vteps(conn: &Connection) -> Result<bool, ChvError> {
    column_absent_runtime(conn, "topologies", "peer_vteps")
}

fn column_absent_runtime(conn: &Connection, table: &str, column: &str) -> Result<bool, ChvError> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| ChvError::Internal {
            reason: format!(
                "nwd migration: PRAGMA table_info({}) prepare failed: {}",
                table, e
            ),
        })?;
    let mut rows = stmt.query([]).map_err(|e| ChvError::Internal {
        reason: format!(
            "nwd migration: PRAGMA table_info({}) query failed: {}",
            table, e
        ),
    })?;
    while let Some(row) = rows.next().map_err(|e| ChvError::Internal {
        reason: format!(
            "nwd migration: PRAGMA table_info({}) row failed: {}",
            table, e
        ),
    })? {
        let existing: String = row.get(1).map_err(|e| ChvError::Internal {
            reason: format!(
                "nwd migration: PRAGMA table_info({}) name read failed: {}",
                table, e
            ),
        })?;
        if existing == column {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Apply every embedded migration that has not yet been recorded in the
/// version table. Safe to call on every boot; idempotent on a fully-migrated
/// database; and idempotent on databases that were upgraded in-place by the
/// prior unversioned ALTER TABLE statements (because the precondition hooks
/// detect the existing columns and skip the SQL body).
pub fn run(conn: &mut Connection) -> Result<(), ChvError> {
    run_with(conn, MIGRATIONS)
}

/// Apply a caller-supplied migration vector. Reserved for tests that need to
/// inject a faulty migration to prove errors propagate instead of being
/// swallowed; production code calls [`run`].
#[doc(hidden)]
pub(crate) fn run_with(conn: &mut Connection, migrations: &[Migration]) -> Result<(), ChvError> {
    ensure_version_table(conn)?;

    for migration in migrations {
        let already_applied = is_applied(conn, migration.version)?;
        if already_applied {
            debug!(
                version = migration.version,
                name = migration.name,
                "nwd migration already applied; skipping"
            );
            continue;
        }

        info!(
            version = migration.version,
            name = migration.name,
            "applying nwd migration"
        );

        let tx = conn.transaction().map_err(|e| ChvError::Internal {
            reason: format!(
                "nwd migration {}: begin tx failed: {}",
                migration.version, e
            ),
        })?;

        let should_run = match migration.precondition {
            Some(check) => check(&tx)?,
            None => true,
        };

        if should_run {
            tx.execute_batch(migration.sql)
                .map_err(|e| ChvError::Internal {
                    reason: format!(
                        "nwd migration {} ({}): execute failed: {}",
                        migration.version, migration.name, e
                    ),
                })?;
        } else {
            debug!(
                version = migration.version,
                name = migration.name,
                "precondition false; recording version without executing body"
            );
        }

        tx.execute(
            &format!(
                "INSERT INTO {} (version, name) VALUES (?1, ?2)",
                VERSION_TABLE
            ),
            rusqlite::params![migration.version, migration.name],
        )
        .map_err(|e| ChvError::Internal {
            reason: format!(
                "nwd migration {} ({}): version insert failed: {}",
                migration.version, migration.name, e
            ),
        })?;

        tx.commit().map_err(|e| ChvError::Internal {
            reason: format!(
                "nwd migration {} ({}): commit failed: {}",
                migration.version, migration.name, e
            ),
        })?;
    }

    Ok(())
}

fn ensure_version_table(conn: &Connection) -> Result<(), ChvError> {
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {} (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            VERSION_TABLE
        ),
        [],
    )
    .map_err(|e| ChvError::Internal {
        reason: format!("nwd migration: version table create failed: {}", e),
    })?;
    Ok(())
}

fn is_applied(conn: &Connection, version: i64) -> Result<bool, ChvError> {
    let row: Option<i64> = conn
        .query_row(
            &format!("SELECT version FROM {} WHERE version = ?1", VERSION_TABLE),
            rusqlite::params![version],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| ChvError::Internal {
            reason: format!("nwd migration: version lookup ({}) failed: {}", version, e),
        })?;
    Ok(row.is_some())
}

/// Read back every migration version recorded in `_chv_nwd_schema_version`.
/// Used by tests; not part of the public API for callers outside this crate.
#[doc(hidden)]
pub fn applied_versions(conn: &Connection) -> Result<Vec<i64>, ChvError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT version FROM {} ORDER BY version",
            VERSION_TABLE
        ))
        .map_err(|e| ChvError::Internal {
            reason: format!("nwd migration: applied_versions prepare failed: {}", e),
        })?;
    let rows = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .map_err(|e| ChvError::Internal {
            reason: format!("nwd migration: applied_versions query failed: {}", e),
        })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| ChvError::Internal {
            reason: format!("nwd migration: applied_versions row failed: {}", e),
        })?);
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::Migration;

    /// Hidden constructor so tests in the parent crate can build a faulty
    /// migration vector without `Migration` becoming part of the public API.
    pub(crate) fn migration(version: i64, name: &'static str, sql: &'static str) -> Migration {
        Migration {
            version,
            name,
            sql,
            precondition: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_db_runs_every_migration() {
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        run(&mut conn).expect("migrate fresh");
        let versions = applied_versions(&conn).expect("read versions");
        assert_eq!(versions, vec![1, 2, 3]);
    }

    #[test]
    fn second_run_is_a_noop() {
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        run(&mut conn).expect("first run");
        run(&mut conn).expect("second run must be idempotent");
        let versions = applied_versions(&conn).expect("read versions");
        assert_eq!(versions, vec![1, 2, 3]);
    }

    #[test]
    fn precondition_skips_alter_when_columns_already_exist() {
        // Simulate a DB that was upgraded in-place by the prior unversioned
        // ALTER TABLE pattern: the columns are already present, but
        // _chv_nwd_schema_version does not exist yet.
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        conn.execute(
            "CREATE TABLE topologies (
                network_id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                bridge_name TEXT NOT NULL,
                namespace_name TEXT NOT NULL,
                subnet_cidr TEXT NOT NULL,
                gateway_ip TEXT NOT NULL,
                runtime_status TEXT NOT NULL,
                vni INTEGER,
                peer_vteps TEXT DEFAULT '[]'
            )",
            [],
        )
        .expect("seed legacy table");

        run(&mut conn).expect("migrate legacy db");
        let versions = applied_versions(&conn).expect("read versions");
        assert_eq!(versions, vec![1, 2, 3]);
    }
}
