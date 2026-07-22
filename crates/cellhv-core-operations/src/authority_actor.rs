//! Serialized async access to the single Core operation service.
//!
//! Sending a request transfers it to the authority queue. If its caller is
//! then cancelled, the actor may still durably accept the request; callers
//! must resolve ambiguity by replaying the same idempotency key. This actor is
//! deliberately transport-neutral and has no VMM or provider execution hooks.

use crate::{
    AcceptedOperation, AttemptToken, ClaimResult, CompletionResult, HostRecord,
    OperationJournalEntry, OperationService, OperationServiceError, RestartOperation,
    SubmitMutation, TerminalOutcome,
};
use async_channel::Sender;
use cellhv_core_types::{OperationEvent, OperationId, VmDefinition, VmId};
use std::sync::OnceLock;
use std::thread;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum AuthorityActorError {
    #[error("authority queue capacity must be greater than zero")]
    InvalidCapacity,
    #[error("Core authority is unavailable")]
    Unavailable,
    #[error(transparent)]
    Service(#[from] OperationServiceError),
    #[error("Core authority task failed: {0}")]
    Join(#[from] JoinError),
    #[error("cannot start Core authority thread: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Core authority thread panicked")]
    ThreadPanicked,
}

pub type Result<T> = std::result::Result<T, AuthorityActorError>;

type Reply<T> = oneshot::Sender<std::result::Result<T, OperationServiceError>>;

enum Request {
    Submit(Box<SubmitMutation>, Reply<AcceptedOperation>),
    Operation(OperationId, Reply<OperationJournalEntry>),
    Vm(VmId, Reply<VmDefinition>),
    Vms(Reply<Vec<VmDefinition>>),
    Operations(Reply<Vec<OperationJournalEntry>>),
    EventsAfter(u64, u32, Reply<Vec<OperationEvent>>),
    Host(Reply<HostRecord>),
    RestartOperations(Reply<Vec<RestartOperation>>),
    PersistObservedVmState(VmId, cellhv_core_types::ObservedPowerState, Reply<()>),
    ClaimAttempt(OperationId, AttemptToken, Reply<ClaimResult>),
    Finish(
        OperationId,
        AttemptToken,
        Box<TerminalOutcome>,
        Reply<CompletionResult>,
    ),
    Shutdown(oneshot::Sender<()>),
    #[cfg(test)]
    Gate(oneshot::Sender<()>, std::sync::mpsc::Receiver<()>),
}

/// Cloneable, backpressured entry point shared by future compatibility and
/// native transports. No production transport constructs this handle yet.
#[derive(Clone)]
pub struct AuthorityHandle {
    sender: Sender<Request>,
}

impl AuthorityHandle {
    pub async fn submit(&self, submission: SubmitMutation) -> Result<AcceptedOperation> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::Submit(Box::new(submission), reply), receive)
            .await
    }

    pub async fn operation(&self, id: OperationId) -> Result<OperationJournalEntry> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::Operation(id, reply), receive).await
    }

    pub async fn vm(&self, id: VmId) -> Result<VmDefinition> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::Vm(id, reply), receive).await
    }

    pub async fn vms(&self) -> Result<Vec<VmDefinition>> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::Vms(reply), receive).await
    }

    pub async fn operations(&self) -> Result<Vec<OperationJournalEntry>> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::Operations(reply), receive).await
    }

    pub async fn events_after(&self, sequence: u64, limit: u32) -> Result<Vec<OperationEvent>> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::EventsAfter(sequence, limit, reply), receive)
            .await
    }

    pub async fn persist_observed_vm_state(
        &self,
        vm_id: VmId,
        observed: cellhv_core_types::ObservedPowerState,
    ) -> Result<()> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .send(Request::PersistObservedVmState(vm_id, observed, reply))
            .await
            .map_err(|_| AuthorityActorError::Unavailable)?;
        receive
            .await
            .map_err(|_| AuthorityActorError::Unavailable)?
            .map_err(AuthorityActorError::Service)
    }

    pub async fn host(&self) -> Result<HostRecord> {
        let (reply, receive) = oneshot::channel();
        self.send(Request::Host(reply), receive).await
    }

    async fn send<T>(
        &self,
        request: Request,
        receive: oneshot::Receiver<crate::Result<T>>,
    ) -> Result<T> {
        self.sender
            .send(request)
            .await
            .map_err(|_| AuthorityActorError::Unavailable)?;
        receive
            .await
            .map_err(|_| AuthorityActorError::Unavailable)?
            .map_err(AuthorityActorError::Service)
    }

    /// Queue-ordered shutdown. Requests accepted before this message finish;
    /// later sends fail once the actor processes it.
    pub async fn shutdown(&self) -> Result<()> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .send(Request::Shutdown(reply))
            .await
            .map_err(|_| AuthorityActorError::Unavailable)?;
        receive.await.map_err(|_| AuthorityActorError::Unavailable)
    }

    pub fn execution_handle(&self) -> ExecutionHandle {
        ExecutionHandle {
            sender: self.sender.clone(),
        }
    }
}

