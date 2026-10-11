//! Optional external monitoring export (ADR-027 decision 9, campaign
//! #602 PR-8, prompt-05 task 7).
//!
//! An opt-in, allowlist-filtered push exporter over Prometheus
//! remote-write v1 (VictoriaMetrics/Prometheus/Mimir-compatible),
//! reading the bounded native monitoring store on a cadence. The
//! worker degrades but never blocks: destination outages back up to a
//! bounded catch-up window, then samples are skipped and counted —
//! ingest, queries, alerting and VM lifecycle are unaffected
//! (ADR-027 decision 11: export must never become load-bearing for
//! the native UI).
//!
//! - [`remote_write`]: the wire format (hand-defined prost
//!   `WriteRequest`, snappy block compression, the maintained
//!   `chv_*` name mapping, golden-bytes tests).
//! - The worker itself lands in the next commit of this series.

pub mod remote_write;

pub use remote_write::{render_push, ExportPoint, ExportSeries, RemoteWriteBody};
