//! NetBox projection domain models (issue #239).
//!
//! Per-architecture NetBox integration configuration and the projection
//! run queue. See `docs/specs/component/architecture-designer-netbox-projection.md`
//! and `docs/specs/architecture-designer/contracts/netbox-api-contract.md`.
//!
//! Security note: [`NetboxProjectionConfig`] deliberately has **no** token
//! field. The encrypted token (`token_ciphertext`) never leaves the store
//! layer's write path; only the dedicated `read_token` repository method
//! decrypts it, for the projection worker.

use crate::architecture::model::{ArchitectureId, ArchitectureVersionId};
use crate::domain::IdentifierError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::architecture::drift::arch_id_newtype;

arch_id_newtype!(NetboxProjectionRunId, "netbox_projection_run_id");

/// What enqueued a projection run. Wire strings: `manual` | `post_apply`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetboxProjectionTrigger {
    Manual,
    PostApply,
}

impl NetboxProjectionTrigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetboxProjectionTrigger::Manual => "manual",
            NetboxProjectionTrigger::PostApply => "post_apply",
        }
    }
}

/// Whether a run only computes the plan (`dry_run`) or executes it
/// (`export`). Wire strings: `dry_run` | `export`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetboxProjectionMode {
    DryRun,
    Export,
}

impl NetboxProjectionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetboxProjectionMode::DryRun => "dry_run",
            NetboxProjectionMode::Export => "export",
        }
    }
}

/// Projection run lifecycle status.
///
/// Status machine (one active — queued or running — run per architecture):
/// `queued → running → succeeded | failed`, with `failed → queued` on retry
/// up to a bounded attempt count.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetboxProjectionRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl NetboxProjectionRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetboxProjectionRunStatus::Queued => "queued",
            NetboxProjectionRunStatus::Running => "running",
            NetboxProjectionRunStatus::Succeeded => "succeeded",
            NetboxProjectionRunStatus::Failed => "failed",
        }
    }
}

/// What happens to NetBox objects whose CHV resource was removed.
///
/// Wire contract: `as_str()` must stay in sync with
/// `chv_netbox_adapter::plan::RetentionPolicy::as_str` (`mark_stale` /
/// `delete`) and the `retention_policy` values in
/// `netbox_projection_config` — the column is a shared wire surface across
/// the two crates, not an internal string.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetboxRetentionPolicy {
    MarkStale,
    Delete,
}

impl NetboxRetentionPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetboxRetentionPolicy::MarkStale => "mark_stale",
            NetboxRetentionPolicy::Delete => "delete",
        }
    }
}

/// Per-architecture NetBox integration configuration.
///
/// One row per architecture (upsert semantics). Contains **no** token
/// material: the encrypted token never leaves the store layer, and the
/// API surface only ever reports `token_secret_ref` plus a `token_set`
/// boolean.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NetboxProjectionConfig {
    pub architecture_id: ArchitectureId,
    /// HTTPS base URL of the NetBox instance. Validated at the BFF's
    /// accept time (`NETBOX_HTTPS_REQUIRED`); the store persists it
    /// verbatim and does not validate the scheme.
    pub endpoint: String,
    /// Reference to the encrypted secret holding the API token.
    pub token_secret_ref: String,
    pub retention_policy: NetboxRetentionPolicy,
    /// Enqueue a projection run when an apply run for this architecture
    /// succeeds.
    pub enable_post_apply: bool,
    /// Namespace prefix for the NetBox ownership custom fields
    /// (default `chv_`).
    pub custom_field_prefix: String,
    /// Optional NetBox site label for projected devices.
    pub site_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One projection run record. `plan_json`/`result_json`/`summary_json`
