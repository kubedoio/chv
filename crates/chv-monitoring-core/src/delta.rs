//! Boot-epoch-scoped, reset-safe counter deltas (ADR-025 counter
//! semantics).
//!
//! Counter rates are only valid as positive deltas between two observations
//! with the same `boot_id`, `identity_epoch`, counter key and source. The
//! G0b fixture campaign proved both reset hazards are real on the qualified
//! pin:
//!
//! - a VMM process restart starts counters at a **new per-process base**
//!   (not zero, not continuing) — a cross-restart subtraction fabricates a
//!   huge negative or wraps;
//! - a non-monotonic reading within one epoch (counter clear, device
//!   re-attach) must surface as a reset, never as a negative rate or a
//!   spike.
//!
//! A reset interval emits **no rate**: [`DeltaOutcome::Reset`] tells the
//! caller to skip the interval, not to interpolate.

/// The epoch a counter reading belongs to: the boot that produced it and
/// the identity fence of the incarnation that owns the counter. Two
/// readings subtract only when both parts match.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Epoch {
    /// Host boot id (`/proc/sys/kernel/random/boot_id`) — required for
    /// counter series from a restartable source.
    pub boot_id: String,
    /// Stable incarnation marker (e.g. a VMM process start-ticks fence).
    pub identity_epoch: String,
}

impl Epoch {
    pub fn new(boot_id: impl Into<String>, identity_epoch: impl Into<String>) -> Self {
        Epoch {
            boot_id: boot_id.into(),
            identity_epoch: identity_epoch.into(),
        }
    }
}

/// The outcome of comparing a counter reading against the retained
/// previous reading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeltaOutcome {
    /// No retained prior reading in this epoch: nothing can be computed
    /// yet. The reading is retained for the next cycle.
    InsufficientSamples,
    /// Positive same-epoch delta. Always ≥ 0 by construction; a zero delta
    /// is a valid measurement of no activity.
    Delta(u64),
    /// The reading moved backwards within an epoch, or the epoch changed
    /// (boot or identity). The interval emits **no rate**; the new reading
    /// is retained.
    Reset {
        previous: u64,
        current: u64,
        /// `true` when the reset was caused by an epoch change rather
        /// than a backwards reading (both are resets; the distinction is
        /// for health counters and log clarity).
        epoch_changed: bool,
    },
}

/// Retained previous reading for one counter series (one metric, target,
/// key and source — the caller keys instances).
///
/// Single-threaded by design: one sampler task owns its counter states, so
/// no locking is needed inside the engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CounterState {
    epoch: Option<Epoch>,
    last_value: Option<u64>,
}

impl CounterState {
    /// A state with no retained reading.
    pub fn new() -> Self {
        CounterState::default()
    }

    /// Feed the next reading. Retains it (even across resets — the newest
    /// reading is always the base for the next interval) and classifies
    /// the interval.
    pub fn observe(&mut self, epoch: Epoch, current: u64) -> DeltaOutcome {
        let epoch_changed = matches!(
            (&self.epoch, self.last_value),
            (Some(prev_epoch), Some(_)) if *prev_epoch != epoch
        );

        let outcome = match (self.last_value, epoch_changed) {
            (Some(prev), false) if current >= prev => DeltaOutcome::Delta(current - prev),
            (Some(prev), false) => DeltaOutcome::Reset {
                previous: prev,
                current,
                epoch_changed: false,
            },
            (Some(prev), true) => DeltaOutcome::Reset {
                previous: prev,
                current,
                epoch_changed: true,
            },
            (None, _) => DeltaOutcome::InsufficientSamples,
        };

        self.epoch = Some(epoch);
        self.last_value = Some(current);
        outcome
    }

    /// The retained epoch, if any.
    pub fn epoch(&self) -> Option<&Epoch> {
        self.epoch.as_ref()
    }

    /// The retained last reading, if any.
    pub fn last_value(&self) -> Option<u64> {
        self.last_value
    }

