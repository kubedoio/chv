//! The bounded periodic sampler (prompt 01 tasks 1 and 7).
//!
//! One task drives all collection, **independent of reconciliation and
//! state reports**: a slow, hung or failing source can never block VM
//! lifecycle, and lifecycle can never silently stop collection. Bounds:
//!
//! - **time** — every source call runs under a timeout;
//! - **concurrency** — at most `max_concurrent_vm_collections` VM
//!   collections in flight (VMM API calls are serialized per VM by the
//!   runtime, but the sampler additionally bounds cross-VM fan-out);
//! - **memory** — samples leave through a **bounded** sink; when the sink
//!   is full the batch is dropped and counted, never queued unboundedly
//!   and never allowed to back-pressure the loop into missing its
//!   cadence.
//!
//! Health (cycles, failures, timeouts, drops, last-success age) is
//! exported as counters/gauges **without VM identifiers** — the
//! Prometheus surface carries only global labels (prompt 01 task 7).
//!
//! The loop runs until the task is aborted or the sink closes; callers
//! own the shutdown (the agent aborts the join handle on graceful
//! shutdown, the same discipline as its other spawned tasks).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Semaphore};

use crate::model::Sample;

/// A source-side collection failure (reported per source; never carries
/// VM identifiers into logs exported as metrics).
#[derive(Clone, Debug, thiserror::Error)]
pub enum SamplerError {
    #[error("source failed: {0}")]
    Source(String),
    #[error("source is not usable in this configuration: {0}")]
    Unsupported(String),
}

/// Linux node resource source (e.g. the retained [`crate::node_os`]
/// collector wrapped by the agent).
#[async_trait::async_trait]
pub trait NodeOsSource: Send + Sync {
    /// Collect one cycle of node samples. The implementation must be
    /// read-only and bounded; the runner enforces the timeout.
    async fn collect_node(&self) -> Result<Vec<Sample>, SamplerError>;
}

/// Owned VM runtime source (the agent's VMM adapter).
#[async_trait::async_trait]
pub trait VmRuntimeSource: Send + Sync {
    /// The VM ids to sample this cycle (running, agent-owned VMs only).
    async fn vm_ids(&self) -> Result<Vec<String>, SamplerError>;

    /// Collect one VM's samples. Called with bounded concurrency; the
    /// runner enforces the timeout.
    async fn collect_vm(&self, vm_id: &str) -> Result<Vec<Sample>, SamplerError>;
}

/// Storage provider source (`chv-stord`). Providers without attributable
/// v1 metrics report an empty sample set — honestly, without faking
/// coverage.
#[async_trait::async_trait]
pub trait StorageProviderSource: Send + Sync {
    async fn collect_storage(&self) -> Result<Vec<Sample>, SamplerError>;
}

/// Network provider source (`chv-nwd`), same honesty rules as
/// [`StorageProviderSource`].
#[async_trait::async_trait]
pub trait NetworkProviderSource: Send + Sync {
    async fn collect_network(&self) -> Result<Vec<Sample>, SamplerError>;
}

/// Sampler bounds and cadence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplerConfig {
    /// Base cadence, measured cycle-start to cycle-start. This is the
    /// native spec's light-node-metrics timer (default 5 s): the node
    /// source runs every cycle.
    pub interval: Duration,
    /// VM collection cadence. Rounded up to a whole multiple of
    /// [`Self::interval`] (10 s over a 5 s base ⇒ every 2nd cycle).
    pub vm_interval: Duration,
    /// Provider (stord/nwd) collection cadence, rounded the same way
    /// (15 s over a 5 s base ⇒ every 3rd cycle).
    pub provider_interval: Duration,
    /// Maximum concurrent VM collections (cross-VM fan-out bound).
    pub max_concurrent_vm_collections: usize,
    /// Per-source-call timeout.
    pub source_timeout: Duration,
}

