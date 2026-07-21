//! Durable SQLite authority beneath the existing `chv-agent` runtime.
//!
//! This crate performs no provider or VMM side effects. Existing databases are
//! opened fail-closed: corruption, unknown schema versions, and altered
//! migration checksums are errors and never cause replacement or re-creation.

use cellhv_core_types::{
    canonical_json as domain_canonical_json, canonical_request_fingerprint, HostCapabilities,
    HostId, HostIdentity, IdempotencyKey, ObservedPowerState, Operation, OperationEvent,
    OperationId, OperationKind, OperationStatus, OperationStep, OwnershipMarker,
    RequestedPowerState, ResourceVersion, VmDefinition, VmId,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use thiserror::Error;

const APPLICATION_ID: i32 = 0x4348_5643; // CHVC
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MIGRATION_SQL: &str = include_str!("../migrations/0001_core_authority.sql");
const EXECUTION_FENCING_MIGRATION_SQL: &str =
    include_str!("../migrations/0002_operation_execution_fencing.sql");
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("core store already exists: {0}")]
    AlreadyExists(PathBuf),
    #[error("core store does not exist: {0}")]
    Missing(PathBuf),
    #[error("core store failed integrity validation: {0}")]
    Integrity(String),
    #[error("core store schema is incompatible: {0}")]
    Schema(String),
    #[error("resource {kind} {id} was not found")]
    NotFound { kind: &'static str, id: String },
    #[error("resource {kind} {id} already exists")]
    Conflict { kind: &'static str, id: String },
    #[error("stale resource version for {kind} {id}: expected {expected}, actual {actual}")]
    StaleVersion {
        kind: &'static str,
        id: String,
        expected: u64,
        actual: u64,
    },
    #[error("idempotency key {key} in scope {scope} has a different request fingerprint")]
    IdempotencyConflict { scope: String, key: String },
    #[error("invalid Core domain value: {0}")]
    InvalidDomain(String),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON operation failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRecord {
    pub identity: HostIdentity,
    pub capabilities: HostCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceptance {
    Accepted,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDisposition {
    Imported,
    Replay,
    Cutover,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedOperation {
    pub disposition: Acceptance,
    pub operation: Operation,
    pub accepted_resource_version: ResourceVersion,
}

#[derive(Debug)]
pub struct AcceptOperation<'a> {
    pub operation: &'a Operation,
    /// Canonical, platform-neutral mutation request retained for replay/audit.
    pub request: &'a serde_json::Value,
    /// Complete desired VM state at the accepted resource version. Required
    /// for create/update/power mutations and absent for delete.
    pub desired_vm: Option<&'a VmDefinition>,
    pub idempotency_scope: &'a str,
    pub idempotency_key: &'a IdempotencyKey,
    pub expected_vm_version: ResourceVersion,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OperationJournalEntry {
    pub operation: Operation,
    pub request: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimDisposition {
    Acquired,
    Replay,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaimedOperation {
    pub disposition: ClaimDisposition,
    pub entry: OperationJournalEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IncompleteExecutionOperation {
    pub entry: OperationJournalEntry,
    pub active_attempt_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionDisposition {
    Applied,
    Replay,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletedOperation {
    pub disposition: CompletionDisposition,
    pub entry: OperationJournalEntry,
}

struct StoredOperationColumns {
    request: String,
    result: Option<String>,
    error: Option<String>,
    active_attempt_token: Option<String>,
    completed_attempt_token: Option<String>,
    completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMigrationState {
    pub source: String,
    pub checksum: String,
    pub cutover: bool,
}

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "core_authority",
        sql: MIGRATION_SQL,
    },
    Migration {
        version: 2,
        name: "operation_execution_fencing",
        sql: EXECUTION_FENCING_MIGRATION_SQL,
    },
];

pub struct CoreStore {
    conn: Connection,
}

impl CoreStore {
    pub fn has_any_migration_state(&self) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM migration_state)", [], |row| {
                row.get::<_, i64>(0)
            })?
            != 0)
    }

    pub fn is_pristine_migration_target(&self) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT (SELECT count(*) FROM host_identity) + (SELECT count(*) FROM vms) + (SELECT count(*) FROM attachments) + (SELECT count(*) FROM operations) + (SELECT count(*) FROM operation_steps) + (SELECT count(*) FROM idempotency_keys) + (SELECT count(*) FROM events) + (SELECT count(*) FROM ownership_markers) + (SELECT count(*) FROM migration_state)",
            [],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }
    pub fn legacy_migration_state(&self, source: &str) -> Result<Option<LegacyMigrationState>> {
        self.conn
            .query_row(
                "SELECT source,source_checksum,state FROM migration_state WHERE source=?1",
                [source],
                |row| {
                    let state: String = row.get(2)?;
                    Ok(LegacyMigrationState {
                        source: row.get(0)?,
                        checksum: row.get(1)?,
                        cutover: state == "cutover",
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Imports a validated legacy snapshot without changing the source file.
    /// The marker, host identity, and VM definitions commit atomically.
    pub fn import_legacy_snapshot(
        &mut self,
        source: &str,
        source_checksum: &str,
        host: &HostIdentity,
        capabilities: &HostCapabilities,
        definitions: &[VmDefinition],
    ) -> Result<MigrationDisposition> {
        if source.trim().is_empty() || source_checksum.trim().is_empty() {
            return Err(StoreError::InvalidDomain(
                "migration source and checksum must not be empty".to_owned(),
            ));
        }
        for definition in definitions {
            validate_definition(definition)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT state,source_checksum FROM migration_state WHERE source=?1",
                [source],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((state, checksum)) = existing {
            if checksum != source_checksum {
                return Err(StoreError::Conflict {
                    kind: "migration",
                    id: source.to_owned(),
                });
            }
            let disposition = match state.as_str() {
                "imported" => MigrationDisposition::Replay,
                "cutover" => MigrationDisposition::Cutover,
                _ => {
                    return Err(StoreError::Conflict {
                        kind: "migration",
                        id: source.to_owned(),
                    })
                }
            };
            tx.commit()?;
            return Ok(disposition);
        }
        let occupied: i64 = tx.query_row(
            "SELECT (SELECT count(*) FROM host_identity) + (SELECT count(*) FROM vms)",
            [],
            |row| row.get(0),
        )?;
        if occupied != 0 {
            return Err(StoreError::Conflict {
                kind: "migration_target",
                id: source.to_owned(),
            });
        }
        tx.execute(
            "INSERT INTO host_identity (singleton_key,host_id,capabilities_json,resource_version) VALUES (1,?1,?2,?3)",
            params![host.id.as_str(), serde_json::to_string(capabilities)?, version_i64(host.resource_version)?],
        )?;
        for definition in definitions {
            tx.execute(
                "INSERT INTO vms (vm_id,definition_json,requested_power_state,observed_power_state,resource_version) VALUES (?1,?2,?3,?4,?5)",
                params![definition.id.as_str(), serde_json::to_string(definition)?, requested_text(definition.requested_power_state), observed_text(definition.observed_power_state), version_i64(definition.resource_version)?],
            ).map_err(|error| map_constraint(error, "vm", definition.id.as_str()))?;
            insert_attachments(&tx, definition)?;
        }
        let mut imported_vm_ids = definitions
            .iter()
            .map(|definition| definition.id.as_str().to_owned())
            .collect::<Vec<_>>();
        imported_vm_ids.sort();
        tx.execute(
            "INSERT INTO migration_state (source,state,source_checksum,imported_host_id,imported_vm_ids_json,imported_at) VALUES (?1,'imported',?2,?3,?4,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![source, source_checksum, host.id.as_str(), canonical_json(&serde_json::json!(imported_vm_ids))?],
        )?;
        tx.commit()?;
        Ok(MigrationDisposition::Imported)
    }

    /// Activates Core authority only after an exact imported-source match.
    pub fn cutover_legacy_snapshot(
        &mut self,
        source: &str,
        checksum: &str,
    ) -> Result<MigrationDisposition> {
        let changed = self.conn.execute(
            "UPDATE migration_state SET state='cutover',cutover_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE source=?1 AND source_checksum=?2 AND state='imported'",
            params![source, checksum],
        )?;
        if changed == 1 {
            return Ok(MigrationDisposition::Cutover);
        }
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT state,source_checksum FROM migration_state WHERE source=?1",
                [source],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match row {
            Some((state, actual)) if actual == checksum && state == "cutover" => {
                Ok(MigrationDisposition::Cutover)
            }
            Some(_) => Err(StoreError::Conflict {
                kind: "migration",
                id: source.to_owned(),
            }),
            None => Err(StoreError::NotFound {
                kind: "migration",
                id: source.to_owned(),
            }),
        }
    }

    /// Removes an unactivated import. Cutover is deliberately irreversible.
    pub fn rollback_legacy_import(
        &mut self,
        source: &str,
        checksum: &str,
    ) -> Result<MigrationDisposition> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state: Option<(String, String, Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT state,source_checksum,imported_host_id,imported_vm_ids_json FROM migration_state WHERE source=?1",
                [source],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        match state {
            Some((state, actual, Some(imported_host), Some(imported_vms)))
                if state == "imported" && actual == checksum =>
            {
                let actual_host: Option<String> = tx
                    .query_row(
                        "SELECT host_id FROM host_identity WHERE singleton_key=1",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?;
                let mut statement = tx.prepare("SELECT vm_id FROM vms ORDER BY vm_id")?;
                let actual_vms = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let expected_vms: Vec<String> =
                    serde_json::from_str(&imported_vms).map_err(|error| {
                        StoreError::Integrity(format!("migration VM manifest is invalid: {error}"))
                    })?;
                if actual_host.as_deref() != Some(imported_host.as_str())
                    || actual_vms != expected_vms
                {
                    return Err(StoreError::Conflict {
                        kind: "migration_rollback_drift",
                        id: source.to_owned(),
                    });
                }
            }
            Some(_) => {
                return Err(StoreError::Conflict {
                    kind: "migration",
                    id: source.to_owned(),
                })
            }
            None => {
                return Err(StoreError::NotFound {
                    kind: "migration",
                    id: source.to_owned(),
                })
            }
        }
        let journal_count: i64 =
            tx.query_row("SELECT count(*) FROM operations", [], |row| row.get(0))?;
        if journal_count != 0 {
            return Err(StoreError::Conflict {
                kind: "migration_rollback",
                id: source.to_owned(),
            });
        }
        tx.execute("DELETE FROM attachments", [])?;
        tx.execute("DELETE FROM ownership_markers", [])?;
        tx.execute("DELETE FROM vms", [])?;
        tx.execute("DELETE FROM host_identity", [])?;
        tx.execute("DELETE FROM migration_state WHERE source=?1", [source])?;
        tx.commit()?;
        Ok(MigrationDisposition::RolledBack)
    }

    /// Atomically reserve a new file and initialize it. Parent directories
    /// must already exist; a pre-existing file is never opened or removed.
    pub fn create_new(path: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    StoreError::AlreadyExists(path.to_path_buf())
                } else {
                    StoreError::Integrity(format!("cannot create {}: {error}", path.display()))
                }
            })?;
        drop(file);

        let result = (|| {
            validate_database_sidecars(path)?;
            let mut conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            configure(&conn)?;
            enforce_database_modes(path)?;
            apply_migrations(&mut conn)?;
            validate(&conn)?;
            Ok(Self { conn })
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(path);
        }
        result
    }

    /// Creates a complete fresh authority and publishes it atomically.
    ///
    /// The externally visible path is never an identity-empty database: schema
    /// and host identity commit together in a private sibling file, which is
    /// fsynced and renamed into place without replacing an existing authority.
    pub fn create_new_with_host(
        path: &Path,
        identity: &HostIdentity,
        capabilities: &HostCapabilities,
    ) -> Result<Self> {
        Self::create_new_with_host_inner(path, identity, capabilities, || Ok(()), |_| Ok(()))
    }

    fn create_new_with_host_inner(
        path: &Path,
        identity: &HostIdentity,
        capabilities: &HostCapabilities,
        before_host_insert: impl FnOnce() -> Result<()>,
        after_publish: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Self> {
        validate_fresh_host(identity)?;
        validate_fresh_parent(path)?;
        let staging = staging_path(path)?;
        let mut published = false;
        let result = (|| {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&staging)
                .map_err(|error| {
                    StoreError::Integrity(format!(
                        "cannot create fresh Core staging file {}: {error}",
                        staging.display()
                    ))
                })?;
            drop(file);

            let mut conn = Connection::open_with_flags(
                &staging,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            configure(&conn)?;
            enforce_database_modes(&staging)?;
            apply_migrations_with_host(&mut conn, identity, capabilities, before_host_insert)?;
            validate(&conn)?;
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            drop(conn);
            remove_sidecars(&staging)?;
            std::fs::OpenOptions::new()
                .read(true)
                .open(&staging)
                .and_then(|file| file.sync_all())
                .map_err(|error| {
                    StoreError::Integrity(format!(
                        "cannot sync fresh Core store {}: {error}",
                        staging.display()
                    ))
                })?;
            rename_noreplace(&staging, path)?;
            published = true;
            after_publish(path)?;
            sync_parent(path)?;
            Self::open_existing(path)
        })();

        if result.is_err() && !published {
            let _ = remove_database_files(&staging);
        }
        result
    }

    pub fn open_existing(path: &Path) -> Result<Self> {
        validate_database_file(path)?;
        validate_database_sidecars(path)?;
        // Validate through a read-only handle first. In particular, an empty,
        // corrupt, or foreign file must not be initialized by connection
        // pragmas before it has proved it is a compatible Core store.
        let read_only = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        read_only.busy_timeout(BUSY_TIMEOUT)?;
        let requires_upgrade = validate_openable_schema(&read_only)?;
        drop(read_only);
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        configure(&conn)?;
        enforce_database_modes(path)?;
        if requires_upgrade {
            apply_pending_migrations(&mut conn)?;
        }
        validate(&conn)?;
        Ok(Self { conn })
    }

    /// Establish the single durable identity for this store.
    pub fn create_host(
        &self,
        identity: &HostIdentity,
        capabilities: &HostCapabilities,
    ) -> Result<HostRecord> {
        self.conn
            .execute(
                "INSERT INTO host_identity (singleton_key,host_id,capabilities_json,resource_version) VALUES (1,?1,?2,?3)",
                params![identity.id.as_str(), serde_json::to_string(capabilities)?, version_i64(identity.resource_version)?],
            )
            .map_err(|error| map_constraint(error, "host", identity.id.as_str()))?;
        self.host()
    }

    pub fn host(&self) -> Result<HostRecord> {
        let raw: Option<(String, String, i64)> = self
            .conn
            .query_row(
                "SELECT host_id,capabilities_json,resource_version FROM host_identity WHERE singleton_key=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (id, capabilities, version) = raw.ok_or_else(|| StoreError::NotFound {
            kind: "host",
            id: "singleton".to_owned(),
        })?;
        Ok(HostRecord {
            identity: HostIdentity {
                id: host_id(id)?,
                resource_version: resource_version(version)?,
            },
            capabilities: serde_json::from_str(&capabilities)?,
        })
    }

    pub fn update_host(
        &self,
        expected: ResourceVersion,
        capabilities: &HostCapabilities,
    ) -> Result<HostRecord> {
        let changed = self.conn.execute(
            "UPDATE host_identity SET capabilities_json=?1,resource_version=resource_version+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE singleton_key=1 AND resource_version=?2",
            params![serde_json::to_string(capabilities)?, version_i64(expected)?],
        )?;
        if changed == 0 {
            return Err(self.host_version_error(expected)?);
        }
        self.host()
    }

    #[cfg(test)]
    fn create_vm(&mut self, definition: &VmDefinition) -> Result<VmDefinition> {
        validate_definition(definition)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO vms (vm_id,definition_json,requested_power_state,observed_power_state,resource_version) VALUES (?1,?2,?3,?4,?5)",
            params![definition.id.as_str(), serde_json::to_string(definition)?, requested_text(definition.requested_power_state), observed_text(definition.observed_power_state), version_i64(definition.resource_version)?],
        ).map_err(|error| map_constraint(error, "vm", definition.id.as_str()))?;
        insert_attachments(&tx, definition)?;
        tx.commit()?;
        self.get_vm(&definition.id)
    }

    pub fn get_vm(&self, vm_id: &VmId) -> Result<VmDefinition> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT definition_json FROM vms WHERE vm_id=?1 AND deleted_at IS NULL",
                [vm_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|json| serde_json::from_str(&json))
            .transpose()?
            .ok_or_else(|| StoreError::NotFound {
                kind: "vm",
                id: vm_id.to_string(),
            })
    }

    pub fn list_vms(&self) -> Result<Vec<VmDefinition>> {
        let mut statement = self
            .conn
            .prepare("SELECT definition_json FROM vms WHERE deleted_at IS NULL ORDER BY vm_id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut definitions = Vec::new();
        for row in rows {
            definitions.push(serde_json::from_str(&row?)?);
        }
        Ok(definitions)
    }

    #[cfg(test)]
    fn update_vm(
        &mut self,
        expected: ResourceVersion,
        definition: &VmDefinition,
    ) -> Result<VmDefinition> {
        validate_definition(definition)?;
        if definition.resource_version.get() != expected.get() + 1 {
            return Err(StoreError::InvalidDomain(
                "updated VM definition must carry expected resource version + 1".to_owned(),
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE vms SET definition_json=?1,requested_power_state=?2,observed_power_state=?3,resource_version=?4,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE vm_id=?5 AND resource_version=?6",
            params![serde_json::to_string(definition)?, requested_text(definition.requested_power_state), observed_text(definition.observed_power_state), version_i64(definition.resource_version)?, definition.id.as_str(), version_i64(expected)?],
        )?;
        if changed == 0 {
            return Err(vm_version_error(&tx, &definition.id, expected)?);
        }
        require_same_attachments(&tx, definition)?;
        tx.commit()?;
        self.get_vm(&definition.id)
    }

    #[cfg(test)]
    fn delete_vm(&self, vm_id: &VmId, expected: ResourceVersion) -> Result<()> {
        let next = expected
            .next()
            .map_err(|error| StoreError::InvalidDomain(error.to_string()))?;
        let changed = self.conn.execute(
            "UPDATE vms SET deleted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),resource_version=?1,definition_json=json_set(definition_json,'$.resource_version',?1),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE vm_id=?2 AND resource_version=?3 AND deleted_at IS NULL",
            params![version_i64(next)?, vm_id.as_str(), version_i64(expected)?],
        )?;
        if changed == 0 {
            return Err(vm_version_error(&self.conn, vm_id, expected)?);
        }
        Ok(())
    }

    /// Atomically writes the operation, scoped idempotency mapping, and first
    /// correlated event before a caller may execute external side effects.
    pub fn accept_operation(&mut self, request: &AcceptOperation<'_>) -> Result<AcceptedOperation> {
        if request.idempotency_scope.trim().is_empty() {
            return Err(StoreError::InvalidDomain(
                "idempotency scope must not be empty".to_owned(),
            ));
        }
        let operation = request.operation;
        let computed_fingerprint = canonical_request_fingerprint(request.request)?;
        if operation.request_fingerprint != computed_fingerprint {
            return Err(StoreError::InvalidDomain(
                "operation request fingerprint does not match canonical request".to_owned(),
            ));
        }
        let request_json = canonical_json(request.request)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((fingerprint, operation_id, accepted_version)) = tx
            .query_row(
                "SELECT request_fingerprint,operation_id,accepted_resource_version FROM idempotency_keys WHERE scope=?1 AND idempotency_key=?2",
                params![request.idempotency_scope, request.idempotency_key.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
            )
            .optional()?
        {
            if fingerprint != operation.request_fingerprint {
                return Err(StoreError::IdempotencyConflict {
                    scope: request.idempotency_scope.to_owned(),
                    key: request.idempotency_key.to_string(),
                });
            }
            let existing = read_operation_entry(&tx, &operation_id)?;
            if canonical_json(&existing.request)? != request_json {
                return Err(StoreError::Integrity(format!(
                    "idempotency mapping for {operation_id} disagrees with its request"
                )));
            }
            tx.commit()?;
            return Ok(AcceptedOperation {
                disposition: Acceptance::Replay,
                operation: existing.operation,
                accepted_resource_version: resource_version(accepted_version)?,
            });
        }

        validate_operation_for_acceptance(operation)?;
        let accepted_version = persist_accepted_desired_state(&tx, request)?;
        tx.execute(
            "INSERT INTO operations (operation_id,kind,vm_id,request_fingerprint,request_json,status,retry_count,max_retries) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![operation.id.as_str(), operation_kind_text(operation.kind), operation.vm_id.as_str(), operation.request_fingerprint, request_json, operation_status_text(operation.status), i64::from(operation.attempt_count), i64::from(operation.max_attempts)],
        ).map_err(|error| map_constraint(error, "operation", operation.id.as_str()))?;
        tx.execute(
            "INSERT INTO idempotency_keys (scope,idempotency_key,request_fingerprint,operation_id,accepted_resource_version) VALUES (?1,?2,?3,?4,?5)",
            params![request.idempotency_scope, request.idempotency_key.as_str(), operation.request_fingerprint, operation.id.as_str(), version_i64(accepted_version)?],
        )?;
        tx.execute(
            "INSERT INTO events (event_id,sequence,operation_id,vm_id,kind,payload_json) VALUES (?1,(SELECT coalesce(max(sequence),0)+1 FROM events),?2,?3,'operation.accepted','{}')",
            params![format!("{}:accepted", operation.id.as_str()), operation.id.as_str(), operation.vm_id.as_str()],
        )?;
        let accepted = read_operation(&tx, operation.id.as_str())?;
        tx.commit()?;
        Ok(AcceptedOperation {
            disposition: Acceptance::Accepted,
            operation: accepted,
            accepted_resource_version: accepted_version,
        })
    }

    /// Resolves an existing scoped idempotency mapping without consulting VM
    /// state. This permits exact replay after later mutation or tombstoning.
    pub fn resolve_idempotency(
        &self,
        scope: &str,
        key: &IdempotencyKey,
        fingerprint: &str,
        request: &serde_json::Value,
    ) -> Result<Option<AcceptedOperation>> {
        let mapping: Option<(String, String, i64)> = self.conn.query_row(
            "SELECT request_fingerprint,operation_id,accepted_resource_version FROM idempotency_keys WHERE scope=?1 AND idempotency_key=?2",
            params![scope, key.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()?;
        let Some((stored_fingerprint, operation_id, accepted_version)) = mapping else {
            return Ok(None);
        };
        if stored_fingerprint != fingerprint {
            return Err(StoreError::IdempotencyConflict {
                scope: scope.to_owned(),
                key: key.to_string(),
            });
        }
        let entry = read_operation_entry(&self.conn, &operation_id)?;
        if canonical_json(&entry.request)? != canonical_json(request)? {
            return Err(StoreError::Integrity(format!(
                "idempotency mapping for {operation_id} disagrees with its request"
            )));
        }
        Ok(Some(AcceptedOperation {
            disposition: Acceptance::Replay,
            operation: entry.operation,
            accepted_resource_version: resource_version(accepted_version)?,
        }))
    }

    pub fn operation(&self, id: &OperationId) -> Result<Operation> {
        read_operation(&self.conn, id.as_str())
    }

    pub fn operation_entry(&self, id: &OperationId) -> Result<OperationJournalEntry> {
        read_operation_entry(&self.conn, id.as_str())
    }

    /// Returns operations whose durable outcome must be classified after a
    /// process restart. Ordering is stable to make recovery planning repeatable.
    pub fn list_incomplete_operations(&self) -> Result<Vec<OperationJournalEntry>> {
        let mut statement = self.conn.prepare(
            "SELECT operation_id FROM operations WHERE status IN ('accepted','running') ORDER BY accepted_at,operation_id",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| read_operation_entry(&self.conn, id))
            .collect()
    }

    #[doc(hidden)]
    pub fn list_incomplete_execution_operations(
        &self,
    ) -> Result<Vec<IncompleteExecutionOperation>> {
        let mut statement = self.conn.prepare(
            "SELECT operation_id,active_attempt_token FROM operations WHERE status IN ('accepted','running') ORDER BY accepted_at,operation_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        rows.map(|row| {
            let (id, active_attempt_token) = row?;
            Ok(IncompleteExecutionOperation {
                entry: read_operation_entry(&self.conn, &id)?,
                active_attempt_token,
            })
        })
        .collect()
    }

    pub fn list_operations(&self) -> Result<Vec<OperationJournalEntry>> {
        let mut statement = self
            .conn
            .prepare("SELECT operation_id FROM operations ORDER BY accepted_at,operation_id")?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| read_operation_entry(&self.conn, id))
            .collect()
    }

    pub fn list_events_after(
        &self,
        after_sequence: u64,
        limit: u32,
    ) -> Result<Vec<OperationEvent>> {
        let limit = limit.clamp(1, 1_000);
        let after = i64::try_from(after_sequence).map_err(|_| {
            StoreError::InvalidDomain("event sequence exceeds SQLite range".to_owned())
        })?;
        let mut statement = self.conn.prepare(
            "SELECT event_id,sequence,operation_id,vm_id,kind,payload_json FROM events WHERE sequence>?1 ORDER BY sequence LIMIT ?2",
        )?;
        let rows = statement.query_map(params![after, i64::from(limit)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (id, sequence, operation_id, vm_id, kind, payload) = row?;
            let event = OperationEvent {
                id: cellhv_core_types::EventId::new(id)
                    .map_err(|error| StoreError::InvalidDomain(error.to_string()))?,
                sequence: u64::try_from(sequence).map_err(|_| {
                    StoreError::Integrity(format!("invalid event sequence {sequence}"))
                })?,
                operation_id: operation_id
                    .map(OperationId::new)
                    .transpose()
                    .map_err(|error| StoreError::InvalidDomain(error.to_string()))?,
                vm_id: vm_id
                    .map(VmId::new)
                    .transpose()
                    .map_err(|error| StoreError::InvalidDomain(error.to_string()))?,
                kind,
                payload: serde_json::from_str(&payload)?,
            };
            event
                .validate()
                .map_err(|error| StoreError::InvalidDomain(error.to_string()))?;
            events.push(event);
        }
        Ok(events)
    }

    /// Persistence primitive for the application service. Callers must not
    /// perform side effects until this transition has committed.
    #[doc(hidden)]
    pub fn claim_operation(
        &mut self,
        id: &OperationId,
        attempt_token: &str,
    ) -> Result<ClaimedOperation> {
        validate_attempt_token(attempt_token)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_operation(&tx, id.as_str())?;
        let active_token: Option<String> = tx.query_row(
            "SELECT active_attempt_token FROM operations WHERE operation_id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?;
        if current.status == OperationStatus::Running
            && active_token.as_deref() == Some(attempt_token)
        {
            tx.commit()?;
            return Ok(ClaimedOperation {
                disposition: ClaimDisposition::Replay,
                entry: self.operation_entry(id)?,
            });
        }
        if current.status != OperationStatus::Accepted
            || active_token.is_some()
            || current.attempt_count >= current.max_attempts
        {
            return Err(StoreError::Conflict {
                kind: "operation",
                id: id.to_string(),
            });
        }
        let attempt = current.attempt_count + 1;
        let changed = tx.execute(
            "UPDATE operations SET status='running',retry_count=?1,active_attempt_token=?2 WHERE operation_id=?3 AND status='accepted' AND retry_count=?4 AND active_attempt_token IS NULL",
            params![i64::from(attempt), attempt_token, id.as_str(), i64::from(current.attempt_count)],
        )?;
        if changed != 1 {
            return Err(StoreError::Conflict {
                kind: "operation",
                id: id.to_string(),
            });
        }
        tx.execute(
            "INSERT INTO events (event_id,sequence,operation_id,vm_id,kind,payload_json) VALUES (?1,(SELECT coalesce(max(sequence),0)+1 FROM events),?2,?3,'operation.running',?4)",
            params![format!("{}:running:{attempt}", id.as_str()), id.as_str(), current.vm_id.as_str(), canonical_json(&serde_json::json!({"attempt":attempt}))?],
        )?;
        let entry = read_operation_entry(&tx, id.as_str())?;
        tx.commit()?;
        Ok(ClaimedOperation {
            disposition: ClaimDisposition::Acquired,
            entry,
        })
    }

    /// Persistence primitive used by the single operation application service.
    #[doc(hidden)]
    pub fn persist_terminal_operation(
        &mut self,
        id: &OperationId,
        attempt_token: &str,
        status: OperationStatus,
        result: Option<&serde_json::Value>,
        error: Option<&serde_json::Value>,
    ) -> Result<CompletedOperation> {
        validate_attempt_token(attempt_token)?;
        if !matches!(
            status,
            OperationStatus::Succeeded | OperationStatus::Failed | OperationStatus::Unsupported
        ) || (result.is_some() && error.is_some())
            || (status == OperationStatus::Succeeded && error.is_some())
            || (matches!(
                status,
                OperationStatus::Failed | OperationStatus::Unsupported
            ) && error.is_none())
        {
            return Err(StoreError::InvalidDomain(
                "terminal operation outcome has inconsistent status/result/error".to_owned(),
            ));
        }
        let result = result.map(canonical_json).transpose()?;
        let error = error.map(canonical_json).transpose()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE operations SET status=?1,result_json=?2,error_json=?3,completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),active_attempt_token=NULL,completed_attempt_token=?5 WHERE operation_id=?4 AND status='running' AND active_attempt_token=?5 AND completed_attempt_token IS NULL",
            params![operation_status_text(status), result, error, id.as_str(), attempt_token],
        )?;
        if changed != 1 {
            let existing = read_operation_entry(&tx, id.as_str())?;
            let completed_token: Option<String> = tx.query_row(
                "SELECT completed_attempt_token FROM operations WHERE operation_id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            let exact_replay = completed_token.as_deref() == Some(attempt_token)
                && existing.operation.status == status
                && existing.result.as_ref().map(canonical_json).transpose()? == result
                && existing.error.as_ref().map(canonical_json).transpose()? == error;
            if exact_replay {
                tx.commit()?;
                return Ok(CompletedOperation {
                    disposition: CompletionDisposition::Replay,
                    entry: self.operation_entry(id)?,
                });
            }
            return Err(StoreError::Conflict {
                kind: "operation",
                id: id.to_string(),
            });
        }
        let operation = read_operation(&tx, id.as_str())?;
        tx.execute(
            "INSERT INTO events (event_id,sequence,operation_id,vm_id,kind,payload_json) VALUES (?1,(SELECT coalesce(max(sequence),0)+1 FROM events),?2,?3,?4,?5)",
            params![format!("{}:terminal", id.as_str()), id.as_str(), operation.vm_id.as_str(), format!("operation.{}", operation_status_text(status)), canonical_json(&serde_json::json!({"status":operation_status_text(status)}))?],
        )?;
        let entry = read_operation_entry(&tx, id.as_str())?;
        tx.commit()?;
        Ok(CompletedOperation {
            disposition: CompletionDisposition::Applied,
            entry,
        })
    }

    fn host_version_error(&self, expected: ResourceVersion) -> Result<StoreError> {
        match self.host() {
            Ok(host) => Ok(StoreError::StaleVersion {
                kind: "host",
                id: host.identity.id.to_string(),
                expected: expected.get(),
                actual: host.identity.resource_version.get(),
            }),
            Err(StoreError::NotFound { .. }) => Ok(StoreError::NotFound {
                kind: "host",
                id: "singleton".to_owned(),
            }),
            Err(error) => Err(error),
        }
    }
}

fn enforce_database_modes(path: &Path) -> Result<()> {
    for candidate in [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata)
                if metadata.file_type().is_file()
                    && metadata.uid() == unsafe { libc::geteuid() }
                    && metadata.nlink() == 1 =>
            {
                std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o600))
                    .map_err(|error| {
                        StoreError::Integrity(format!(
                            "cannot secure {}: {error}",
                            candidate.display()
                        ))
                    })?;
            }
            Ok(_) => {
                return Err(StoreError::Integrity(format!(
                    "{} must be an owner-owned regular file with one link",
                    candidate.display()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(StoreError::Integrity(format!(
                    "cannot inspect {}: {error}",
                    candidate.display()
                )))
            }
        }
    }
    Ok(())
}

fn validate_database_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{suffix}", path.display()));
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata)
                if metadata.file_type().is_file()
                    && metadata.uid() == unsafe { libc::geteuid() }
                    && metadata.permissions().mode() & 0o077 == 0
                    && metadata.nlink() == 1 => {}
            Ok(_) => {
                return Err(StoreError::Integrity(format!(
                    "{} must be an owner-owned owner-only regular file with one link",
                    candidate.display()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(StoreError::Integrity(format!(
                    "cannot inspect {}: {error}",
                    candidate.display()
                )))
            }
        }
    }
    Ok(())
}

fn validate_database_file(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StoreError::Missing(path.to_path_buf())
        } else {
            StoreError::Integrity(format!("cannot inspect {}: {error}", path.display()))
        }
    })?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(StoreError::Integrity(format!(
            "{} must be an owner-owned 0600 regular file with one link",
            path.display()
        )));
    }
    Ok(())
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA trusted_schema=OFF;",
    )?;
    let mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    let foreign_keys: i64 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
    let synchronous: i64 = conn.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") || foreign_keys != 1 || synchronous != 2 {
        return Err(StoreError::Integrity(
            "WAL, foreign_keys, or FULL synchronous is inactive".to_owned(),
        ));
    }
    Ok(())
}

fn checksum(sql: &str) -> String {
    format!("{:x}", Sha256::digest(sql.as_bytes()))
}

fn apply_migrations_with_host(
    conn: &mut Connection,
    identity: &HostIdentity,
    capabilities: &HostCapabilities,
    before_host_insert: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY,name TEXT NOT NULL UNIQUE,checksum TEXT NOT NULL,schema_fingerprint TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))) STRICT;")?;
    for migration in MIGRATIONS {
        tx.execute_batch(migration.sql)?;
        tx.pragma_update(None, "user_version", migration.version)?;
        let fingerprint = schema_fingerprint(&tx)?;
        tx.execute(
            "INSERT INTO schema_migrations (version,name,checksum,schema_fingerprint) VALUES (?1,?2,?3,?4)",
            params![migration.version, migration.name, checksum(migration.sql), fingerprint],
        )?;
    }
    before_host_insert()?;
    tx.execute(
        "INSERT INTO host_identity (singleton_key,host_id,capabilities_json,resource_version) VALUES (1,?1,?2,?3)",
        params![identity.id.as_str(), serde_json::to_string(capabilities)?, version_i64(identity.resource_version)?],
    )
    .map_err(|error| map_constraint(error, "host", identity.id.as_str()))?;
    tx.commit()?;
    Ok(())
}

