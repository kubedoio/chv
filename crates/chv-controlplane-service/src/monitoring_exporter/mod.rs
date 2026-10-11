//! Optional external monitoring export (ADR-027 decision 9, campaign
//! #602 PR-8, prompt-05 task 7).
//!
//! An opt-in, allowlist-filtered push exporter over Prometheus
//! remote-write v1 (VictoriaMetrics/Prometheus/Mimir-compatible),
//! reading the bounded native monitoring store on a cadence. The
//! worker degrades but never blocks: destination outages back up to a
//! bounded catch-up window, then samples are skipped with a
//! time-range warning (the exact skipped count is deliberately not
//! reported — counting would mean reading the data being discarded) —
//! ingest, queries, alerting and VM lifecycle are unaffected
//! (ADR-027 decision 11: export must never become load-bearing for
//! the native UI).
//!
//! - [`remote_write`]: the wire format (hand-defined prost
//!   `WriteRequest`, snappy block compression, the maintained
//!   `chv_*` name mapping, golden-bytes tests).
//! - [`worker`]: the cadence-pass worker (lag cursor, bounded
//!   backfill, pass-level backoff, degraded signal, transport seam).

pub mod remote_write;
pub mod worker;

#[cfg(test)]
mod worker_tests;

pub use remote_write::{render_push, ExportPoint, ExportPushError, ExportSeries, RemoteWriteBody};
pub use worker::{
    ExportTransport, ExporterSettings, MonitoringExporter, PassOutcome, ReqwestExportTransport,
};