impl Default for SamplerConfig {
    fn default() -> Self {
        SamplerConfig {
            // The native spec's timer split: node 5 s, VM 10 s,
            // provider 15 s (PR-2).
            interval: Duration::from_secs(5),
            vm_interval: Duration::from_secs(10),
            provider_interval: Duration::from_secs(15),
            max_concurrent_vm_collections: 8,
            source_timeout: Duration::from_secs(2),
        }
    }
}

impl SamplerConfig {
    /// How many base cycles pass between VM collections (>= 1).
    fn vm_stride(&self) -> u64 {
        stride(self.interval, self.vm_interval)
    }

    /// How many base cycles pass between provider collections (>= 1).
    fn provider_stride(&self) -> u64 {
        stride(self.interval, self.provider_interval)
    }
}

fn stride(base: Duration, target: Duration) -> u64 {
    if base.is_zero() {
        return 1;
    }
    let n = target.as_nanos().div_ceil(base.as_nanos());
    n.clamp(1, u64::MAX as u128) as u64
}

/// Point-in-time health snapshot for the Prometheus surface (global
/// labels only — no VM identifiers).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SamplerHealthSnapshot {
    pub cycles_completed: u64,
    pub cycle_failures: u64,
    pub source_timeouts: u64,
    pub vm_collection_failures: u64,
    pub dropped_samples: u64,
    pub last_cycle_duration_ms: u64,
    /// Unix ms of the last fully-successful cycle (0 before the first).
    pub last_success_unix_ms: u64,
}

/// Cross-thread health counters, updated by the sampler loop and read by
/// the metrics server.
#[derive(Debug, Default)]
pub struct SamplerHealth {
    cycles_completed: AtomicU64,
    cycle_failures: AtomicU64,
    source_timeouts: AtomicU64,
    vm_collection_failures: AtomicU64,
    dropped_samples: AtomicU64,
    last_cycle_duration_ms: AtomicU64,
    last_success_unix_ms: AtomicU64,
}

impl SamplerHealth {
    /// A zeroed health set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the current counters.
    pub fn snapshot(&self) -> SamplerHealthSnapshot {
        SamplerHealthSnapshot {
            cycles_completed: self.cycles_completed.load(Ordering::Relaxed),
            cycle_failures: self.cycle_failures.load(Ordering::Relaxed),
            source_timeouts: self.source_timeouts.load(Ordering::Relaxed),
            vm_collection_failures: self.vm_collection_failures.load(Ordering::Relaxed),
            dropped_samples: self.dropped_samples.load(Ordering::Relaxed),
            last_cycle_duration_ms: self.last_cycle_duration_ms.load(Ordering::Relaxed),
            last_success_unix_ms: self.last_success_unix_ms.load(Ordering::Relaxed),
        }
    }
}

/// The sampler's inputs: every source is optional except the node source
/// (a node without node metrics has nothing to sample).
pub struct SamplerSources {
    pub node: Arc<dyn NodeOsSource>,
    pub vms: Option<Arc<dyn VmRuntimeSource>>,
    pub storage: Option<Arc<dyn StorageProviderSource>>,
    pub network: Option<Arc<dyn NetworkProviderSource>>,
}

/// One event from the sampler loop to its consumer.
#[derive(Debug)]
pub enum SamplerEvent {
    /// A batch of samples from one source collection (the node source
    /// per cycle, one batch per VM, provider batches on their stride).
    Samples(Vec<Sample>),
    /// The VM roster observed on a VM-due cycle: the target ids
    /// currently on this node (possibly empty). Emitted only when the
    /// listing SUCCEEDED — a failed listing emits nothing, because an
    /// unknown roster must not look like an empty one. Consumers
    /// reconcile: series for VM targets no longer in the roster are
    /// stale by construction (the VM left the node, was deleted, or
    /// was migrated away) and must stop being shipped — the control
    /// plane fails the WHOLE batch for an unowned VM target.
    VmRoster(Vec<String>),
}