fn apply_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(&format!("PRAGMA application_id={APPLICATION_ID};"))?;
    conn.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY,name TEXT NOT NULL UNIQUE,checksum TEXT NOT NULL,schema_fingerprint TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))) STRICT;")?;
    for migration in MIGRATIONS {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(migration.sql)?;
        tx.pragma_update(None, "user_version", migration.version)?;
        let fingerprint = schema_fingerprint(&tx)?;
        tx.execute(
            "INSERT INTO schema_migrations (version,name,checksum,schema_fingerprint) VALUES (?1,?2,?3,?4)",
            params![migration.version, migration.name, checksum(migration.sql), fingerprint],
        )?;
        tx.commit()?;
    }
    Ok(())
}

fn apply_pending_migrations(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current)
    {
        tx.execute_batch(migration.sql)?;
        tx.pragma_update(None, "user_version", migration.version)?;
        let fingerprint = schema_fingerprint(&tx)?;
        tx.execute(
            "INSERT INTO schema_migrations (version,name,checksum,schema_fingerprint) VALUES (?1,?2,?3,?4)",
            params![migration.version, migration.name, checksum(migration.sql), fingerprint],
        )?;
    }
    validate(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Verifies that an existing database is either current or an exact,
/// registered schema prefix that can be upgraded transactionally.
fn validate_openable_schema(conn: &Connection) -> Result<bool> {
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| StoreError::Integrity(error.to_string()))?;
    if integrity != "ok" {
        return Err(StoreError::Integrity(integrity));
    }
    let application_id: i32 = conn.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if application_id != APPLICATION_ID {
        return Err(StoreError::Schema(format!(
            "application_id {application_id} is not CellHV Core"
        )));
    }
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let latest = MIGRATIONS
        .last()
        .expect("migration registry nonempty")
        .version;
    if user_version == latest {
        validate(conn)?;
        return Ok(false);
    }
    if user_version <= 0 || user_version >= latest {
        return Err(StoreError::Schema(format!(
            "user_version {user_version} is unsupported; expected at most {latest}"
        )));
    }
    let expected = MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= user_version)
        .collect::<Vec<_>>();
    let mut statement = conn
        .prepare("SELECT version,name,checksum FROM schema_migrations ORDER BY version")
        .map_err(|error| StoreError::Schema(format!("migration ledger unavailable: {error}")))?;
    let actual = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if actual.len() != expected.len()
        || actual.iter().zip(expected.iter()).any(|(row, migration)| {
            row != &(
                migration.version,
                migration.name.to_owned(),
                checksum(migration.sql),
            )
        })
    {
        return Err(StoreError::Schema(
            "upgrade source migration ledger mismatch".to_owned(),
        ));
    }
    let recorded_fingerprint: String = conn.query_row(
        "SELECT schema_fingerprint FROM schema_migrations WHERE version=?1",
        [user_version],
        |row| row.get(0),
    )?;
    if recorded_fingerprint != schema_fingerprint(conn)? {
        return Err(StoreError::Schema(
            "upgrade source schema fingerprint mismatch".to_owned(),
        ));
    }
    Ok(true)
}

