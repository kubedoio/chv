//! Pure, production-unwired mapping from ownership evidence to a durable
//! recovery assessment. This crate performs no process or VM side effects.

use cellhv_core_operations::{RecoveryAssessment, RecoveryClassification, RecoveryDisposition};
use cellhv_core_runtime_ownership::Classification;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RecoveryMappingError {
    #[error("recovery evidence must be a JSON object")]
    EvidenceMustBeObject,
}

pub fn assessment_for(
    classification: Classification,
    expected_assessment_revision: u64,
    evidence: serde_json::Value,
) -> Result<RecoveryAssessment, RecoveryMappingError> {
    if !evidence.is_object() {
        return Err(RecoveryMappingError::EvidenceMustBeObject);
    }
    let (classification, disposition) = match classification {
        Classification::OwnershipMatched => (
            RecoveryClassification::OwnershipMatched,
            RecoveryDisposition::OwnershipMatchedPendingControl,
        ),
        Classification::ExitedOwned => (
            RecoveryClassification::ExitedOwned,
            RecoveryDisposition::ExitedPendingPolicy,
        ),
        Classification::OwnedAliveSocketUnavailable => (
            RecoveryClassification::OwnedAliveSocketUnavailable,
            RecoveryDisposition::Quarantined,
        ),
        Classification::ForeignConflict => (
            RecoveryClassification::ForeignConflict,
            RecoveryDisposition::Quarantined,
        ),
        Classification::AmbiguousPreserve => (
            RecoveryClassification::AmbiguousPreserve,
            RecoveryDisposition::Quarantined,
        ),
        Classification::DuplicateConflict => (
            RecoveryClassification::DuplicateConflict,
            RecoveryDisposition::Quarantined,
        ),
        Classification::CorruptOwnership => (
            RecoveryClassification::CorruptOwnership,
            RecoveryDisposition::Quarantined,
        ),
    };
    Ok(RecoveryAssessment {
        expected_assessment_revision,
        classification,
        disposition,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_classification_maps_fail_closed() {
        let cases = [
            (
                Classification::OwnershipMatched,
                RecoveryDisposition::OwnershipMatchedPendingControl,
            ),
            (
                Classification::OwnedAliveSocketUnavailable,
                RecoveryDisposition::Quarantined,
            ),
            (
                Classification::ExitedOwned,
                RecoveryDisposition::ExitedPendingPolicy,
            ),
            (
                Classification::ForeignConflict,
                RecoveryDisposition::Quarantined,
            ),
            (
                Classification::AmbiguousPreserve,
                RecoveryDisposition::Quarantined,
            ),
            (
                Classification::DuplicateConflict,
                RecoveryDisposition::Quarantined,
            ),
            (
                Classification::CorruptOwnership,
                RecoveryDisposition::Quarantined,
            ),
        ];
        for (classification, disposition) in cases {
            let assessment =
                assessment_for(classification, 4, serde_json::json!({"proof":"bounded"})).unwrap();
            assert_eq!(assessment.expected_assessment_revision, 4);
            assert_eq!(assessment.disposition, disposition);
        }
    }

    #[test]
    fn evidence_requires_a_structured_summary() {
        assert_eq!(
            assessment_for(
                Classification::AmbiguousPreserve,
                0,
                serde_json::json!("raw")
            ),
            Err(RecoveryMappingError::EvidenceMustBeObject)
        );
    }
}
