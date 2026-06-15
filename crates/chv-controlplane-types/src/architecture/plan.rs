//! Plan and inventory-snapshot domain models.
//!
//! Matches the validation/plan contract in
//! `docs/specs/architecture-designer/contracts/validation-plan-contract.md`
//! (loose; we keep the types open to additional fields rather than enforce
//! a closed enum for `resource_type`, since the YAML kinds are still
//! evolving in v1alpha1).

use crate::architecture::model::{ArchitectureId, ArchitectureVersionId};
use crate::domain::IdentifierError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! arch_id_newtype {
    ($name:ident, $field_name:literal) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(IdentifierError::empty($field_name));
                }
                Ok(Self(trimmed.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

arch_id_newtype!(ArchitecturePlanId, "architecture_plan_id");
arch_id_newtype!(InventorySnapshotId, "inventory_snapshot_id");

/// Plan execution mode.
///
/// * `dry_run` produces a plan without recording an apply intent.
/// * `confirm` produces a plan that may be confirmed-and-applied (legacy
///   alias retained for Phase-0 store callers; Phase 4 prefers `apply`).
/// * `apply` is the standard mode for [`POST /v1/architectures/plan`].
///   It produces a plan that, after explicit confirmation, will reconcile
///   the desired model into the fleet.
/// * `destroy` is produced by `POST /v1/architectures/destroy-plan`. It
///   inverts the diff so every desired resource becomes a `Delete` change,
///   tearing down the architecture as a unit.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanMode {
    DryRun,
    Confirm,
    Apply,
    Destroy,
}

impl PlanMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanMode::DryRun => "dry_run",
            PlanMode::Confirm => "confirm",
            PlanMode::Apply => "apply",
            PlanMode::Destroy => "destroy",
        }
    }
}

/// Plan lifecycle status. Mirrors the contract's "Plan status" enum.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    FailedValidation,
    RequiresConfirmation,
    ReadyToApply,
    Applying,
    Applied,
    Failed,
    Expired,
    Discarded,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanStatus::Draft => "draft",
            PlanStatus::FailedValidation => "failed_validation",
            PlanStatus::RequiresConfirmation => "requires_confirmation",
            PlanStatus::ReadyToApply => "ready_to_apply",
            PlanStatus::Applying => "applying",
            PlanStatus::Applied => "applied",
            PlanStatus::Failed => "failed",
            PlanStatus::Expired => "expired",
            PlanStatus::Discarded => "discarded",
        }
    }
}

/// Operation that a plan change describes.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanAction {
    Create,
    Update,
    Delete,
    Replace,
    NoOp,
}

/// Coarse type of resource a plan change targets. Not exhaustive — matches
/// the v1alpha1 YAML top-level sections.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Server,
    Network,
    Datastore,
    BackupTarget,
    BackupPolicy,
    Image,
    Template,
    Instance,
    SshKey,
    InstanceUser,
    Role,
    User,
    Project,
}

/// Risk classification for a plan change. Used to drive whether the UI
/// should require an explicit confirmation step.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    High,
    Destructive,
}

/// Single change line in a plan. Matches the plan contract exactly.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanChange {
    pub action: PlanAction,
    pub resource_type: ResourceType,
    pub resource_name: String,
    pub resource_ref: String,
    pub description: String,
    pub risk: Risk,
    pub requires_confirmation: bool,
}

/// Top-level plan record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitecturePlan {
    pub id: ArchitecturePlanId,
    pub architecture_id: ArchitectureId,
    pub architecture_version_id: ArchitectureVersionId,
    pub inventory_snapshot_id: Option<InventorySnapshotId>,
    pub mode: PlanMode,
    pub status: PlanStatus,
    /// Serialized JSON of the full plan (changes + metadata). Kept opaque
    /// at this layer so the contract can evolve without breaking persistence.
    pub plan_json: Option<String>,
    pub summary_json: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Hard expiry; computed as `created_at + 15min` per ADR-004-Designer.
    pub expires_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub confirmed_by: Option<String>,
    pub discarded_at: Option<DateTime<Utc>>,
    /// Subject (user id) that discarded the plan. Recorded by the
    /// discard-plan handler in Phase 4 so audit logs and incident review
    /// can identify the actor without joining against an external table.
    pub discarded_by: Option<String>,
}

/// Captured fleet inventory used as the basis for a plan or drift report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub id: InventorySnapshotId,
    pub source: String,
    pub snapshot_json: String,
    pub summary_json: Option<String>,
    pub captured_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-13T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn plan_status_serializes_snake_case() {
        let json = serde_json::to_string(&PlanStatus::FailedValidation).unwrap();
        assert_eq!(json, "\"failed_validation\"");

        let json = serde_json::to_string(&PlanStatus::ReadyToApply).unwrap();
        assert_eq!(json, "\"ready_to_apply\"");
    }

    #[test]
    fn plan_status_round_trip_all() {
        for s in [
            PlanStatus::Draft,
            PlanStatus::FailedValidation,
            PlanStatus::RequiresConfirmation,
            PlanStatus::ReadyToApply,
            PlanStatus::Applying,
            PlanStatus::Applied,
            PlanStatus::Failed,
            PlanStatus::Expired,
            PlanStatus::Discarded,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: PlanStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn plan_mode_round_trip_all() {
        for (mode, expected) in [
            (PlanMode::DryRun, "\"dry_run\""),
            (PlanMode::Confirm, "\"confirm\""),
            (PlanMode::Apply, "\"apply\""),
            (PlanMode::Destroy, "\"destroy\""),
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, expected, "unexpected wire form for {mode:?}");
            let back: PlanMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
            assert_eq!(mode.as_str(), expected.trim_matches('"'));
        }
    }

    #[test]
    fn plan_change_round_trip() {
        let change = PlanChange {
            action: PlanAction::Create,
            resource_type: ResourceType::Instance,
            resource_name: "app-01".to_string(),
            resource_ref: "instance/app-01".to_string(),
            description: "create new instance app-01 on chv-node-01".to_string(),
            risk: Risk::Medium,
            requires_confirmation: true,
        };

        let json = serde_json::to_string(&change).unwrap();
        let back: PlanChange = serde_json::from_str(&json).unwrap();
        assert_eq!(change, back);
    }

    #[test]
    fn plan_round_trip() {
        let plan = ArchitecturePlan {
            id: ArchitecturePlanId::new("plan-1").unwrap(),
            architecture_id: ArchitectureId::new("topo-1").unwrap(),
            architecture_version_id: ArchitectureVersionId::new("v-1").unwrap(),
            inventory_snapshot_id: Some(InventorySnapshotId::new("snap-1").unwrap()),
            mode: PlanMode::Confirm,
            status: PlanStatus::RequiresConfirmation,
            plan_json: Some("{\"changes\":[]}".to_string()),
            summary_json: None,
            created_by: Some("senol".to_string()),
            created_at: sample_ts(),
            expires_at: sample_ts(),
            confirmed_at: None,
            confirmed_by: None,
            discarded_at: None,
            discarded_by: None,
        };

        let json = serde_json::to_string(&plan).unwrap();
        let back: ArchitecturePlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, back);
    }

    #[test]
    fn inventory_snapshot_round_trip() {
        let snap = InventorySnapshot {
            id: InventorySnapshotId::new("snap-1").unwrap(),
            source: "node-agent".to_string(),
            snapshot_json: "{}".to_string(),
            summary_json: None,
            captured_by: None,
            created_at: sample_ts(),
        };

        let json = serde_json::to_string(&snap).unwrap();
        let back: InventorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }
}
