//! Tracks control plane connectivity state for partition-autonomous operation.
//!
//! The agent continues VM lifecycle operations regardless of control plane
//! reachability. This module makes the implicit partition tolerance explicit
//! by tracking connectivity state transitions, emitting structured tracing
//! events, and exposing metrics for Prometheus scraping.

use tracing::{debug, info};

/// Default number of consecutive failures before transitioning to Disconnected.
const DISCONNECT_THRESHOLD: u32 = 3;

/// Connectivity state of the control plane link.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectivityState {
    /// Active, healthy connection to the control plane.
    Connected,
    /// Control plane is unreachable; agent operates autonomously.
    #[default]
    Disconnected,
    /// Attempting to re-establish connection after a disconnect.
    Reconnecting,
}

impl ConnectivityState {
    /// Returns a string label suitable for metrics and logs.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Reconnecting => "reconnecting",
        }
    }
}

/// Snapshot of connectivity metrics for Prometheus exposition.
#[derive(Debug, Clone)]
pub struct ConnectivityMetrics {
    /// Current state label.
    pub state: ConnectivityState,
    /// Unix milliseconds when disconnect was first detected, if currently disconnected.
    pub disconnected_since_ms: Option<i64>,
    /// Duration in milliseconds the agent has been disconnected (0 if connected).
    pub disconnected_duration_ms: i64,
    /// Unix milliseconds of last successful control plane contact.
    pub last_successful_contact_ms: Option<i64>,
    /// Number of consecutive send/connect failures.
    pub consecutive_failures: u32,
    /// Total messages deferred due to unavailable control plane.
    pub total_deferred_messages: u64,
}

/// Tracks control plane connectivity state for partition-autonomous operation.
///
/// This struct is designed to be cheap: no allocations, no async, no locks.
/// It lives in the agent's main loop and is updated synchronously on each tick.
pub struct ConnectivityTracker {
    state: ConnectivityState,
    disconnected_since: Option<i64>,
    last_successful_contact: Option<i64>,
    consecutive_failures: u32,
    total_deferred_messages: u64,
    disconnect_threshold: u32,
}

impl ConnectivityTracker {
    /// Create a new tracker starting in the Disconnected state (no CP contact yet).
    pub fn new() -> Self {
        Self {
            state: ConnectivityState::Disconnected,
            disconnected_since: None,
            last_successful_contact: None,
            consecutive_failures: 0,
            total_deferred_messages: 0,
            disconnect_threshold: DISCONNECT_THRESHOLD,
        }
    }

    /// Create a tracker with a custom disconnect threshold (for testing).
    #[cfg(test)]
    pub fn with_threshold(threshold: u32) -> Self {
        Self {
            disconnect_threshold: threshold,
            ..Self::new()
        }
    }

    /// Record a successful control plane interaction.
    ///
    /// Transitions to Connected, resets consecutive failure counter.
    /// Emits an INFO log if transitioning from Disconnected/Reconnecting.
    pub fn record_success(&mut self, now_ms: i64) {
        let previous = self.state;
        self.last_successful_contact = Some(now_ms);
        self.consecutive_failures = 0;

        if previous != ConnectivityState::Connected {
            self.state = ConnectivityState::Connected;
            let duration = self
                .disconnected_since
                .map(|since| now_ms - since)
                .unwrap_or(0);
            info!(
                previous_state = previous.as_str(),
                disconnected_duration_ms = duration,
                "control plane connectivity restored — exiting autonomous mode"
            );
            self.disconnected_since = None;
        } else {
            debug!("control plane contact successful");
        }
    }