fn validate_fresh_host(identity: &HostIdentity) -> Result<()> {
    if identity.resource_version.get() != 1 {
        return Err(StoreError::InvalidDomain(
            "fresh host identity resource_version must be 1".to_owned(),
        ));
    }
    let id = identity.id.as_str().trim();
    if id.is_empty()
        || ["unknown", "unset", "none", "null"]
            .iter()
            .any(|reserved| id.eq_ignore_ascii_case(reserved))
    {
        return Err(StoreError::InvalidDomain(
            "fresh host identity must not be empty or reserved".to_owned(),
        ));
    }
    Ok(())
}

fn validate_fresh_parent(path: &Path) -> Result<()> {
    if path.as_os_str().to_str().is_none() {
        return Err(StoreError::Integrity(
            "fresh Core authority path must be valid UTF-8".to_owned(),
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        StoreError::Integrity(format!("{} has no parent directory", path.display()))
    })?;
    let metadata = std::fs::symlink_metadata(parent).map_err(|error| {
        StoreError::Integrity(format!(
            "cannot inspect fresh Core parent {}: {error}",
            parent.display()
        ))
    })?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(StoreError::Integrity(format!(
            "fresh Core parent {} must be a real euid-owned 0700 directory",
            parent.display()
        )));
    }
    Ok(())
}

