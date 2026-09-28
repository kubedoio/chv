//! Deterministic fault points for the fault-injection matrix (M2.4).
//!
//! The operation lifecycle has five crash windows, and each has a
//! deterministic injection idiom:
//!
//! | Fault point | Window | Injection |
//! |---|---|---|
//! | 1 | before durable acceptance | actor dropped-reply / SQLite `TEMP TRIGGER` abort (see `cellhv-core-operations` and `cellhv-core-store` tests) |
//! | 2 | after acceptance + claim, before the provider effect | [`FaultRuntime`] at [`FaultPoint::BeforeEffect`] |
//! | 3 | during the provider effect | `MockHostResourceController::new_with_fail` / `MockCloudHypervisorAdapter::fail_delete` (see `chv-agent-runtime-ch`) |
//! | 4 | after the provider effect, before the compatibility projection | [`FaultRuntime`] at [`FaultPoint::AfterEffect`] wrapped *inside* a projecting runtime |
//! | 5 | after the effect (and projection), before terminal persistence | [`FaultRuntime`] at [`FaultPoint::AfterEffect`] as the outermost runtime |
//!
//! [`FaultRuntime`] parks the operation's executor task inside the chosen
//! window. Parking — not failing — is the crash simulation: the composition
//! is then torn down (aborting the parked task, exactly what process death
//! does to an in-flight task), leaving the durable state that a real crash
//! at that point would leave. Re-opening the journal afterwards exercises
//! the restart/replay path.
//!
//! This is a test affordance in the same spirit as
//! `MockHostResourceController`: always compiled (so cross-crate
//! integration tests and canaries can use it without new dependency
//! edges), inert unless explicitly constructed, and never referenced by
//! production composition. It holds no capability beyond the runtime it
//! wraps.

use crate::{CoreVmRuntime, RuntimeFailure};
use async_trait::async_trait;
use cellhv_core_operations::OperationJournalEntry;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// The deterministic crash window a [`FaultRuntime`] parks an operation in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    /// Park after the claim, before the provider effect is delegated: the
    /// operation is durably `running` with an attempt token and no side
    /// effect has started (fault point 2).
    BeforeEffect,
    /// Park after the wrapped runtime completed the provider effect, before
    /// the result is returned to the executor — and therefore before any
    /// compatibility projection or terminal persistence. Which of fault
    /// points 4/5 this exercises is determined by the wrapper's position in
    /// the composition: inside a projecting wrapper it is "effect done,
    /// projection pending"; outermost it is "effect (and projection) done,
    /// terminal persistence pending".
    AfterEffect,
}

/// `CoreVmRuntime` wrapper that parks the executor task at a deterministic
/// fault point.
///
/// Parked tasks never return; the test tears the composition down to
/// simulate process death at that exact point. [`Self::disarm`] makes the
/// wrapper a pass-through so a restarted composition (or successor
/// operations in the same composition) can run to completion while
/// [`Self::inner_completions`] keeps counting effects for no-double-effect
/// assertions.
///
/// [`Self::reached`] fires when the first armed park happens; the stored
/// permit makes `notified()` deterministic regardless of await ordering.
/// Scenarios that need to park more than one operation at distinct moments
/// should use separate instances.
pub struct FaultRuntime {
    point: FaultPoint,
    inner: Arc<dyn CoreVmRuntime>,
    /// When `false`, the wrapper is a pure pass-through.
    armed: AtomicBool,
    /// Notified (once) when an armed park happens.
    pub reached: Notify,
    /// Number of times the wrapped runtime completed an effect successfully.
    pub inner_completions: AtomicUsize,
}

impl FaultRuntime {
    /// Wraps `inner` so the first executed operation parks at `point`.
    pub fn park_at(point: FaultPoint, inner: Arc<dyn CoreVmRuntime>) -> Arc<Self> {
        Arc::new(Self {
            point,
            inner,
            armed: AtomicBool::new(true),
            reached: Notify::new(),
            inner_completions: AtomicUsize::new(0),
        })
    }

    /// Makes the wrapper a pass-through: later operations execute the
    /// wrapped runtime to completion and are counted, but never parked.
    pub fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
    }

    fn armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CoreVmRuntime for FaultRuntime {
    async fn execute(
        &self,
        operation: OperationJournalEntry,
    ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
        if self.armed() && matches!(self.point, FaultPoint::BeforeEffect) {
            self.reached.notify_one();
            // Crash window: claim is durable, effect never starts.
            std::future::pending::<()>().await;
        }
        // A failing effect is the runtime's own failure path (fault point 3
        // is injected by the mocks), not a crash window: propagate it.
        let outcome = self.inner.execute(operation).await?;
        self.inner_completions.fetch_add(1, Ordering::SeqCst);
        if self.armed() {
            self.reached.notify_one();
            // Crash window: the effect is done; projection (if any wraps
            // this) and terminal persistence have not run.
            std::future::pending::<()>().await;
        }
        Ok(outcome)
    }
}
