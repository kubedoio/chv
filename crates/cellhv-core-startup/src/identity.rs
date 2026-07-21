use cellhv_core_operations::{OperationService, OperationServiceError};
use cellhv_core_types::{HostId, HostIdentity, ResourceVersion};
use std::path::Path;
use thiserror::Error;

const RESERVED_HOST_IDS: [&str; 4] = ["unknown", "unset", "none", "null"];

#[derive(Debug, Clone, Default)]
pub struct HostIdentityInputs {
    pub existing_core: Option<HostIdentity>,
    pub importable_nodecache: Option<HostIdentity>,
    pub configured_seed: Option<String>,
    pub precreation_enrollment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshIdentitySource {
    ConfiguredSeed,
    PrecreationEnrollment,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshHostIdentity {
    identity: HostIdentity,
    source: FreshIdentitySource,
}

impl FreshHostIdentity {
    pub fn identity(&self) -> &HostIdentity {
        &self.identity
    }

    pub fn source(&self) -> FreshIdentitySource {
        self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostIdentityDecision {
    UseExistingCore(HostIdentity),
    UseImportableNodeCache(HostIdentity),
    InitializeFresh(FreshHostIdentity),
}

impl HostIdentityDecision {
    pub fn identity(&self) -> &HostIdentity {
        match self {
            Self::UseExistingCore(identity) | Self::UseImportableNodeCache(identity) => identity,
            Self::InitializeFresh(fresh) => fresh.identity(),
        }
    }

    pub fn fresh_source(&self) -> Option<FreshIdentitySource> {
        match self {
            Self::InitializeFresh(fresh) => Some(fresh.source()),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum HostIdentityError {
    #[error("{identity_source} host identity is invalid: {reason}")]
    Invalid {
        identity_source: &'static str,
        reason: String,
    },
    #[error("host identity conflict: {conflicting_source} does not match {authoritative_source}")]
    Conflict {
        authoritative_source: &'static str,
        conflicting_source: &'static str,
    },
    #[error("host identity decision does not authorize fresh Core creation")]
    NotFresh,
    #[error(transparent)]
    Operations(#[from] OperationServiceError),
}

pub type IdentityResult<T> = std::result::Result<T, HostIdentityError>;

pub fn resolve_host_identity(inputs: HostIdentityInputs) -> IdentityResult<HostIdentityDecision> {
    resolve_host_identity_with(inputs, || uuid::Uuid::new_v4().to_string())
}

/// Resolves host identity without touching the filesystem.
///
/// The generator is called exactly once only when every external identity
/// source is absent. Injecting it makes the one-time initialization decision
/// deterministic in tests; persistence remains a separate explicit step.
pub fn resolve_host_identity_with(
    inputs: HostIdentityInputs,
    generate: impl FnOnce() -> String,
) -> IdentityResult<HostIdentityDecision> {
    let existing = validate_identity(inputs.existing_core, "existing Core")?;
    let nodecache = validate_identity(inputs.importable_nodecache, "importable NodeCache")?;
    let configured = validate_raw(inputs.configured_seed, "configured seed")?;
    let enrollment = validate_raw(inputs.precreation_enrollment, "precreation enrollment")?;

    if let Some(identity) = existing {
        assert_matches(
            &identity.id,
            nodecache.as_ref().map(|value| &value.id),
            "existing Core",
            "importable NodeCache",
        )?;
        assert_matches(
            &identity.id,
            configured.as_ref(),
            "existing Core",
            "configured seed",
        )?;
        assert_matches(
            &identity.id,
            enrollment.as_ref(),
            "existing Core",
            "precreation enrollment",
        )?;
        return Ok(HostIdentityDecision::UseExistingCore(identity));
    }

    if let Some(identity) = nodecache {
        assert_matches(
            &identity.id,
            configured.as_ref(),
            "importable NodeCache",
            "configured seed",
        )?;
        assert_matches(
            &identity.id,
            enrollment.as_ref(),
            "importable NodeCache",
            "precreation enrollment",
        )?;
        return Ok(HostIdentityDecision::UseImportableNodeCache(identity));
    }

    match (configured, enrollment) {
        (Some(configured), Some(enrollment)) => {
            assert_matches(
                &configured,
                Some(&enrollment),
                "configured seed",
                "precreation enrollment",
            )?;
            Ok(fresh(configured, FreshIdentitySource::ConfiguredSeed))
        }
        (Some(configured), None) => Ok(fresh(configured, FreshIdentitySource::ConfiguredSeed)),
        (None, Some(enrollment)) => Ok(fresh(
            enrollment,
            FreshIdentitySource::PrecreationEnrollment,
        )),
        (None, None) => {
            let generated = validate_raw(Some(generate()), "generated identity")?
                .expect("provided generated identity is present");
            Ok(fresh(generated, FreshIdentitySource::Generated))
        }
    }
}

/// Creates the sole fresh Core store only after pure identity resolution.
pub fn create_fresh_authority(
    path: &Path,
    decision: &HostIdentityDecision,
) -> IdentityResult<OperationService> {
    let HostIdentityDecision::InitializeFresh(fresh) = decision else {
        return Err(HostIdentityError::NotFresh);
    };
    Ok(OperationService::create_new(path, fresh.identity())?)
}

fn fresh(id: HostId, source: FreshIdentitySource) -> HostIdentityDecision {
    HostIdentityDecision::InitializeFresh(FreshHostIdentity {
        identity: HostIdentity {
            id,
            resource_version: ResourceVersion::new(1).expect("one is a valid resource version"),
        },
        source,
    })
}

fn validate_identity(
    value: Option<HostIdentity>,
    source: &'static str,
) -> IdentityResult<Option<HostIdentity>> {
    value
        .map(|identity| {
            validate_text(identity.id.as_str(), source)?;
            Ok(identity)
        })
        .transpose()
}

fn validate_raw(value: Option<String>, source: &'static str) -> IdentityResult<Option<HostId>> {
    value
        .map(|value| {
            validate_text(&value, source)?;
            HostId::new(value).map_err(|error| HostIdentityError::Invalid {
                identity_source: source,
                reason: error.to_string(),
            })
        })
        .transpose()
}

fn validate_text(value: &str, source: &'static str) -> IdentityResult<()> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(HostIdentityError::Invalid {
            identity_source: source,
            reason: "must not be empty".to_owned(),
        });
    }
    if RESERVED_HOST_IDS
        .iter()
        .any(|reserved| normalized.eq_ignore_ascii_case(reserved))
    {
        return Err(HostIdentityError::Invalid {
            identity_source: source,
            reason: "reserved Core identity placeholder".to_owned(),
        });
    }
    Ok(())
}

fn assert_matches(
    authoritative: &HostId,
    candidate: Option<&HostId>,
    authoritative_source: &'static str,
    conflicting_source: &'static str,
) -> IdentityResult<()> {
    if candidate.is_some_and(|candidate| candidate != authoritative) {
        return Err(HostIdentityError::Conflict {
            authoritative_source,
            conflicting_source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn identity(value: &str, version: u64) -> HostIdentity {
        HostIdentity {
            id: HostId::new(value).unwrap(),
            resource_version: ResourceVersion::new(version).unwrap(),
        }
    }

    fn inputs(
        existing: Option<&str>,
        cache: Option<&str>,
        configured: Option<&str>,
        enrollment: Option<&str>,
    ) -> HostIdentityInputs {
        HostIdentityInputs {
            existing_core: existing.map(|value| identity(value, 7)),
            importable_nodecache: cache.map(|value| identity(value, 1)),
            configured_seed: configured.map(str::to_owned),
            precreation_enrollment: enrollment.map(str::to_owned),
        }
    }

    #[test]
    fn precedence_requires_every_lower_source_to_match() {
        let decision = resolve_host_identity_with(
            inputs(
                Some("host-a"),
                Some("host-a"),
                Some("host-a"),
                Some("host-a"),
            ),
            || panic!("generator must not run"),
        )
        .unwrap();
        assert!(matches!(decision, HostIdentityDecision::UseExistingCore(_)));
        assert_eq!(decision.identity().resource_version.get(), 7);

        let decision = resolve_host_identity_with(
            inputs(None, Some("host-a"), Some("host-a"), Some("host-a")),
            || panic!("generator must not run"),
        )
        .unwrap();
        assert!(matches!(
            decision,
            HostIdentityDecision::UseImportableNodeCache(_)
        ));
    }

    #[test]
    fn every_source_conflict_blocks_without_generation() {
        for case in [
            inputs(Some("host-a"), Some("host-b"), None, None),
            inputs(Some("host-a"), None, Some("host-b"), None),
            inputs(Some("host-a"), None, None, Some("host-b")),
            inputs(None, Some("host-a"), Some("host-b"), None),
            inputs(None, Some("host-a"), None, Some("host-b")),
            inputs(None, None, Some("host-a"), Some("host-b")),
        ] {
            assert!(matches!(
                resolve_host_identity_with(case, || panic!("generator must not run")),
                Err(HostIdentityError::Conflict { .. })
            ));
        }
    }

    #[test]
    fn source_matrix_matches_precedence_model() {
        let values = [None, Some("host-a"), Some("host-b")];
        for existing in values {
            for cache in values {
                for configured in values {
                    for enrollment in values {
                        let ordered = [existing, cache, configured, enrollment];
                        let authoritative = ordered.into_iter().flatten().next();
                        let conflict = authoritative.is_some_and(|expected| {
                            ordered
                                .into_iter()
                                .flatten()
                                .any(|candidate| candidate != expected)
                        });
                        let generated = Cell::new(0);
                        let result = resolve_host_identity_with(
                            inputs(existing, cache, configured, enrollment),
                            || {
                                generated.set(generated.get() + 1);
                                "generated".to_owned()
                            },
                        );
                        if conflict {
                            assert!(matches!(result, Err(HostIdentityError::Conflict { .. })));
                            assert_eq!(generated.get(), 0);
                            continue;
                        }

                        let decision = result.unwrap();
                        assert_eq!(
                            decision.identity().id.as_str(),
                            authoritative.unwrap_or("generated")
                        );
                        assert_eq!(generated.get(), usize::from(authoritative.is_none()));
                        match (existing, cache, configured, enrollment, decision) {
                            (Some(_), _, _, _, HostIdentityDecision::UseExistingCore(_)) => {}
                            (
                                None,
                                Some(_),
                                _,
                                _,
                                HostIdentityDecision::UseImportableNodeCache(_),
                            ) => {}
                            (
                                None,
                                None,
                                Some(_),
                                _,
                                HostIdentityDecision::InitializeFresh(fresh),
                            ) if fresh.source() == FreshIdentitySource::ConfiguredSeed => {}
                            (
                                None,
                                None,
                                None,
                                Some(_),
                                HostIdentityDecision::InitializeFresh(fresh),
                            ) if fresh.source() == FreshIdentitySource::PrecreationEnrollment => {}
                            (
                                None,
                                None,
                                None,
                                None,
                                HostIdentityDecision::InitializeFresh(fresh),
                            ) if fresh.source() == FreshIdentitySource::Generated => {}
                            unexpected => panic!("unexpected model decision: {unexpected:?}"),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fresh_sources_and_generator_are_deterministic() {
        for (case, expected_source) in [
            (
                inputs(None, None, Some("configured"), None),
                FreshIdentitySource::ConfiguredSeed,
            ),
            (
                inputs(None, None, None, Some("enrolled")),
                FreshIdentitySource::PrecreationEnrollment,
            ),
            (
                inputs(None, None, Some("same"), Some("same")),
                FreshIdentitySource::ConfiguredSeed,
            ),
        ] {
            let decision =
                resolve_host_identity_with(case, || panic!("generator must not run")).unwrap();
            assert_eq!(decision.fresh_source(), Some(expected_source));
            assert_eq!(decision.identity().resource_version.get(), 1);
        }

        let calls = Cell::new(0);
        let decision = resolve_host_identity_with(HostIdentityInputs::default(), || {
            calls.set(calls.get() + 1);
            "generated-once".to_owned()
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(decision.identity().id.as_str(), "generated-once");
        assert_eq!(
            decision.fresh_source(),
            Some(FreshIdentitySource::Generated)
        );
    }

    #[test]
    fn reserved_or_empty_values_block_for_every_source() {
        for reserved in ["unknown", " UNKNOWN ", "unset", "none", "null"] {
            for case in [
                inputs(Some(reserved), None, None, None),
                inputs(None, Some(reserved), None, None),
                inputs(None, None, Some(reserved), None),
                inputs(None, None, None, Some(reserved)),
            ] {
                assert!(matches!(
                    resolve_host_identity_with(case, || panic!("generator must not run")),
                    Err(HostIdentityError::Invalid { .. })
                ));
            }
        }
        for empty in ["", " "] {
            for case in [
                inputs(None, None, Some(empty), None),
                inputs(None, None, None, Some(empty)),
            ] {
                assert!(matches!(
                    resolve_host_identity_with(case, || panic!("generator must not run")),
                    Err(HostIdentityError::Invalid { .. })
                ));
            }
        }
        assert!(matches!(
            resolve_host_identity_with(HostIdentityInputs::default(), || "null".to_owned()),
            Err(HostIdentityError::Invalid {
                identity_source: "generated identity",
                ..
            })
        ));
    }

    #[test]
    fn fresh_store_creation_is_explicit_and_restart_stable() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("core.db");
        assert!(!path.exists());
        let decision =
            resolve_host_identity_with(inputs(None, None, Some("fresh-host"), None), || {
                panic!("generator must not run")
            })
            .unwrap();
        assert!(
            !path.exists(),
            "pure resolution must not touch the filesystem"
        );

        let service = create_fresh_authority(&path, &decision).unwrap();
        assert_eq!(service.host().unwrap().identity.id.as_str(), "fresh-host");
        drop(service);
        assert_eq!(
            OperationService::open_existing(&path)
                .unwrap()
                .host()
                .unwrap()
                .identity
                .id
                .as_str(),
            "fresh-host"
        );

        let existing = HostIdentityDecision::UseExistingCore(identity("fresh-host", 1));
        assert!(matches!(
            create_fresh_authority(&directory.path().join("other.db"), &existing),
            Err(HostIdentityError::NotFresh)
        ));
        assert!(!directory.path().join("other.db").exists());
    }

    #[test]
    fn forged_fresh_decisions_cannot_create_an_authority() {
        for decision in [
            HostIdentityDecision::InitializeFresh(FreshHostIdentity {
                identity: identity("forged-version", 2),
                source: FreshIdentitySource::ConfiguredSeed,
            }),
            HostIdentityDecision::InitializeFresh(FreshHostIdentity {
                identity: identity("unknown", 1),
                source: FreshIdentitySource::Generated,
            }),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("core.db");
            assert!(matches!(
                create_fresh_authority(&path, &decision),
                Err(HostIdentityError::Operations(_))
            ));
            assert!(!path.exists());
        }

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("core.db");
        let forged_source = HostIdentityDecision::UseImportableNodeCache(identity("legacy", 1));
        assert!(matches!(
            create_fresh_authority(&path, &forged_source),
            Err(HostIdentityError::NotFresh)
        ));
        assert!(!path.exists());
    }
}