    /// Drop the retained reading (e.g. when the source reports the series
    /// gone) so a later re-appearance starts with `InsufficientSamples`
    /// instead of subtracting across the gap.
    pub fn forget(&mut self) {
        self.epoch = None;
        self.last_value = None;
    }
}

/// Converts a same-epoch counter delta into a per-second rate, exactly as
/// the registry defines rate metrics: delta divided by monotonic wall
/// seconds. A non-positive elapsed time yields `None` (no rate, never a
/// fabricated one); integer counters keep integer precision in the delta
/// and only become f64 at this final division.
pub fn rate_per_second(delta: u64, elapsed_secs: f64) -> Option<f64> {
    if elapsed_secs > 0.0 && elapsed_secs.is_finite() {
        Some(delta as f64 / elapsed_secs)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch(n: u8) -> Epoch {
        Epoch::new(format!("boot-{n}"), "identity-1")
    }

    #[test]
    fn first_reading_is_insufficient() {
        let mut c = CounterState::new();
        assert_eq!(c.observe(epoch(1), 100), DeltaOutcome::InsufficientSamples);
        assert_eq!(c.last_value(), Some(100));
    }

    #[test]
    fn monotonic_delta() {
        let mut c = CounterState::new();
        c.observe(epoch(1), 100);
        assert_eq!(c.observe(epoch(1), 250), DeltaOutcome::Delta(150));
        assert_eq!(c.observe(epoch(1), 250), DeltaOutcome::Delta(0));
        assert_eq!(c.observe(epoch(1), 251), DeltaOutcome::Delta(1));
    }

    #[test]
    fn backwards_reading_is_reset_not_negative() {
        let mut c = CounterState::new();
        c.observe(epoch(1), 500);
        assert_eq!(
            c.observe(epoch(1), 100),
            DeltaOutcome::Reset {
                previous: 500,
                current: 100,
                epoch_changed: false
            }
        );
        // The new base is retained: the next interval is a normal delta.
        assert_eq!(c.observe(epoch(1), 150), DeltaOutcome::Delta(50));
    }

    #[test]
    fn epoch_change_is_reset_even_when_monotonic() {
        let mut c = CounterState::new();
        c.observe(epoch(1), 500);
        // Same value in a new boot: monotonic-looking but a different
        // epoch — subtracting across it is forbidden (G0b: a restarted
        // VMM's counters start at a new per-process base).
        assert_eq!(
            c.observe(epoch(2), 500),
            DeltaOutcome::Reset {
                previous: 500,
                current: 500,
                epoch_changed: true
            }
        );
        // ...and a new-boot base LOWER than the old reading must not wrap.
        assert_eq!(
            c.observe(epoch(3), 10),
            DeltaOutcome::Reset {
                previous: 500,
                current: 10,
                epoch_changed: true
            }
        );
        assert_eq!(c.observe(epoch(3), 30), DeltaOutcome::Delta(20));
    }

    #[test]
    fn identity_epoch_alone_fences() {
        let mut c = CounterState::new();
        c.observe(Epoch::new("boot-1", "vmm-A"), 0);
        assert_eq!(
            c.observe(Epoch::new("boot-1", "vmm-B"), 50),
            DeltaOutcome::Reset {
                previous: 0,
                current: 50,
                epoch_changed: true
            }
        );
    }

    #[test]
    fn forget_restarts_the_series() {
        let mut c = CounterState::new();
        c.observe(epoch(1), 100);
        c.forget();
        assert_eq!(c.observe(epoch(1), 120), DeltaOutcome::InsufficientSamples);
    }

    #[test]
    fn u64_boundary_deltas_do_not_overflow() {
        let mut c = CounterState::new();
        c.observe(epoch(1), u64::MAX - 10);
        assert_eq!(c.observe(epoch(1), u64::MAX), DeltaOutcome::Delta(10));
    }

    #[test]
    fn rate_math() {
        assert_eq!(rate_per_second(150, 10.0), Some(15.0));
        assert_eq!(rate_per_second(0, 10.0), Some(0.0));
        assert_eq!(rate_per_second(10, 0.0), None);
        assert_eq!(rate_per_second(10, f64::NAN), None);
    }
}