/// Run the bounded sampler loop until aborted or the sink closes.
///
/// Each cycle: node source, then every VM (bounded fan-out, per-call
/// timeout), then the optional provider sources; every produced batch is
/// `try_send`-ed to the bounded sink — a full sink drops the batch and
/// counts the samples, it never blocks or grows memory. On every VM-due
/// cycle with a successful listing, a [`SamplerEvent::VmRoster`] is
/// emitted BEFORE the per-VM sample batches so consumers can evict
/// departed targets first.
pub async fn run_sampler(
    config: SamplerConfig,
    sources: SamplerSources,
    sink: mpsc::Sender<SamplerEvent>,
    health: Arc<SamplerHealth>,
) {
    let semaphore = Arc::new(Semaphore::new(config.max_concurrent_vm_collections.max(1)));
    let vm_stride = config.vm_stride();
    let provider_stride = config.provider_stride();
    let mut cycle_index: u64 = 0;
    loop {
        let cycle_start = std::time::Instant::now();
        let mut cycle_ok = true;
        let vm_due = cycle_index.is_multiple_of(vm_stride);
        let providers_due = cycle_index.is_multiple_of(provider_stride);
        cycle_index = cycle_index.wrapping_add(1);

        // --- node source ---
        match tokio::time::timeout(config.source_timeout, sources.node.collect_node()).await {
            Ok(Ok(samples)) => {
                if !send_batch(&sink, SamplerEvent::Samples(samples), &health) {
                    return;
                }
                health.cycles_completed.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Err(e)) => {
                cycle_ok = false;
                tracing::warn!(error = %e, "node sample collection failed");
            }
            Err(_) => {
                cycle_ok = false;
                health.source_timeouts.fetch_add(1, Ordering::Relaxed);
                tracing::warn!("node sample collection timed out");
            }
        }

        // --- VM sources, bounded fan-out (every vm_stride cycles) ---
        if vm_due {
            if let Some(vms) = sources.vms.clone() {
                // A failed listing yields an UNKNOWN roster: emit no
                // roster event (an unknown roster must not evict live
                // VM series), and skip this cycle's collections.
                let listing = match tokio::time::timeout(config.source_timeout, vms.vm_ids()).await
                {
                    Ok(Ok(ids)) => Some(ids),
                    Ok(Err(e)) => {
                        cycle_ok = false;
                        tracing::warn!(error = %e, "vm id listing failed");
                        None
                    }
                    Err(_) => {
                        cycle_ok = false;
                        health.source_timeouts.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!("vm id listing timed out");
                        None
                    }
                };
                // Roster first (see `SamplerEvent::VmRoster`); an
                // UNKNOWN listing (None) emits nothing — no eviction,
                // no collections this cycle — and the provider sources
                // below still run. A full channel drops the event
                // (eviction retries next cycle); a closed channel ends
                // the loop.
                if let Some(ids) = listing {
                    match sink.try_send(SamplerEvent::VmRoster(ids.clone())) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            tracing::debug!("sample sink full; VM roster event dropped");
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => return,
                    }

                    let mut pending = tokio::task::JoinSet::new();
                    for vm_id in ids {
                        // Acquiring the permit before spawn keeps the JoinSet
                        // itself bounded to the concurrency limit.
                        let permit = match semaphore.clone().acquire_owned().await {
                            Ok(p) => p,
                            Err(_) => return, // sampler torn down
                        };
                        let vms = vms.clone();
                        let sink = sink.clone();
                        let health = health.clone();
                        let timeout = config.source_timeout;
                        pending.spawn(async move {
                            let _permit = permit;
                            match tokio::time::timeout(timeout, vms.collect_vm(&vm_id)).await {
                                Ok(Ok(samples)) => {
                                    if !send_batch(&sink, SamplerEvent::Samples(samples), &health) {
                                        tracing::warn!("sample sink closed; sampler exiting");
                                    }
                                }
                                Ok(Err(e)) => {
                                    health
                                        .vm_collection_failures
                                        .fetch_add(1, Ordering::Relaxed);
                                    // The error text may name the VM; it goes to
                                    // logs only, never to metric labels.
                                    tracing::warn!(error = %e, "vm sample collection failed");
                                }
                                Err(_) => {
                                    health.source_timeouts.fetch_add(1, Ordering::Relaxed);
                                    tracing::warn!("vm sample collection timed out");
                                }
                            }
                        });
                    }
                    while let Some(joined) = pending.join_next().await {
                        if joined.is_err() {
                            cycle_ok = false;
                        }
                    }
                }
            }
        }

        // --- optional provider sources (every provider_stride cycles) ---
        if providers_due {
            type ProviderOutcome =
                Option<Result<Result<Vec<Sample>, SamplerError>, tokio::time::error::Elapsed>>;
            let provider_results: [(&str, ProviderOutcome); 2] = [
                (
                    "storage",
                    match &sources.storage {
                        Some(s) => Some(
                            tokio::time::timeout(config.source_timeout, s.collect_storage()).await,
                        ),
                        None => None,
                    },
                ),
                (
                    "network",
                    match &sources.network {
                        Some(s) => Some(
                            tokio::time::timeout(config.source_timeout, s.collect_network()).await,
                        ),
                        None => None,
                    },
                ),
            ];
            for (name, result) in provider_results {
                match result {
                    Some(Ok(Ok(samples))) => {
                        if !samples.is_empty()
                            && !send_batch(&sink, SamplerEvent::Samples(samples), &health)
                        {
                            return;
                        }
                    }
                    Some(Ok(Err(e))) => {
                        cycle_ok = false;
                        tracing::warn!(provider = name, error = %e, "provider sample collection failed");
                    }
                    Some(Err(_)) => {
                        cycle_ok = false;
                        health.source_timeouts.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(provider = name, "provider sample collection timed out");
                    }
                    None => {}
                }
            }
        }

        health
            .last_cycle_duration_ms
            .store(cycle_start.elapsed().as_millis() as u64, Ordering::Relaxed);
        if cycle_ok {
            health
                .last_success_unix_ms
                .store(crate::node_os::unix_now_ms(), Ordering::Relaxed);
        } else {
            health.cycle_failures.fetch_add(1, Ordering::Relaxed);
        }

        // Cadence from cycle start, so a slow cycle shortens the sleep
        // instead of drifting the schedule.
        let elapsed = cycle_start.elapsed();
        if elapsed < config.interval {
            tokio::time::sleep(config.interval - elapsed).await;
        }
    }
}