/// Capability reserved for a journal executor. Protocol transports receive
/// only [`AuthorityHandle`] and therefore cannot claim or finish execution.
#[derive(Clone)]
pub struct ExecutionHandle {
    sender: Sender<Request>,
}

impl ExecutionHandle {
    pub async fn claim_attempt(
        &self,
        id: OperationId,
        attempt_token: AttemptToken,
    ) -> Result<ClaimResult> {
        let (reply, receive) = oneshot::channel();
        send_request(
            &self.sender,
            Request::ClaimAttempt(id, attempt_token, reply),
            receive,
        )
        .await
    }

    pub async fn finish(
        &self,
        id: OperationId,
        attempt_token: AttemptToken,
        outcome: TerminalOutcome,
    ) -> Result<CompletionResult> {
        let (reply, receive) = oneshot::channel();
        send_request(
            &self.sender,
            Request::Finish(id, attempt_token, Box::new(outcome), reply),
            receive,
        )
        .await
    }

    /// Returns active attempt tokens only to the recovery/executor capability.
    pub async fn restart_operations(&self) -> Result<Vec<RestartOperation>> {
        let (reply, receive) = oneshot::channel();
        send_request(&self.sender, Request::RestartOperations(reply), receive).await
    }
}

async fn send_request<T>(
    sender: &Sender<Request>,
    request: Request,
    receive: oneshot::Receiver<crate::Result<T>>,
) -> Result<T> {
    sender
        .send(request)
        .await
        .map_err(|_| AuthorityActorError::Unavailable)?;
    receive
        .await
        .map_err(|_| AuthorityActorError::Unavailable)?
        .map_err(AuthorityActorError::Service)
}

pub struct AuthorityActor;

impl AuthorityActor {
    /// Starts an unwired serialization boundary around one existing service.
    /// Database creation, startup cutover, and process-wide lease ownership
    /// remain the responsibility of later `chv-agent` startup wiring.
    pub fn spawn(
        service: OperationService,
        queue_capacity: usize,
    ) -> Result<(AuthorityHandle, AuthorityActorJoin)> {
        let (authority, _execution, join) = Self::spawn_with_execution(service, queue_capacity)?;
        Ok((authority, join))
    }

