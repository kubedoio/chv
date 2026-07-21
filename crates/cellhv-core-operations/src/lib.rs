//! Transport-neutral mutation application service for the `chv-agent` authority.
//!
//! This crate accepts and journals desired-state mutations. It deliberately has
//! no runtime or provider dependencies and performs no external side effects.

mod authority_actor;

pub use authority_actor::{
    AuthorityActor, AuthorityActorError, AuthorityActorJoin, AuthorityHandle, ExecutionHandle,
};

use cellhv_core_store::{AcceptOperation, CoreStore};
pub use cellhv_core_store::{
    Acceptance, AcceptedOperation, HostRecord, MigrationDisposition, OperationJournalEntry,
};
use cellhv_core_types::{
    canonical_request_fingerprint, IdempotencyKey, ObservedPowerState, Operation, OperationEvent,
    OperationId, OperationKind, OperationStatus, RequestedPowerState, ResourceVersion,
    VmDefinition, VmId,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

const DEFAULT_MAX_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttemptToken(String);

impl AttemptToken {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .as_bytes()
                .iter()
                .all(|byte| (b'!'..=b'~').contains(byte))
        {
            return Err(OperationServiceError::Invalid(
                "attempt token must be 1..=128 visible ASCII bytes".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum MutationCommand {
    CreateVm { definition: VmDefinition },
    UpdateVm { definition: VmDefinition },
    DeleteVm { vm_id: VmId },
    StartVm { vm_id: VmId },
    StopVm { vm_id: VmId },
    RebootVm { vm_id: VmId },
}

impl MutationCommand {
    pub fn vm_id(&self) -> &VmId {
        match self {
            Self::CreateVm { definition } | Self::UpdateVm { definition } => &definition.id,
            Self::DeleteVm { vm_id }
            | Self::StartVm { vm_id }
            | Self::StopVm { vm_id }
            | Self::RebootVm { vm_id } => vm_id,
        }
    }

    pub fn kind(&self) -> OperationKind {
        match self {
            Self::CreateVm { .. } => OperationKind::CreateVm,
            Self::UpdateVm { .. } => OperationKind::UpdateVm,
            Self::DeleteVm { .. } => OperationKind::DeleteVm,
            Self::StartVm { .. } => OperationKind::StartVm,
            Self::StopVm { .. } => OperationKind::StopVm,
            Self::RebootVm { .. } => OperationKind::RebootVm,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitMutation {
    pub operation_id: OperationId,
    pub idempotency_scope: String,
    pub idempotency_key: IdempotencyKey,
    pub expected_vm_version: ResourceVersion,
    pub command: MutationCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDisposition {
    Ready,
    InspectRequired,
    Terminal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RestartOperation {
    pub entry: OperationJournalEntry,
    pub disposition: RestartDisposition,
    pub active_attempt_token: Option<AttemptToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClaimResult {
    Acquired(OperationJournalEntry),
    Replay(OperationJournalEntry),
}

impl ClaimResult {
    pub fn entry(&self) -> &OperationJournalEntry {
        match self {
            Self::Acquired(entry) | Self::Replay(entry) => entry,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionDisposition {
    Applied,
    Replay,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionResult {
    pub disposition: CompletionDisposition,
    pub entry: OperationJournalEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalOutcome {
    Succeeded(Option<serde_json::Value>),
    Failed(serde_json::Value),
    Unsupported(serde_json::Value),
}

#[derive(Debug, Error)]
pub enum OperationServiceError {
    #[error("invalid mutation: {0}")]
    Invalid(String),
    #[error("unsupported mutation feature: {0}")]
    Unsupported(&'static str),
    #[error(transparent)]
    Store(#[from] cellhv_core_store::StoreError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Invalid,
    Unsupported,
    NotFound,
    Conflict,
    Precondition,
    Internal,
}

impl OperationServiceError {
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::Invalid(_) => ErrorClass::Invalid,
            Self::Unsupported(_) => ErrorClass::Unsupported,
            Self::Store(cellhv_core_store::StoreError::NotFound { .. }) => ErrorClass::NotFound,
            Self::Store(
                cellhv_core_store::StoreError::Conflict { .. }
                | cellhv_core_store::StoreError::IdempotencyConflict { .. },
            ) => ErrorClass::Conflict,
            Self::Store(cellhv_core_store::StoreError::StaleVersion { .. }) => {
                ErrorClass::Precondition
            }
            Self::Store(_) | Self::Json(_) => ErrorClass::Internal,
        }
    }
}

pub type Result<T> = std::result::Result<T, OperationServiceError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMigrationState {
    pub source: String,
    pub checksum: String,
    pub cutover: bool,
}

pub struct OperationService {
    store: CoreStore,
}

impl OperationService {
    pub fn has_any_migration_state(&self) -> Result<bool> {
        Ok(self.store.has_any_migration_state()?)
    }

    /// Creates an empty authority exclusively for a validated legacy import.
    pub fn create_migration_target(path: &Path) -> Result<Self> {
        Ok(Self::new(CoreStore::create_new(path)?))
    }

    pub fn create_new(path: &Path, host: &cellhv_core_types::HostIdentity) -> Result<Self> {
        let store = CoreStore::create_new_with_host(
            path,
            host,
            &cellhv_core_types::HostCapabilities::default(),
        )?;
        Ok(Self::new(store))
    }

    pub fn open_existing(path: &Path) -> Result<Self> {
        Ok(Self::new(CoreStore::open_existing(path)?))
    }

    pub fn new(store: CoreStore) -> Self {
        Self { store }
    }

    pub fn submit(&mut self, submission: SubmitMutation) -> Result<AcceptedOperation> {
        if submission.idempotency_scope.trim().is_empty() {
            return Err(OperationServiceError::Invalid(
                "idempotency scope must not be empty".to_owned(),
            ));
        }
        let request = canonical_request(&submission.command, submission.expected_vm_version);
        let fingerprint = canonical_request_fingerprint(&request)?;
        if let Some(replay) = self.store.resolve_idempotency(
            &submission.idempotency_scope,
            &submission.idempotency_key,
            &fingerprint,
            &request,
        )? {
            return Ok(replay);
        }
        let desired = self.desired_state(&submission)?;
        let operation = Operation {
            id: submission.operation_id,
            kind: submission.command.kind(),
            vm_id: submission.command.vm_id().clone(),
            status: OperationStatus::Accepted,
            request_fingerprint: fingerprint,
            attempt_count: 0,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        };
        Ok(self.store.accept_operation(&AcceptOperation {
            operation: &operation,
            request: &request,
            desired_vm: desired.as_ref(),
            idempotency_scope: &submission.idempotency_scope,
            idempotency_key: &submission.idempotency_key,
            expected_vm_version: submission.expected_vm_version,
        })?)
    }

    pub fn operation(&self, id: &OperationId) -> Result<OperationJournalEntry> {
        Ok(self.store.operation_entry(id)?)
    }

    pub fn host(&self) -> Result<HostRecord> {
        Ok(self.store.host()?)
    }

    pub fn vms(&self) -> Result<Vec<VmDefinition>> {
        Ok(self.store.list_vms()?)
    }

    pub fn operations(&self) -> Result<Vec<OperationJournalEntry>> {
        Ok(self.store.list_operations()?)
    }

    pub fn events_after(&self, sequence: u64, limit: u32) -> Result<Vec<OperationEvent>> {
        Ok(self.store.list_events_after(sequence, limit)?)
    }

    /// Durably claims the next bounded attempt before any external side effect.
    pub(crate) fn claim_attempt(
        &mut self,
        id: &OperationId,
        attempt_token: &AttemptToken,
    ) -> Result<ClaimResult> {
        let claimed = self.store.claim_operation(id, attempt_token.as_str())?;
        Ok(match claimed.disposition {
            cellhv_core_store::ClaimDisposition::Acquired => ClaimResult::Acquired(claimed.entry),
            cellhv_core_store::ClaimDisposition::Replay => ClaimResult::Replay(claimed.entry),
        })
    }

    /// Durably records one terminal outcome and its correlated event.
    pub(crate) fn finish(
        &mut self,
        id: &OperationId,
        attempt_token: &AttemptToken,
        outcome: TerminalOutcome,
    ) -> Result<CompletionResult> {
        let (status, result, error) = match &outcome {
            TerminalOutcome::Succeeded(result) => {
                (OperationStatus::Succeeded, result.as_ref(), None)
            }
            TerminalOutcome::Failed(error) => (OperationStatus::Failed, None, Some(error)),
            TerminalOutcome::Unsupported(error) => {
                (OperationStatus::Unsupported, None, Some(error))
            }
        };
        let completed = self.store.persist_terminal_operation(
            id,
            attempt_token.as_str(),
            status,
            result,
            error,
        )?;
        Ok(CompletionResult {
            disposition: match completed.disposition {
                cellhv_core_store::CompletionDisposition::Applied => CompletionDisposition::Applied,
                cellhv_core_store::CompletionDisposition::Replay => CompletionDisposition::Replay,
            },
            entry: completed.entry,
        })
    }

    pub(crate) fn restart_operations(&self) -> Result<Vec<RestartOperation>> {
        self.store
            .list_incomplete_execution_operations()?
            .into_iter()
            .map(|record| {
                Ok(RestartOperation {
                    disposition: classify_restart(&record.entry.operation),
                    entry: record.entry,
                    active_attempt_token: record
                        .active_attempt_token
                        .map(AttemptToken::new)
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    pub fn vm(&self, id: &VmId) -> Result<VmDefinition> {
        Ok(self.store.get_vm(id)?)
    }

    pub fn import_legacy_snapshot(
        &mut self,
        source: &str,
        source_checksum: &str,
        host: &cellhv_core_types::HostIdentity,
        definitions: &[VmDefinition],
    ) -> Result<MigrationDisposition> {
        Ok(self.store.import_legacy_snapshot(
            source,
            source_checksum,
            host,
            &cellhv_core_types::HostCapabilities::default(),
            definitions,
        )?)
    }

    pub fn cutover_legacy_snapshot(
        &mut self,
        source: &str,
        checksum: &str,
    ) -> Result<MigrationDisposition> {
        Ok(self.store.cutover_legacy_snapshot(source, checksum)?)
    }

    pub fn rollback_legacy_import(
        &mut self,
        source: &str,
        checksum: &str,
    ) -> Result<MigrationDisposition> {
        Ok(self.store.rollback_legacy_import(source, checksum)?)
    }

    pub fn legacy_migration_state(&self, source: &str) -> Result<Option<LegacyMigrationState>> {
        Ok(self
            .store
            .legacy_migration_state(source)?
            .map(|state| LegacyMigrationState {
                source: state.source,
                checksum: state.checksum,
                cutover: state.cutover,
            }))
    }

    pub fn is_pristine_migration_target(&self) -> Result<bool> {
        Ok(self.store.is_pristine_migration_target()?)
    }

    fn desired_state(&self, submission: &SubmitMutation) -> Result<Option<VmDefinition>> {
        let expected = submission.expected_vm_version;
        match &submission.command {
            MutationCommand::CreateVm { definition } => {
                definition
                    .validate()
                    .map_err(|error| OperationServiceError::Invalid(error.to_string()))?;
                if expected.get() != 1 || definition.resource_version != expected {
                    return Err(OperationServiceError::Invalid(
                        "create definition and expected resource version must both be one"
                            .to_owned(),
                    ));
                }
                if definition.observed_power_state != ObservedPowerState::Unknown {
                    return Err(OperationServiceError::Invalid(
                        "create definition observed power state must be unknown".to_owned(),
                    ));
                }
                Ok(Some(definition.clone()))
            }
            MutationCommand::UpdateVm { definition } => {
                definition
                    .validate()
                    .map_err(|error| OperationServiceError::Invalid(error.to_string()))?;
                let current = self.store.get_vm(&definition.id)?;
                if definition.resource_version != expected_next(expected)? {
                    return Err(OperationServiceError::Invalid(
                        "update definition must carry expected resource version + 1".to_owned(),
                    ));
                }
                if definition.observed_power_state != current.observed_power_state {
                    return Err(OperationServiceError::Invalid(
                        "mutations cannot set observed power state".to_owned(),
                    ));
                }
                if current.resource_version == expected
                    && definition.requested_power_state != current.requested_power_state
                {
                    return Err(OperationServiceError::Invalid(
                        "use a power action to change requested power state".to_owned(),
                    ));
                }
                if current.resource_version == expected
                    && (definition.storage != current.storage
                        || definition.networks != current.networks)
                {
                    return Err(OperationServiceError::Unsupported(
                        "attachment topology updates",
                    ));
                }
                Ok(Some(definition.clone()))
            }
            MutationCommand::DeleteVm { vm_id } => {
                // The store performs the conditional tombstone write after its
                // atomic idempotency replay check.
                let _ = vm_id;
                Ok(None)
            }
            MutationCommand::StartVm { vm_id } => Ok(Some(power_desired(
                &self.store,
                vm_id,
                expected,
                RequestedPowerState::Running,
            )?)),
            MutationCommand::StopVm { vm_id } => Ok(Some(power_desired(
                &self.store,
                vm_id,
                expected,
                RequestedPowerState::Stopped,
            )?)),
            MutationCommand::RebootVm { vm_id } => {
                let current =
                    power_desired(&self.store, vm_id, expected, RequestedPowerState::Running)?;
                Ok(Some(current))
            }
        }
    }
}

pub fn request_fingerprint(
    command: &MutationCommand,
    expected_vm_version: ResourceVersion,
) -> Result<String> {
    Ok(canonical_request_fingerprint(&canonical_request(
        command,
        expected_vm_version,
    ))?)
}

pub fn classify_restart(operation: &Operation) -> RestartDisposition {
    match operation.status {
        OperationStatus::Accepted => RestartDisposition::Ready,
        OperationStatus::Running => RestartDisposition::InspectRequired,
        OperationStatus::Succeeded | OperationStatus::Failed | OperationStatus::Unsupported => {
            RestartDisposition::Terminal
        }
    }
}

fn canonical_request(
    command: &MutationCommand,
    expected_vm_version: ResourceVersion,
) -> serde_json::Value {
    serde_json::json!({
        "command": command,
        "expected_vm_version": expected_vm_version,
    })
}

fn expected_next(version: ResourceVersion) -> Result<ResourceVersion> {
    version
        .next()
        .map_err(|error| OperationServiceError::Invalid(error.to_string()))
}

fn power_desired(
    store: &CoreStore,
    vm_id: &VmId,
    expected: ResourceVersion,
    requested: RequestedPowerState,
) -> Result<VmDefinition> {
    let mut current = store.get_vm(vm_id)?;
    current.requested_power_state = requested;
    current.resource_version = expected_next(expected)?;
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cellhv_core_store::{Acceptance, StoreError};
    use cellhv_core_types::{BootSpec, ComputeSpec, ObservedPowerState};
    use tempfile::TempDir;

    fn version(value: u64) -> ResourceVersion {
        ResourceVersion::new(value).unwrap()
    }

    #[test]
    fn attempt_tokens_have_one_canonical_visible_ascii_form() {
        assert_eq!(
            AttemptToken::new("token-._:~").unwrap().as_str(),
            "token-._:~"
        );
        for invalid in ["", " leading", "trailing ", "line\nbreak"] {
            assert!(AttemptToken::new(invalid).is_err(), "accepted {invalid:?}");
        }
        assert!(AttemptToken::new("x".repeat(128)).is_ok());
        assert!(AttemptToken::new("x".repeat(129)).is_err());
        assert!(AttemptToken::new("café").is_err());
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
            idempotency_scope: "local-api".to_owned(),
            idempotency_key: IdempotencyKey::new(key).unwrap(),
            expected_vm_version: version(expected),
            command,
        }
    }

    fn service() -> (TempDir, std::path::PathBuf, OperationService) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core.db");
        let store = CoreStore::create_new(&path).unwrap();
        (dir, path, OperationService::new(store))
    }

    #[test]
    fn fingerprint_is_deterministic_and_command_sensitive() {
        let command = MutationCommand::CreateVm {
            definition: vm("a"),
        };
        let expected = request_fingerprint(&command, version(1)).unwrap();
        for _ in 0..100 {
            assert_eq!(request_fingerprint(&command, version(1)).unwrap(), expected);
        }
        assert_ne!(
            expected,
            request_fingerprint(
                &MutationCommand::CreateVm {
                    definition: vm("b")
                },
                version(1)
            )
            .unwrap()
        );
        assert_ne!(expected, request_fingerprint(&command, version(2)).unwrap());
    }

    #[test]
    fn create_is_durable_before_returning() {
        let (_dir, path, mut service) = service();
        let accepted = service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "key-1",
                1,
            ))
            .unwrap();
        assert_eq!(accepted.disposition, Acceptance::Accepted);
        drop(service);
        let reopened = CoreStore::open_existing(&path).unwrap();
        assert_eq!(reopened.get_vm(&VmId::new("a").unwrap()).unwrap(), vm("a"));
        assert_eq!(
            reopened
                .operation(&OperationId::new("op-1").unwrap())
                .unwrap()
                .status,
            OperationStatus::Accepted
        );
    }

    #[test]
    fn replay_returns_original_and_changed_request_conflicts() {
        let (_dir, _path, mut service) = service();
        let first = submission(
            MutationCommand::CreateVm {
                definition: vm("a"),
            },
            "op-1",
            "key",
            1,
        );
        service.submit(first).unwrap();
        let replay = service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-2",
                "key",
                1,
            ))
            .unwrap();
        assert_eq!(replay.disposition, Acceptance::Replay);
        assert_eq!(replay.operation.id.as_str(), "op-1");
        let error = service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("b"),
                },
                "op-3",
                "key",
                1,
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            OperationServiceError::Store(StoreError::IdempotencyConflict { .. })
        ));
        let precondition_conflict = service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-4",
                "key",
                2,
            ))
            .unwrap_err();
        assert!(matches!(
            precondition_conflict,
            OperationServiceError::Store(StoreError::IdempotencyConflict { .. })
        ));
    }

    #[test]
    fn stale_power_request_does_not_reserve_state_or_journal() {
        let (_dir, _path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        let error = service
            .submit(submission(
                MutationCommand::StartVm {
                    vm_id: VmId::new("a").unwrap(),
                },
                "op-2",
                "start",
                2,
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            OperationServiceError::Store(StoreError::StaleVersion { .. })
        ));
        assert!(matches!(
            service.operation(&OperationId::new("op-2").unwrap()),
            Err(OperationServiceError::Store(StoreError::NotFound { .. }))
        ));
        assert_eq!(
            service
                .vm(&VmId::new("a").unwrap())
                .unwrap()
                .resource_version,
            version(1)
        );
    }

    #[test]
    fn power_actions_update_only_requested_state_and_version() {
        let (_dir, _path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        service
            .submit(submission(
                MutationCommand::StartVm {
                    vm_id: VmId::new("a").unwrap(),
                },
                "op-2",
                "start",
                1,
            ))
            .unwrap();
        let current = service.vm(&VmId::new("a").unwrap()).unwrap();
        assert_eq!(current.requested_power_state, RequestedPowerState::Running);
        assert_eq!(current.observed_power_state, ObservedPowerState::Unknown);
        assert_eq!(current.resource_version, version(2));
    }

    #[test]
    fn power_replay_survives_the_version_reserved_by_first_acceptance() {
        let (_dir, _path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        let command = MutationCommand::StartVm {
            vm_id: VmId::new("a").unwrap(),
        };
        service
            .submit(submission(command.clone(), "op-2", "start", 1))
            .unwrap();
        let replay = service
            .submit(submission(command, "op-3", "start", 1))
            .unwrap();
        assert_eq!(replay.disposition, Acceptance::Replay);
        assert_eq!(replay.operation.id.as_str(), "op-2");
    }

    #[test]
    fn exact_replay_resolves_after_vm_is_tombstoned() {
        let (_dir, _path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        let start = MutationCommand::StartVm {
            vm_id: VmId::new("a").unwrap(),
        };
        service
            .submit(submission(start.clone(), "op-2", "start", 1))
            .unwrap();
        service
            .submit(submission(
                MutationCommand::DeleteVm {
                    vm_id: VmId::new("a").unwrap(),
                },
                "op-3",
                "delete",
                2,
            ))
            .unwrap();
        let replay = service
            .submit(submission(start, "ignored", "start", 1))
            .unwrap();
        assert_eq!(replay.disposition, Acceptance::Replay);
        assert_eq!(replay.operation.id.as_str(), "op-2");
    }

    #[test]
    fn observed_state_is_never_client_controlled() {
        let (_dir, _path, mut service) = service();
        let mut invalid_create = vm("a");
        invalid_create.observed_power_state = ObservedPowerState::Running;
        assert!(matches!(
            service.submit(submission(
                MutationCommand::CreateVm {
                    definition: invalid_create,
                },
                "op-1",
                "create-invalid",
                1,
            )),
            Err(OperationServiceError::Invalid(_))
        ));
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-2",
                "create",
                1,
            ))
            .unwrap();
        let mut stale_update = vm("a");
        stale_update.resource_version = version(3);
        stale_update.observed_power_state = ObservedPowerState::Running;
        assert!(matches!(
            service.submit(submission(
                MutationCommand::UpdateVm {
                    definition: stale_update,
                },
                "op-3",
                "stale-update",
                2,
            )),
            Err(OperationServiceError::Invalid(_))
        ));
    }

    #[test]
    fn attachment_topology_update_is_explicitly_unsupported() {
        let (_dir, _path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        let mut changed = vm("a");
        changed.resource_version = version(2);
        changed
            .networks
            .push(cellhv_core_types::NetworkAttachmentRef {
                attachment_id: "nic-1".to_owned(),
                network_ref: "network-1".to_owned(),
                mac_address: None,
            });
        let error = service
            .submit(submission(
                MutationCommand::UpdateVm {
                    definition: changed,
                },
                "op-2",
                "update",
                1,
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            OperationServiceError::Unsupported("attachment topology updates")
        ));
    }

    #[test]
    fn restart_reconstructs_incomplete_journal_in_stable_order() {
        let (_dir, path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        drop(service);
        let service = OperationService::new(CoreStore::open_existing(&path).unwrap());
        let pending = service.restart_operations().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].entry.operation.id.as_str(), "op-1");
        assert_eq!(pending[0].disposition, RestartDisposition::Ready);
    }

    #[test]
    fn restart_classification_covers_retry_boundaries() {
        let mut operation = Operation {
            id: OperationId::new("op").unwrap(),
            kind: OperationKind::StartVm,
            vm_id: VmId::new("vm").unwrap(),
            status: OperationStatus::Accepted,
            request_fingerprint: "fingerprint".to_owned(),
            attempt_count: 0,
            max_attempts: 3,
        };
        assert_eq!(classify_restart(&operation), RestartDisposition::Ready);
        operation.status = OperationStatus::Running;
        operation.attempt_count = 2;
        assert_eq!(
            classify_restart(&operation),
            RestartDisposition::InspectRequired
        );
        operation.attempt_count = 3;
        assert_eq!(
            classify_restart(&operation),
            RestartDisposition::InspectRequired
        );
        operation.status = OperationStatus::Succeeded;
        assert_eq!(classify_restart(&operation), RestartDisposition::Terminal);
    }

    #[test]
    fn execution_transitions_are_token_fenced_and_terminal_is_immutable() {
        let (_dir, _path, mut service) = service();
        service
            .submit(submission(
                MutationCommand::CreateVm {
                    definition: vm("a"),
                },
                "op-1",
                "create",
                1,
            ))
            .unwrap();
        let id = OperationId::new("op-1").unwrap();
        let token = AttemptToken::new("attempt-1").unwrap();
        let other = AttemptToken::new("attempt-2").unwrap();
        assert!(matches!(
            service.finish(&id, &token, TerminalOutcome::Succeeded(None)),
            Err(OperationServiceError::Store(StoreError::Conflict { .. }))
        ));
        let entry = service.claim_attempt(&id, &token).unwrap();
        assert!(matches!(entry, ClaimResult::Acquired(_)));
        assert_eq!(entry.entry().operation.status, OperationStatus::Running);
        assert_eq!(entry.entry().operation.attempt_count, 1);
        let replay = service.claim_attempt(&id, &token).unwrap();
        assert!(matches!(replay, ClaimResult::Replay(_)));
        assert_eq!(replay.entry().operation.attempt_count, 1);
        assert!(matches!(
            service.claim_attempt(&id, &other),
            Err(OperationServiceError::Store(StoreError::Conflict { .. }))
        ));
        assert!(matches!(
            service.finish(
                &id,
                &other,
                TerminalOutcome::Failed(serde_json::json!({"code":"stale"})),
            ),
            Err(OperationServiceError::Store(StoreError::Conflict { .. }))
        ));
        let terminal = service
            .finish(
                &id,
                &token,
                TerminalOutcome::Failed(serde_json::json!({"code":"exhausted"})),
            )
            .unwrap();
        assert_eq!(terminal.disposition, CompletionDisposition::Applied);
        assert_eq!(terminal.entry.operation.status, OperationStatus::Failed);
        assert_eq!(
            service
                .finish(
                    &id,
                    &token,
                    TerminalOutcome::Failed(serde_json::json!({"code":"exhausted"})),
                )
                .unwrap()
                .disposition,
            CompletionDisposition::Replay
        );
        assert!(matches!(
            service.claim_attempt(&id, &token),
            Err(OperationServiceError::Store(StoreError::Conflict { .. }))
        ));
        assert!(matches!(
            service.finish(&id, &token, TerminalOutcome::Succeeded(None)),
            Err(OperationServiceError::Store(StoreError::Conflict { .. }))
        ));
    }
}