fn staging_path(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().ok_or_else(|| {
        StoreError::Integrity(format!("{} has no parent directory", path.display()))
    })?;
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::Integrity(format!("{} has no file name", path.display())))?;
    let name = name.to_str().ok_or_else(|| {
        StoreError::Integrity("fresh Core authority file name must be valid UTF-8".to_owned())
    })?;
    for _ in 0..64 {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{}.fresh-{}-{sequence}", name, std::process::id()));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(StoreError::Integrity(format!(
        "cannot allocate a fresh Core staging path beside {}",
        path.display()
    )))
}

fn remove_database_files(path: &Path) -> std::io::Result<()> {
    for candidate in [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        match std::fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn remove_sidecars(path: &Path) -> Result<()> {
    for candidate in [
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        match std::fs::remove_file(&candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(StoreError::Integrity(format!(
                    "cannot remove staging sidecar {}: {error}",
                    candidate.display()
                )))
            }
        }
    }
    Ok(())
}

fn rename_noreplace(source: &Path, destination: &Path) -> Result<()> {
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| StoreError::Integrity("fresh Core staging path contains NUL".to_owned()))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| StoreError::Integrity("fresh Core authority path contains NUL".to_owned()))?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Err(StoreError::AlreadyExists(PathBuf::from(
            destination.to_string_lossy().into_owned(),
        )))
    } else {
        Err(StoreError::Integrity(format!(
            "cannot publish fresh Core authority: {error}"
        )))
    }
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        StoreError::Integrity(format!("{} has no parent directory", path.display()))
    })?;
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            StoreError::Integrity(format!(
                "cannot sync Core authority directory {}: {error}",
                parent.display()
            ))
        })
}

fn validate(conn: &Connection) -> Result<()> {
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| StoreError::Integrity(error.to_string()))?;
    if integrity != "ok" {
        return Err(StoreError::Integrity(integrity));
    }
    let application_id: i32 = conn.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if application_id != APPLICATION_ID {
        return Err(StoreError::Schema(format!(
            "application_id {application_id} is not CellHV Core"
        )));
    }
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let latest = MIGRATIONS
        .last()
        .expect("migration registry nonempty")
        .version;
    if user_version != latest {
        return Err(StoreError::Schema(format!(
            "user_version {user_version} is unsupported; expected {latest}"
        )));
    }
    let mut statement = conn
        .prepare("SELECT version,name,checksum FROM schema_migrations ORDER BY version")
        .map_err(|error| StoreError::Schema(format!("migration ledger unavailable: {error}")))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let actual = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    if actual.len() != MIGRATIONS.len() {
        return Err(StoreError::Schema("migration count mismatch".to_owned()));
    }
    for (row, migration) in actual.iter().zip(MIGRATIONS) {
        if row
            != &(
                migration.version,
                migration.name.to_owned(),
                checksum(migration.sql),
            )
        {
            return Err(StoreError::Schema(format!(
                "migration {} metadata/checksum mismatch",
                migration.version
            )));
        }
    }
    let recorded_fingerprint: String = conn.query_row(
        "SELECT schema_fingerprint FROM schema_migrations WHERE version=?1",
        [latest],
        |row| row.get(0),
    )?;
    if recorded_fingerprint != schema_fingerprint(conn)? {
        return Err(StoreError::Schema(
            "sqlite_master schema fingerprint mismatch".to_owned(),
        ));
    }
    let foreign_key_failure: Option<(String, i64, String, i64)> = conn
        .query_row("PRAGMA foreign_key_check", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()?;
    if foreign_key_failure.is_some() {
        return Err(StoreError::Integrity(
            "foreign_key_check reported an invalid reference".to_owned(),
        ));
    }
    validate_schema_objects(conn)?;
    validate_host_rows(conn)?;
    validate_vm_rows(conn)?;
    validate_journal_rows(conn)?;
    validate_migration_rows(conn)?;
    Ok(())
}

fn validate_migration_rows(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(
        "SELECT source,state,source_checksum,imported_host_id,imported_vm_ids_json,imported_at,cutover_at FROM migration_state",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    for row in rows {
        let (source, state, checksum, host, vm_ids, imported_at, cutover_at) = row?;
        if source.trim().is_empty() || checksum.trim().is_empty() {
            return Err(StoreError::Integrity(
                "migration source/checksum is empty".to_owned(),
            ));
        }
        match state.as_str() {
            "pending"
                if host.is_none()
                    && vm_ids.is_none()
                    && imported_at.is_none()
                    && cutover_at.is_none() => {}
            "imported" | "cutover" => {
                if host.as_deref().is_none_or(|value| value.trim().is_empty())
                    || imported_at
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                {
                    return Err(StoreError::Integrity(format!(
                        "migration {source} has incomplete import metadata"
                    )));
                }
                let ids: Vec<String> = serde_json::from_str(vm_ids.as_deref().unwrap_or(""))
                    .map_err(|error| {
                        StoreError::Integrity(format!(
                            "migration {source} VM manifest is invalid: {error}"
                        ))
                    })?;
                if ids.iter().any(|id| id.trim().is_empty())
                    || !ids.windows(2).all(|pair| pair[0] < pair[1])
                {
                    return Err(StoreError::Integrity(format!(
                        "migration {source} VM manifest is not sorted and unique"
                    )));
                }
                let imported_valid: i64 = conn.query_row(
                    "SELECT julianday(?1) IS NOT NULL",
                    [imported_at.as_deref().unwrap_or("")],
                    |row| row.get(0),
                )?;
                if imported_valid != 1 {
                    return Err(StoreError::Integrity(format!(
                        "migration {source} imported timestamp is invalid"
                    )));
                }
                if (state == "cutover")
                    != cutover_at
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty())
                {
                    return Err(StoreError::Integrity(format!(
                        "migration {source} cutover timestamp disagrees with state"
                    )));
                }
                if let Some(cutover_at) = cutover_at.as_deref() {
                    let cutover_valid: i64 =
                        conn.query_row("SELECT julianday(?1) IS NOT NULL", [cutover_at], |row| {
                            row.get(0)
                        })?;
                    if cutover_valid != 1 {
                        return Err(StoreError::Integrity(format!(
                            "migration {source} cutover timestamp is invalid"
                        )));
                    }
                }
                if state == "imported" {
                    let actual_host: Option<String> = conn
                        .query_row(
                            "SELECT host_id FROM host_identity WHERE singleton_key=1",
                            [],
                            |row| row.get(0),
                        )
                        .optional()?;
                    let mut vm_statement = conn.prepare("SELECT vm_id FROM vms ORDER BY vm_id")?;
                    let actual_ids = vm_statement
                        .query_map([], |row| row.get::<_, String>(0))?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    if actual_host.as_deref() != host.as_deref() || actual_ids != ids {
                        return Err(StoreError::Integrity(format!(
                            "migration {source} import manifest disagrees with authority state"
                        )));
                    }
                }
            }
            _ => {
                return Err(StoreError::Integrity(format!(
                    "migration {source} state is invalid"
                )))
            }
        }
    }
    Ok(())
}

fn validate_host_rows(conn: &Connection) -> Result<()> {
    let row: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT host_id,capabilities_json,resource_version FROM host_identity WHERE singleton_key=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((id, capabilities, version)) = row {
        host_id(id.clone()).map_err(|error| {
            StoreError::Integrity(format!("host {id} identity is invalid: {error}"))
        })?;
        resource_version(version).map_err(|error| {
            StoreError::Integrity(format!("host {id} resource version is invalid: {error}"))
        })?;
        serde_json::from_str::<HostCapabilities>(&capabilities).map_err(|error| {
            StoreError::Integrity(format!("host {id} capabilities are invalid: {error}"))
        })?;
    }
    Ok(())
}

fn validate_schema_objects(conn: &Connection) -> Result<()> {
    const REQUIRED: &[(&str, &str)] = &[
        ("table", "schema_migrations"),
        ("table", "host_identity"),
        ("table", "vms"),
        ("table", "attachments"),
        ("table", "operations"),
        ("table", "operation_steps"),
        ("table", "idempotency_keys"),
        ("table", "events"),
        ("table", "ownership_markers"),
        ("table", "migration_state"),
        ("index", "operations_vm_id_idx"),
        ("index", "events_operation_id_idx"),
        ("index", "events_vm_id_idx"),
    ];
    for (kind, name) in REQUIRED {
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type=?1 AND name=?2",
            params![kind, name],
            |row| row.get(0),
        )?;
        if count != 1 {
            return Err(StoreError::Schema(format!(
                "required {kind} {name} is missing"
            )));
        }
    }
    Ok(())
}

