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
use cellhv_core_types::{canonical_json, OperationId, OperationStatus, VmId};
use std::collections::{HashMap, HashSet, VecDeque};
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
    #[error("executor terminated after a task failure: {fatality:?}")]
    Fatal { fatality: ExecutorFatality },
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

impl ExecutionFailureCode {
    /// Stable wire string recorded in recovery evidence when the executor
    /// abandons an operation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaimAmbiguous => "CLAIM_AMBIGUOUS",
            Self::ClaimReplay => "CLAIM_REPLAY",
            Self::FinishAmbiguous => "FINISH_AMBIGUOUS",
            Self::ResultInvalid => "RESULT_INVALID",
            Self::TaskPanicked => "TASK_PANICKED",
            Self::TaskCancelled => "TASK_CANCELLED",
            Self::VmQuarantined => "VM_QUARANTINED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailure {
    pub operation_id: Option<OperationId>,
    /// The VM the failed operation targeted (quarantine + operator context).
    pub vm_id: VmId,
    pub code: ExecutionFailureCode,
}

/// Why the executor terminated and can no longer execute anything: a task
/// failed catastrophically (panic or cancellation) and the scheduler closed
/// ingress for every VM. Surfaced through [`ExecutorError::Fatal`] so the
/// composition fails the process for supervisor restart instead of silently
/// not executing while the authority keeps accepting operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorFatality {
    pub code: ExecutionFailureCode,
    pub operation_id: Option<OperationId>,
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

/// Failure containment for VMs, split by lifetime:
///
/// - `failure` is sticky: an ambiguous claim/finish must never be retried
///   in-process. An entry is released only when its operation reached a
///   terminal status in the journal (normally: the operator resolved the
///   InspectRequired operation the abandonment marking produced) — the
///   ambiguity is then decided and new work for the VM may run. Entries
///   whose op is still claimable (`Ready`: the ambiguous claim never took)
///   or invisible (`running` without a marker: the best-effort abandonment
///   marking failed) stay quarantined until a process restart re-derives
///   everything from the journal.
/// - `sticky` holds vm-only entries (no resolvable operation): never
///   released in-process.
/// - `inspect` is re-derived from the journal on every scan: a VM is
///   restart-quarantined exactly while it has an unresolved `InspectRequired`
///   operation, so an operator resolution un-quarantines the VM on the next
///   scan instead of poisoning it for the process lifetime.
#[derive(Default)]
struct QuarantineState {
    failure: Mutex<HashMap<VmId, Vec<OperationId>>>,
    sticky: Mutex<HashSet<VmId>>,
    inspect: Mutex<HashSet<VmId>>,
}

/// One `QuarantineState` failure entry's release rule, evaluated by
/// [`QuarantineState::reconcile_failures`] each scan: `dispositions` is the
/// current incomplete-snapshot disposition per operation; `statuses` carries
/// point-looked-up statuses for entries whose op is absent from that
/// snapshot (a `running` op without the restart marker is excluded from the
/// snapshot — see `OperationService::restart_operations`).
fn failure_entry_keeps(
    operation_id: &OperationId,
    dispositions: &HashMap<OperationId, RestartDisposition>,
    statuses: &HashMap<OperationId, OperationStatus>,
) -> bool {
    match dispositions.get(operation_id) {
        // Still visibly interrupted: keep quarantining.
        Some(RestartDisposition::InspectRequired) => true,
        // The ambiguous claim never took and the op is claimable again: the
        // ambiguity is unresolved and the op must not be retried
        // in-process — keep until restart.
        Some(RestartDisposition::Ready) => true,
        // Terminal in the snapshot: released. (Defensive arm: the real
        // snapshot source — `list_incomplete_execution_operations` — only
        // carries `accepted`/`running` ops, so a Terminal disposition
        // cannot appear today; the release path that actually fires is the
        // `None` + terminal point-lookup below. Kept so a future snapshot
        // source that includes terminal dispositions behaves correctly.)
        Some(RestartDisposition::Terminal) => false,
        // Absent from the incomplete snapshot: released only when the
        // journal says the op reached a terminal status (the operator
        // resolved it, or it finished while we were not scanning). A
        // `running` op absent here is unmarked — the best-effort
        // abandonment marking failed — and an unknown status is treated the
        // same: keep until restart.
        None => !matches!(
            statuses.get(operation_id),
            Some(
                OperationStatus::Succeeded | OperationStatus::Failed | OperationStatus::Unsupported
            )
        ),
    }
}

