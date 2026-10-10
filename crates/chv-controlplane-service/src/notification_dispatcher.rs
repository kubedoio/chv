//! Notification dispatcher pure core (ADR-027, prompt 05 PR-6).
//!
//! This module holds the total, side-effect-free half of the
//! notification dispatcher described in
//! `docs/specs/contracts/chv-monitoring-query-alerts-v1.md`
//! ("Notifications and integrations") and
//! `docs/specs/component/chv-monitoring-history-alerts-spec.md`
//! ("Notification integration"):
//!
//! * [`sign_payload`] — HMAC-SHA256 webhook signatures
//!   (`v1=<64 lowercase hex>`), verifying the operator-configured
//!   secret without ever placing it in the payload.
//! * [`backoff_delay`] — capped exponential backoff between delivery
//!   attempts (no jitter; the worker layers seeded jitter on top).
//! * [`classify`] — HTTP outcome to delivery class (2xx delivered;
//!   network errors/timeouts, 5xx and 429 retryable; other 4xx
//!   permanent → dead-letter).
//! * [`render_envelope`] — the contract's versioned JSON webhook
//!   envelope (`schema_version` 1). Redaction here is structural: the
//!   payload is built from a fixed field set and can never carry
//!   tenant secrets, agent claims, command lines, plugin output or
//!   raw SQL.
//! * [`render_slack`] — the Slack incoming-webhook `{"text": "..."}`
//!   single-line adapter.
//!
//! The async worker (claim/sign/POST/mark/retry/dead-letter loop over
//! the durable `notification_outbox`) lives elsewhere in this crate
//! and composes these primitives.

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::fmt::Write as _;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

/// Envelope schema version (contract `chv-monitoring-query-alerts-v1`).
const SCHEMA_VERSION: i64 = 1;

/// First backoff step: 5 seconds.
const BACKOFF_BASE_SECS: u64 = 5;

/// Backoff ceiling: 1 hour.
const BACKOFF_CAP_SECS: u64 = 60 * 60;

/// The contract's versioned webhook envelope. Serialization order is
/// the declaration order, which matches the contract example exactly;
/// the field set is closed, which is the structural redaction
/// guarantee (nothing beyond these fields can ever be emitted).
#[derive(Serialize)]
struct Envelope<'a> {
    schema_version: i64,
    event_id: &'a str,
    incident_id: &'a str,
    event_type: &'a str,
    severity: &'a str,
    target_kind: &'a str,
    target_id: &'a str,
    summary: &'a str,
    occurred_at_ms: i64,
    resource_url: &'a str,
}

/// Slack incoming-webhook payload: a single `text` field, nothing else.
#[derive(Serialize)]
struct SlackText {
    text: String,
}

/// Sign a webhook body with the operator-configured secret.
///
/// Returns `v1=<hmac-sha256 of body keyed by secret, 64 lowercase hex
/// digits>`. The secret is never part of the output. Total over all
/// inputs (HMAC accepts keys of any length).
pub fn sign_payload(secret: &[u8], body: &[u8]) -> String {
    // `new_from_slice` is infallible for HMAC (any key length is
    // valid), so the error branch is unreachable by construction.
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("v1={hex}")
}

/// Exponential backoff between delivery attempts, with no jitter (the
/// worker applies seeded jitter on top of this value).
///
/// `attempts` is the attempt number about to be scheduled, `>= 1` by
/// contract: step *n* waits `5s * 2^(attempts-1)`, capped at 1 hour.
/// `attempts == 0` saturates to the first step so the function stays
/// total; very large values saturate at the cap.
pub fn backoff_delay(attempts: u32) -> Duration {
    // Cap the shift so the intermediate value always fits; 5s << 64
    // already exceeds the 1h ceiling by many orders of magnitude.
    let exponent = attempts.saturating_sub(1).min(64);
    let secs = ((BACKOFF_BASE_SECS as u128) << exponent).min(BACKOFF_CAP_SECS as u128) as u64;
    Duration::from_secs(secs)
}

/// What the dispatcher should do after a delivery attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryClass {
    /// 2xx — mark delivered.
    Delivered,
    /// Network error/timeout, 5xx or 429 — schedule a retry.
    Retryable,
    /// Any other 4xx — dead-letter immediately.
    Permanent,
}

