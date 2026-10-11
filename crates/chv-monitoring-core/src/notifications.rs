//! The notification envelope (query/alerts contract v1, campaign
//! #602, prompt 05 / PR-6): the exact, closed field set every
//! outbound notification carries.
//!
//! This lives in the shared core crate because BOTH the evaluator
//! side (enqueuing pre-rendered events into the outbox, via
//! `chv-controlplane-service`) and the BFF (the authorized delivery
//! test) must render byte-identical payloads — one definition, no
//! drift.
//!
//! Redaction is structural: the type is closed. A payload built from
//! these fields can never carry tenant secrets, agent claims,
//! command lines, plugin output or raw SQL — anything beyond the
//! contract's ten fields.

use serde::Serialize;

/// Envelope schema version (contract `chv-monitoring-query-alerts-v1`).
const SCHEMA_VERSION: i64 = 1;

/// The contract's versioned webhook envelope. Serialization order is
/// the declaration order, which matches the contract example exactly;
/// the field set is closed, which is the structural redaction
/// guarantee (nothing beyond these fields can ever be emitted).
#[derive(Serialize)]
pub struct NotificationEnvelope<'a> {
    pub schema_version: i64,
    pub event_id: &'a str,
    pub incident_id: &'a str,
    pub event_type: &'a str,
    pub severity: &'a str,
    pub target_kind: &'a str,
    pub target_id: &'a str,
    pub summary: &'a str,
    pub occurred_at_ms: i64,
    pub resource_url: &'a str,
}

/// Render the contract's versioned JSON webhook envelope
/// (`schema_version` 1) as compact JSON.
#[allow(clippy::too_many_arguments)] // the contract fixes the field set
pub fn render_envelope(
    event_id: &str,
    incident_id: &str,
    event_type: &str,
    severity: &str,
    target_kind: &str,
    target_id: &str,
    summary: &str,
    occurred_at_ms: i64,
    resource_url: &str,
) -> String {
    let envelope = NotificationEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id,
        incident_id,
        event_type,
        severity,
        target_kind,
        target_id,
        summary,
        occurred_at_ms,
        resource_url,
    };
    // All fields are strings or integers, so serialization cannot
    // fail (no map keys, no NaN, no invalid UTF-8 in `&str`).
    serde_json::to_string(&envelope).expect("envelope serialization is infallible")
}
