//! Bounded, runtime-neutral execution of the CellHV Core operation journal.
//!
//! This crate has no production runtime implementation or composition. A
//! future runtime must be injected through [`CoreVmRuntime`]; it cannot bypass
//! the fenced operation capability.

use async_trait::async_trait;
use cellhv_core_operations::{
    AttemptToken, ClaimResult, ExecutionHandle, OperationJournalEntry, RestartDisposition,
    TerminalOutcome,
};
use cellhv_core_types::{canonical_json, OperationId, VmId};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio::task::{JoinError, JoinSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFailure {
    InvalidRequest,
    Unsupported,
    NotFound,
    Conflict,
    RuntimeUnavailable,
    Internal,
}
impl RuntimeFailure {
    fn into_json(self) -> serde_json::Value {
        let code = match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::Unsupported => "UNSUPPORTED",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::RuntimeUnavailable => "RUNTIME_UNAVAILABLE",
            Self::Internal => "INTERNAL",
        };
        serde_json::json!({"code": code})
    }
}

#[async_trait]
pub trait CoreVmRuntime: Send + Sync + 'static {
    async fn execute(
        &self,
        operation: OperationJournalEntry,
    ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure>;
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("executor concurrency must be greater than zero")]
    InvalidConcurrency,
    #[error("executor queue capacity must be greater than zero")]
    InvalidQueueCapacity,
    #[error("executor is closed")]
    Closed,
    #[error(transparent)]
    Authority(#[from] cellhv_core_operations::AuthorityActorError),
    #[error("executor task failed: {0}")]
    Join(#[from] JoinError),
    #[error("executor drain exceeded {budget:?}; in-flight tasks were cancelled")]
    DrainTimedOut { budget: Duration },
}

pub type Result<T> = std::result::Result<T, ExecutorError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionFailureCode {
    ClaimAmbiguous,
    ClaimReplay,
    FinishAmbiguous,
    ResultInvalid,
    TaskPanicked,
    TaskCancelled,
    VmQuarantined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailure {
    pub operation_id: Option<OperationId>,
    pub code: ExecutionFailureCode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionReport {
    pub acquired: usize,
    pub claim_replays: usize,
    pub completed: usize,
    pub failures: Vec<ExecutionFailure>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestartScheduleReport {
    pub scheduled: Vec<OperationId>,
    pub inspect_required: Vec<OperationId>,
    pub quarantined: Vec<OperationId>,
    pub capacity_reached: bool,
}

struct Work {
    operation_id: OperationId,
    vm_id: VmId,
    attempt_token: AttemptToken,
    _permit: OwnedSemaphorePermit,
}

type TokenFactory = Arc<dyn Fn() -> AttemptToken + Send + Sync>;

/// Owns one bounded execution scheduler. Explicit shutdown is required to
/// establish the executor-before-authority shutdown ordering contract.
pub struct JournalExecutor {
    sender: Option<mpsc::Sender<Work>>,
    execution: ExecutionHandle,
    scheduled: Arc<Mutex<HashSet<OperationId>>>,
    quarantined_vms: Arc<Mutex<HashSet<VmId>>>,
    capacity: Arc<Semaphore>,
    scan_lock: tokio::sync::Mutex<()>,
    token_factory: TokenFactory,
    task: Option<tokio::task::JoinHandle<ExecutionReport>>,
}

impl JournalExecutor {
    pub fn start(
        execution: ExecutionHandle,
        runtime: Arc<dyn CoreVmRuntime>,
        concurrency: usize,
        queue_capacity: usize,
    ) -> Result<Self> {
        Self::start_with_token_factory(
            execution,
            runtime,
            concurrency,
            queue_capacity,
            Arc::new(|| AttemptToken::new(uuid::Uuid::now_v7().to_string()).unwrap()),
        )
    }

    fn start_with_token_factory(
        execution: ExecutionHandle,
        runtime: Arc<dyn CoreVmRuntime>,
        concurrency: usize,
        queue_capacity: usize,
        token_factory: TokenFactory,
    ) -> Result<Self> {
        if concurrency == 0 {
            return Err(ExecutorError::InvalidConcurrency);
        }
        if queue_capacity == 0 {
            return Err(ExecutorError::InvalidQueueCapacity);
        }
        let (sender, receiver) = mpsc::channel(queue_capacity);
        let quarantined_vms = Arc::new(Mutex::new(HashSet::new()));
        let capacity = Arc::new(Semaphore::new(queue_capacity));
        let task = tokio::spawn(run_scheduler(
            receiver,
            execution.clone(),
            runtime,
            concurrency,
            quarantined_vms.clone(),
        ));
        Ok(Self {
            sender: Some(sender),
            execution,
            scheduled: Arc::new(Mutex::new(HashSet::new())),
            quarantined_vms,
            capacity,
            scan_lock: tokio::sync::Mutex::new(()),
            token_factory,
            task: Some(task),
        })
    }

    /// Scans the durable journal in authority order. This is the only ingress.
    pub async fn scan_ready(&self) -> Result<RestartScheduleReport> {
        let _scan = self.scan_lock.lock().await;
        let mut report = RestartScheduleReport::default();
        let restart_snapshot = self.execution.restart_operations().await?;
        // Bounded bookkeeping: `scheduled` must only ever hold operations the
        // scheduler is still driving. Every scan prunes it to the set the
        // authority reports as Ready — exactly the operations that could be
        // (re)admitted. Operations that are Running/InspectRequired are already
        // claimed by the scheduler (execute_one only ever reaches a terminal or
        // quarantined state; it never reverts a claimed op to Ready — see
        // execute_one/merge_outcome), so dropping them from `scheduled` cannot
        // cause a duplicate admission. Without this prune, the always-on
        // production poller would grow `scheduled` unboundedly over the
        // lifetime of the node.
        let mut ready_ids: HashSet<OperationId> = HashSet::new();
        for restart in &restart_snapshot {
            match restart.disposition {
                RestartDisposition::Ready => {
                    ready_ids.insert(restart.entry.operation.id.clone());
                }
                RestartDisposition::InspectRequired => {
                    self.quarantined_vms
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(restart.entry.operation.vm_id.clone());
                    report
                        .inspect_required
                        .push(restart.entry.operation.id.clone());
                }
                RestartDisposition::Terminal => {}
            }
        }
        for restart in restart_snapshot {
            match restart.disposition {
                RestartDisposition::Ready => {
                    let id = restart.entry.operation.id.clone();
                    let vm_id = restart.entry.operation.vm_id;
                    if self
                        .quarantined_vms
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .contains(&vm_id)
                    {
                        report.quarantined.push(id);
                        continue;
                    }
                    if self
                        .scheduled
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .contains(&id)
                    {
                        continue;
                    }
                    let permit = match self.capacity.clone().try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            report.capacity_reached = true;
                            break;
                        }
                    };
                    self.schedule(id.clone(), vm_id, permit)?;
                    report.scheduled.push(id);
                }
                RestartDisposition::InspectRequired => {
                    // Quarantined in the complete first pass.
                }
                RestartDisposition::Terminal => {}
            }
        }
        self.scheduled
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|id| ready_ids.contains(id));
        Ok(report)
    }

    fn schedule(
        &self,
        operation_id: OperationId,
        vm_id: VmId,
        permit: OwnedSemaphorePermit,
    ) -> Result<()> {
        let sender = self.sender.as_ref().ok_or(ExecutorError::Closed)?;
        {
            let mut scheduled = self.scheduled.lock().unwrap_or_else(|e| e.into_inner());
            scheduled.insert(operation_id.clone());
        }
        let work = Work {
            operation_id: operation_id.clone(),
            vm_id,
            attempt_token: (self.token_factory)(),
            _permit: permit,
        };
        if let Err(error) = sender.try_send(work) {
            self.scheduled
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&operation_id);
            return match error {
                mpsc::error::TrySendError::Full(_) => {
                    unreachable!("permit bounds channel admission")
                }
                mpsc::error::TrySendError::Closed(_) => Err(ExecutorError::Closed),
            };
        }
        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<ExecutionReport> {
        self.close_ingress();
        Ok(self
            .task
            .take()
            .expect("executor task is present before shutdown")
            .await?)
    }

    /// Graceful shutdown bounded by `budget`, as used by production
    /// compositions. If the scheduler fails to drain within the budget (for
    /// example a wedged runtime), the scheduler task is explicitly cancelled so
    /// the executor stops issuing any further claim/finish RPCs — a task still
    /// talking to a gone authority actor would be a second, unaccounted
    /// effector. Note the boundary of this guarantee: cancellation stops the
    /// executor *task*; a runtime effect that was already initiated is not
    /// rolled back by the executor and is reconciled through the operation's
    /// `InspectRequired` disposition on restart (see
    /// `chv-agent-runtime-ch` for the process-lifecycle counterpart).
    /// Acquired-but-unfinished operations remain `Running` and therefore
    /// `InspectRequired` after restart (the documented crash semantics).
    /// Returns [`ExecutorError::DrainTimedOut`] when the budget expires.
    pub async fn shutdown_bounded(mut self, budget: Duration) -> Result<ExecutionReport> {
        self.close_ingress();
        let task = self
            .task
            .take()
            .expect("executor task is present before shutdown");
        let deadline = tokio::time::Instant::now() + budget;
        loop {
            if task.is_finished() {
                return task.await.map_err(ExecutorError::Join);
            }
            if tokio::time::Instant::now() >= deadline {
                task.abort();
                match task.await {
                    Err(error) if error.is_cancelled() => {
                        return Err(ExecutorError::DrainTimedOut { budget });
                    }
                    Err(error) => return Err(ExecutorError::Join(error)),
                    Ok(_) => return Err(ExecutorError::DrainTimedOut { budget }),
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    pub fn close_ingress(&mut self) {
        drop(self.sender.take());
    }

    /// Cancels in-flight tasks. Any acquired operation remains Running and is
    /// therefore `InspectRequired` after restart.
    pub async fn abort(mut self) -> Result<()> {
        drop(self.sender.take());
        let task = self
            .task
            .take()
            .expect("executor task is present before abort");
        task.abort();
        match task.await {
            Err(error) if error.is_cancelled() => Ok(()),
            Err(error) => Err(ExecutorError::Join(error)),
            Ok(_) => Ok(()),
        }
    }
}

impl Drop for JournalExecutor {
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn run_scheduler(
    mut receiver: mpsc::Receiver<Work>,
    execution: ExecutionHandle,
    runtime: Arc<dyn CoreVmRuntime>,
    concurrency: usize,
    quarantined_vms: Arc<Mutex<HashSet<VmId>>>,
) -> ExecutionReport {
    let mut report = ExecutionReport::default();
    let mut pending = VecDeque::new();
    let mut active_vms = HashSet::new();
    let mut tasks = JoinSet::new();
    let mut task_owners = std::collections::HashMap::new();
    let mut ingress_closed = false;

    loop {
        while tasks.len() < concurrency {
            pending.retain(|work: &Work| {
                !quarantined_vms
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains(&work.vm_id)
            });
            let Some(index) = pending
                .iter()
                .position(|work: &Work| !active_vms.contains(&work.vm_id))
            else {
                break;
            };
            let work = pending.remove(index).expect("pending index exists");
            active_vms.insert(work.vm_id.clone());
            let execution = execution.clone();
            let runtime = runtime.clone();
            let operation_id = work.operation_id.clone();
            let task = tasks.spawn(async move {
                let vm_id = work.vm_id.clone();
                let result = execute_one(work, execution, runtime).await;
                (vm_id, result)
            });
            task_owners.insert(task.id(), operation_id);
        }

        if ingress_closed && pending.is_empty() && tasks.is_empty() {
            break;
        }

        tokio::select! {
            completed = tasks.join_next_with_id(), if !tasks.is_empty() => {
                if let Some(completed) = completed {
                    match completed {
                        Ok((task_id, (vm_id, outcome))) => {
                            task_owners.remove(&task_id);
                            active_vms.remove(&vm_id);
                            if outcome.quarantines() { quarantined_vms.lock().unwrap_or_else(|e| e.into_inner()).insert(vm_id); }
                            merge_outcome(&mut report, outcome);
                        }
                        Err(error) => {
                            let operation_id = task_owners.remove(&error.id());
                            report.failures.push(ExecutionFailure {
                                operation_id,
                                code: if error.is_cancelled() { ExecutionFailureCode::TaskCancelled } else { ExecutionFailureCode::TaskPanicked },
                            });
                            receiver.close();
                            ingress_closed = true;
                            pending.clear();
                            tasks.abort_all();
                        }
                    }
                }
            }
            work = receiver.recv(), if !ingress_closed => {
                match work {
                    Some(work) => pending.push_back(work),
                    None => ingress_closed = true,
                }
            }
        }
    }
    report
}

enum WorkOutcome {
    AcquiredCompleted,
    ClaimReplay(OperationId),
    Failure(ExecutionFailure),
}
impl WorkOutcome {
    fn quarantines(&self) -> bool {
        !matches!(self, Self::AcquiredCompleted)
    }
}

async fn execute_one(
    work: Work,
    execution: ExecutionHandle,
    runtime: Arc<dyn CoreVmRuntime>,
) -> WorkOutcome {
    let claimed = match execution
        .claim_attempt(work.operation_id.clone(), work.attempt_token.clone())
        .await
    {
        Ok(claimed) => claimed,
        Err(_) => {
            return WorkOutcome::Failure(ExecutionFailure {
                operation_id: Some(work.operation_id),
                code: ExecutionFailureCode::ClaimAmbiguous,
            })
        }
    };
    let entry = match claimed {
        ClaimResult::Acquired(entry) => entry,
        ClaimResult::Replay(_) => return WorkOutcome::ClaimReplay(work.operation_id),
    };
    let terminal = match runtime.execute(entry).await {
        Ok(result) if valid_result(&result) => TerminalOutcome::Succeeded(result),
        Ok(_) => {
            return WorkOutcome::Failure(ExecutionFailure {
                operation_id: Some(work.operation_id),
                code: ExecutionFailureCode::ResultInvalid,
            })
        }
        Err(RuntimeFailure::Unsupported) => {
            TerminalOutcome::Unsupported(RuntimeFailure::Unsupported.into_json())
        }
        Err(error) => TerminalOutcome::Failed(error.into_json()),
    };
    match execution
        .finish(work.operation_id.clone(), work.attempt_token, terminal)
        .await
    {
        Ok(_) => WorkOutcome::AcquiredCompleted,
        Err(_) => WorkOutcome::Failure(ExecutionFailure {
            operation_id: Some(work.operation_id),
            code: ExecutionFailureCode::FinishAmbiguous,
        }),
    }
}

fn merge_outcome(report: &mut ExecutionReport, outcome: WorkOutcome) {
    match outcome {
        WorkOutcome::AcquiredCompleted => {
            report.acquired += 1;
            report.completed += 1;
        }
        WorkOutcome::ClaimReplay(operation_id) => {
            report.claim_replays += 1;
            report.failures.push(ExecutionFailure {
                operation_id: Some(operation_id),
                code: ExecutionFailureCode::ClaimReplay,
            });
        }
        WorkOutcome::Failure(failure) => report.failures.push(failure),
    }
}

fn valid_result(result: &Option<serde_json::Value>) -> bool {
    let Some(value) = result else {
        return true;
    };
    if !value.is_object() || canonical_json(value).map_or(true, |bytes| bytes.len() > 64 * 1024) {
        return false;
    }
    fn walk(value: &serde_json::Value, depth: usize, nodes: &mut usize) -> bool {
        *nodes += 1;
        if depth > 16 || *nodes > 4096 {
            return false;
        }
        match value {
            serde_json::Value::String(s) => s.len() <= 16 * 1024,
            serde_json::Value::Array(xs) => xs.iter().all(|v| walk(v, depth + 1, nodes)),
            serde_json::Value::Object(map) => map
                .iter()
                .all(|(k, v)| !k.is_empty() && k.len() <= 128 && walk(v, depth + 1, nodes)),
            _ => true,
        }
    }
    walk(value, 0, &mut 0)
}

#[cfg(test)]
mod tests;
