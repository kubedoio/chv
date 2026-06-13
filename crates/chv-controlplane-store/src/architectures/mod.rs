//! Architecture Designer repositories.
//!
//! One repository struct per table, mirroring the layout in
//! `cmd/chv-controlplane/migrations/0046..0050_*.sql`. Repos that back
//! validate/plan/apply (the read-only-from-Phase-0 surfaces) are stubs
//! returning [`StoreError::NotImplemented`] until later phases land.

mod apply_run;
mod drift;
mod plan;
mod snapshot;
mod topology;
mod version;

pub use apply_run::{ApplyRunCreateInput, ApplyRunRepository, ApplyRunUpdateInput};
pub use drift::{DriftReportCreateInput, DriftReportRepository};
pub use plan::{PlanCreateInput, PlanRepository, PlanStatusUpdateInput};
pub use snapshot::{InventorySnapshotCreateInput, InventorySnapshotRepository};
pub use topology::{
    TopologyCreateInput, TopologyListFilter, TopologyRepository, TopologyUpdateInput,
};
pub use version::{VersionCreateInput, VersionRepository};

use crate::StoreError;
use chrono::{DateTime, TimeZone, Utc};

/// Parses the `text` timestamp columns produced by
/// `strftime('%Y-%m-%dT%H:%M:%SZ', ...)` back into chrono.
fn parse_ts(value: &str, column: &'static str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid timestamp in column {column}: {err} (value={value:?})"),
        })
}

fn parse_ts_opt(
    value: Option<&str>,
    column: &'static str,
) -> Result<Option<DateTime<Utc>>, StoreError> {
    match value {
        None => Ok(None),
        Some(v) => parse_ts(v, column).map(Some),
    }
}

/// Formats a chrono timestamp into the SQLite RFC3339-Z form used
/// throughout the schema. The companion of [`parse_ts`].
fn format_ts(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Convenience for constructing a literal Utc timestamp without dragging
/// chrono::Utc into every callsite (used in tests and a few places that
/// need a stable "now").
#[allow(dead_code)]
fn from_unix_seconds(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("epoch is valid"))
}
