//! Generic circuit breaker for service calls.
//!
//! Implements the standard circuit breaker pattern with three states:
//! - Closed: requests flow through normally
//! - Open: requests are immediately rejected
//! - HalfOpen: a limited number of probe requests are allowed through
//!
//! This module provides a general-purpose circuit breaker that can wrap any
//! async operation, complementing the per-method circuit breaker in `node_client`.

use chv_errors::ChvError;
use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// The three states of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Circuit is tripped — requests are rejected immediately.
    Open,
    /// Testing recovery — limited requests allowed through.
    HalfOpen,
}

/// A generic circuit breaker for wrapping service calls.
///
/// Thread-safe via internal `Mutex`. Use `with_circuit_breaker` for ergonomic
/// wrapping of async operations.
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count_half_open: u32,
    failure_threshold: u32,
    recovery_timeout: Duration,
    last_failure: Option<Instant>,
    half_open_max_calls: u32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given parameters.
    ///
    /// - `failure_threshold`: number of consecutive failures before opening
    /// - `recovery_timeout`: how long to stay open before transitioning to half-open
    /// - `half_open_max_calls`: number of successful calls in half-open to close the circuit
    pub fn new(
        failure_threshold: u32,
        recovery_timeout: Duration,
        half_open_max_calls: u32,
    ) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count_half_open: 0,
            failure_threshold,
            recovery_timeout,
            last_failure: None,
            half_open_max_calls,
        }
    }

    /// Create a circuit breaker with sensible defaults.
    ///
    /// Defaults: 5 failures to trip, 30s recovery, 3 successful half-open calls to close.
    pub fn with_defaults() -> Self {
        Self::new(5, Duration::from_secs(30), 3)
    }

    /// Check whether a request can be executed.
    ///
    /// Returns `true` if the circuit is closed or half-open (allowing probe requests).
    /// Returns `false` if the circuit is open.
    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if recovery timeout has elapsed
                if let Some(last_failure) = self.last_failure {
                    if last_failure.elapsed() >= self.recovery_timeout {
                        debug!("circuit breaker transitioning from Open to HalfOpen");
                        self.state = CircuitState::HalfOpen;
                        self.success_count_half_open = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    // No last failure recorded, transition to half-open
                    self.state = CircuitState::HalfOpen;
                    self.success_count_half_open = 0;
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call.
    ///
    /// In half-open state, accumulates successes until threshold is met,
    /// then transitions back to closed.
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count_half_open += 1;
                if self.success_count_half_open >= self.half_open_max_calls {
                    info!("circuit breaker closing after successful half-open probes");
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count_half_open = 0;
                    self.last_failure = None;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count = 0;
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
            }
        }
    }

    /// Record a failed call.
    ///
    /// Increments the failure counter. If threshold is exceeded, opens the circuit.
    /// In half-open state, immediately re-opens the circuit.
    pub fn record_failure(&mut self) {
        self.last_failure = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    warn!(
                        failure_count = self.failure_count,
                        threshold = self.failure_threshold,
                        "circuit breaker opening due to failure threshold"
                    );
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                warn!("circuit breaker re-opening from half-open after failure");
                self.state = CircuitState::Open;
                self.success_count_half_open = 0;
            }
            CircuitState::Open => {
                // Already open, just update last_failure time
            }
        }
    }

    /// Get the current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Reset the circuit breaker to closed state.
    pub fn reset(&mut self) {
        info!("circuit breaker manually reset to Closed");
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count_half_open = 0;
        self.last_failure = None;
    }
}

/// Execute an async operation through a circuit breaker.
///
/// Returns `Err(ChvError::BackendUnavailable)` if the circuit is open.
/// Otherwise executes the future and records success/failure accordingly.
pub async fn with_circuit_breaker<F, T, E>(
    cb: &Mutex<CircuitBreaker>,
    service_name: &str,
    f: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: From<ChvError>,
{
    // Check if we can execute
    {
        let mut breaker = cb.lock().unwrap_or_else(|e| e.into_inner());
        if !breaker.can_execute() {
            return Err(E::from(ChvError::BackendUnavailable {
                backend: service_name.to_string(),
                reason: "circuit breaker is open".to_string(),
            }));
        }
    }

    // Execute the operation
    let result = f.await;

    // Record outcome
    {
        let mut breaker = cb.lock().unwrap_or_else(|e| e.into_inner());
        match &result {
            Ok(_) => breaker.record_success(),
            Err(_) => breaker.record_failure(),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_circuit_breaker_is_closed() {
        let cb = CircuitBreaker::with_defaults();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_can_execute_when_closed() {
        let mut cb = CircuitBreaker::with_defaults();
        assert!(cb.can_execute());
    }

    #[test]
    fn test_opens_after_threshold() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(30), 2);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_cannot_execute_when_open() {
        let mut cb = CircuitBreaker::new(2, Duration::from_secs(30), 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());
    }

    #[test]
    fn test_transitions_to_half_open_after_timeout() {
        let mut cb = CircuitBreaker::new(2, Duration::from_millis(1), 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(5));

        assert!(cb.can_execute());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_half_open_closes_on_success() {
        let mut cb = CircuitBreaker::new(2, Duration::from_millis(1), 2);
        cb.record_failure();
        cb.record_failure();

        std::thread::sleep(Duration::from_millis(5));
        assert!(cb.can_execute()); // transitions to HalfOpen

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen); // need 2 successes
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_reopens_on_failure() {
        let mut cb = CircuitBreaker::new(2, Duration::from_millis(1), 2);
        cb.record_failure();
        cb.record_failure();

        std::thread::sleep(Duration::from_millis(5));
        assert!(cb.can_execute()); // transitions to HalfOpen

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(30), 1);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        // Failure count should be reset
        cb.record_failure();
        cb.record_failure();
        // Still closed because success reset the counter
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_manual_reset() {
        let mut cb = CircuitBreaker::new(2, Duration::from_secs(30), 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());
    }

    #[tokio::test]
    async fn test_with_circuit_breaker_success() {
        let cb = Mutex::new(CircuitBreaker::with_defaults());
        let result: Result<i32, ChvError> =
            with_circuit_breaker(&cb, "test", async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_circuit_breaker_rejects_when_open() {
        let cb = Mutex::new(CircuitBreaker::new(1, Duration::from_secs(60), 1));

        // Trip the breaker
        {
            let mut breaker = cb.lock().unwrap();
            breaker.record_failure();
        }

        let result: Result<i32, ChvError> =
            with_circuit_breaker(&cb, "test-svc", async { Ok(42) }).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ChvError::BackendUnavailable { backend, .. } => {
                assert_eq!(backend, "test-svc");
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
