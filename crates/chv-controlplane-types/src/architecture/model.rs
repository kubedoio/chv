//! Topology and version domain models.

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

        impl TryFrom<String> for $name {
            type Error = IdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdentifierError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

arch_id_newtype!(ArchitectureId, "architecture_id");
arch_id_newtype!(ArchitectureVersionId, "architecture_version_id");

/// High-level topology status. Mirrors the enum strings in
/// `architecture-designer-data-model.md` ("Architecture status").
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureStatus {
    Draft,
    Valid,
    Invalid,
    Planned,
    Applying,
    Applied,
    Drifted,
    Failed,
    Archived,
}

impl ArchitectureStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArchitectureStatus::Draft => "draft",
            ArchitectureStatus::Valid => "valid",
            ArchitectureStatus::Invalid => "invalid",
            ArchitectureStatus::Planned => "planned",
            ArchitectureStatus::Applying => "applying",
            ArchitectureStatus::Applied => "applied",
            ArchitectureStatus::Drifted => "drifted",
            ArchitectureStatus::Failed => "failed",
            ArchitectureStatus::Archived => "archived",
        }
    }
}

/// Last validation result summary for the topology.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Unknown,
    Passed,
    Failed,
}

impl ValidationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationStatus::Unknown => "unknown",
            ValidationStatus::Passed => "passed",
            ValidationStatus::Failed => "failed",
        }
    }
}

/// Last fleet-consistency check result summary.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetCheckStatus {
    Unknown,
    Passed,
    Failed,
}

impl FleetCheckStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FleetCheckStatus::Unknown => "unknown",
            FleetCheckStatus::Passed => "passed",
            FleetCheckStatus::Failed => "failed",
        }
    }
}

/// Top-level architecture topology row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureTopology {
    pub id: ArchitectureId,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub environment: Option<String>,
    pub status: ArchitectureStatus,
    pub owner_user_id: Option<String>,
    pub design_graph_json: Option<String>,
    pub latest_yaml: Option<String>,
    pub latest_version_id: Option<ArchitectureVersionId>,
    pub last_validation_status: Option<ValidationStatus>,
    pub last_fleet_check_status: Option<FleetCheckStatus>,
    pub last_plan_id: Option<String>,
    pub last_apply_run_id: Option<String>,
    pub last_apply_task_id: Option<String>,
    pub last_drift_status: Option<String>,
    /// Optimistic-concurrency token. PUTs must echo the value they read; the
    /// store enforces `WHERE version_number = ?` and bumps it on success.
    pub version_number: i64,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Immutable per-save version record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureVersion {
    pub id: ArchitectureVersionId,
    pub architecture_id: ArchitectureId,
    pub version_number: i64,
    pub yaml_content: String,
    pub design_graph_json: Option<String>,
    pub normalized_model_json: Option<String>,
    pub change_summary: Option<String>,
    pub created_by: Option<String>,
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
    fn architecture_id_rejects_blank() {
        assert!(ArchitectureId::new("").is_err());
        assert!(ArchitectureId::new("   ").is_err());
        assert_eq!(ArchitectureId::new(" abc ").unwrap().as_str(), "abc");
    }

    #[test]
    fn architecture_status_round_trip() {
        for s in [
            ArchitectureStatus::Draft,
            ArchitectureStatus::Valid,
            ArchitectureStatus::Invalid,
            ArchitectureStatus::Planned,
            ArchitectureStatus::Applying,
            ArchitectureStatus::Applied,
            ArchitectureStatus::Drifted,
            ArchitectureStatus::Failed,
            ArchitectureStatus::Archived,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: ArchitectureStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back, "round-trip failed for {s:?}");
        }
    }

    #[test]
    fn architecture_status_serializes_snake_case() {
        let json = serde_json::to_string(&ArchitectureStatus::Applying).unwrap();
        assert_eq!(json, "\"applying\"");
    }

    #[test]
    fn topology_round_trip() {
        let topo = ArchitectureTopology {
            id: ArchitectureId::new("topo-1").unwrap(),
            name: "customer-a-prod".to_string(),
            display_name: Some("Customer A Production".to_string()),
            description: None,
            environment: Some("production".to_string()),
            status: ArchitectureStatus::Draft,
            owner_user_id: Some("u-1".to_string()),
            design_graph_json: Some("{}".to_string()),
            latest_yaml: None,
            latest_version_id: Some(ArchitectureVersionId::new("v-1").unwrap()),
            last_validation_status: Some(ValidationStatus::Passed),
            last_fleet_check_status: Some(FleetCheckStatus::Unknown),
            last_plan_id: None,
            last_apply_run_id: None,
            last_apply_task_id: None,
            last_drift_status: None,
            version_number: 3,
            archived_at: None,
            created_at: sample_ts(),
            updated_at: sample_ts(),
        };

        let json = serde_json::to_string(&topo).unwrap();
        let back: ArchitectureTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(topo, back);
    }

    #[test]
    fn version_round_trip() {
        let v = ArchitectureVersion {
            id: ArchitectureVersionId::new("v-1").unwrap(),
            architecture_id: ArchitectureId::new("topo-1").unwrap(),
            version_number: 1,
            yaml_content: "apiVersion: chv.kubedo.io/v1alpha1\n".to_string(),
            design_graph_json: None,
            normalized_model_json: None,
            change_summary: Some("initial".to_string()),
            created_by: Some("senol".to_string()),
            created_at: sample_ts(),
        };

        let json = serde_json::to_string(&v).unwrap();
        let back: ArchitectureVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }
}
