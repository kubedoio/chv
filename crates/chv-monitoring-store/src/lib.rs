//! The isolated monitoring store (ADR-027): a bounded SQLite database
//! for disposable telemetry — samples, rollups, dedup proofs — with its
//! own connection pool, migrations, and maintenance, so monitoring can
//! never hold operational-database locks and the control plane stays
//! alive (monitoring `degraded`) when this file is missing, corrupt, or
//! the filesystem is full.
//!
//! Layers:
//! - [`db`] — pool, migrations, integrity probe, size accounting
//! - [`ingest`] — durable dedup + high-water marks + series caps
//! - [`query`] — bounded history/current reads with honest absence
//! - [`maintenance`] — idempotent rollups, retention, eviction, WAL
//!   checkpointing
//! - [`headroom`] — filesystem headroom probing (disk-full is a tested
//!   failure mode, not an assumption)
//!
//! Nothing in this crate may be used from VM lifecycle paths: a failure
//! here is a degraded monitoring signal, never a lifecycle error.

pub mod config;
pub mod db;
pub mod error;
pub mod headroom;
pub mod ingest;
pub mod maintenance;
pub mod query;

pub use config::MonitoringStoreConfig;
pub use db::MonitoringStore;
pub use error::{IngestOutcome, MonitoringStoreError};
pub use ingest::NodeBatch;
pub use maintenance::MaintenanceReport;
pub use query::{
    CurrentSample, HistoryPoint, HistorySeries, Resolution, SeriesReason,
    DEFAULT_MAX_POINTS_PER_SERIES, MAX_AGGREGATED_RANGE_MS, MAX_DETAILED_RANGE_MS,
    MAX_METRIC_IDS_PER_QUERY, MAX_POINTS_PER_SERIES,
};

#[cfg(test)]
mod tests;
