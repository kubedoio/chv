//! Apply run and drift report domain models.

use crate::architecture::model::{ArchitectureId, ArchitectureVersionId};
use crate::architecture::plan::{ArchitecturePlanId, InventorySnapshotId};
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

arch_id_newtype!(ArchitectureApplyRunId, "architecture_apply_run_id");
arch_id_newtype!(ArchitectureDriftReportId, "architecture_drift_report_id");

/// Apply-run lifecycle status. Mirrors the contract's "Run status" enum.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    PartiallyFailed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Succeeded => "succeeded",
            RunStatus::PartiallyFailed => "partially_failed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

/// Drift status for the most recent drift detection.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    Unknown,
    NoDrift,
    Drifted,
    CheckFailed,
}

impl DriftStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriftStatus::Unknown => "unknown",
            DriftStatus::NoDrift => "no_drift",
            DriftStatus::Drifted => "drifted",
            DriftStatus::CheckFailed => "check_failed",
        }
    }
}

/// Apply run record. `task_id` ties the run to the long-running operations
/// table for streaming logs and progress.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureApplyRun {
    pub id: ArchitectureApplyRunId,
    pub architecture_id: ArchitectureId,
    pub architecture_version_id: ArchitectureVersionId,
    pub plan_id: Option<ArchitecturePlanId>,
    pub task_id: Option<String>,
    pub status: RunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub requested_by: Option<String>,
    pub result_json: Option<String>,
    pub logs_ref: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Drift report record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureDriftReport {
    pub id: ArchitectureDriftReportId,
    pub architecture_id: ArchitectureId,
    pub baseline_version_id: ArchitectureVersionId,
    pub inventory_snapshot_id: Option<InventorySnapshotId>,
    pub status: DriftStatus,
    pub summary_json: Option<String>,
    pub findings_json: Option<String>,
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
    fn run_status_serializes_snake_case() {
        let json = serde_json::to_string(&RunStatus::PartiallyFailed).unwrap();
        assert_eq!(json, "\"partially_failed\"");
    }

    #[test]
    fn run_status_round_trip_all() {
        for s in [
            RunStatus::Queued,
            RunStatus::Running,
            RunStatus::Succeeded,
            RunStatus::PartiallyFailed,
            RunStatus::Failed,
            RunStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: RunStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn drift_status_serializes_snake_case() {
        let json = serde_json::to_string(&DriftStatus::NoDrift).unwrap();
        assert_eq!(json, "\"no_drift\"");
    }

    #[test]
    fn drift_status_round_trip_all() {
        for s in [
            DriftStatus::Unknown,
            DriftStatus::NoDrift,
            DriftStatus::Drifted,
            DriftStatus::CheckFailed,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: DriftStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn apply_run_round_trip() {
        let run = ArchitectureApplyRun {
            id: ArchitectureApplyRunId::new("run-1").unwrap(),
            architecture_id: ArchitectureId::new("topo-1").unwrap(),
            architecture_version_id: ArchitectureVersionId::new("v-1").unwrap(),
            plan_id: Some(ArchitecturePlanId::new("plan-1").unwrap()),
            task_id: Some("task-abc".to_string()),
            status: RunStatus::Running,
            started_at: Some(sample_ts()),
            finished_at: None,
            requested_by: Some("senol".to_string()),
            result_json: None,
            logs_ref: Some("operations/task-abc/logs".to_string()),
            error_message: None,
            created_at: sample_ts(),
            updated_at: sample_ts(),
        };

        let json = serde_json::to_string(&run).unwrap();
        let back: ArchitectureApplyRun = serde_json::from_str(&json).unwrap();
        assert_eq!(run, back);
    }

    #[test]
    fn drift_report_round_trip() {
        let drift = ArchitectureDriftReport {
            id: ArchitectureDriftReportId::new("drift-1").unwrap(),
            architecture_id: ArchitectureId::new("topo-1").unwrap(),
            baseline_version_id: ArchitectureVersionId::new("v-1").unwrap(),
            inventory_snapshot_id: Some(InventorySnapshotId::new("snap-1").unwrap()),
            status: DriftStatus::Drifted,
            summary_json: Some("{\"diff_count\":3}".to_string()),
            findings_json: Some("[]".to_string()),
            created_at: sample_ts(),
        };

        let json = serde_json::to_string(&drift).unwrap();
        let back: ArchitectureDriftReport = serde_json::from_str(&json).unwrap();
        assert_eq!(drift, back);
    }
}
