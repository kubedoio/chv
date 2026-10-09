//! # chv-monitoring-core
//!
//! Typed, bounded monitoring sampling primitives for the native monitoring
//! campaign (ADR-025, gate G1). This crate is the shared foundation used by
//! `chv-agent` (and, in later gates, the ingest and query paths):
//!
//! - [`model`] — the v1 sample model: [`model::Sample`], quality markers,
//!   sources, kinds, units and bounded dimensions, as defined by
//!   `docs/specs/contracts/chv-monitoring-metrics-v1.md`.
//! - [`registry`] — the explicit metric allowlist. Every sample's
//!   `metric_id` must exist here; unknown metrics are never stored.
//! - [`delta`] — boot-epoch-scoped, reset-safe counter deltas. A
//!   non-monotonic reading within an epoch, or an epoch change, is a reset —
//!   never a fabricated spike or a negative rate.
//! - [`vmm_counters`] — the pinned Cloud Hypervisor v53.0 `vm.counters`
//!   parser. v53.0 (like v43.0 before it) returns a flat device-keyed map
//!   (`_disk0`, `_net1` → int64 counters) with u64::MAX-shaped no-data
//!   sentinels on latency fields — verified against real-host fixtures
//!   (`docs/evidence/native-monitoring/g0b/`). Missing fields are
//!   unavailable, never zero.
//! - [`node_os`] — the retained-snapshot Linux node collector. CPU usage
//!   requires two refreshes separated by sysinfo's minimum interval; a
//!   fresh `System` per scrape yields the since-boot average, which is
//!   wrong, so the collector is long-lived and reports
//!   `insufficient_samples` until a valid interval exists.
//! - [`process_probe`] — read-only `/proc/<pid>/stat` and `statm` probes
//!   with a configurable proc root (test isolation) and start-ticks fencing
//!   helpers.
//! - [`cgroup`] — read-only cgroup v2 probes (`cpu.stat`, `memory.current`)
//!   plus an explicit [`cgroup::ProcessFence`] (pid + start-ticks +
//!   boot-id). If ownership cannot be established the caller must report
//!   `unsupported`, never guess a process.
//! - [`sampler`] — the bounded periodic sampler: source traits for node
//!   OS, VM runtime, storage and network providers, per-source timeouts,
//!   bounded concurrency, a bounded snapshot sink and health counters.
//!
//! ## Invariants (from ADR-025 and the v1 contracts)
//!
//! - Missing data is never a zero. `quality != valid` samples carry no
//!   value.
//! - Samples never mutate VM state, block lifecycle, or write journals.
//! - Sources are truthful: a value is attributed only to the layer that
//!   actually observed it.
//! - All collections are bounded in time, concurrency and memory.
//!
//! This crate contains no daemon, no history storage and no guest agent;
//! those belong to later gates.

pub mod cgroup;
pub mod delta;
pub mod model;
pub mod node_os;
pub mod process_probe;
pub mod registry;
pub mod sampler;
pub mod vmm_counters;

pub use delta::{CounterState, DeltaOutcome, Epoch};
pub use model::{
    DimensionError, MetricKind, Sample, SampleBuilder, SampleQuality, SampleValue, Source,
    TargetKind, Unit,
};
pub use registry::{MetricDef, REGISTRY};
pub use sampler::{
    run_sampler, NetworkProviderSource, NodeOsSource, SamplerConfig, SamplerError, SamplerHealth,
    StorageProviderSource, VmRuntimeSource,
};