impl QuarantineState {
    fn contains(&self, vm: &VmId) -> bool {
        self.sticky
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(vm)
            || self
                .failure
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(vm)
            || self
                .inspect
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(vm)
    }

    fn record_failure(&self, vm: VmId, operation_id: Option<OperationId>) {
        match operation_id {
            Some(operation_id) => {
                let mut failure = self.failure.lock().unwrap_or_else(|e| e.into_inner());
                let entries = failure.entry(vm).or_default();
                if !entries.contains(&operation_id) {
                    entries.push(operation_id);
                }
            }
            None => {
                self.sticky
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(vm);
            }
        }
    }

    /// Operation ids behind `failure` entries (for the per-scan status
    /// pre-fetch of entries whose op left the incomplete snapshot).
    fn failure_operation_ids(&self) -> Vec<OperationId> {
        self.failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .flatten()
            .cloned()
            .collect()
    }

    /// Releases `failure` entries whose operation reached a terminal status
    /// in the journal. See [`failure_entry_keeps`] for the release rule.
    fn reconcile_failures(
        &self,
        dispositions: &HashMap<OperationId, RestartDisposition>,
        statuses: &HashMap<OperationId, OperationStatus>,
    ) {
        let mut failure = self.failure.lock().unwrap_or_else(|e| e.into_inner());
        failure.retain(|_vm, entries| {
            entries
                .retain(|operation_id| failure_entry_keeps(operation_id, dispositions, statuses));
            !entries.is_empty()
        });
    }

    fn set_inspect(&self, vms: HashSet<VmId>) {
        *self.inspect.lock().unwrap_or_else(|e| e.into_inner()) = vms;
    }
}

/// Upper bound on the retained failure-event ring: a composition that never
/// drains must not grow executor memory without limit.
const MAX_FAILURE_EVENTS: usize = 256;