    /// Record a failed control plane interaction.
    ///
    /// Increments consecutive failures. After reaching the threshold,
    /// transitions to Disconnected and emits an INFO log.
    pub fn record_failure(&mut self, now_ms: i64) {
        self.consecutive_failures += 1;
        debug!(
            consecutive_failures = self.consecutive_failures,
            threshold = self.disconnect_threshold,
            "control plane contact failed"
        );

        match self.state {
            ConnectivityState::Connected => {
                if self.consecutive_failures >= self.disconnect_threshold {
                    self.state = ConnectivityState::Disconnected;
                    self.disconnected_since = Some(now_ms);
                    info!(
                        consecutive_failures = self.consecutive_failures,
                        "entering autonomous mode — control plane unreachable"
                    );
                } else {
                    self.state = ConnectivityState::Reconnecting;
                    info!(
                        consecutive_failures = self.consecutive_failures,
                        threshold = self.disconnect_threshold,
                        "control plane contact lost, attempting reconnection"
                    );
                }
            }
            ConnectivityState::Reconnecting => {
                if self.consecutive_failures >= self.disconnect_threshold {
                    self.state = ConnectivityState::Disconnected;
                    self.disconnected_since = Some(now_ms);
                    info!(
                        consecutive_failures = self.consecutive_failures,
                        "entering autonomous mode — control plane unreachable"
                    );
                }
            }
            ConnectivityState::Disconnected => {
                // Already disconnected; no state transition to log.
            }
        }
    }

    /// Record that a message was deferred because the control plane is unavailable.
    pub fn record_message_deferred(&mut self) {
        self.total_deferred_messages += 1;
        debug!(
            total_deferred = self.total_deferred_messages,
            "message deferred due to control plane unavailability"
        );
    }

    /// Current connectivity state.
    pub fn state(&self) -> ConnectivityState {
        self.state
    }

    /// How long the agent has been disconnected, in milliseconds.
    /// Returns `None` if currently connected.
    pub fn disconnected_duration_ms(&self, now_ms: i64) -> Option<i64> {
        self.disconnected_since.map(|since| now_ms - since)
    }

    /// Returns the unix timestamp (ms) of last successful contact, if any.
    pub fn last_successful_contact_ms(&self) -> Option<i64> {
        self.last_successful_contact
    }

    /// Number of consecutive failures since last success.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Total number of deferred messages since agent start.
    pub fn total_deferred_messages(&self) -> u64 {
        self.total_deferred_messages
    }

    /// Produce a metrics snapshot for Prometheus exposition.
    pub fn metrics_snapshot(&self, now_ms: i64) -> ConnectivityMetrics {
        ConnectivityMetrics {
            state: self.state,
            disconnected_since_ms: self.disconnected_since,
            disconnected_duration_ms: self.disconnected_duration_ms(now_ms).unwrap_or(0),
            last_successful_contact_ms: self.last_successful_contact,
            consecutive_failures: self.consecutive_failures,
            total_deferred_messages: self.total_deferred_messages,
        }
    }
}