    /// Starts one actor and returns a separately typed execution capability.
    /// Runtime composition must keep this handle away from protocol adapters.
    pub fn spawn_with_execution(
        service: OperationService,
        queue_capacity: usize,
    ) -> Result<(AuthorityHandle, ExecutionHandle, AuthorityActorJoin)> {
        if queue_capacity == 0 {
            return Err(AuthorityActorError::InvalidCapacity);
        }
        let (sender, receiver) = async_channel::bounded(queue_capacity);
        let task = thread::Builder::new()
            .name("cellhv-core-authority".to_owned())
            .spawn(move || {
                let mut service = service;
                while let Ok(request) = receiver.recv_blocking() {
                    match request {
                        Request::Submit(value, reply) => {
                            let _ = reply.send(service.submit(*value));
                        }
                        Request::Operation(id, reply) => {
                            let _ = reply.send(service.operation(&id));
                        }
                        Request::Vm(id, reply) => {
                            let _ = reply.send(service.vm(&id));
                        }
                        Request::Vms(reply) => {
                            let _ = reply.send(service.vms());
                        }
                        Request::Operations(reply) => {
                            let _ = reply.send(service.operations());
                        }
                        Request::EventsAfter(sequence, limit, reply) => {
                            let _ = reply.send(service.events_after(sequence, limit));
                        }
                        Request::Host(reply) => {
                            let _ = reply.send(service.host());
                        }
                        Request::PersistObservedVmState(vm_id, observed, reply) => {
                            let _ = reply.send(service.persist_observed_vm_state(&vm_id, observed));
                        }

                        Request::RestartOperations(reply) => {
                            let _ = reply.send(service.restart_operations());
                        }
                        Request::ClaimAttempt(id, attempt_token, reply) => {
                            let _ = reply.send(service.claim_attempt(&id, &attempt_token));
                        }
                        Request::Finish(id, attempt_token, outcome, reply) => {
                            let _ = reply.send(service.finish(&id, &attempt_token, *outcome));
                        }
                        Request::Shutdown(reply) => {
                            receiver.close();
                            while receiver.try_recv().is_ok() {}
                            let _ = reply.send(());
                            break;
                        }
                        #[cfg(test)]
                        Request::Gate(entered, release) => {
                            let _ = entered.send(());
                            let _ = release.recv();
                        }
                    }
                }
            })
            .map_err(AuthorityActorError::Spawn)?;
        Ok((
            AuthorityHandle {
                sender: sender.clone(),
            },
            ExecutionHandle {
                sender: sender.clone(),
            },
            AuthorityActorJoin {
                task: Some(task),
                sender,
            },
        ))
    }
}

pub struct AuthorityActorJoin {
    task: Option<thread::JoinHandle<()>>,
    sender: Sender<Request>,
}

impl AuthorityActorJoin {
    pub async fn join(mut self) -> Result<()> {
        self.sender.close();
        let task = self
            .task
            .take()
            .expect("authority thread owner lost its task");
        tokio::task::spawn_blocking(move || task.join())
            .await?
            .map_err(|_| AuthorityActorError::ThreadPanicked)
    }
}

impl Drop for AuthorityActorJoin {
    fn drop(&mut self) {
        self.sender.close();
        if let Some(task) = self.task.take() {
            // Dropping an owner may happen on an async runtime worker. Transfer
            // the blocking join to a named reaper so queued SQLite work cannot
            // stall that worker. Explicit `join()` remains the observable path.
            if let Err(error) = authority_reaper().send(task) {
                let _ = error.0.join();
            }
        }
    }
}

