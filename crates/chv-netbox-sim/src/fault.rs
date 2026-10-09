//! Error injection (`POST /__faults`).

use serde::{Deserialize, Serialize};

/// One injected fault configuration. Set globally (`kind: null`) or
/// for a single kind; a per-kind entry **replaces** the global
/// configuration for that kind (an all-false per-kind entry is
/// therefore a per-kind opt-out of a global fault).
///
/// Latency composes with the terminal faults (it is applied first);
/// at most one terminal fault applies, in the order
/// `connection_drop` → `auth_failure` → `rate_limit` → `server_error`.
///
/// Faults apply only to the six NetBox endpoint families — never to
/// the `__`-prefixed control plane, so an injected fault can always
/// be cleared again.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultConfig {
    /// Force `401 {"detail": "Invalid token"}` for every request,
    /// regardless of the presented token.
    #[serde(default)]
    pub auth_failure: bool,
    /// Force `429` with NetBox's throttle body shape.
    #[serde(default)]
    pub rate_limit: bool,
    /// Force a 5xx response with the given status (500–599).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_error: Option<u16>,
    /// Delay every response by this many milliseconds.
    #[serde(default)]
    pub latency_ms: u64,
    /// Abort the response mid-transmission: the response begins and
    /// is then cut off, so clients observe a transport/body-read
    /// failure, never a well-formed response.
    #[serde(default)]
    pub connection_drop: bool,
}

impl FaultConfig {
    /// Whether any fault is configured at all.
    pub fn is_active(&self) -> bool {
        self.auth_failure
            || self.rate_limit
            || self.server_error.is_some()
            || self.latency_ms > 0
            || self.connection_drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_by_default() {
        assert!(!FaultConfig::default().is_active());
        assert!(FaultConfig {
            latency_ms: 1,
            ..FaultConfig::default()
        }
        .is_active());
        assert!(FaultConfig {
            server_error: Some(503),
            ..FaultConfig::default()
        }
        .is_active());
    }

    #[test]
    fn serializes_with_stable_shape() {
        let fault = FaultConfig {
            rate_limit: true,
            ..FaultConfig::default()
        };
        assert_eq!(
            serde_json::to_string(&fault).expect("serializes"),
            r#"{"auth_failure":false,"rate_limit":true,"latency_ms":0,"connection_drop":false}"#
        );
    }
}