/// Classify a delivery attempt outcome.
///
/// `None` means the request never completed (connect failure, timeout,
/// TLS error) and is retryable. 2xx is delivered; 5xx and 429 are
/// retryable; every other 4xx is permanent. 1xx/3xx never occur on the
/// production path (the client follows no redirects), and are treated
/// as retryable anomalies rather than permanent failures.
pub fn classify(status: Option<u16>) -> DeliveryClass {
    match status {
        Some(code) if (200..300).contains(&code) => DeliveryClass::Delivered,
        // 429 is a 4xx, so it must be matched before the permanent arm.
        Some(429) | None => DeliveryClass::Retryable,
        Some(code) if (400..500).contains(&code) => DeliveryClass::Permanent,
        Some(_) => DeliveryClass::Retryable,
    }
}

/// Render the contract's versioned webhook envelope as compact JSON.
///
/// Field order matches the contract example (`schema_version` first,
/// `resource_url` last). Redaction is structural: only the ten
/// contract fields exist, so no caller data beyond these parameters
/// can leak into the payload. Callers validate inputs; this function
/// only renders.
// The flat parameter list is fixed by the PR-6 interface contract;
// grouping them would drift the shared signature.
#[allow(clippy::too_many_arguments)]
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
    let envelope = Envelope {
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

/// Render the Slack incoming-webhook payload: a single human-readable
/// line wrapped as `{"text": "..."}`.
///
/// The line is `[<event_type>] <summary> (<severity>) —
/// <resource_url>`, with the resource URL rendered relative (e.g.
/// `/vms/<id>`). No other Slack blocks or fields are emitted.
pub fn render_slack(event_type: &str, severity: &str, summary: &str, resource_url: &str) -> String {
    let text = format!("[{event_type}] {summary} ({severity}) — {resource_url}");
    serde_json::to_string(&SlackText { text }).expect("slack payload serialization is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use sha2::Digest;

    /// Independent HMAC-SHA256 built directly on sha2 (RFC 2104
    /// ipad/opad construction), so the vectors below are not simply
    /// re-deriving `sign_payload` through the same `hmac` code path.
    fn hmac_sha256_reference(key: &[u8], message: &[u8]) -> [u8; 32] {
        const BLOCK: usize = 64;
        let mut key_block = [0u8; BLOCK];
        if key.len() > BLOCK {
            let hashed = Sha256::digest(key);
            key_block[..hashed.len()].copy_from_slice(&hashed);
        } else {
            key_block[..key.len()].copy_from_slice(key);
        }
        let mut ipad = [0x36u8; BLOCK];
        let mut opad = [0x5cu8; BLOCK];
        for i in 0..BLOCK {
            ipad[i] ^= key_block[i];
            opad[i] ^= key_block[i];
        }
        let mut inner = Sha256::new();
        inner.update(ipad);
        inner.update(message);
        let inner_digest = inner.finalize();
        let mut outer = Sha256::new();
        outer.update(opad);
        outer.update(inner_digest);
        let digest = outer.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn hex_string(bytes: &[u8]) -> String {
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(hex, "{byte:02x}");
        }
        hex
    }

    #[test]
    fn sign_payload_matches_rfc4231_vector() {
        // RFC 4231 test case 2 (verified independently of this crate).
        assert_eq!(
            sign_payload(b"Jefe", b"what do ya want for nothing?"),
            "v1=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn sign_payload_matches_reference_implementation() {
        for (secret, body) in [
            (&b"Jefe"[..], &b"what do ya want for nothing?"[..]),
            (&b""[..], &b""[..]),
            (&b"0123456789abcdef"[..], &b"\x00\x01\x02\xff"[..]),
            (&b"sixteen-byte-key!"[..], &b"repeat repeat repeat"[..]),
            // Key longer than the 64-byte block (must be hashed down).
            (&[0x42u8; 100][..], &b"long key"[..]),
        ] {
            let expected = hex_string(&hmac_sha256_reference(secret, body));
            assert_eq!(sign_payload(secret, body), format!("v1={expected}"));
        }
    }

    #[test]
    fn sign_payload_shape_is_v1_lowercase_hex_64() {
        let signed = sign_payload(b"any-secret", b"any-body");
        assert!(signed.starts_with("v1="));
        let hex_part = &signed[3..];
        assert_eq!(hex_part.len(), 64);
        assert!(
            hex_part
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "signature must be lowercase hex, got {hex_part}"
        );
        // Same inputs must be deterministic; different body must not.
        assert_eq!(signed, sign_payload(b"any-secret", b"any-body"));
        assert_ne!(signed, sign_payload(b"any-secret", b"any-body-2"));
        assert_ne!(signed, sign_payload(b"other-secret", b"any-body"));
    }

    #[test]
    fn backoff_delay_follows_capped_exponential_sequence() {
        assert_eq!(backoff_delay(1), Duration::from_secs(5));
        assert_eq!(backoff_delay(2), Duration::from_secs(10));
        assert_eq!(backoff_delay(3), Duration::from_secs(20));
        assert_eq!(backoff_delay(4), Duration::from_secs(40));
        assert_eq!(backoff_delay(5), Duration::from_secs(80));
        assert_eq!(backoff_delay(6), Duration::from_secs(160));
        assert_eq!(backoff_delay(7), Duration::from_secs(320));
        assert_eq!(backoff_delay(8), Duration::from_secs(640));
        assert_eq!(backoff_delay(9), Duration::from_secs(1280));
        assert_eq!(backoff_delay(10), Duration::from_secs(2560));
    }

    #[test]
    fn backoff_delay_caps_at_one_hour() {
        // 5s * 2^10 = 5120s would exceed the 1h cap.
        assert_eq!(backoff_delay(11), Duration::from_secs(3600));
        assert_eq!(backoff_delay(20), Duration::from_secs(3600));
        assert_eq!(backoff_delay(u32::MAX), Duration::from_secs(3600));
        // Monotone non-decreasing across the whole range.
        let mut previous = Duration::ZERO;
        for attempts in 1..=40u32 {
            let delay = backoff_delay(attempts);
            assert!(delay >= previous, "delay decreased at attempt {attempts}");
            previous = delay;
        }
    }

    #[test]
    fn backoff_delay_is_total_below_one() {
        // Contract says attempts >= 1; 0 must not panic and saturates
        // to the first step.
        assert_eq!(backoff_delay(0), Duration::from_secs(5));
    }

    #[test]
    fn classify_delivered_for_2xx() {
        assert_eq!(classify(Some(200)), DeliveryClass::Delivered);
        assert_eq!(classify(Some(201)), DeliveryClass::Delivered);
        assert_eq!(classify(Some(204)), DeliveryClass::Delivered);
        assert_eq!(classify(Some(299)), DeliveryClass::Delivered);
    }

    #[test]
    fn classify_permanent_for_other_4xx() {
        assert_eq!(classify(Some(400)), DeliveryClass::Permanent);
        assert_eq!(classify(Some(401)), DeliveryClass::Permanent);
        assert_eq!(classify(Some(403)), DeliveryClass::Permanent);
        assert_eq!(classify(Some(404)), DeliveryClass::Permanent);
        assert_eq!(classify(Some(410)), DeliveryClass::Permanent);
        assert_eq!(classify(Some(499)), DeliveryClass::Permanent);
    }

    #[test]
    fn classify_retryable_for_none_5xx_and_429() {
        assert_eq!(classify(None), DeliveryClass::Retryable);
        assert_eq!(classify(Some(429)), DeliveryClass::Retryable);
        assert_eq!(classify(Some(500)), DeliveryClass::Retryable);
        assert_eq!(classify(Some(502)), DeliveryClass::Retryable);
        assert_eq!(classify(Some(503)), DeliveryClass::Retryable);
        assert_eq!(classify(Some(599)), DeliveryClass::Retryable);
    }

    #[test]
    fn classify_retryable_for_non_2xx_non_4xx_anomalies() {
        // 1xx/3xx cannot occur on the production path (no redirects);
        // they are treated as transient, never permanent.
        assert_eq!(classify(Some(100)), DeliveryClass::Retryable);
        assert_eq!(classify(Some(301)), DeliveryClass::Retryable);
        assert_eq!(classify(Some(304)), DeliveryClass::Retryable);
    }

    #[test]
    fn render_envelope_matches_contract_example_exactly() {
        // Values from the contract example in
        // docs/specs/contracts/chv-monitoring-query-alerts-v1.md.
        let rendered = render_envelope(
            "d0b2a2f7-2a77-4f86-ae55-53f0c8bf4aac",
            "f4528846-4925-435e-a69a-a4a147501527",
            "firing",
            "warning",
            "vm",
            "83dab870-4903-48a9-9d37-486e100ed009",
            "Guest filesystem nearly full",
            1791576000000,
            "/vms/83dab870-4903-48a9-9d37-486e100ed009",
        );
        // Exact compact string, field order per the contract example.
        let expected = concat!(
            r#"{"schema_version":1,"#,
            r#""event_id":"d0b2a2f7-2a77-4f86-ae55-53f0c8bf4aac","#,
            r#""incident_id":"f4528846-4925-435e-a69a-a4a147501527","#,
            r#""event_type":"firing","#,
            r#""severity":"warning","#,
            r#""target_kind":"vm","#,
            r#""target_id":"83dab870-4903-48a9-9d37-486e100ed009","#,
            r#""summary":"Guest filesystem nearly full","#,
            r#""occurred_at_ms":1791576000000,"#,
            r#""resource_url":"/vms/83dab870-4903-48a9-9d37-486e100ed009"}"#
        );
        assert_eq!(rendered, expected);

        // Same conclusion via parsed Values (shape equality).
        let parsed: Value = serde_json::from_str(&rendered).expect("envelope must be valid JSON");
        let expected_value = serde_json::json!({
            "schema_version": 1,
            "event_id": "d0b2a2f7-2a77-4f86-ae55-53f0c8bf4aac",
            "incident_id": "f4528846-4925-435e-a69a-a4a147501527",
            "event_type": "firing",
            "severity": "warning",
            "target_kind": "vm",
            "target_id": "83dab870-4903-48a9-9d37-486e100ed009",
            "summary": "Guest filesystem nearly full",
            "occurred_at_ms": 1791576000000i64,
            "resource_url": "/vms/83dab870-4903-48a9-9d37-486e100ed009"
        });
        assert_eq!(parsed, expected_value);
    }

    #[test]
    fn render_envelope_field_order_and_closed_field_set() {
        let rendered = render_envelope(
            "event",
            "incident",
            "resolved",
            "critical",
            "node",
            "node-1",
            "Node unreachable",
            -1,
            "/nodes/node-1",
        );
        // Field order must follow the contract example.
        let expected_order = [
            "schema_version",
            "event_id",
            "incident_id",
            "event_type",
            "severity",
            "target_kind",
            "target_id",
            "summary",
            "occurred_at_ms",
            "resource_url",
        ];
        let positions: Vec<Option<usize>> = expected_order
            .iter()
            .map(|field| rendered.find(&format!("\"{field}\":")))
            .collect();
        assert!(
            positions.iter().all(Option::is_some),
            "missing field in {rendered}"
        );
        let mut sorted = positions.clone();
        sorted.sort();
        assert_eq!(
            positions, sorted,
            "fields out of contract order: {rendered}"
        );

        // Structural redaction: exactly the ten contract fields.
        let parsed: Value = serde_json::from_str(&rendered).expect("must be valid JSON");
        let object = parsed.as_object().expect("envelope must be an object");
        assert_eq!(object.len(), 10);
        // Negative timestamps pass through untouched (callers validate).
        assert_eq!(object["occurred_at_ms"], serde_json::json!(-1));
    }

    #[test]
    fn render_envelope_escapes_untrusted_strings() {
        // Summary is rule-operator text; it must be JSON-escaped, never
        // able to break out of the envelope or inject fields.
        let rendered = render_envelope(
            "e",
            "i",
            "firing",
            "warning",
            "vm",
            "vm-1",
            "quote \" backslash \\ newline\n brace } ctrl\u{1}",
            0,
            "/vms/vm-1",
        );
        let parsed: Value = serde_json::from_str(&rendered).expect("must stay valid JSON");
        let object = parsed.as_object().expect("must stay an object");
        assert_eq!(object.len(), 10);
        assert_eq!(
            object["summary"],
            serde_json::json!("quote \" backslash \\ newline\n brace } ctrl\u{1}")
        );
    }

    #[test]
    fn render_slack_is_single_text_line() {
        let rendered = render_slack(
            "firing",
            "warning",
            "Guest filesystem nearly full",
            "/vms/83dab870-4903-48a9-9d37-486e100ed009",
        );
        assert_eq!(
            rendered,
            "{\"text\":\"[firing] Guest filesystem nearly full (warning) — \
             /vms/83dab870-4903-48a9-9d37-486e100ed009\"}"
        );
        let parsed: Value = serde_json::from_str(&rendered).expect("slack payload must be JSON");
        let object = parsed.as_object().expect("slack payload must be an object");
        // No blocks, no fields beyond text.
        assert_eq!(object.len(), 1);
        assert!(object.contains_key("text"));
        let text = object["text"].as_str().expect("text must be a string");
        assert_eq!(
            text,
            "[firing] Guest filesystem nearly full (warning) — \
             /vms/83dab870-4903-48a9-9d37-486e100ed009"
        );
        assert!(!text.contains('\n'), "must be a single line");
    }

    #[test]
    fn render_slack_escapes_untrusted_strings() {
        let rendered = render_slack("test", "info", "say \"hi\" </b>", "/nodes/n1");
        let parsed: Value = serde_json::from_str(&rendered).expect("must stay valid JSON");
        let object = parsed.as_object().expect("must stay an object");
        assert_eq!(object.len(), 1);
        assert_eq!(
            object["text"],
            serde_json::json!("[test] say \"hi\" </b> (info) — /nodes/n1")
        );
    }
}