/// Bounded, non-blocking batch delivery. Returns `false` when the sink is
/// closed (sampler should exit); `true` otherwise (a full sink drops and
/// counts).
fn send_batch(
    sink: &mpsc::Sender<SamplerEvent>,
    event: SamplerEvent,
    health: &Arc<SamplerHealth>,
) -> bool {
    let samples_len = match &event {
        SamplerEvent::Samples(samples) => samples.len(),
        SamplerEvent::VmRoster(_) => 0,
    };
    if samples_len == 0 {
        return !sink.is_closed();
    }
    match sink.try_send(event) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            health
                .dropped_samples
                .fetch_add(samples_len as u64, Ordering::Relaxed);
            tracing::warn!(
                count = samples_len,
                "sample sink full; dropping batch (counted, never unbounded)"
            );
            true
        }
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SampleBuilder, SampleValue, Source, TargetKind};
    use std::sync::atomic::AtomicUsize;

    struct FakeNode {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl NodeOsSource for FakeNode {
        async fn collect_node(&self) -> Result<Vec<Sample>, SamplerError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(vec![SampleBuilder::new(
                TargetKind::Node,
                "node-1",
                "node.cpu.capacity_ratio",
                Source::NodeOs,
                crate::node_os::unix_now_ms(),
            )
            .unwrap()
            .value(SampleValue::Float(0.5))
            .build()
            .unwrap()])
        }
    }

    struct FakeVms {
        ids: Vec<String>,
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl VmRuntimeSource for FakeVms {
        async fn vm_ids(&self) -> Result<Vec<String>, SamplerError> {
            Ok(self.ids.clone())
        }

        async fn collect_vm(&self, vm_id: &str) -> Result<Vec<Sample>, SamplerError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(vec![SampleBuilder::new(
                TargetKind::Vm,
                vm_id,
                "vm.cpu.cores_used",
                Source::Vmm,
                crate::node_os::unix_now_ms(),
            )
            .unwrap()
            .value(SampleValue::Float(1.0))
            .epoch("boot", format!("epoch-{vm_id}"))
            .build()
            .unwrap()])
        }
    }

    #[tokio::test(start_paused = true)]
    async fn samples_flow_and_health_updates() {
        let node = Arc::new(FakeNode {
            calls: AtomicUsize::new(0),
        });
        let vms = Arc::new(FakeVms {
            ids: vec!["vm-a".into(), "vm-b".into()],
            calls: AtomicUsize::new(0),
        });
        let (tx, mut rx) = mpsc::channel(64);
        let health = Arc::new(SamplerHealth::new());

        let config = SamplerConfig {
            interval: Duration::from_secs(1),
            vm_interval: Duration::from_secs(1),
            provider_interval: Duration::from_secs(1),
            max_concurrent_vm_collections: 2,
            source_timeout: Duration::from_secs(1),
        };
        let handle = tokio::spawn(run_sampler(
            config,
            SamplerSources {
                node: node.clone(),
                vms: Some(vms),
                storage: None,
                network: None,
            },
            tx,
            health.clone(),
        ));

        // Let two cycles run.
        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut total = 0;
        let mut roster_events = 0;
        while let Ok(event) = rx.try_recv() {
            match event {
                SamplerEvent::Samples(batch) => total += batch.len(),
                SamplerEvent::VmRoster(_) => roster_events += 1,
            }
        }
        // Paused-clock tick boundaries may start a further cycle before
        // the sleep returns, so this is a lower bound, not an exact
        // count; each completed cycle contributes 1 node + 2 VM samples.
        assert!(
            total >= 2 * (1 + 2),
            "two cycles of node+2vm samples, got {total}"
        );
        // Each VM-due cycle with a successful listing emits exactly one
        // roster event before its sample batches.
        assert!(roster_events >= 1, "at least one VM roster event");
        let snap = health.snapshot();
        assert!(snap.cycles_completed >= 2);
        assert_eq!(snap.cycle_failures, 0);
        assert_eq!(snap.dropped_samples, 0);
        assert!(snap.last_success_unix_ms > 0);

        handle.abort();
    }

    #[test]
    fn cadence_strides_round_up_to_whole_cycles() {
        let config = SamplerConfig::default();
        // 5 s base: 10 s VM cadence ⇒ every 2nd cycle, 15 s provider
        // cadence ⇒ every 3rd.
        assert_eq!(config.vm_stride(), 2);
        assert_eq!(config.provider_stride(), 3);
        // Sub-base targets round up to every cycle, never to zero.
        let fast = SamplerConfig {
            interval: Duration::from_secs(5),
            vm_interval: Duration::from_secs(2),
            provider_interval: Duration::from_secs(4),
            ..SamplerConfig::default()
        };
        assert_eq!(fast.vm_stride(), 1);
        assert_eq!(fast.provider_stride(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn vm_cadence_splits_from_node_cadence() {
        let node = Arc::new(FakeNode {
            calls: AtomicUsize::new(0),
        });
        let vms = Arc::new(FakeVms {
            ids: vec!["vm-a".into()],
            calls: AtomicUsize::new(0),
        });
        let (tx, _rx) = mpsc::channel(64);
        let health = Arc::new(SamplerHealth::new());
        // 100 ms base, VM every 300 ms ⇒ stride 3: over ~1 s the node
        // source runs ~10 times, the VM source ~3-4.
        let config = SamplerConfig {
            interval: Duration::from_millis(100),
            vm_interval: Duration::from_millis(300),
            provider_interval: Duration::from_millis(300),
            max_concurrent_vm_collections: 2,
            source_timeout: Duration::from_secs(1),
        };
        let handle = tokio::spawn(run_sampler(
            config,
            SamplerSources {
                node: node.clone(),
                vms: Some(vms.clone()),
                storage: None,
                network: None,
            },
            tx,
            health.clone(),
        ));
        tokio::time::sleep(Duration::from_secs(1)).await;
        let node_calls = node.calls.load(Ordering::Relaxed);
        let vm_calls = vms.calls.load(Ordering::Relaxed);
        assert!(node_calls >= 8, "node runs every cycle, got {node_calls}");
        assert!(
            vm_calls >= 2 && vm_calls * 3 <= node_calls + 3,
            "vm runs at most every 3rd cycle: {vm_calls} vm vs {node_calls} node"
        );
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn full_sink_drops_and_counts_instead_of_blocking() {
        let node = Arc::new(FakeNode {
            calls: AtomicUsize::new(0),
        });
        // Capacity 1, receiver never drained: every batch after the
        // first must be dropped and counted, never block the loop.
        let (tx, _rx) = mpsc::channel(1);
        let health = Arc::new(SamplerHealth::new());
        let config = SamplerConfig {
            interval: Duration::from_millis(100),
            ..SamplerConfig::default()
        };
        let handle = tokio::spawn(run_sampler(
            config,
            SamplerSources {
                node: node.clone(),
                vms: None,
                storage: None,
                network: None,
            },
            tx,
            health.clone(),
        ));
        tokio::time::sleep(Duration::from_secs(1)).await;
        let snap = health.snapshot();
        assert!(snap.cycles_completed >= 2);
        assert!(snap.dropped_samples >= 1, "drops are counted");
        // The loop kept its cadence despite the full sink.
        assert!(node.calls.load(Ordering::Relaxed) >= 5);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn hung_source_times_out_and_cycle_continues() {
        struct HungNode;
        #[async_trait::async_trait]
        impl NodeOsSource for HungNode {
            async fn collect_node(&self) -> Result<Vec<Sample>, SamplerError> {
                std::future::pending::<()>().await;
                unreachable!()
            }
        }
        let (tx, mut rx) = mpsc::channel(64);
        let health = Arc::new(SamplerHealth::new());
        let config = SamplerConfig {
            interval: Duration::from_millis(200),
            source_timeout: Duration::from_millis(50),
            ..SamplerConfig::default()
        };
        let handle = tokio::spawn(run_sampler(
            config,
            SamplerSources {
                node: Arc::new(HungNode),
                vms: None,
                storage: None,
                network: None,
            },
            tx,
            health.clone(),
        ));
        tokio::time::sleep(Duration::from_secs(1)).await;
        let snap = health.snapshot();
        assert!(snap.source_timeouts >= 2, "timeouts counted");
        assert!(snap.cycle_failures >= 2);
        assert!(snap.cycles_completed == 0);
        assert!(rx.try_recv().is_err(), "no samples from a hung source");
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn closed_sink_exits_the_loop() {
        let node = Arc::new(FakeNode {
            calls: AtomicUsize::new(0),
        });
        let (tx, rx) = mpsc::channel(64);
        let health = Arc::new(SamplerHealth::new());
        drop(rx);
        let config = SamplerConfig {
            interval: Duration::from_millis(50),
            ..SamplerConfig::default()
        };
        let handle = tokio::spawn(run_sampler(
            config,
            SamplerSources {
                node,
                vms: None,
                storage: None,
                network: None,
            },
            tx,
            health,
        ));
        // The sampler exits on its own once the sink is closed.
        let exited = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(exited.is_ok(), "sampler exits when the sink closes");
    }
}