fn validate_vm_rows(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("SELECT vm_id,definition_json,requested_power_state,observed_power_state,resource_version FROM vms")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    for row in rows {
        let (id, json, requested, observed, version) = row?;
        let definition: VmDefinition = serde_json::from_str(&json).map_err(|error| {
            StoreError::Integrity(format!("VM {id} definition is invalid: {error}"))
        })?;
        if definition.id.as_str() != id
            || requested_text(definition.requested_power_state) != requested
            || observed_text(definition.observed_power_state) != observed
            || version_i64(definition.resource_version)? != version
        {
            return Err(StoreError::Integrity(format!(
                "VM {id} indexed columns disagree with definition_json"
            )));
        }
        let mut expected = definition
            .storage
            .iter()
            .map(|item| {
                (
                    item.attachment_id.clone(),
                    "storage".to_owned(),
                    item.storage_ref.clone(),
                )
            })
            .chain(definition.networks.iter().map(|item| {
                (
                    item.attachment_id.clone(),
                    "network".to_owned(),
                    item.network_ref.clone(),
                )
            }))
            .collect::<Vec<_>>();
        expected.sort();
        let mut attachment_statement = conn.prepare(
            "SELECT attachment_id,kind,provider_ref FROM attachments WHERE vm_id=?1 ORDER BY attachment_id",
        )?;
        let mut actual = attachment_statement
            .query_map([id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<(String, String, String)>, _>>()?;
        actual.sort();
        if actual != expected {
            return Err(StoreError::Integrity(format!(
                "VM {id} attachment rows disagree with definition_json"
            )));
        }
    }
    Ok(())
}

fn validate_journal_rows(conn: &Connection) -> Result<()> {
    let mut operations =
        conn.prepare("SELECT operation_id FROM operations ORDER BY operation_id")?;
    for id in operations.query_map([], |row| row.get::<_, String>(0))? {
        let entry = read_operation_entry(conn, &id?)?;
        let stored = conn.query_row(
            "SELECT request_json,result_json,error_json,active_attempt_token,completed_attempt_token,completed_at FROM operations WHERE operation_id=?1",
            [entry.operation.id.as_str()],
            |row| Ok(StoredOperationColumns {
                request: row.get(0)?,
                result: row.get(1)?,
                error: row.get(2)?,
                active_attempt_token: row.get(3)?,
                completed_attempt_token: row.get(4)?,
                completed_at: row.get(5)?,
            }),
        )?;
        let canonical_result = entry.result.as_ref().map(canonical_json).transpose()?;
        let canonical_error = entry.error.as_ref().map(canonical_json).transpose()?;
        if stored.request != canonical_json(&entry.request)?
            || stored.result != canonical_result
            || stored.error != canonical_error
        {
            return Err(StoreError::Integrity(format!(
                "operation {} request/result/error JSON is not canonical",
                entry.operation.id
            )));
        }
        if let Some(token) = stored
            .active_attempt_token
            .as_deref()
            .or(stored.completed_attempt_token.as_deref())
        {
            validate_attempt_token(token).map_err(|_| {
                StoreError::Integrity(format!(
                    "operation {} has a non-canonical attempt token",
                    entry.operation.id
                ))
            })?;
        }
        let execution_columns_valid = match entry.operation.status {
            OperationStatus::Accepted => {
                entry.operation.attempt_count == 0
                    && stored.active_attempt_token.is_none()
                    && stored.completed_attempt_token.is_none()
                    && stored.completed_at.is_none()
            }
            OperationStatus::Running => {
                entry.operation.attempt_count > 0
                    && stored.active_attempt_token.is_some()
                    && stored.completed_attempt_token.is_none()
                    && stored.completed_at.is_none()
            }
            OperationStatus::Succeeded | OperationStatus::Failed | OperationStatus::Unsupported => {
                stored.active_attempt_token.is_none()
                    && stored.completed_attempt_token.is_some()
                    && stored.completed_at.is_some()
            }
        };
        if !execution_columns_valid {
            return Err(StoreError::Integrity(format!(
                "operation {} execution columns disagree with status",
                entry.operation.id
            )));
        }
        let computed = canonical_request_fingerprint(&entry.request)?;
        if entry.operation.request_fingerprint != computed {
            return Err(StoreError::Integrity(format!(
                "operation {} request fingerprint mismatch",
                entry.operation.id
            )));
        }
    }

    let mut mappings = conn.prepare("SELECT scope,idempotency_key,request_fingerprint,operation_id FROM idempotency_keys ORDER BY scope,idempotency_key")?;
    for row in mappings.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })? {
        let (scope, key, fingerprint, operation_id) = row?;
        let operation = read_operation_entry(conn, &operation_id)?;
        if fingerprint != operation.operation.request_fingerprint
            || fingerprint != canonical_request_fingerprint(&operation.request)?
        {
            return Err(StoreError::Integrity(format!(
                "idempotency mapping {scope}/{key} disagrees with operation {operation_id}"
            )));
        }
    }

    let mut steps = conn.prepare("SELECT operation_id,step_index,name,status,attempt_count,last_error FROM operation_steps ORDER BY operation_id,step_index")?;
    for row in steps.query_map([], |row| {
        Ok(serde_json::json!({
            "operation_id": row.get::<_, String>(0)?,
            "index": row.get::<_, i64>(1)?,
            "name": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "attempt_count": row.get::<_, i64>(4)?,
            "last_error": row.get::<_, Option<String>>(5)?,
        }))
    })? {
        serde_json::from_value::<OperationStep>(row?)
            .map_err(|error| StoreError::Integrity(format!("invalid operation step: {error}")))?;
    }

    let mut events = conn.prepare("SELECT event_id,sequence,operation_id,vm_id,kind,payload_json FROM events ORDER BY sequence")?;
    for row in events.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })? {
        let (id, sequence, operation_id, vm_id, kind, payload) = row?;
        let value = serde_json::json!({
            "id": id, "sequence": sequence, "operation_id": operation_id,
            "vm_id": vm_id, "kind": kind,
            "payload": serde_json::from_str::<serde_json::Value>(&payload)?,
        });
        serde_json::from_value::<OperationEvent>(value)
            .map_err(|error| StoreError::Integrity(format!("invalid event: {error}")))?;
    }

    let mut markers = conn.prepare("SELECT vm_id,owner_id,ownership,recovery,marker_version FROM ownership_markers ORDER BY vm_id")?;
    for row in markers.query_map([], |row| {
        Ok(serde_json::json!({
            "vm_id": row.get::<_, String>(0)?, "owner_id": row.get::<_, String>(1)?,
            "ownership": row.get::<_, String>(2)?, "recovery": row.get::<_, String>(3)?,
            "marker_version": row.get::<_, i64>(4)?,
        }))
    })? {
        serde_json::from_value::<OwnershipMarker>(row?)
            .map_err(|error| StoreError::Integrity(format!("invalid ownership marker: {error}")))?;
    }
    Ok(())
}

fn persist_accepted_desired_state(
    conn: &Connection,
    request: &AcceptOperation<'_>,
) -> Result<ResourceVersion> {
    let operation = request.operation;
    match operation.kind {
        OperationKind::CreateVm => {
            let desired = required_desired_vm(request)?;
            if desired.resource_version != request.expected_vm_version {
                return Err(StoreError::InvalidDomain(
                    "create desired state must carry the accepted resource version".to_owned(),
                ));
            }
            conn.execute(
                "INSERT INTO vms (vm_id,definition_json,requested_power_state,observed_power_state,resource_version) VALUES (?1,?2,?3,?4,?5)",
                params![desired.id.as_str(), serde_json::to_string(desired)?, requested_text(desired.requested_power_state), observed_text(desired.observed_power_state), version_i64(desired.resource_version)?],
            ).map_err(|error| map_constraint(error, "vm", desired.id.as_str()))?;
            insert_attachments(conn, desired)?;
            Ok(desired.resource_version)
        }
        OperationKind::DeleteVm => {
            if request.desired_vm.is_some() {
                return Err(StoreError::InvalidDomain(
                    "delete must not include desired VM state".to_owned(),
                ));
            }
            let accepted = request
                .expected_vm_version
                .next()
                .map_err(|error| StoreError::InvalidDomain(error.to_string()))?;
            let changed = conn.execute(
                "UPDATE vms SET deleted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),resource_version=?1,definition_json=json_set(definition_json,'$.resource_version',?1),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE vm_id=?2 AND resource_version=?3 AND deleted_at IS NULL",
                params![version_i64(accepted)?, operation.vm_id.as_str(), version_i64(request.expected_vm_version)?],
            )?;
            if changed != 1 {
                return Err(vm_version_error(
                    conn,
                    &operation.vm_id,
                    request.expected_vm_version,
                )?);
            }
            Ok(accepted)
        }
        _ => {
            let desired = required_desired_vm(request)?;
            let accepted = request
                .expected_vm_version
                .next()
                .map_err(|error| StoreError::InvalidDomain(error.to_string()))?;
            if desired.resource_version != accepted {
                return Err(StoreError::InvalidDomain(
                    "desired VM state must carry expected resource version + 1".to_owned(),
                ));
            }
            let changed = conn.execute(
                "UPDATE vms SET definition_json=?1,requested_power_state=?2,observed_power_state=?3,resource_version=?4,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE vm_id=?5 AND resource_version=?6 AND deleted_at IS NULL",
                params![serde_json::to_string(desired)?, requested_text(desired.requested_power_state), observed_text(desired.observed_power_state), version_i64(accepted)?, desired.id.as_str(), version_i64(request.expected_vm_version)?],
            )?;
            if changed != 1 {
                return Err(vm_version_error(
                    conn,
                    &operation.vm_id,
                    request.expected_vm_version,
                )?);
            }
            require_same_attachments(conn, desired)?;
            Ok(accepted)
        }
    }
}

fn required_desired_vm<'a>(request: &'a AcceptOperation<'_>) -> Result<&'a VmDefinition> {
    let desired = request.desired_vm.ok_or_else(|| {
        StoreError::InvalidDomain("mutation requires complete desired VM state".to_owned())
    })?;
    validate_definition(desired)?;
    if desired.id != request.operation.vm_id {
        return Err(StoreError::InvalidDomain(
            "operation and desired VM identifiers differ".to_owned(),
        ));
    }
    Ok(desired)
}

fn canonical_json(value: &serde_json::Value) -> Result<String> {
    Ok(domain_canonical_json(value)?)
}

