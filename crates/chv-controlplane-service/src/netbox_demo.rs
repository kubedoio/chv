//! Plain-HTTP NetBox demo path — **double-gated, never shipped**
//! (ADR-024 decision 5, issue kubedoio/chv#586).
//!
//! ⚠️ **HIGH-RISK SURFACE — READ BEFORE TOUCHING** ⚠️
//!
//! The production NetBox client is HTTPS-only, fail-closed at
//! construction: [`chv_netbox_adapter::NetBoxClient::new`] rejects any
//! non-`https` endpoint, because the component spec forbids sending the
//! `Authorization: Token …` header over an unencrypted transport. This
//! module is the **only** sanctioned exception, and it exists solely so
//! the `make netbox-demo` harness (issue #586, PR 4) can point the
//! controlplane at the local `chv-netbox-sim` simulator, which serves
//! plain HTTP on loopback.
//!
//! The exception is gated **twice**, because either gate alone is a
//! realistic accident (ADR-024 rationale):
//!
//! 1. **Compile-time gate** — this entire module only exists when the
//!    default-off `netbox-demo` cargo feature is enabled (it forwards to
//!    the adapter's `test-http` feature, home of
//!    [`NetBoxClient::new_unchecked_for_tests`]). Release packaging
//!    builds default features only, so a shipped binary cannot contain
//!    this code path.
//! 2. **Runtime gate** — even in a feature-enabled build, the factory
//!    checks `CHV_NETBOX_ALLOW_HTTP == "1"` on every invocation and
//!    returns a fail-closed error otherwise. A feature accidentally
//!    enabled, or this env var leaking into a real deployment, is not
//!    enough on its own.
//!
//! Default-feature builds are byte-identical in behavior: the module is
//! not compiled, the worker keeps [`NetBoxClient::new`] as its factory,
//! and plain HTTP stays impossible. Any change to the gate conditions
//! (feature name, env var name, factory seam) is a high-risk change and
//! must re-verify the fail-closed default path (see
//! `netbox_projection_worker_tests.rs`'s HTTPS-only pin).
//!
//! This module is the recorded high-risk-change disclosure for the demo
//! gate per `CONTRIBUTING.md`; ADR-024
//! (`docs/specs/adr/024-netbox-test-doubles-and-demo-gate.md`) is its
//! permanent home.

use std::sync::Arc;

use chv_netbox_adapter::{ClientError, NetBoxClient, NetBoxToken};

use crate::netbox_projection_worker::ClientFactory;

/// The runtime half of the double gate: plain HTTP is allowed only when
/// this env var is set to exactly `1`.
const ALLOW_HTTP_ENV: &str = "CHV_NETBOX_ALLOW_HTTP";

/// Build the demo-mode NetBox [`ClientFactory`]: a closure that
/// constructs clients via the adapter's test-only plain-HTTP
/// constructor — but only when the runtime gate
/// (`CHV_NETBOX_ALLOW_HTTP=1`) is also open.
///
/// Behavior, in order:
///
/// 1. `CHV_NETBOX_ALLOW_HTTP` unset or not `"1"` →
///    [`Err(ClientError::HttpsRequired)`] **fail closed**, with an
///    error message naming both gates so an operator who enabled only
///    one of them gets an actionable message instead of a silent
///    downgrade.
/// 2. Gate open → [`NetBoxClient::new_unchecked_for_tests`], which
///    accepts `http://` endpoints (the simulator's loopback listener).
///
/// The env var is read inside the closure, i.e. at client-construction
/// time on every projection run — not once at startup — so a demo
/// process cannot cache an early gate decision.
pub fn plain_http_client_factory() -> ClientFactory {
    Arc::new(|endpoint: &str, token: NetBoxToken| {
        if std::env::var(ALLOW_HTTP_ENV).ok().as_deref() != Some("1") {
            return Err(ClientError::HttpsRequired {
                endpoint: endpoint.to_string(),
            });
        }
        NetBoxClient::new_unchecked_for_tests(endpoint, token)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes env manipulation across the tests in this module (the
    /// gate is process-global state). std-only, no extra dev-dep.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Removes the gate env var and returns the previous value so the
    /// caller can restore it.
    fn unset_gate() -> Option<String> {
        let previous = std::env::var(ALLOW_HTTP_ENV).ok();
        std::env::remove_var(ALLOW_HTTP_ENV);
        previous
    }

    #[test]
    fn gate_closed_fails_closed_and_gate_open_allows_http() {
        // Both gate polarities run sequentially in ONE test (plus the
        // mutex) so parallel tests can never observe a half-mutated env.
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let factory = plain_http_client_factory();
        let token = NetBoxToken::new("demo-token".to_string());

        // --- Gate closed: env var absent → fail closed, both gates named.
        let previous = unset_gate();
        let error = factory("http://127.0.0.1:8080", token.clone())
            .expect_err("factory must fail closed without the env gate");
        assert!(
            matches!(error, ClientError::HttpsRequired { .. }),
            "expected HttpsRequired, got: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("NETBOX_HTTPS_REQUIRED"),
            "error must carry the stable code: {message}"
        );
        assert!(
            message.contains("http://127.0.0.1:8080"),
            "error must name the endpoint: {message}"
        );

        // --- Gate closed: env var present but not "1" → still fail closed.
        std::env::set_var(ALLOW_HTTP_ENV, "0");
        assert!(
            factory("http://127.0.0.1:8080", token.clone()).is_err(),
            "CHV_NETBOX_ALLOW_HTTP=0 must not open the gate"
        );

        // --- Gate open: http:// endpoint constructs through the
        //     test-only unchecked constructor.
        std::env::set_var(ALLOW_HTTP_ENV, "1");
        let client = factory("http://127.0.0.1:8080", token.clone())
            .expect("both gates open must construct the plain-HTTP client");
        assert!(
            format!("{client:?}").contains("http://127.0.0.1:8080"),
            "client must be pointed at the http endpoint: {client:?}"
        );

        // Restore whatever the environment had before the test.
        match previous {
            Some(value) => std::env::set_var(ALLOW_HTTP_ENV, value),
            None => std::env::remove_var(ALLOW_HTTP_ENV),
        }
    }

    #[test]
    fn gate_open_still_accepts_https_endpoints() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var(ALLOW_HTTP_ENV).ok();
        std::env::set_var(ALLOW_HTTP_ENV, "1");

        let factory = plain_http_client_factory();
        let token = NetBoxToken::new("demo-token".to_string());
        // The demo factory must not *require* http — an https endpoint
        // keeps working (the unchecked constructor only relaxes the
        // scheme check, it does not force plain HTTP).
        assert!(
            factory("https://netbox.example.internal", token).is_ok(),
            "https endpoints must still construct in demo mode"
        );

        match previous {
            Some(value) => std::env::set_var(ALLOW_HTTP_ENV, value),
            None => std::env::remove_var(ALLOW_HTTP_ENV),
        }
    }
}
