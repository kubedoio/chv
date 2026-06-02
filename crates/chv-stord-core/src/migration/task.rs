use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::watch;

/// Phase of a disk migration as tracked by the stord service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationPhase {
    Pending,
    BulkCopy,
    DirtySync,
    PausedFinalSync,
    Completed,
    Failed,
}

/// Mutable state for an active migration task.
#[derive(Debug, Clone)]
pub struct MigrationTaskState {
    pub phase: MigrationPhase,
    pub convergence_round: u32,
    pub dirty_blocks_remaining: u64,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub needs_vm_pause: bool,
    pub error_message: String,
}

/// An active disk migration tracked by the stord service.
///
/// The background `MigrationSender` task updates `state` as it progresses,
/// and RPC handlers read `state` to service `GetDiskMigrationStatus` requests.
/// The `pause_tx` channel is used by `ResumeDiskMigration` to signal the
/// sender that the VM has been paused and it may proceed with `FinalSync`.
#[derive(Debug)]
pub struct MigrationTask {
    pub volume_id: String,
    pub handle: String,
    pub dest_endpoint: String,
    pub state: tokio::sync::RwLock<MigrationTaskState>,
    pub pause_tx: watch::Sender<bool>,
}

impl MigrationTask {
    pub fn new(
        volume_id: String,
        handle: String,
        dest_endpoint: String,
    ) -> (Arc<Self>, watch::Receiver<bool>) {
        let (pause_tx, pause_rx) = watch::channel(false);
        let task = Arc::new(Self {
            volume_id,
            handle,
            dest_endpoint,
            state: tokio::sync::RwLock::new(MigrationTaskState {
                phase: MigrationPhase::Pending,
                convergence_round: 0,
                dirty_blocks_remaining: 0,
                bytes_transferred: 0,
                total_bytes: 0,
                needs_vm_pause: false,
                error_message: String::new(),
            }),
            pause_tx,
        });
        (task, pause_rx)
    }

    /// Mark the task as failed with the given error message.
    pub fn mark_failed(&self, message: String) {
        match self.state.try_write() {
            Ok(mut state) => {
                state.phase = MigrationPhase::Failed;
                state.error_message = message;
            }
            Err(_) => {
                tracing::warn!(
                    volume_id = %self.volume_id,
                    "failed to acquire task lock to mark failed"
                );
            }
        }
    }
}

/// Thread-safe table of active disk migrations.
#[derive(Debug, Clone, Default)]
pub struct MigrationTaskTable {
    inner: Arc<DashMap<String, Arc<MigrationTask>>>,
}

impl MigrationTaskTable {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, migration_id: String, task: Arc<MigrationTask>) {
        self.inner.insert(migration_id, task);
    }

    pub fn get(&self, migration_id: &str) -> Option<Arc<MigrationTask>> {
        self.inner.get(migration_id).map(|entry| entry.clone())
    }

    pub fn remove(&self, migration_id: &str) -> Option<Arc<MigrationTask>> {
        self.inner.remove(migration_id).map(|(_, task)| task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_table_insert_and_get() {
        let table = MigrationTaskTable::new();
        let (task, _rx) = MigrationTask::new(
            "vol-1".to_string(),
            "handle-1".to_string(),
            "http://dest:50052".to_string(),
        );
        table.insert("mig-1".to_string(), task.clone());
        let got = table.get("mig-1").unwrap();
        assert_eq!(got.volume_id, "vol-1");
    }

    #[test]
    fn task_table_remove_missing_is_none() {
        let table = MigrationTaskTable::new();
        assert!(table.remove("mig-1").is_none());
    }

    #[tokio::test]
    async fn task_pause_channel_signals() {
        let (task, mut pause_rx) = MigrationTask::new(
            "vol-1".to_string(),
            "handle-1".to_string(),
            "http://dest:50052".to_string(),
        );
        assert!(!*pause_rx.borrow());
        task.pause_tx.send(true).unwrap();
        assert!(*pause_rx.borrow_and_update());
    }
}