/// Owns one bounded execution scheduler. Explicit shutdown is required to
/// establish the executor-before-authority shutdown ordering contract.
pub struct JournalExecutor {
    sender: Option<mpsc::Sender<Work>>,
    execution: ExecutionHandle,
    scheduled: Arc<Mutex<HashSet<OperationId>>>,
    quarantined: Arc<QuarantineState>,
    capacity: Arc<Semaphore>,
    scan_lock: tokio::sync::Mutex<()>,
    token_factory: TokenFactory,
    fatality: Arc<Mutex<Option<ExecutorFatality>>>,
    task: Option<tokio::task::JoinHandle<ExecutionReport>>,
    /// Bounded ring of recent in-process execution failures, drained by the
    /// composition (runtime-owner) for logging/telemetry: this crate is
    /// runtime-neutral and holds no logging facade (architecture guard).
    failure_events: Arc<Mutex<VecDeque<ExecutionFailure>>>,
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
        let quarantined = Arc::new(QuarantineState::default());
        let fatality: Arc<Mutex<Option<ExecutorFatality>>> = Arc::new(Mutex::new(None));
        let capacity = Arc::new(Semaphore::new(queue_capacity));
        let scheduled: Arc<Mutex<HashSet<OperationId>>> = Arc::new(Mutex::new(HashSet::new()));
        let failure_events: Arc<Mutex<VecDeque<ExecutionFailure>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let task = tokio::spawn(run_scheduler(
            receiver,
            execution.clone(),
            runtime,
            concurrency,
            SchedulerState {
                scheduled: scheduled.clone(),
                quarantined: quarantined.clone(),
                fatality: fatality.clone(),
                failure_events: Arc::clone(&failure_events),
            },
        ));
        Ok(Self {
            sender: Some(sender),
            execution,
            scheduled,
            quarantined,
            capacity,
            scan_lock: tokio::sync::Mutex::new(()),
            token_factory,
            fatality,
            task: Some(task),
            failure_events,
        })
    }

    /// Scans the durable journal in authority order. This is the only ingress.
    pub async fn scan_ready(&self) -> Result<RestartScheduleReport> {
        // A terminated scheduler is fatal, not retryable: every later scan
        // would fail while the authority keeps accepting operations.
        if let Some(fatality) = self
            .fatality
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Err(ExecutorError::Fatal { fatality });
        }
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
        let mut inspect_vms: HashSet<VmId> = HashSet::new();
        let mut dispositions: HashMap<OperationId, RestartDisposition> = HashMap::new();
        for restart in &restart_snapshot {
            dispositions.insert(restart.entry.operation.id.clone(), restart.disposition);
            match restart.disposition {
                RestartDisposition::Ready => {
                    ready_ids.insert(restart.entry.operation.id.clone());
                }
                RestartDisposition::InspectRequired => {
                    inspect_vms.insert(restart.entry.operation.vm_id.clone());
                    report
                        .inspect_required
                        .push(restart.entry.operation.id.clone());
                }
                RestartDisposition::Terminal => {}
            }
        }
        // Release failure quarantines whose operation reached a terminal
        // status (normally: the operator resolved the InspectRequired
        // operation). Entries whose op left the incomplete snapshot need a
        // point lookup to distinguish "resolved" from "running without a
        // marker" (the latter must stay quarantined — see
        // failure_entry_keeps).
        let mut statuses: HashMap<OperationId, OperationStatus> = HashMap::new();
        for operation_id in self.quarantined.failure_operation_ids() {
            if !dispositions.contains_key(&operation_id) {
                if let Ok(entry) = self.execution.operation(operation_id.clone()).await {
                    statuses.insert(operation_id, entry.operation.status);
                }
            }
        }
        self.quarantined
            .reconcile_failures(&dispositions, &statuses);
        // Re-derive the restart quarantine from the current snapshot: a VM is
        // quarantined exactly while it has an unresolved InspectRequired
        // operation, so an operator resolution un-quarantines the VM on the
        // next scan. (An in-flight operation of the live process is not
        // classified InspectRequired — see OperationService::restart_operations
        // — so this cannot quarantine a VM whose operation is executing.)
        self.quarantined.set_inspect(inspect_vms);
        for restart in restart_snapshot {
            match restart.disposition {
                RestartDisposition::Ready => {
                    let id = restart.entry.operation.id.clone();
                    let vm_id = restart.entry.operation.vm_id;
                    if self.quarantined.contains(&vm_id) {
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
                    // Quarantined via set_inspect in the first pass.
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

    /// Drains and returns recent in-process execution failures (bounded
    /// ring; see [`MAX_FAILURE_EVENTS`]). The composition logs these with
    /// full context at its boundary — this crate holds no logging facade.
    pub fn drain_failure_events(&self) -> Vec<ExecutionFailure> {
        let mut events = self
            .failure_events
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        events.drain(..).collect()
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

    /// Cancels in-flight tasks. Any acquired operation remains Running; it
    /// becomes `InspectRequired` only via the abandonment marker or the next
    /// startup classification.
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

/// Shared scheduler state handed to the background task (mirrors the
/// `JournalExecutor` fields).
struct SchedulerState {
    scheduled: Arc<Mutex<HashSet<OperationId>>>,
    quarantined: Arc<QuarantineState>,
    fatality: Arc<Mutex<Option<ExecutorFatality>>>,
    failure_events: Arc<Mutex<VecDeque<ExecutionFailure>>>,
}

async fn run_scheduler(
    mut receiver: mpsc::Receiver<Work>,
    execution: ExecutionHandle,
    runtime: Arc<dyn CoreVmRuntime>,
    concurrency: usize,
    state: SchedulerState,
) -> ExecutionReport {
    let SchedulerState {
        scheduled,
        quarantined,
        fatality,
        failure_events,
    } = state;
    let mut report = ExecutionReport::default();
    let mut pending = VecDeque::new();
    let mut active_vms = HashSet::new();
    let mut tasks = JoinSet::new();
    let mut task_owners = std::collections::HashMap::new();
    // Task id → owning VM, so a task that panics (whose return value is
    // lost) still reports the VM it was executing for.
    let mut task_vms: std::collections::HashMap<tokio::task::Id, VmId> =
        std::collections::HashMap::new();
    let mut ingress_closed = false;

    loop {
        while tasks.len() < concurrency {
            let mut dropped_by_quarantine = Vec::new();
            pending.retain(|work: &Work| {
                if quarantined.contains(&work.vm_id) {
                    dropped_by_quarantine.push(work.operation_id.clone());
                    false
                } else {
                    true
                }
            });
            // A work dropped before its task ever started was never claimed,
            // so it is safely re-admittable: release its admission record so
            // a later scan (after the VM is un-quarantined) can schedule it
            // again instead of skipping it forever behind the `scheduled`
            // guard.
            if !dropped_by_quarantine.is_empty() {
                let mut scheduled = scheduled.lock().unwrap_or_else(|e| e.into_inner());
                for operation_id in dropped_by_quarantine {
                    scheduled.remove(&operation_id);
                }
            }
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
            let work_vm_id = work.vm_id.clone();
            let task = tasks.spawn(async move {
                let vm_id = work.vm_id.clone();
                let result = execute_one(work, execution, runtime).await;
                (vm_id, result)
            });
            task_owners.insert(task.id(), operation_id);
            task_vms.insert(task.id(), work_vm_id);
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
                            task_vms.remove(&task_id);
                            active_vms.remove(&vm_id);
                            if let WorkOutcome::Failure(failure) = &outcome {
                                // Every `Failure` outcome failure-quarantines
                                // the VM (a claim replay is the idempotent-
                                // success path and never reaches this arm):
                                // an ambiguous claim/finish/result must not be
                                // retried in-process. Released by
                                // reconcile_failures once the operation
                                // reaches a terminal status in the journal.
                                if let Some(operation_id) = &failure.operation_id {
                                    // The operation may have been left `running`
                                    // (finishing it would guess a terminal
                                    // outcome): mark it abandoned so it becomes
                                    // InspectRequired — visible and resolvable —
                                    // without waiting for a restart. Best-effort:
                                    // when the authority is unavailable the
                                    // operation stays unmarked and restart
                                    // classification covers it on the next boot.
                                    let code = failure.code.as_str().to_owned();
                                    // Record the failure event for the
                                    // composition to drain and log: the journal
                                    // is the source of truth, but the operator
                                    // needs the causal link (which operation,
                                    // which failure code) at the moment it
                                    // happens, not only via the next scan's
                                    // inspect-required diff. Bounded ring: a
                                    // wedged composition cannot grow memory.
                                    {
                                        let mut events = failure_events
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner());
                                        events.push_back(failure.clone());
                                        while events.len() > MAX_FAILURE_EVENTS {
                                            events.pop_front();
                                        }
                                    }
                                    let _ = execution
                                        .mark_operation_abandoned(operation_id.clone(), code)
                                        .await;
                                }
                                // Failure-quarantine the VM against the
                                // ambiguous operation.
                                if outcome.quarantines() {
                                    quarantined.record_failure(
                                        vm_id,
                                        failure.operation_id.clone(),
                                    );
                                }
                            }
                            merge_outcome(&mut report, outcome);
                        }
                        Err(error) => {
                            let operation_id = task_owners.remove(&error.id());
                            let vm_id = task_vms.remove(&error.id()).unwrap_or_else(|| {
                                // A spawned task always has an owner entry;
                                // fall back to the null VM rather than
                                // panicking in the scheduler loop.
                                VmId::new("vm-unknown").expect("fallback vm id is valid")
                            });
                            let code = if error.is_cancelled() { ExecutionFailureCode::TaskCancelled } else { ExecutionFailureCode::TaskPanicked };
                            report.failures.push(ExecutionFailure {
                                operation_id: operation_id.clone(),
                                vm_id: vm_id.clone(),
                                code,
                            });
                            // The containment below is the single-effector
                            // invariant (no further claim/finish after a task
                            // failed catastrophically), but it must not be
                            // *silent*: record the fatality — first cause wins,
                            // the abort cascade only appends to the report — so
                            // scan_ready fails the composition instead of
                            // leaving a healthy-looking authority that executes
                            // nothing.
                            {
                                let mut slot = fatality.lock().unwrap_or_else(|e| e.into_inner());
                                if slot.is_none() {
                                    *slot = Some(ExecutorFatality { code, operation_id });
                                }
                            }
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
    ClaimReplay,
    Failure(ExecutionFailure),
}
impl WorkOutcome {
    /// Whether this outcome must failure-quarantine the VM. A claim replay
    /// is the idempotent-success path (another sender already holds this
    /// claim) and must not poison the VM.
    fn quarantines(&self) -> bool {
        matches!(self, Self::Failure(_))
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
                vm_id: work.vm_id.clone(),
                code: ExecutionFailureCode::ClaimAmbiguous,
            })
        }
    };
    let entry = match claimed {
        ClaimResult::Acquired(entry) => entry,
        ClaimResult::Replay(_) => return WorkOutcome::ClaimReplay,
    };
    let terminal = match runtime.execute(entry).await {
        Ok(result) if valid_result(&result) => TerminalOutcome::Succeeded(result),
        Ok(_) => {
            return WorkOutcome::Failure(ExecutionFailure {
                operation_id: Some(work.operation_id),
                vm_id: work.vm_id.clone(),
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
            vm_id: work.vm_id,
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
        WorkOutcome::ClaimReplay => {
            // Idempotent no-op success: the claim is already held. Counted,
            // never reported as a failure, never quarantine-worthy.
            report.claim_replays += 1;
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

pub mod fault;
pub use fault::{FaultPoint, FaultRuntime};

#[cfg(test)]
mod tests;