fn schema_fingerprint(conn: &Connection) -> Result<String> {
    let mut statement = conn.prepare(
        "SELECT type,name,tbl_name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY type,name,tbl_name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut digest = Sha256::new();
    for row in rows {
        let (kind, name, table, sql) = row?;
        for value in [kind, name, table, normalize_schema_sql(&sql)] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn insert_attachments(conn: &Connection, definition: &VmDefinition) -> Result<()> {
    for attachment in &definition.storage {
        conn.execute("INSERT INTO attachments (attachment_id,vm_id,kind,provider_ref,requested_state,observed_state,resource_version) VALUES (?1,?2,'storage',?3,'attached','unknown',?4)", params![attachment.attachment_id, definition.id.as_str(), attachment.storage_ref, version_i64(definition.resource_version)?])?;
    }
    for attachment in &definition.networks {
        conn.execute("INSERT INTO attachments (attachment_id,vm_id,kind,provider_ref,requested_state,observed_state,resource_version) VALUES (?1,?2,'network',?3,'attached','unknown',?4)", params![attachment.attachment_id, definition.id.as_str(), attachment.network_ref, version_i64(definition.resource_version)?])?;
    }
    Ok(())
}

fn require_same_attachments(conn: &Connection, definition: &VmDefinition) -> Result<()> {
    let mut expected = definition
        .storage
        .iter()
        .map(|item| {
            (
                item.attachment_id.clone(),
                "storage".to_owned(),
                item.storage_ref.clone(),
            )
        })
        .chain(definition.networks.iter().map(|item| {
            (
                item.attachment_id.clone(),
                "network".to_owned(),
                item.network_ref.clone(),
            )
        }))
        .collect::<Vec<_>>();
    expected.sort();
    let mut statement = conn.prepare("SELECT attachment_id,kind,provider_ref FROM attachments WHERE vm_id=?1 ORDER BY attachment_id")?;
    let mut actual = statement
        .query_map([definition.id.as_str()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<(String, String, String)>, _>>()?;
    actual.sort();
    if actual != expected {
        return Err(StoreError::InvalidDomain(
            "VM attachment changes require the dedicated attachment operation".to_owned(),
        ));
    }
    Ok(())
}

fn validate_definition(definition: &VmDefinition) -> Result<()> {
    definition
        .validate()
        .map_err(|error| StoreError::InvalidDomain(error.to_string()))
}

fn validate_operation_for_acceptance(operation: &Operation) -> Result<()> {
    operation
        .validate()
        .map_err(|error| StoreError::InvalidDomain(error.to_string()))?;
    if operation.status != OperationStatus::Accepted
        || operation.attempt_count != 0
        || operation.max_attempts == 0
        || operation.request_fingerprint.trim().is_empty()
    {
        return Err(StoreError::InvalidDomain(
            "new operation must be accepted, unattempted, retry-bounded, and fingerprinted"
                .to_owned(),
        ));
    }
    Ok(())
}

fn read_operation(conn: &Connection, id: &str) -> Result<Operation> {
    Ok(read_operation_entry(conn, id)?.operation)
}

fn read_operation_entry(conn: &Connection, id: &str) -> Result<OperationJournalEntry> {
    type RawOperationRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        Option<String>,
    );
    let raw: Option<RawOperationRow> = conn
        .query_row("SELECT operation_id,kind,vm_id,request_fingerprint,request_json,status,retry_count,max_retries,result_json,error_json FROM operations WHERE operation_id=?1", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?)))
        .optional()?;
    let (id, kind, vm_id, fingerprint, request, status, attempts, max_attempts, result, error) =
        raw.ok_or_else(|| StoreError::NotFound {
            kind: "operation",
            id: id.to_owned(),
        })?;
    let operation = Operation {
        id: operation_id(id.clone())?,
        kind: parse_operation_kind(&kind)?,
        vm_id: vm_id_type(vm_id)?,
        status: parse_operation_status(&status)?,
        request_fingerprint: fingerprint,
        attempt_count: u32_value(attempts, "attempt_count")?,
        max_attempts: u32_value(max_attempts, "max_attempts")?,
    };
    operation
        .validate()
        .map_err(|error| StoreError::Integrity(format!("operation {id} is invalid: {error}")))?;
    let entry = OperationJournalEntry {
        operation,
        request: serde_json::from_str(&request)?,
        result: result
            .map(|value| serde_json::from_str(&value))
            .transpose()?,
        error: error
            .map(|value| serde_json::from_str(&value))
            .transpose()?,
    };
    let outcome_valid = match entry.operation.status {
        OperationStatus::Accepted | OperationStatus::Running => {
            entry.result.is_none() && entry.error.is_none()
        }
        OperationStatus::Succeeded => entry.error.is_none(),
        OperationStatus::Failed | OperationStatus::Unsupported => entry.error.is_some(),
    };
    if !outcome_valid {
        return Err(StoreError::Integrity(format!(
            "operation {id} has an inconsistent status/result/error outcome"
        )));
    }
    Ok(entry)
}

fn vm_version(conn: &Connection, id: &VmId) -> Result<ResourceVersion> {
    let value: Option<i64> = conn
        .query_row(
            "SELECT resource_version FROM vms WHERE vm_id=?1 AND deleted_at IS NULL",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    resource_version(value.ok_or_else(|| StoreError::NotFound {
        kind: "vm",
        id: id.to_string(),
    })?)
}

fn vm_version_error(conn: &Connection, id: &VmId, expected: ResourceVersion) -> Result<StoreError> {
    match vm_version(conn, id) {
        Ok(actual) => Ok(StoreError::StaleVersion {
            kind: "vm",
            id: id.to_string(),
            expected: expected.get(),
            actual: actual.get(),
        }),
        Err(StoreError::NotFound { .. }) => Ok(StoreError::NotFound {
            kind: "vm",
            id: id.to_string(),
        }),
        Err(error) => Err(error),
    }
}

fn map_constraint(error: rusqlite::Error, kind: &'static str, id: &str) -> StoreError {
    match &error {
        rusqlite::Error::SqliteFailure(code, message)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            let detail = message.as_deref().unwrap_or("constraint violation");
            if detail.contains("UNIQUE constraint failed")
                || detail.contains("PRIMARY KEY constraint failed")
            {
                StoreError::Conflict {
                    kind,
                    id: id.to_owned(),
                }
            } else {
                StoreError::InvalidDomain(format!(
                    "{kind} {id} violates store constraint: {detail}"
                ))
            }
        }
        _ => StoreError::Sqlite(error),
    }
}

fn host_id(value: String) -> Result<HostId> {
    HostId::new(value).map_err(|error| StoreError::InvalidDomain(error.to_string()))
}
fn vm_id_type(value: String) -> Result<VmId> {
    VmId::new(value).map_err(|error| StoreError::InvalidDomain(error.to_string()))
}
fn operation_id(value: String) -> Result<OperationId> {
    OperationId::new(value).map_err(|error| StoreError::InvalidDomain(error.to_string()))
}
fn resource_version(value: i64) -> Result<ResourceVersion> {
    let value = u64::try_from(value)
        .map_err(|_| StoreError::Schema("negative resource version".to_owned()))?;
    ResourceVersion::new(value).map_err(|error| StoreError::InvalidDomain(error.to_string()))
}
fn version_i64(value: ResourceVersion) -> Result<i64> {
    i64::try_from(value.get())
        .map_err(|_| StoreError::InvalidDomain("resource version exceeds SQLite range".to_owned()))
}
fn u32_value(value: i64, field: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| StoreError::Schema(format!("invalid {field}")))
}

fn requested_text(value: RequestedPowerState) -> &'static str {
    match value {
        RequestedPowerState::Running => "running",
        RequestedPowerState::Stopped => "stopped",
    }
}
fn observed_text(value: ObservedPowerState) -> &'static str {
    match value {
        ObservedPowerState::Unknown => "unknown",
        ObservedPowerState::Created => "created",
        ObservedPowerState::Running => "running",
        ObservedPowerState::Stopped => "stopped",
        ObservedPowerState::Paused => "paused",
        ObservedPowerState::Failed => "failed",
    }
}
fn operation_kind_text(value: OperationKind) -> &'static str {
    match value {
        OperationKind::CreateVm => "create_vm",
        OperationKind::UpdateVm => "update_vm",
        OperationKind::DeleteVm => "delete_vm",
        OperationKind::StartVm => "start_vm",
        OperationKind::StopVm => "stop_vm",
        OperationKind::RebootVm => "reboot_vm",
    }
}
fn parse_operation_kind(value: &str) -> Result<OperationKind> {
    match value {
        "create_vm" => Ok(OperationKind::CreateVm),
        "update_vm" => Ok(OperationKind::UpdateVm),
        "delete_vm" => Ok(OperationKind::DeleteVm),
        "start_vm" => Ok(OperationKind::StartVm),
        "stop_vm" => Ok(OperationKind::StopVm),
        "reboot_vm" => Ok(OperationKind::RebootVm),
        _ => Err(StoreError::Schema(format!(
            "unknown operation kind {value}"
        ))),
    }
}
fn operation_status_text(value: OperationStatus) -> &'static str {
    match value {
        OperationStatus::Accepted => "accepted",
        OperationStatus::Running => "running",
        OperationStatus::Succeeded => "succeeded",
        OperationStatus::Failed => "failed",
        OperationStatus::Unsupported => "unsupported",
    }
}