/// carry the deterministic plan, per-entry outcomes, and summary counts
/// produced by `chv-netbox-adapter`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NetboxProjectionRun {
    pub id: NetboxProjectionRunId,
    pub architecture_id: ArchitectureId,
    pub architecture_version_id: ArchitectureVersionId,
    pub trigger_kind: NetboxProjectionTrigger,
    pub mode: NetboxProjectionMode,
    pub status: NetboxProjectionRunStatus,
    pub plan_json: Option<String>,
    pub result_json: Option<String>,
    pub summary_json: Option<String>,
    /// Redacted by callers (the worker); the store does not scrub.
    pub error_message: Option<String>,
    /// Retries so far; incremented on each `mark_failed`.
    pub attempt_count: i64,
    pub requested_by: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-08T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn trigger_serializes_snake_case() {
        let json = serde_json::to_string(&NetboxProjectionTrigger::PostApply).unwrap();
        assert_eq!(json, "\"post_apply\"");
    }

    #[test]
    fn trigger_round_trip_all() {
        for t in [
            NetboxProjectionTrigger::Manual,
            NetboxProjectionTrigger::PostApply,
        ] {
            let json = serde_json::to_string(&t).unwrap();
            let back: NetboxProjectionTrigger = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn mode_serializes_snake_case() {
        let json = serde_json::to_string(&NetboxProjectionMode::DryRun).unwrap();
        assert_eq!(json, "\"dry_run\"");
    }

    #[test]
    fn mode_round_trip_all() {
        for m in [NetboxProjectionMode::DryRun, NetboxProjectionMode::Export] {
            let json = serde_json::to_string(&m).unwrap();
            let back: NetboxProjectionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(m, back);
        }
    }

    #[test]
    fn run_status_serializes_snake_case() {
        let json = serde_json::to_string(&NetboxProjectionRunStatus::Succeeded).unwrap();
        assert_eq!(json, "\"succeeded\"");
    }

    #[test]
    fn run_status_round_trip_all() {
        for s in [
            NetboxProjectionRunStatus::Queued,
            NetboxProjectionRunStatus::Running,
            NetboxProjectionRunStatus::Succeeded,
            NetboxProjectionRunStatus::Failed,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: NetboxProjectionRunStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn retention_policy_serializes_snake_case() {
        let json = serde_json::to_string(&NetboxRetentionPolicy::MarkStale).unwrap();
        assert_eq!(json, "\"mark_stale\"");
    }

    #[test]
    fn retention_policy_round_trip_all() {
        for r in [
            NetboxRetentionPolicy::MarkStale,
            NetboxRetentionPolicy::Delete,
        ] {
            let json = serde_json::to_string(&r).unwrap();
            let back: NetboxRetentionPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(r, back);
        }
    }

    #[test]
    fn config_round_trip() {
        let config = NetboxProjectionConfig {
            architecture_id: ArchitectureId::new("topo-1").unwrap(),
            endpoint: "https://netbox.example.internal".to_string(),
            token_secret_ref: "netbox-topo-1".to_string(),
            retention_policy: NetboxRetentionPolicy::MarkStale,
            enable_post_apply: true,
            custom_field_prefix: "chv_".to_string(),
            site_name: Some("dc1".to_string()),
            created_at: sample_ts(),
            updated_at: sample_ts(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let back: NetboxProjectionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn run_round_trip() {
        let run = NetboxProjectionRun {
            id: NetboxProjectionRunId::new("netrun-1").unwrap(),
            architecture_id: ArchitectureId::new("topo-1").unwrap(),
            architecture_version_id: ArchitectureVersionId::new("v-1").unwrap(),
            trigger_kind: NetboxProjectionTrigger::PostApply,
            mode: NetboxProjectionMode::Export,
            status: NetboxProjectionRunStatus::Failed,
            plan_json: Some("{\"entries\":[]}".to_string()),
            result_json: None,
            summary_json: Some("{\"create\":0}".to_string()),
            error_message: Some("netbox unreachable".to_string()),
            attempt_count: 2,
            requested_by: Some("senol".to_string()),
            started_at: Some(sample_ts()),
            finished_at: Some(sample_ts()),
            created_at: sample_ts(),
        };

        let json = serde_json::to_string(&run).unwrap();
        let back: NetboxProjectionRun = serde_json::from_str(&json).unwrap();
        assert_eq!(run, back);
    }

    #[test]
    fn run_id_rejects_empty() {
        assert!(NetboxProjectionRunId::new("  ").is_err());
    }

    #[test]
    fn run_id_try_from_string_and_str() {
        // The drift.rs macro provides the same TryFrom impls as
        // model.rs's ArchitectureId.
        let from_string: NetboxProjectionRunId = "netrun-9".to_string().try_into().unwrap();
        assert_eq!(from_string.as_str(), "netrun-9");
        let from_str: NetboxProjectionRunId = "netrun-9".try_into().unwrap();
        assert_eq!(from_str, from_string);
        assert!(NetboxProjectionRunId::try_from("").is_err());
    }
}