fn authority_reaper() -> &'static std::sync::mpsc::Sender<thread::JoinHandle<()>> {
    static REAPER: OnceLock<std::sync::mpsc::Sender<thread::JoinHandle<()>>> = OnceLock::new();
    REAPER.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel::<thread::JoinHandle<()>>();
        thread::Builder::new()
            .name("cellhv-core-authority-reaper".to_owned())
            .spawn(move || {
                while let Ok(task) = receiver.recv() {
                    let _ = task.join();
                }
            })
            .expect("cannot start Core authority reaper thread");
        sender
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Acceptance, MutationCommand, RestartDisposition};
    use cellhv_core_store::{CoreStore, StoreError};
    use cellhv_core_types::{
        BootSpec, ComputeSpec, IdempotencyKey, ObservedPowerState, OperationId, OperationStatus,
        RequestedPowerState, ResourceVersion, VmDefinition, VmId,
    };
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Barrier;
    use tokio::time::{sleep, Duration};

    fn version(value: u64) -> ResourceVersion {
        ResourceVersion::new(value).unwrap()
    }

    fn vm(id: &str) -> VmDefinition {
        VmDefinition {
            id: VmId::new(id).unwrap(),
            name: format!("vm-{id}"),
            boot: BootSpec::new("/kernel").unwrap(),
            compute: ComputeSpec::new(2, 1024).unwrap(),
            storage: vec![],
            networks: vec![],
            requested_power_state: RequestedPowerState::Stopped,
            observed_power_state: ObservedPowerState::Unknown,
            resource_version: version(1),
        }
    }

    fn submission(
        command: MutationCommand,
        operation: &str,
        key: &str,
        expected: u64,
    ) -> SubmitMutation {
        SubmitMutation {
            operation_id: OperationId::new(operation).unwrap(),
            idempotency_scope: "actor-test".to_owned(),
            idempotency_key: IdempotencyKey::new(key).unwrap(),
            expected_vm_version: version(expected),
            command,
        }
    }

    fn service() -> (TempDir, std::path::PathBuf, OperationService) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("core.db");
        let store = CoreStore::create_new(&path).unwrap();
        (directory, path, OperationService::new(store))
    }

    async fn gate_actor(handle: &AuthorityHandle) -> std::sync::mpsc::Sender<()> {
        let (entered, wait_entered) = oneshot::channel();
        let (release, wait_release) = std::sync::mpsc::channel();
        handle
            .sender
            .send(Request::Gate(entered, wait_release))
            .await
            .unwrap();
        wait_entered.await.unwrap();
        release
    }

    async fn wait_for_queue_len(handle: &AuthorityHandle, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while handle.sender.len() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("authority queue did not reach expected occupancy");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_identical_requests_share_one_operation() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 2).unwrap();
        let barrier = Arc::new(Barrier::new(17));
        let mut tasks = Vec::new();
        for index in 0..16 {
            let handle = handle.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                handle
                    .submit(submission(
                        MutationCommand::CreateVm {
                            definition: vm("a"),
                        },
                        &format!("op-{index}"),
                        "same-key",
                        1,
                    ))
                    .await
                    .unwrap()
            }));
        }
        barrier.wait().await;
        let mut accepted = 0;
        for task in tasks {
            let result = task.await.unwrap();
            accepted += usize::from(result.disposition == Acceptance::Accepted);
        }
        assert_eq!(accepted, 1);
        assert_eq!(handle.operations().await.unwrap().len(), 1);
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn serialized_fingerprint_conflict_does_not_create_a_second_operation() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 2).unwrap();
        let first = handle.submit(submission(
            MutationCommand::CreateVm {
                definition: vm("a"),
            },
            "op-a",
            "key",
            1,
        ));
        let second = handle.submit(submission(
            MutationCommand::CreateVm {
                definition: vm("b"),
            },
            "op-b",
            "key",
            1,
        ));
        let (first, second) = tokio::join!(first, second);
        assert!(first.is_ok());
        assert!(matches!(
            second,
            Err(AuthorityActorError::Service(OperationServiceError::Store(
                StoreError::IdempotencyConflict { .. }
            )))
        ));
        assert_eq!(handle.operations().await.unwrap().len(), 1);
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn serialized_same_version_mutations_accept_exactly_one() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 2).unwrap();
        handle
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "create",
                "create",
                1,
            ))
            .await
            .unwrap();
        let start = handle.submit(submission(
            MutationCommand::StartVm {
                vm_id: VmId::new("a").unwrap(),
            },
            "start",
            "start",
            1,
        ));
        let reboot = handle.submit(submission(
            MutationCommand::RebootVm {
                vm_id: VmId::new("a").unwrap(),
            },
            "reboot",
            "reboot",
            1,
        ));
        let (start, reboot) = tokio::join!(start, reboot);
        let accepted = [&start, &reboot]
            .into_iter()
            .filter(|result| result.is_ok())
            .count();
        let stale = [&start, &reboot]
            .into_iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(AuthorityActorError::Service(OperationServiceError::Store(
                        StoreError::StaleVersion { .. }
                    )))
                )
            })
            .count();
        assert_eq!((accepted, stale), (1, 1));
        assert_eq!(
            handle
                .vm(VmId::new("a").unwrap())
                .await
                .unwrap()
                .resource_version,
            version(2)
        );
        assert_eq!(handle.operations().await.unwrap().len(), 2);
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn replay_survives_actor_restart() {
        let (_directory, path, service) = service();
        let original = submission(
            MutationCommand::CreateVm {
                definition: vm("a"),
            },
            "original",
            "key",
            1,
        );
        let (handle, join) = AuthorityActor::spawn(service, 1).unwrap();
        handle.submit(original.clone()).await.unwrap();
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
        let (handle, join) =
            AuthorityActor::spawn(OperationService::open_existing(&path).unwrap(), 1).unwrap();
        let replay = handle
            .submit(SubmitMutation {
                operation_id: OperationId::new("replay").unwrap(),
                ..original
            })
            .await
            .unwrap();
        assert_eq!(replay.disposition, Acceptance::Replay);
        assert_eq!(replay.operation.id.as_str(), "original");
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_same_token_claims_are_idempotent_and_differently_fenced() {
        let (_directory, _path, service) = service();
        let (handle, execution, join) = AuthorityActor::spawn_with_execution(service, 4).unwrap();
        handle
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op",
                "key",
                1,
            ))
            .await
            .unwrap();

        let barrier = Arc::new(Barrier::new(5));
        let mut claims = Vec::new();
        for _ in 0..4 {
            let execution = execution.clone();
            let barrier = barrier.clone();
            claims.push(tokio::spawn(async move {
                barrier.wait().await;
                execution
                    .claim_attempt(
                        OperationId::new("op").unwrap(),
                        AttemptToken::new("attempt-1").unwrap(),
                    )
                    .await
            }));
        }
        barrier.wait().await;

        let mut attempts = Vec::new();
        for claim in claims {
            match claim.await.unwrap() {
                Ok(ClaimResult::Acquired(entry)) => {
                    attempts.push((true, entry.operation.attempt_count))
                }
                Ok(ClaimResult::Replay(entry)) => {
                    attempts.push((false, entry.operation.attempt_count))
                }
                other => panic!("unexpected claim result: {other:?}"),
            }
        }
        assert_eq!(attempts.iter().filter(|(acquired, _)| *acquired).count(), 1);
        assert!(attempts.iter().all(|(_, attempt)| *attempt == 1));
        assert!(matches!(
            execution
                .claim_attempt(
                    OperationId::new("op").unwrap(),
                    AttemptToken::new("attempt-2").unwrap(),
                )
                .await,
            Err(AuthorityActorError::Service(OperationServiceError::Store(
                StoreError::Conflict { .. }
            )))
        ));
        let operation = handle
            .operation(OperationId::new("op").unwrap())
            .await
            .unwrap();
        assert_eq!(operation.operation.status, OperationStatus::Running);
        assert_eq!(operation.operation.attempt_count, 1);
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn dropped_claim_reply_remains_durable_across_actor_restart() {
        let (_directory, path, service) = service();
        let (handle, _execution, join) = AuthorityActor::spawn_with_execution(service, 1).unwrap();
        handle
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op",
                "key",
                1,
            ))
            .await
            .unwrap();

        let (reply, receive) = oneshot::channel();
        handle
            .sender
            .send(Request::ClaimAttempt(
                OperationId::new("op").unwrap(),
                AttemptToken::new("attempt-1").unwrap(),
                reply,
            ))
            .await
            .unwrap();
        drop(receive);
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();

        let (handle, execution, join) = AuthorityActor::spawn_with_execution(
            OperationService::open_existing(&path).unwrap(),
            1,
        )
        .unwrap();
        let restart = execution.restart_operations().await.unwrap();
        assert_eq!(restart.len(), 1);
        assert_eq!(restart[0].disposition, RestartDisposition::InspectRequired);
        assert_eq!(restart[0].entry.operation.status, OperationStatus::Running);
        assert_eq!(restart[0].entry.operation.attempt_count, 1);
        assert_eq!(
            execution
                .claim_attempt(
                    OperationId::new("op").unwrap(),
                    AttemptToken::new("attempt-1").unwrap(),
                )
                .await
                .unwrap()
                .entry()
                .operation
                .attempt_count,
            1
        );
        assert!(execution
            .claim_attempt(
                OperationId::new("op").unwrap(),
                AttemptToken::new("attempt-2").unwrap(),
            )
            .await
            .is_err());
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn dropped_finish_reply_is_resolved_only_by_exact_completion_replay() {
        let (_directory, path, service) = service();
        let (handle, execution, join) = AuthorityActor::spawn_with_execution(service, 2).unwrap();
        handle
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op",
                "key",
                1,
            ))
            .await
            .unwrap();
        let token = AttemptToken::new("attempt-1").unwrap();
        assert!(matches!(
            execution
                .claim_attempt(OperationId::new("op").unwrap(), token.clone())
                .await
                .unwrap(),
            ClaimResult::Acquired(_)
        ));
        let outcome = TerminalOutcome::Succeeded(Some(serde_json::json!({"pid": 42})));
        let (reply, receive) = oneshot::channel();
        execution
            .sender
            .send(Request::Finish(
                OperationId::new("op").unwrap(),
                token.clone(),
                Box::new(outcome.clone()),
                reply,
            ))
            .await
            .unwrap();
        drop(receive);

        let replay = execution
            .finish(
                OperationId::new("op").unwrap(),
                token.clone(),
                outcome.clone(),
            )
            .await
            .unwrap();
        assert_eq!(replay.disposition, crate::CompletionDisposition::Replay);
        assert!(execution
            .finish(
                OperationId::new("op").unwrap(),
                token.clone(),
                TerminalOutcome::Succeeded(Some(serde_json::json!({"pid": 43}))),
            )
            .await
            .is_err());
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();

        let (handle, execution, join) = AuthorityActor::spawn_with_execution(
            OperationService::open_existing(&path).unwrap(),
            2,
        )
        .unwrap();
        assert_eq!(
            execution
                .finish(OperationId::new("op").unwrap(), token, outcome)
                .await
                .unwrap()
                .disposition,
            crate::CompletionDisposition::Replay
        );
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn finish_is_ordered_before_shutdown_and_persists_terminal_event() {
        let (_directory, path, service) = service();
        let (handle, execution, join) = AuthorityActor::spawn_with_execution(service, 2).unwrap();
        handle
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op",
                "key",
                1,
            ))
            .await
            .unwrap();
        execution
            .claim_attempt(
                OperationId::new("op").unwrap(),
                AttemptToken::new("attempt-1").unwrap(),
            )
            .await
            .unwrap();

        let release = gate_actor(&handle).await;
        let finish_execution = execution.clone();
        let finish = tokio::spawn(async move {
            finish_execution
                .finish(
                    OperationId::new("op").unwrap(),
                    AttemptToken::new("attempt-1").unwrap(),
                    TerminalOutcome::Succeeded(Some(serde_json::json!({"runtime": "created"}))),
                )
                .await
        });
        wait_for_queue_len(&handle, 1).await;
        let shutdown_handle = handle.clone();
        let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
        wait_for_queue_len(&handle, 2).await;
        release.send(()).unwrap();
        let finished = finish.await.unwrap();
        assert_eq!(
            finished.unwrap().entry.operation.status,
            OperationStatus::Succeeded
        );
        shutdown.await.unwrap().unwrap();
        join.join().await.unwrap();

        let (handle, reopened_join) =
            AuthorityActor::spawn(OperationService::open_existing(&path).unwrap(), 2).unwrap();
        let operation = handle
            .operation(OperationId::new("op").unwrap())
            .await
            .unwrap();
        assert_eq!(operation.operation.status, OperationStatus::Succeeded);
        let events = handle.events_after(0, 100).await.unwrap();
        assert!(events.iter().any(|event| event.kind == "operation.running"));
        assert!(events
            .iter()
            .any(|event| event.kind == "operation.succeeded"));
        handle.shutdown().await.unwrap();
        reopened_join.join().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_drains_prior_requests_and_rejects_later_requests() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 4).unwrap();
        let prior = handle.submit(submission(
            MutationCommand::CreateVm {
                definition: vm("a"),
            },
            "op",
            "key",
            1,
        ));
        let shutdown = handle.shutdown();
        let (prior, shutdown) = tokio::join!(prior, shutdown);
        assert!(prior.is_ok());
        shutdown.unwrap();
        assert!(matches!(
            handle.vms().await,
            Err(AuthorityActorError::Unavailable)
        ));
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn dropped_reply_does_not_cancel_durable_acceptance() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 1).unwrap();
        let (reply, receive) = oneshot::channel();
        handle
            .sender
            .send(Request::Submit(
                Box::new(submission(
                    MutationCommand::CreateVm {
                        definition: vm("a"),
                    },
                    "op",
                    "key",
                    1,
                )),
                reply,
            ))
            .await
            .unwrap();
        drop(receive);
        let operation = handle
            .operation(OperationId::new("op").unwrap())
            .await
            .unwrap();
        assert_eq!(operation.operation.id.as_str(), "op");
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn blocked_database_thread_does_not_stall_current_thread_runtime() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 1).unwrap();
        let release = gate_actor(&handle).await;

        tokio::time::timeout(Duration::from_millis(100), sleep(Duration::from_millis(10)))
            .await
            .expect("authority work blocked the Tokio runtime");

        release.send(()).unwrap();
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn full_queue_backpressures_and_cancel_before_enqueue_has_no_mutation() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 1).unwrap();
        let release = gate_actor(&handle).await;

        let first_handle = handle.clone();
        let first = tokio::spawn(async move {
            first_handle
                .submit(submission(
                    MutationCommand::CreateVm {
                        definition: vm("a"),
                    },
                    "first",
                    "first",
                    1,
                ))
                .await
        });
        wait_for_queue_len(&handle, 1).await;

        let cancelled_handle = handle.clone();
        let cancelled = tokio::spawn(async move {
            cancelled_handle
                .submit(submission(
                    MutationCommand::CreateVm {
                        definition: vm("b"),
                    },
                    "cancelled",
                    "cancelled",
                    1,
                ))
                .await
        });
        tokio::task::yield_now().await;
        assert!(!cancelled.is_finished());
        cancelled.abort();
        assert!(cancelled.await.unwrap_err().is_cancelled());

        release.send(()).unwrap();
        assert!(first.await.unwrap().is_ok());
        let operations = handle.operations().await.unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].operation.id.as_str(), "first");
        handle.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_orders_full_queue_and_rejects_request_behind_it() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 2).unwrap();
        let release = gate_actor(&handle).await;

        let prior_handle = handle.clone();
        let prior = tokio::spawn(async move {
            prior_handle
                .submit(submission(
                    MutationCommand::CreateVm {
                        definition: vm("a"),
                    },
                    "prior",
                    "prior",
                    1,
                ))
                .await
        });
        wait_for_queue_len(&handle, 1).await;
        let shutdown_handle = handle.clone();
        let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
        wait_for_queue_len(&handle, 2).await;
        let later_handle = handle.clone();
        let later = tokio::spawn(async move {
            later_handle
                .submit(submission(
                    MutationCommand::CreateVm {
                        definition: vm("b"),
                    },
                    "later",
                    "later",
                    1,
                ))
                .await
        });
        tokio::task::yield_now().await;
        assert!(!later.is_finished());

        release.send(()).unwrap();
        assert!(prior.await.unwrap().is_ok());
        assert!(shutdown.await.unwrap().is_ok());
        assert!(matches!(
            later.await.unwrap(),
            Err(AuthorityActorError::Unavailable)
        ));
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn concurrent_shutdown_callers_have_one_linearization_winner() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 2).unwrap();
        let left_handle = handle.clone();
        let right_handle = handle.clone();
        let (left, right) = tokio::join!(left_handle.shutdown(), right_handle.shutdown());
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        assert!(left.is_ok() || matches!(left, Err(AuthorityActorError::Unavailable)));
        assert!(right.is_ok() || matches!(right, Err(AuthorityActorError::Unavailable)));
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn dropping_owner_closes_handles_and_joins_worker() {
        let (_directory, _path, service) = service();
        let (handle, join) = AuthorityActor::spawn(service, 1).unwrap();
        drop(join);
        assert!(matches!(
            handle.vms().await,
            Err(AuthorityActorError::Unavailable)
        ));
    }
}
