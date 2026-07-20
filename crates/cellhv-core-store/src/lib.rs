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
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

const APPLICATION_ID: i32 = 0x4348_5643; // CHVC
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MIGRATION_SQL: &str = include_str!("../migrations/0001_core_authority.sql");

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

#[derive(Debug, Clone, PartialEq)]
pub struct OperationJournalEntry {
    pub operation: Operation,
    pub request: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "core_authority",
    sql: MIGRATION_SQL,
}];

pub struct CoreStore {
    conn: Connection,
}

impl CoreStore {
    /// Atomically reserve a new file and initialize it. Parent directories
    /// must already exist; a pre-existing file is never opened or removed.
    pub fn create_new(path: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
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
            let mut conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            configure(&conn)?;
            apply_migrations(&mut conn)?;
            validate(&conn)?;
            Ok(Self { conn })
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(path);
        }
        result
    }

    pub fn open_existing(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(StoreError::Missing(path.to_path_buf()));
        }
        // Validate through a read-only handle first. In particular, an empty,
        // corrupt, or foreign file must not be initialized by connection
        // pragmas before it has proved it is a compatible Core store.
        let read_only = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        read_only.busy_timeout(BUSY_TIMEOUT)?;
        validate(&read_only)?;
        drop(read_only);
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        configure(&conn)?;
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

    /// Persistence primitive for the application service. Callers must not
    /// perform side effects until this transition has committed.
    #[doc(hidden)]
    pub fn claim_operation(&mut self, id: &OperationId) -> Result<OperationJournalEntry> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_operation(&tx, id.as_str())?;
        if !matches!(
            current.status,
            OperationStatus::Accepted | OperationStatus::Running
        ) || current.attempt_count >= current.max_attempts
        {
            return Err(StoreError::Conflict {
                kind: "operation",
                id: id.to_string(),
            });
        }
        let attempt = current.attempt_count + 1;
        let changed = tx.execute(
            "UPDATE operations SET status='running',retry_count=?1 WHERE operation_id=?2 AND status=?3 AND retry_count=?4",
            params![i64::from(attempt), id.as_str(), operation_status_text(current.status), i64::from(current.attempt_count)],
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
        Ok(entry)
    }

    /// Persistence primitive used by the single operation application service.
    #[doc(hidden)]
    pub fn persist_terminal_operation(
        &mut self,
        id: &OperationId,
        status: OperationStatus,
        result: Option<&serde_json::Value>,
        error: Option<&serde_json::Value>,
    ) -> Result<OperationJournalEntry> {
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
        let allowed_source = if status == OperationStatus::Unsupported {
            "status IN ('accepted','running')"
        } else {
            "status='running'"
        };
        let changed = tx.execute(
            &format!("UPDATE operations SET status=?1,result_json=?2,error_json=?3,completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE operation_id=?4 AND {allowed_source}"),
            params![operation_status_text(status), result, error, id.as_str()],
        )?;
        if changed != 1 {
            return match read_operation(&tx, id.as_str()) {
                Ok(_) => Err(StoreError::Conflict {
                    kind: "operation",
                    id: id.to_string(),
                }),
                Err(error) => Err(error),
            };
        }
        let operation = read_operation(&tx, id.as_str())?;
        tx.execute(
            "INSERT INTO events (event_id,sequence,operation_id,vm_id,kind,payload_json) VALUES (?1,(SELECT coalesce(max(sequence),0)+1 FROM events),?2,?3,?4,?5)",
            params![format!("{}:terminal", id.as_str()), id.as_str(), operation.vm_id.as_str(), format!("operation.{}", operation_status_text(status)), canonical_json(&serde_json::json!({"status":operation_status_text(status)}))?],
        )?;
        let entry = read_operation_entry(&tx, id.as_str())?;
        tx.commit()?;
        Ok(entry)
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
        let stored: (String, Option<String>, Option<String>) = conn.query_row(
            "SELECT request_json,result_json,error_json FROM operations WHERE operation_id=?1",
            [entry.operation.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let canonical_result = entry.result.as_ref().map(canonical_json).transpose()?;
        let canonical_error = entry.error.as_ref().map(canonical_json).transpose()?;
        if stored.0 != canonical_json(&entry.request)?
            || stored.1 != canonical_result
            || stored.2 != canonical_error
        {
            return Err(StoreError::Integrity(format!(
                "operation {} request/result/error JSON is not canonical",
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
        store.claim_operation(&op.id).unwrap();
        let result = serde_json::json!({"z":true,"a":1});
        let entry = store
            .persist_terminal_operation(&op.id, OperationStatus::Succeeded, Some(&result), None)
            .unwrap();
        assert_eq!(entry.result, Some(result));
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
}
