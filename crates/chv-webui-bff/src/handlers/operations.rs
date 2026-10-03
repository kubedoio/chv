//! Shared per-resource idempotency-key handling for BFF operation rows
//! (issue #406).
//!
//! Several BFF handlers derive a per-resource idempotency key
//! (`delete-vm-<vm_id>`, `resize-vm-<vm_id>-<cpu>-<mem>`, ...) and record
//! it as a UNIQUE row in the shared `operations` table. The
//! authority-side retention model (M2.5) keeps the `vms`/
//! `vm_desired_state` rows after a delete, so a retried operation on the
//! same resource re-enters the handler and re-derives the same key.
//! Before #406, the plain `INSERT INTO operations` collided on the
//! UNIQUE constraint and the error surfaced as an unhandled
//! `BffError::Internal` → HTTP 500.
//!
//! The contract here: a key collision means the operation was already
//! recorded by a previous (accepted) request, so the retry must REPLAY
//! the recorded outcome class — the original 2xx with the recorded
//! `task_id`/`next_refresh_path` — and must NOT re-execute the
//! underlying mutation. This mirrors the controlplane store's own
//! `OperationRepository::create_or_get` (`ON CONFLICT (idempotency_key)
//! DO NOTHING` + re-select), which the raw handler SQL bypassed.
//!
//! Nothing in this module changes first-operation semantics, key
//! derivation, or retention behavior.

use crate::BffError;

/// An operation row previously recorded under a per-resource
/// idempotency key.
pub struct RecordedOperation {
    pub operation_id: String,
    pub status: String,
}

/// Look up the operation recorded under `idempotency_key`, if any.
///
/// Callers must invoke this inside the same write transaction
/// (`BEGIN IMMEDIATE`) that later performs the operation INSERT, and
/// BEFORE executing any mutation: a hit means the request is a retry of
/// an already-recorded operation and must replay the recorded outcome
/// instead of re-running the mutation. The `BEGIN IMMEDIATE`
/// serialization makes this check-then-insert pair race-free against
/// concurrent writers of the same key.
pub async fn find_recorded_operation(
    conn: &mut sqlx::SqliteConnection,
    idempotency_key: &str,
) -> Result<Option<RecordedOperation>, BffError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT operation_id, status FROM operations WHERE idempotency_key = ?")
            .bind(idempotency_key)
            .fetch_optional(conn)
            .await
            .map_err(|e| {
                BffError::Internal(format!(
                    "failed to look up idempotency key {}: {}",
                    idempotency_key, e
                ))
            })?;
    Ok(row.map(|(operation_id, status)| RecordedOperation {
        operation_id,
        status,
    }))
}

/// Fail-closed mapping for a failed operation INSERT (#406).
///
/// The replay pre-check plus `BEGIN IMMEDIATE` make an
/// idempotency-key collision at INSERT time unreachable; this guard
/// keeps the collision class from ever regressing to an opaque 500 if
/// a future call site skips the pre-check or the serialization. If the
/// INSERT failed because a row already exists under the key, surface a
/// 409 that names the idempotent-retry condition — the caller's
/// transaction rolls back, so the mutation is not re-executed either
/// way. Anything else is a genuine internal error.
/// Classify an operation-INSERT failure for the per-resource key
/// surfaces: if a row is now recorded under `idempotency_key`, surface a
/// 409 naming the idempotent-retry condition; anything else is a genuine
/// internal error (500).
///
/// Classification is by RE-SELECT heuristic, not by inspecting the
/// sqlx error's constraint: a failed INSERT plus a row found under the
/// key is treated as a collision. Under the handlers' `BEGIN IMMEDIATE`
/// tx + in-tx pre-check this arm is unreachable defense-in-depth, so
/// the heuristic's theoretical imprecision (a non-collision insert
/// failure while a row happens to exist) has no reachable impact.
pub async fn map_operation_insert_error(
    conn: &mut sqlx::SqliteConnection,
    idempotency_key: &str,
    err: sqlx::Error,
) -> BffError {
    match find_recorded_operation(conn, idempotency_key).await {
        Ok(Some(recorded)) => BffError::Conflict(format!(
            "an operation is already recorded under idempotency key {} (operation {}, status {}); \
             the retry must replay that recorded outcome",
            idempotency_key, recorded.operation_id, recorded.status
        )),
        _ => BffError::Internal(format!("failed to insert operation: {}", err)),
    }
}
