//! Pluggable wall-clock abstraction.
//!
//! The CHV control plane uses [`chrono::DateTime<Utc>`] timestamps for plan
//! lifetimes (apply gating, expiry, drift) and apply-run bookkeeping. Tests
//! that exercise time-sensitive logic must not depend on the real clock —
//! they need to be able to advance time deterministically. This module
//! provides:
//!
//! * [`Clock`] — the abstraction. `Send + Sync + 'static` so it can be
//!   shared across async tasks behind an `Arc<dyn Clock>`.
//! * [`SystemClock`] — production implementation that returns
//!   [`chrono::Utc::now`].
//! * [`ManualClock`] — test implementation backed by an [`Arc<Mutex<…>>`]
//!   so multiple references can advance the held instant.
//!
//! ```
//! use chv_common::clock::{Clock, ManualClock};
//! use chrono::{Duration, TimeZone, Utc};
//!
//! let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
//! let clock = ManualClock::new(start);
//! assert_eq!(clock.now(), start);
//! clock.advance(Duration::seconds(30));
//! assert_eq!(clock.now(), start + Duration::seconds(30));
//! ```

use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};

/// Abstraction over the wall clock. Implementations must be cheap to call
/// and free of side effects beyond reading time.
pub trait Clock: Send + Sync + 'static {
    /// Current UTC instant according to this clock.
    fn now(&self) -> DateTime<Utc>;
}

/// Production [`Clock`] that returns [`chrono::Utc::now`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    /// Construct a new [`SystemClock`]. Equivalent to [`SystemClock::default`].
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Test [`Clock`] whose held instant can be advanced or set explicitly.
///
/// Cloning a [`ManualClock`] yields a handle to the same underlying instant,
/// so a test fixture and the system under test can share one clock.
#[derive(Debug, Clone)]
pub struct ManualClock {
    inner: Arc<Mutex<DateTime<Utc>>>,
}

impl ManualClock {
    /// Construct a [`ManualClock`] starting at `initial`.
    pub fn new(initial: DateTime<Utc>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(initial)),
        }
    }

    /// Advance the held instant by `delta`. `delta` may be negative; the
    /// clock will move backwards in that case (useful for testing
    /// pathological skew).
    ///
    /// Panics in tests if the inner mutex is poisoned. Production code does
    /// not use [`ManualClock`].
    pub fn advance(&self, delta: chrono::Duration) {
        let mut guard = self
            .inner
            .lock()
            .expect("ManualClock mutex poisoned by panic in another thread");
        *guard += delta;
    }

    /// Set the held instant to exactly `t`.
    ///
    /// Panics in tests if the inner mutex is poisoned. Production code does
    /// not use [`ManualClock`].
    pub fn set(&self, t: DateTime<Utc>) {
        let mut guard = self
            .inner
            .lock()
            .expect("ManualClock mutex poisoned by panic in another thread");
        *guard = t;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self
            .inner
            .lock()
            .expect("ManualClock mutex poisoned by panic in another thread")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn system_clock_returns_recent_instants() {
        let clock = SystemClock::new();
        let before = Utc::now();
        let observed = clock.now();
        let after = Utc::now();
        assert!(
            observed >= before,
            "clock went backwards: {observed} < {before}"
        );
        assert!(
            observed <= after,
            "clock jumped forward: {observed} > {after}"
        );
    }

    #[test]
    fn manual_clock_honors_advance() {
        let start = Utc.with_ymd_and_hms(2026, 6, 13, 10, 0, 0).unwrap();
        let clock = ManualClock::new(start);
        assert_eq!(clock.now(), start);
        clock.advance(Duration::minutes(15));
        assert_eq!(clock.now(), start + Duration::minutes(15));
        clock.advance(Duration::seconds(-30));
        assert_eq!(
            clock.now(),
            start + Duration::minutes(14) + Duration::seconds(30)
        );
    }

    #[test]
    fn manual_clock_honors_set() {
        let start = Utc.with_ymd_and_hms(2026, 6, 13, 10, 0, 0).unwrap();
        let target = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();
        let clock = ManualClock::new(start);
        clock.set(target);
        assert_eq!(clock.now(), target);
    }

    #[test]
    fn manual_clock_clones_share_instant() {
        let start = Utc.with_ymd_and_hms(2026, 6, 13, 10, 0, 0).unwrap();
        let a = ManualClock::new(start);
        let b = a.clone();
        a.advance(Duration::minutes(5));
        assert_eq!(b.now(), start + Duration::minutes(5));
    }
}
