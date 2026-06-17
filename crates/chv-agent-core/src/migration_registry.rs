//! Tracking registry for in-flight VM migration tasks.
//!
//! Background
//! ----------
//! `migrate_vm` is a fire-and-forget RPC: the agent ACKs immediately and runs
//! the actual migration on a detached `tokio::spawn`. Before this registry
//! existed, the returned `JoinHandle` was dropped on the floor, so the agent
//! had no way to:
//!
//! - **Abort on operator-issued cancel** (the control plane's
//!   `request_migration_cancel` only flips a DB flag the agent never re-checks).
//! - **Reap on agent shutdown** (in-flight tasks would be torn down abruptly
//!   by the runtime exit, with no chance to surface terminal failure to the
//!   control plane).
//! - **Observe terminal `Err`** from the spawned future (stdout-only logging
//!   was the previous "interface").
//!
//! See `docs/specs/adr/ADR-008-error-handling.md` and
//! `ADR-009-async-safety.md` for the agent-side cancel/shutdown contracts.
//!
//! Design
//! ------
//! - Each tracked migration registers an `AbortHandle` keyed by `operation_id`.
//! - A reaper task removes the entry when the future completes, regardless of
//!   `Ok` / `Err` / panic / abort.
//! - `cancel(op_id)` triggers the `AbortHandle`. The `CancellationToken`
//!   companion (passed into the migration future) gives the future a chance
//!   to unwind cleanly at phase boundaries before the abort hits an `.await`.
//! - The registry is stored as `Arc<MigrationTaskRegistry>` inside `AgentServer`.
//!   When the last `AgentServer` clone drops, the inner `Drop` aborts every
//!   tracked task — the agent's "graceful shutdown" entry point.

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

/// A handle for a single tracked migration.
struct TrackedTask {
    abort: AbortHandle,
    cancel_token: CancellationToken,
}

/// Tracks the abort/cancel handles of every in-flight migration task.
#[derive(Default)]
pub struct MigrationTaskRegistry {
    inner: Mutex<HashMap<String, TrackedTask>>,
}

impl MigrationTaskRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a tracked task. Replaces any existing entry with the same
    /// `operation_id` (and aborts the displaced task — duplicate operation_ids
    /// would otherwise leak the older handle).
    pub fn insert(&self, op_id: String, abort: AbortHandle, cancel_token: CancellationToken) {
        let mut guard = self.inner.lock().expect("migration_tasks mutex poisoned");
        if let Some(prev) = guard.insert(
            op_id,
            TrackedTask {
                abort,
                cancel_token,
            },
        ) {
            prev.cancel_token.cancel();
            prev.abort.abort();
        }
    }

    /// Remove the entry for `op_id` (called by the reaper when the task ends).
    pub fn remove(&self, op_id: &str) {
        let mut guard = self.inner.lock().expect("migration_tasks mutex poisoned");
        guard.remove(op_id);
    }

    /// Cancel the migration task associated with `op_id`.
    ///
    /// Triggers both the `CancellationToken` (so the future can unwind at the
    /// next phase boundary) and the `AbortHandle` (so the future is force-dropped
    /// at the next `.await` if it doesn't honor the token quickly).
    ///
    /// Returns `true` if a task was found and signalled.
    pub fn cancel(&self, op_id: &str) -> bool {
        let guard = self.inner.lock().expect("migration_tasks mutex poisoned");
        if let Some(task) = guard.get(op_id) {
            task.cancel_token.cancel();
            task.abort.abort();
            true
        } else {
            false
        }
    }

    /// Number of in-flight migration tasks.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("migration_tasks mutex poisoned")
            .len()
    }

    /// Whether there are no in-flight migration tasks.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether a task with `op_id` is currently tracked.
    pub fn contains(&self, op_id: &str) -> bool {
        self.inner
            .lock()
            .expect("migration_tasks mutex poisoned")
            .contains_key(op_id)
    }

    /// Abort every tracked task. Used on agent shutdown.
    pub fn abort_all(&self) {
        let mut guard = self.inner.lock().expect("migration_tasks mutex poisoned");
        for (op_id, task) in guard.drain() {
            tracing::warn!(
                operation_id = %op_id,
                "aborting in-flight migration task on agent shutdown"
            );
            task.cancel_token.cancel();
            task.abort.abort();
        }
    }
}