impl Default for ConnectivityTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_starts_disconnected() {
        let tracker = ConnectivityTracker::new();
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);
        assert_eq!(tracker.consecutive_failures(), 0);
        assert_eq!(tracker.total_deferred_messages(), 0);
        assert_eq!(tracker.last_successful_contact_ms(), None);
    }

    #[test]
    fn record_success_transitions_to_connected() {
        let mut tracker = ConnectivityTracker::new();
        tracker.record_success(1000);
        assert_eq!(tracker.state(), ConnectivityState::Connected);
        assert_eq!(tracker.last_successful_contact_ms(), Some(1000));
        assert_eq!(tracker.consecutive_failures(), 0);
    }

    #[test]
    fn single_failure_from_connected_transitions_to_reconnecting() {
        let mut tracker = ConnectivityTracker::new();
        tracker.record_success(1000);
        assert_eq!(tracker.state(), ConnectivityState::Connected);

        tracker.record_failure(2000);
        assert_eq!(tracker.state(), ConnectivityState::Reconnecting);
        assert_eq!(tracker.consecutive_failures(), 1);
    }

    #[test]
    fn threshold_failures_transitions_to_disconnected() {
        let mut tracker = ConnectivityTracker::with_threshold(3);
        tracker.record_success(1000);

        tracker.record_failure(2000);
        assert_eq!(tracker.state(), ConnectivityState::Reconnecting);

        tracker.record_failure(3000);
        assert_eq!(tracker.state(), ConnectivityState::Reconnecting);

        tracker.record_failure(4000);
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);
        assert_eq!(tracker.consecutive_failures(), 3);
    }

    #[test]
    fn disconnected_since_tracks_partition_start() {
        let mut tracker = ConnectivityTracker::with_threshold(2);
        tracker.record_success(1000);

        tracker.record_failure(2000);
        tracker.record_failure(3000);
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);

        // disconnected_since should be the moment we crossed threshold
        assert_eq!(tracker.disconnected_duration_ms(5000), Some(2000));
    }

    #[test]
    fn success_after_disconnect_clears_state() {
        let mut tracker = ConnectivityTracker::with_threshold(2);
        tracker.record_success(1000);
        tracker.record_failure(2000);
        tracker.record_failure(3000);
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);

        tracker.record_success(5000);
        assert_eq!(tracker.state(), ConnectivityState::Connected);
        assert_eq!(tracker.disconnected_duration_ms(6000), None);
        assert_eq!(tracker.consecutive_failures(), 0);
        assert_eq!(tracker.last_successful_contact_ms(), Some(5000));
    }

    #[test]
    fn record_message_deferred_increments_counter() {
        let mut tracker = ConnectivityTracker::new();
        assert_eq!(tracker.total_deferred_messages(), 0);

        tracker.record_message_deferred();
        tracker.record_message_deferred();
        tracker.record_message_deferred();
        assert_eq!(tracker.total_deferred_messages(), 3);
    }

    #[test]
    fn metrics_snapshot_reflects_current_state() {
        let mut tracker = ConnectivityTracker::with_threshold(2);
        tracker.record_success(1000);
        tracker.record_failure(2000);
        tracker.record_failure(3000);
        tracker.record_message_deferred();
        tracker.record_message_deferred();

        let snapshot = tracker.metrics_snapshot(5000);
        assert_eq!(snapshot.state, ConnectivityState::Disconnected);
        assert_eq!(snapshot.disconnected_since_ms, Some(3000));
        assert_eq!(snapshot.disconnected_duration_ms, 2000);
        assert_eq!(snapshot.last_successful_contact_ms, Some(1000));
        assert_eq!(snapshot.consecutive_failures, 2);
        assert_eq!(snapshot.total_deferred_messages, 2);
    }

    #[test]
    fn metrics_snapshot_connected_shows_zero_duration() {
        let mut tracker = ConnectivityTracker::new();
        tracker.record_success(1000);

        let snapshot = tracker.metrics_snapshot(2000);
        assert_eq!(snapshot.state, ConnectivityState::Connected);
        assert_eq!(snapshot.disconnected_since_ms, None);
        assert_eq!(snapshot.disconnected_duration_ms, 0);
    }

    #[test]
    fn additional_failures_while_disconnected_do_not_change_state() {
        let mut tracker = ConnectivityTracker::with_threshold(2);
        tracker.record_success(1000);
        tracker.record_failure(2000);
        tracker.record_failure(3000);
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);

        // More failures should keep Disconnected, not panic or change disconnected_since
        tracker.record_failure(4000);
        tracker.record_failure(5000);
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);
        assert_eq!(tracker.consecutive_failures(), 4);
        // disconnected_since should still be the original timestamp
        assert_eq!(tracker.disconnected_duration_ms(6000), Some(3000));
    }

    #[test]
    fn default_threshold_is_three() {
        let mut tracker = ConnectivityTracker::new();
        tracker.record_success(1000);

        tracker.record_failure(2000);
        assert_eq!(tracker.state(), ConnectivityState::Reconnecting);
        tracker.record_failure(3000);
        assert_eq!(tracker.state(), ConnectivityState::Reconnecting);
        tracker.record_failure(4000);
        assert_eq!(tracker.state(), ConnectivityState::Disconnected);
    }
}