fn validate_attempt_token(token: &str) -> Result<()> {
    if token.is_empty()
        || token.len() > 128
        || !token
            .as_bytes()
            .iter()
            .all(|byte| (b'!'..=b'~').contains(byte))
    {
        return Err(StoreError::InvalidDomain(
            "attempt token must be 1..=128 visible ASCII bytes".to_owned(),
        ));
    }
    Ok(())
}
fn parse_operation_status(value: &str) -> Result<OperationStatus> {
    match value {
        "accepted" => Ok(OperationStatus::Accepted),
        "running" => Ok(OperationStatus::Running),
        "succeeded" => Ok(OperationStatus::Succeeded),
        "failed" => Ok(OperationStatus::Failed),
        "unsupported" => Ok(OperationStatus::Unsupported),
        _ => Err(StoreError::Schema(format!(
            "unknown operation status {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cellhv_core_types::{BootSpec, ComputeSpec, NetworkAttachmentRef, StorageAttachmentRef};

    fn version(value: u64) -> ResourceVersion {
        ResourceVersion::new(value).unwrap()
    }
    fn vm(id: &str, resource_version: u64) -> VmDefinition {
        VmDefinition {
            id: VmId::new(id).unwrap(),
            name: "test".to_owned(),
            boot: BootSpec::new("kernel-ref").unwrap(),
            compute: ComputeSpec::new(2, 1_073_741_824).unwrap(),
            storage: vec![StorageAttachmentRef {
                attachment_id: "disk-0".to_owned(),
                storage_ref: "volume-1".to_owned(),
                read_only: false,
            }],
            networks: vec![NetworkAttachmentRef {
                attachment_id: "nic-0".to_owned(),
                network_ref: "network-1".to_owned(),
                mac_address: None,
            }],
            requested_power_state: RequestedPowerState::Stopped,
            observed_power_state: ObservedPowerState::Unknown,
            resource_version: version(resource_version),
        }
    }
    fn operation(id: &str, fingerprint: &str) -> Operation {
        Operation {
            id: OperationId::new(id).unwrap(),
            kind: OperationKind::StartVm,
            vm_id: VmId::new("vm-1").unwrap(),
            status: OperationStatus::Accepted,
            request_fingerprint: fingerprint.to_owned(),
            attempt_count: 0,
            max_attempts: 3,
        }
    }
    fn new_store() -> (tempfile::TempDir, PathBuf, CoreStore) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("core.db");
        let store = CoreStore::create_new(&path).unwrap();
        (directory, path, store)
    }

    fn create_v1_store(path: &Path) {
        let mut conn = Connection::open(path).unwrap();
        configure(&conn).unwrap();
        conn.execute_batch(&format!("PRAGMA application_id={APPLICATION_ID};"))
            .unwrap();
        conn.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY,name TEXT NOT NULL UNIQUE,checksum TEXT NOT NULL,schema_fingerprint TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))) STRICT;")
            .unwrap();
        let migration = &MIGRATIONS[0];
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        tx.execute_batch(migration.sql).unwrap();
        tx.pragma_update(None, "user_version", migration.version)
            .unwrap();
        let fingerprint = schema_fingerprint(&tx).unwrap();
        tx.execute(
            "INSERT INTO schema_migrations (version,name,checksum,schema_fingerprint) VALUES (?1,?2,?3,?4)",
            params![migration.version, migration.name, checksum(migration.sql), fingerprint],
        )
        .unwrap();
        tx.commit().unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(conn);
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn host(id: &str, resource_version: u64) -> HostIdentity {
        HostIdentity {
            id: HostId::new(id).unwrap(),
            resource_version: version(resource_version),
        }
    }

    #[test]
    fn fresh_authority_is_published_with_identity_and_survives_reopen() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("core.db");
        let store = CoreStore::create_new_with_host(
            &path,
            &host("host-fresh", 1),
            &HostCapabilities::default(),
        )
        .unwrap();
        assert_eq!(store.host().unwrap().identity.id.as_str(), "host-fresh");
        drop(store);
        assert_eq!(
            CoreStore::open_existing(&path)
                .unwrap()
                .host()
                .unwrap()
                .identity
                .id
                .as_str(),
            "host-fresh"
        );
    }

    #[test]
    fn exact_v1_authority_upgrades_transactionally_to_execution_fencing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("core.db");
        create_v1_store(&path);
        let conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        let definition = vm("vm-1", 1);
        conn.execute(
            "INSERT INTO vms (vm_id,definition_json,requested_power_state,observed_power_state,resource_version) VALUES (?1,?2,'stopped','unknown',1)",
            params![definition.id.as_str(), serde_json::to_string(&definition).unwrap()],
        )
        .unwrap();
        insert_attachments(&conn, &definition).unwrap();
        for (id, status, retries, completed_at) in [
            ("accepted-v1", "accepted", 0, None),
            ("running-v1", "running", 1, None),
            ("succeeded-v1", "succeeded", 1, Some("2026-01-01T00:00:00Z")),
        ] {
            let request = serde_json::json!({"legacy": id});
            conn.execute(
                "INSERT INTO operations (operation_id,kind,vm_id,request_fingerprint,request_json,status,retry_count,max_retries,completed_at) VALUES (?1,'start_vm','vm-1',?2,?3,?4,?5,3,?6)",
                params![id, canonical_request_fingerprint(&request).unwrap(), canonical_json(&request).unwrap(), status, retries, completed_at],
            )
            .unwrap();
        }
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(conn);

        let store = CoreStore::open_existing(&path).unwrap();
        let user_version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, 2);
        let fencing_column: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('operations') WHERE name='active_attempt_token'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fencing_column, 1);
        let running_token: String = store
            .conn
            .query_row(
                "SELECT active_attempt_token FROM operations WHERE operation_id='running-v1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(running_token, "legacy-ambiguous-v1");
        let terminal_token: String = store
            .conn
            .query_row(
                "SELECT completed_attempt_token FROM operations WHERE operation_id='succeeded-v1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(terminal_token, "legacy-completed-v1");
        let incomplete = store.list_incomplete_execution_operations().unwrap();
        assert_eq!(incomplete.len(), 2);
        assert_eq!(
            incomplete[1].active_attempt_token.as_deref(),
            Some("legacy-ambiguous-v1")
        );
        drop(store);
        CoreStore::open_existing(&path).unwrap();
    }

    #[test]
    fn invalid_v1_execution_state_rolls_back_the_entire_upgrade() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("core.db");
        create_v1_store(&path);
        let conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        let definition = vm("vm-1", 1);
        conn.execute(
            "INSERT INTO vms (vm_id,definition_json,requested_power_state,observed_power_state,resource_version) VALUES (?1,?2,'stopped','unknown',1)",
            params![definition.id.as_str(), serde_json::to_string(&definition).unwrap()],
        )
        .unwrap();
        insert_attachments(&conn, &definition).unwrap();
        let request = serde_json::json!({"legacy":"invalid-running"});
        conn.execute(
            "INSERT INTO operations (operation_id,kind,vm_id,request_fingerprint,request_json,status,retry_count,max_retries) VALUES ('invalid-running','start_vm','vm-1',?1,?2,'running',0,3)",
            params![canonical_request_fingerprint(&request).unwrap(), canonical_json(&request).unwrap()],
        )
        .unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(conn);

        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));
        let conn = Connection::open(&path).unwrap();
        let user_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, 1);
        let fencing_columns: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('operations') WHERE name IN ('active_attempt_token','completed_attempt_token')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fencing_columns, 0);
    }

    #[test]
    fn host_insert_failure_leaves_no_authority_or_staging_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("core.db");
        let result = CoreStore::create_new_with_host_inner(
            &path,
            &host("host-fresh", 1),
            &HostCapabilities::default(),
            || {
                Err(StoreError::Integrity(
                    "injected host insertion failure".to_owned(),
                ))
            },
            |_| Ok(()),
        );
        assert!(matches!(result, Err(StoreError::Integrity(_))));
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn post_publish_error_preserves_complete_observable_authority() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("core.db");
        let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observer = std::sync::Arc::clone(&observed);
        let result = CoreStore::create_new_with_host_inner(
            &path,
            &host("host-published", 1),
            &HostCapabilities::default(),
            || Ok(()),
            move |published| {
                let store = CoreStore::open_existing(published)?;
                observer.store(
                    store.host()?.identity.id.as_str() == "host-published",
                    Ordering::SeqCst,
                );
                Err(StoreError::Integrity(
                    "injected post-publication failure".to_owned(),
                ))
            },
        );
        assert!(matches!(result, Err(StoreError::Integrity(_))));
        assert!(observed.load(Ordering::SeqCst));
        assert_eq!(
            CoreStore::open_existing(&path)
                .unwrap()
                .host()
                .unwrap()
                .identity
                .id
                .as_str(),
            "host-published"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn fresh_authority_rejects_unsafe_parent_and_non_utf8_path_before_mutation() {
        use std::os::unix::ffi::OsStringExt;

        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o750)).unwrap();
        let path = directory.path().join("core.db");
        assert!(matches!(
            CoreStore::create_new_with_host(
                &path,
                &host("host-fresh", 1),
                &HostCapabilities::default()
            ),
            Err(StoreError::Integrity(_))
        ));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let invalid = directory
            .path()
            .join(std::ffi::OsString::from_vec(vec![b'c', 0xff, b'v']));
        assert!(matches!(
            CoreStore::create_new_with_host(
                &invalid,
                &host("host-fresh", 1),
                &HostCapabilities::default()
            ),
            Err(StoreError::Integrity(_))
        ));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn invalid_fresh_identity_never_creates_a_file() {
        for identity in [host("reserved", 2), host("unknown", 1)] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("core.db");
            assert!(matches!(
                CoreStore::create_new_with_host(&path, &identity, &HostCapabilities::default()),
                Err(StoreError::InvalidDomain(_))
            ));
            assert!(!path.exists());
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn concurrent_fresh_creators_publish_exactly_one_authority() {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = Arc::new(directory.path().join("core.db"));
        let barrier = Arc::new(Barrier::new(2));
        let mut joins = Vec::new();
        for id in ["host-a", "host-b"] {
            let path = Arc::clone(&path);
            let barrier = Arc::clone(&barrier);
            joins.push(std::thread::spawn(move || {
                barrier.wait();
                CoreStore::create_new_with_host(&path, &host(id, 1), &HostCapabilities::default())
                    .map(|store| store.host().unwrap().identity.id.to_string())
            }));
        }
        let results: Vec<_> = joins.into_iter().map(|join| join.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(StoreError::AlreadyExists(_))))
                .count(),
            1
        );
        let winner = results.into_iter().find_map(Result::ok).unwrap();
        assert_eq!(
            CoreStore::open_existing(&path)
                .unwrap()
                .host()
                .unwrap()
                .identity
                .id
                .as_str(),
            winner
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn typed_host_and_vm_crud_survive_reopen() {
        let (_directory, path, mut store) = new_store();
        let identity = HostIdentity {
            id: HostId::new("host-1").unwrap(),
            resource_version: version(1),
        };
        store
            .create_host(&identity, &HostCapabilities::default())
            .unwrap();
        assert!(matches!(
            store.create_host(&identity, &HostCapabilities::default()),
            Err(StoreError::Conflict { .. })
        ));
        assert_eq!(
            store
                .update_host(version(1), &HostCapabilities::default())
                .unwrap()
                .identity
                .resource_version,
            version(2)
        );
        store.create_vm(&vm("vm-1", 1)).unwrap();
        store.update_vm(version(1), &vm("vm-1", 2)).unwrap();
        drop(store);
        let reopened = CoreStore::open_existing(&path).unwrap();
        assert_eq!(reopened.host().unwrap().identity.id.as_str(), "host-1");
        assert_eq!(reopened.list_vms().unwrap()[0].resource_version, version(2));
    }

    #[test]
    fn scoped_idempotency_replays_and_conflicts() {
        let (_directory, _path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let key = IdempotencyKey::new("request-1").unwrap();
        let request_json = serde_json::json!({"action":"start"});
        let first = operation(
            "op-1",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        let request = AcceptOperation {
            operation: &first,
            request: &request_json,
            desired_vm: Some(&desired),
            idempotency_scope: "caller-a",
            idempotency_key: &key,
            expected_vm_version: version(1),
        };
        assert_eq!(
            store.accept_operation(&request).unwrap().disposition,
            Acceptance::Accepted
        );
        let replay_operation = operation(
            "ignored",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let replay = AcceptOperation {
            operation: &replay_operation,
            ..request
        };
        assert_eq!(
            store
                .accept_operation(&replay)
                .unwrap()
                .operation
                .id
                .as_str(),
            "op-1"
        );
        let other_scope_operation = operation(
            "op-2",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let mut desired_v3 = desired.clone();
        desired_v3.resource_version = version(3);
        let other_scope = AcceptOperation {
            operation: &other_scope_operation,
            idempotency_scope: "caller-b",
            expected_vm_version: version(2),
            desired_vm: Some(&desired_v3),
            ..request
        };
        assert_eq!(
            store.accept_operation(&other_scope).unwrap().disposition,
            Acceptance::Accepted
        );
        let changed_json = serde_json::json!({"action":"stop"});
        let conflict_operation = operation(
            "op-3",
            &canonical_request_fingerprint(&changed_json).unwrap(),
        );
        let conflict = AcceptOperation {
            operation: &conflict_operation,
            request: &changed_json,
            ..request
        };
        assert!(matches!(
            store.accept_operation(&conflict),
            Err(StoreError::IdempotencyConflict { .. })
        ));
    }

    #[test]
    fn stale_acceptance_and_failed_event_roll_back_completely() {
        let (_directory, _path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let key = IdempotencyKey::new("request-1").unwrap();
        let request_json = serde_json::json!({"action":"start"});
        let op = operation(
            "op-stale",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let mut desired = vm("vm-1", 3);
        desired.requested_power_state = RequestedPowerState::Running;
        let stale = AcceptOperation {
            operation: &op,
            request: &request_json,
            desired_vm: Some(&desired),
            idempotency_scope: "caller",
            idempotency_key: &key,
            expected_vm_version: version(2),
        };
        assert!(matches!(
            store.accept_operation(&stale),
            Err(StoreError::StaleVersion { .. })
        ));
        assert!(matches!(
            store.operation(&op.id),
            Err(StoreError::NotFound { .. })
        ));

        store.conn.execute_batch("CREATE TRIGGER reject_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        let failed_op = operation(
            "op-failed",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let mut failed_desired = desired.clone();
        failed_desired.resource_version = version(2);
        let failed = AcceptOperation {
            operation: &failed_op,
            expected_vm_version: version(1),
            desired_vm: Some(&failed_desired),
            ..stale
        };
        assert!(store.accept_operation(&failed).is_err());
        assert!(matches!(
            store.operation(&failed_op.id),
            Err(StoreError::NotFound { .. })
        ));
        let mappings: i64 = store
            .conn
            .query_row("SELECT count(*) FROM idempotency_keys", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(mappings, 0);
    }

    #[test]
    fn mismatched_request_fingerprint_is_rejected_without_writes() {
        let (_directory, _path, mut store) = new_store();
        let original = vm("vm-1", 1);
        store.create_vm(&original).unwrap();
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        let operation = operation("op-mismatch", "not-the-request-digest");
        let error = store
            .accept_operation(&AcceptOperation {
                operation: &operation,
                request: &serde_json::json!({"action":"start"}),
                desired_vm: Some(&desired),
                idempotency_scope: "caller",
                idempotency_key: &IdempotencyKey::new("key-mismatch").unwrap(),
                expected_vm_version: version(1),
            })
            .unwrap_err();
        assert!(matches!(error, StoreError::InvalidDomain(_)));
        assert_eq!(store.get_vm(&original.id).unwrap(), original);
        assert!(matches!(
            store.operation(&operation.id),
            Err(StoreError::NotFound { .. })
        ));
        for table in ["operations", "idempotency_keys", "events"] {
            let count: i64 = store
                .conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "unexpected row in {table}");
        }
    }

    #[test]
    fn corruption_empty_unknown_and_future_schema_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            ("corrupt.db", b"not sqlite".as_slice()),
            ("empty.db", b"".as_slice()),
        ] {
            let path = directory.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            assert!(CoreStore::open_existing(&path).is_err());
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
        let (_owned, path, store) = new_store();
        store.conn.pragma_update(None, "user_version", 999).unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Schema(_))
        ));
    }

    #[test]
    fn missing_parent_and_existing_file_are_never_removed() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing/core.db");
        assert!(CoreStore::create_new(&missing).is_err());
        assert!(!missing.exists());
        let existing = directory.path().join("existing.db");
        std::fs::write(&existing, b"keep me").unwrap();
        assert!(matches!(
            CoreStore::create_new(&existing),
            Err(StoreError::AlreadyExists(_))
        ));
        assert_eq!(std::fs::read(existing).unwrap(), b"keep me");
    }

    #[test]
    fn migration_checksum_and_foreign_keys_are_enforced() {
        let (_directory, path, store) = new_store();
        assert!(store.conn.execute("INSERT INTO attachments (attachment_id,vm_id,kind,provider_ref,requested_state,observed_state,resource_version) VALUES ('bad','missing','network','n','attached','unknown',1)", []).is_err());
        store
            .conn
            .execute(
                "UPDATE schema_migrations SET checksum='tampered' WHERE version=1",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Schema(_))
        ));
    }

    #[test]
    fn distinct_keys_cannot_accept_the_same_resource_version() {
        let (_directory, _path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let first_key = IdempotencyKey::new("key-1").unwrap();
        let request_json = serde_json::json!({"action":"start"});
        let first_operation = operation(
            "op-1",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        let first = AcceptOperation {
            operation: &first_operation,
            request: &request_json,
            desired_vm: Some(&desired),
            idempotency_scope: "caller",
            idempotency_key: &first_key,
            expected_vm_version: version(1),
        };
        let accepted = store.accept_operation(&first).unwrap();
        assert_eq!(accepted.accepted_resource_version, version(2));

        let second_operation = operation(
            "op-2",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let second_key = IdempotencyKey::new("key-2").unwrap();
        let second = AcceptOperation {
            operation: &second_operation,
            idempotency_key: &second_key,
            ..first
        };
        assert!(matches!(
            store.accept_operation(&second),
            Err(StoreError::StaleVersion {
                expected: 1,
                actual: 2,
                ..
            })
        ));
        assert!(matches!(
            store.operation(&second_operation.id),
            Err(StoreError::NotFound { .. })
        ));
    }

    #[test]
    fn vm_deletion_is_a_tombstone_and_preserves_operation_history() {
        let (_directory, _path, mut store) = new_store();
        let vm_id = VmId::new("vm-1").unwrap();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let key = IdempotencyKey::new("key-1").unwrap();
        let request_json = serde_json::json!({"action":"start"});
        let op = operation(
            "op-1",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        store
            .accept_operation(&AcceptOperation {
                operation: &op,
                request: &request_json,
                desired_vm: Some(&desired),
                idempotency_scope: "caller",
                idempotency_key: &key,
                expected_vm_version: version(1),
            })
            .unwrap();
        store.delete_vm(&vm_id, version(2)).unwrap();
        assert!(matches!(
            store.get_vm(&vm_id),
            Err(StoreError::NotFound { .. })
        ));
        assert!(store.list_vms().unwrap().is_empty());
        assert_eq!(store.operation(&op.id).unwrap().id, op.id);
    }

    #[test]
    fn schema_and_vm_column_tampering_fail_on_reopen() {
        let (_directory, path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        store
            .conn
            .execute(
                "UPDATE vms SET requested_power_state='running' WHERE vm_id='vm-1'",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));

        let (_directory, path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        store
            .conn
            .execute(
                "UPDATE attachments SET provider_ref='tampered' WHERE attachment_id='disk-0'",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));

        let (_directory, path, store) = new_store();
        store
            .conn
            .execute_batch(
                "DROP INDEX events_vm_id_idx; CREATE INDEX events_vm_id_idx ON events(kind);",
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Schema(_))
        ));
    }

    #[test]
    fn request_fingerprint_and_idempotency_tampering_fail_on_reopen() {
        fn accepted_store() -> (tempfile::TempDir, PathBuf, CoreStore) {
            let (directory, path, mut store) = new_store();
            store.create_vm(&vm("vm-1", 1)).unwrap();
            let request = serde_json::json!({"command":"start","expected_vm_version":1});
            let operation = operation("op-1", &canonical_request_fingerprint(&request).unwrap());
            let mut desired = vm("vm-1", 2);
            desired.requested_power_state = RequestedPowerState::Running;
            store
                .accept_operation(&AcceptOperation {
                    operation: &operation,
                    request: &request,
                    desired_vm: Some(&desired),
                    idempotency_scope: "caller",
                    idempotency_key: &IdempotencyKey::new("key").unwrap(),
                    expected_vm_version: version(1),
                })
                .unwrap();
            (directory, path, store)
        }

        let (_directory, path, store) = accepted_store();
        store
            .conn
            .execute(
                "UPDATE operations SET request_fingerprint='tampered' WHERE operation_id='op-1'",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));

        let (_directory, path, store) = accepted_store();
        store
            .conn
            .execute(
                "UPDATE idempotency_keys SET request_fingerprint='tampered' WHERE idempotency_key='key'",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));
    }

    #[test]
    fn create_acceptance_atomically_persists_vm_request_and_journal() {
        let (_directory, path, mut store) = new_store();
        let definition = vm("vm-created", 1);
        let mut op = operation("op-create", "sha256:create");
        op.kind = OperationKind::CreateVm;
        op.vm_id = definition.id.clone();
        let key = IdempotencyKey::new("create-1").unwrap();
        let request_json = serde_json::json!({"definition": definition});
        op.request_fingerprint = canonical_request_fingerprint(&request_json).unwrap();
        let accepted = store
            .accept_operation(&AcceptOperation {
                operation: &op,
                request: &request_json,
                desired_vm: Some(&definition),
                idempotency_scope: "caller",
                idempotency_key: &key,
                expected_vm_version: version(1),
            })
            .unwrap();
        assert_eq!(accepted.accepted_resource_version, version(1));
        assert_eq!(store.get_vm(&definition.id).unwrap(), definition);
        assert_eq!(store.operation_entry(&op.id).unwrap().request, request_json);
        drop(store);
        assert_eq!(
            CoreStore::open_existing(&path)
                .unwrap()
                .get_vm(&definition.id)
                .unwrap(),
            definition
        );
    }

    #[test]
    fn accepted_power_and_delete_desired_state_survive_restart() {
        let (_directory, path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        let key = IdempotencyKey::new("start-1").unwrap();
        let request_json = serde_json::json!({"action":"start"});
        let start = operation(
            "op-start",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        store
            .accept_operation(&AcceptOperation {
                operation: &start,
                request: &request_json,
                desired_vm: Some(&desired),
                idempotency_scope: "caller",
                idempotency_key: &key,
                expected_vm_version: version(1),
            })
            .unwrap();
        assert_eq!(store.get_vm(&desired.id).unwrap(), desired);

        let mut delete = operation("op-delete", "sha256:delete");
        delete.kind = OperationKind::DeleteVm;
        let delete_key = IdempotencyKey::new("delete-1").unwrap();
        let delete_json = serde_json::json!({"delete":true});
        delete.request_fingerprint = canonical_request_fingerprint(&delete_json).unwrap();
        store
            .accept_operation(&AcceptOperation {
                operation: &delete,
                request: &delete_json,
                desired_vm: None,
                idempotency_scope: "caller",
                idempotency_key: &delete_key,
                expected_vm_version: version(2),
            })
            .unwrap();
        drop(store);
        let reopened = CoreStore::open_existing(&path).unwrap();
        assert!(matches!(
            reopened.get_vm(&desired.id),
            Err(StoreError::NotFound { .. })
        ));
        assert_eq!(
            reopened.operation(&delete.id).unwrap().kind,
            OperationKind::DeleteVm
        );
    }

    #[test]
    fn attachment_ids_are_scoped_per_vm_and_duplicates_are_rejected() {
        let (_directory, _path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        store.create_vm(&vm("vm-2", 1)).unwrap();
        let mut duplicate = vm("vm-3", 1);
        duplicate.networks[0].attachment_id = duplicate.storage[0].attachment_id.clone();
        assert!(matches!(
            store.create_vm(&duplicate),
            Err(StoreError::InvalidDomain(_))
        ));
    }

    #[test]
    fn terminal_outcomes_are_canonical_typed_and_single_assignment() {
        let (_directory, path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        let key = IdempotencyKey::new("start-1").unwrap();
        let request_json = serde_json::json!({"z":1,"a":{"y":2,"b":3}});
        let op = operation(
            "op-1",
            &canonical_request_fingerprint(&request_json).unwrap(),
        );
        store
            .accept_operation(&AcceptOperation {
                operation: &op,
                request: &request_json,
                desired_vm: Some(&desired),
                idempotency_scope: "caller",
                idempotency_key: &key,
                expected_vm_version: version(1),
            })
            .unwrap();
        store.claim_operation(&op.id, "attempt-1").unwrap();
        assert_eq!(
            store
                .claim_operation(&op.id, "attempt-1")
                .unwrap()
                .entry
                .operation
                .attempt_count,
            1
        );
        assert!(matches!(
            store.claim_operation(&op.id, "attempt-2"),
            Err(StoreError::Conflict { .. })
        ));
        let result = serde_json::json!({"z":true,"a":1});
        assert!(matches!(
            store.persist_terminal_operation(
                &op.id,
                "attempt-2",
                OperationStatus::Succeeded,
                Some(&result),
                None,
            ),
            Err(StoreError::Conflict { .. })
        ));
        let entry = store
            .persist_terminal_operation(
                &op.id,
                "attempt-1",
                OperationStatus::Succeeded,
                Some(&result),
                None,
            )
            .unwrap();
        assert_eq!(entry.disposition, CompletionDisposition::Applied);
        assert_eq!(entry.entry.result, Some(result));
        assert_eq!(
            store
                .persist_terminal_operation(
                    &op.id,
                    "attempt-1",
                    OperationStatus::Succeeded,
                    entry.entry.result.as_ref(),
                    None,
                )
                .unwrap()
                .disposition,
            CompletionDisposition::Replay
        );
        let event_kinds: String = store
            .conn
            .query_row(
                "SELECT group_concat(kind, ',') FROM events WHERE operation_id=?1 ORDER BY sequence",
                [op.id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            event_kinds,
            "operation.accepted,operation.running,operation.succeeded"
        );
        assert!(matches!(
            store.persist_terminal_operation(
                &op.id,
                "attempt-1",
                OperationStatus::Failed,
                None,
                Some(&serde_json::json!({"message":"late"})),
            ),
            Err(StoreError::Conflict { .. })
        ));
        drop(store);
        assert_eq!(
            CoreStore::open_existing(&path)
                .unwrap()
                .operation(&op.id)
                .unwrap()
                .status,
            OperationStatus::Succeeded
        );
    }

    #[test]
    fn attempt_token_sql_constraints_and_cross_column_reopen_checks_fail_closed() {
        let (_directory, path, mut store) = new_store();
        store.create_vm(&vm("vm-1", 1)).unwrap();
        let mut desired = vm("vm-1", 2);
        desired.requested_power_state = RequestedPowerState::Running;
        let request = serde_json::json!({"command":"start"});
        let op = operation(
            "op-token",
            &canonical_request_fingerprint(&request).unwrap(),
        );
        store
            .accept_operation(&AcceptOperation {
                operation: &op,
                request: &request,
                desired_vm: Some(&desired),
                idempotency_scope: "token-test",
                idempotency_key: &IdempotencyKey::new("token-test").unwrap(),
                expected_vm_version: version(1),
            })
            .unwrap();
        for invalid in ["", "has space", "line\nbreak"] {
            assert!(store
                .conn
                .execute(
                    "UPDATE operations SET active_attempt_token=?1 WHERE operation_id='op-token'",
                    [invalid],
                )
                .is_err());
        }
        assert!(store
            .conn
            .execute(
                "UPDATE operations SET active_attempt_token=?1 WHERE operation_id='op-token'",
                ["x".repeat(129)],
            )
            .is_err());
        store
            .conn
            .execute(
                "UPDATE operations SET active_attempt_token='valid-token' WHERE operation_id='op-token'",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));
    }

    #[test]
    fn imported_and_cutover_migration_markers_survive_validation() {
        let (_directory, path, mut store) = new_store();
        let host = HostIdentity {
            id: HostId::new("legacy-host").unwrap(),
            resource_version: version(1),
        };
        let definitions = vec![vm("vm-b", 2), vm("vm-a", 3)];
        assert_eq!(
            store
                .import_legacy_snapshot(
                    "node-cache-v1",
                    "checksum",
                    &host,
                    &HostCapabilities::default(),
                    &definitions
                )
                .unwrap(),
            MigrationDisposition::Imported
        );
        drop(store);
        let mut reopened = CoreStore::open_existing(&path).unwrap();
        assert_eq!(
            reopened
                .cutover_legacy_snapshot("node-cache-v1", "checksum")
                .unwrap(),
            MigrationDisposition::Cutover
        );
        drop(reopened);
        CoreStore::open_existing(&path).unwrap();
    }

    #[test]
    fn migration_marker_semantic_tampering_fails_reopen() {
        let (_directory, path, mut store) = new_store();
        let host = HostIdentity {
            id: HostId::new("legacy-host").unwrap(),
            resource_version: version(1),
        };
        store
            .import_legacy_snapshot(
                "node-cache-v1",
                "checksum",
                &host,
                &HostCapabilities::default(),
                &[vm("vm-a", 1)],
            )
            .unwrap();
        store
            .conn
            .execute(
                "UPDATE migration_state SET imported_vm_ids_json='[\"z\",\"a\"]'",
                [],
            )
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));

        let (_directory, path, mut store) = new_store();
        let host = HostIdentity {
            id: HostId::new("legacy-host").unwrap(),
            resource_version: version(1),
        };
        store
            .import_legacy_snapshot(
                "node-cache-v1",
                "checksum",
                &host,
                &HostCapabilities::default(),
                &[vm("vm-a", 1)],
            )
            .unwrap();
        store
            .conn
            .execute("UPDATE migration_state SET imported_at='not-a-time'", [])
            .unwrap();
        drop(store);
        assert!(matches!(
            CoreStore::open_existing(&path),
            Err(StoreError::Integrity(_))
        ));
    }

    #[test]
    fn rollback_refuses_authority_drift() {
        let (_directory, _path, mut store) = new_store();
        let host = HostIdentity {
            id: HostId::new("legacy-host").unwrap(),
            resource_version: version(1),
        };
        store
            .import_legacy_snapshot(
                "node-cache-v1",
                "checksum",
                &host,
                &HostCapabilities::default(),
                &[vm("vm-a", 1)],
            )
            .unwrap();
        store.create_vm(&vm("post-import", 1)).unwrap();
        assert!(matches!(
            store.rollback_legacy_import("node-cache-v1", "checksum"),
            Err(StoreError::Conflict {
                kind: "migration_rollback_drift",
                ..
            })
        ));
        assert_eq!(store.list_vms().unwrap().len(), 2);
    }

    #[test]
    fn sqlite_sidecars_are_rejected_before_open() {
        for suffix in ["-wal", "-shm"] {
            let (_directory, path, store) = new_store();
            drop(store);
            let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
            let target = path.parent().unwrap().join(format!("target{suffix}"));
            std::fs::write(&target, b"external").unwrap();
            std::os::unix::fs::symlink(&target, &sidecar).unwrap();
            assert!(matches!(
                CoreStore::open_existing(&path),
                Err(StoreError::Integrity(_))
            ));

            std::fs::remove_file(&sidecar).unwrap();
            std::fs::create_dir(&sidecar).unwrap();
            assert!(matches!(
                CoreStore::open_existing(&path),
                Err(StoreError::Integrity(_))
            ));
        }
    }
}