impl Drop for MigrationTaskRegistry {
    /// Best-effort: when the last `Arc<MigrationTaskRegistry>` is dropped (i.e.
    /// the last `AgentServer` clone is gone), abort any remaining tasks rather
    /// than leak them into runtime-shutdown chaos.
    fn drop(&mut self) {
        // Avoid panicking in Drop: skip if the lock is poisoned.
        if let Ok(mut guard) = self.inner.lock() {
            for (op_id, task) in guard.drain() {
                tracing::warn!(
                    operation_id = %op_id,
                    "aborting in-flight migration task on registry drop"
                );
                task.cancel_token.cancel();
                task.abort.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::oneshot;

    /// `migration_task_aborts_on_cancel_signal`: spawn a task that would block
    /// "forever", register it under an `operation_id`, then call
    /// `cancel(op_id)` and assert the task observes the abort and the registry
    /// is reaped.
    #[tokio::test]
    async fn migration_task_aborts_on_cancel_signal() {
        let registry = Arc::new(MigrationTaskRegistry::new());
        let cancel_token = CancellationToken::new();
        let op_id = "op-cancel-1".to_string();

        let started = Arc::new(tokio::sync::Notify::new());
        let started_clone = started.clone();
        let token_clone = cancel_token.clone();
        let handle = tokio::spawn(async move {
            started_clone.notify_one();
            // Block forever unless cancelled (or aborted).
            tokio::select! {
                _ = token_clone.cancelled() => Err::<(), String>("cancelled".to_string()),
                _ = tokio::time::sleep(Duration::from_secs(60)) => Ok(()),
            }
        });
        registry.insert(op_id.clone(), handle.abort_handle(), cancel_token.clone());

        // Reaper: remove on completion.
        let reg_for_reap = registry.clone();
        let reap_op = op_id.clone();
        tokio::spawn(async move {
            let _ = handle.await;
            reg_for_reap.remove(&reap_op);
        });

        started.notified().await;
        assert!(registry.contains(&op_id), "task should be tracked");
        assert_eq!(registry.len(), 1);

        let signalled = registry.cancel(&op_id);
        assert!(signalled, "cancel must return true for known op_id");
        assert!(cancel_token.is_cancelled(), "token flipped");

        // Wait up to 1s for the reaper to remove the entry.
        for _ in 0..50 {
            if !registry.contains(&op_id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            !registry.contains(&op_id),
            "reaper must remove cancelled task from registry"
        );
        assert_eq!(registry.len(), 0);
    }

    /// `migration_task_aborts_on_agent_shutdown`: drop the registry's owning
    /// `Arc` and assert all in-flight tasks abort within 1s.
    #[tokio::test]
    async fn migration_task_aborts_on_agent_shutdown() {
        let registry = Arc::new(MigrationTaskRegistry::new());

        // Two long-running "migration" tasks.
        let mut handles = Vec::new();
        let mut completion_rxs = Vec::new();
        for i in 0..2 {
            let op_id = format!("op-shutdown-{i}");
            let token = CancellationToken::new();
            let token_clone = token.clone();
            let (tx, rx) = oneshot::channel::<()>();
            let h = tokio::spawn(async move {
                let _guard = scopeguard_send(tx);
                tokio::select! {
                    _ = token_clone.cancelled() => {}
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
            });
            registry.insert(op_id, h.abort_handle(), token);
            handles.push(h);
            completion_rxs.push(rx);
        }

        // Yield once so the spawned tasks reach their first `.await`.
        tokio::task::yield_now().await;
        assert_eq!(registry.len(), 2);

        // Drop the only Arc — fires `Drop for MigrationTaskRegistry`, which
        // aborts all tracked tasks.
        drop(registry);

        // Assert every task ran its drop guard within 1s (proves it was
        // either cancelled cooperatively or force-aborted).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        for rx in completion_rxs {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            tokio::time::timeout(remaining, rx)
                .await
                .expect("task did not finish within 1s of registry drop")
                .expect("drop-guard channel closed without firing");
        }
    }

    /// `migration_task_terminal_err_observable`: when the spawned future
    /// returns `Err(...)`, the reaper must (1) leave a log breadcrumb and
    /// (2) clean up the registry entry. We can't assert log capture in a
    /// pure unit test, so we assert behaviour: the task runs to its `Err`,
    /// the registry entry is removed, and the surfaced `Err` value matches
    /// what the future returned (proving the JoinHandle is no longer dropped
    /// on the floor).
    #[tokio::test]
    async fn migration_task_terminal_err_observable() {
        let registry = Arc::new(MigrationTaskRegistry::new());
        let op_id = "op-err-1".to_string();
        let cancel_token = CancellationToken::new();

        let observed: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
        let observed_clone = observed.clone();
        let handle = tokio::spawn(async move {
            // Simulate a migration future that fails terminally.
            let result: Result<(), String> = Err("test failure".to_string());
            *observed_clone.lock().unwrap() = Some(result.clone());
            result
        });
        registry.insert(op_id.clone(), handle.abort_handle(), cancel_token);

        // Reaper task — same shape we use in production code.
        let reg_for_reap = registry.clone();
        let reap_op = op_id.clone();
        let reap_handle = tokio::spawn(async move {
            let join_result = handle.await;
            // Surface the terminal Err — proves the JoinHandle is reaped.
            let surfaced = join_result.expect("task did not panic");
            reg_for_reap.remove(&reap_op);
            surfaced
        });

        let surfaced = reap_handle.await.expect("reaper joined");
        assert_eq!(
            surfaced,
            Err("test failure".to_string()),
            "terminal Err must be observable via the reaper"
        );
        assert!(
            !registry.contains(&op_id),
            "reaper must clean up on terminal Err"
        );
        assert_eq!(registry.len(), 0);
        assert_eq!(
            observed.lock().unwrap().as_ref(),
            Some(&Err("test failure".to_string())),
            "task body actually ran to completion"
        );
    }

    /// Helper: a tiny "send on drop" guard so a spawned task can signal
    /// completion via a oneshot channel without depending on `scopeguard`.
    fn scopeguard_send(tx: oneshot::Sender<()>) -> impl Drop {
        struct G(Option<oneshot::Sender<()>>);
        impl Drop for G {
            fn drop(&mut self) {
                if let Some(tx) = self.0.take() {
                    let _ = tx.send(());
                }
            }
        }
        G(Some(tx))
    }
}
