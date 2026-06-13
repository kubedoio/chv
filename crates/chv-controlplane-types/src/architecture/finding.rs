//! Validation and fleet-check finding type.
//!
//! Matches the validation contract in
//! `docs/specs/architecture-designer/contracts/validation-plan-contract.md`
//! exactly: a single Finding describes one diagnostic produced by the YAML
//! validator, the fleet-consistency checker, or any other gate.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Severity bucket for a [`Finding`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// One diagnostic produced by validation or fleet-consistency checking.
///
/// `code` is `Cow<'static, str>` so the validator can emit string literal
/// codes (e.g. `"CHV-001"`) zero-cost, while the type still round-trips
/// through serde (deserializing into an owned `String`). The plan called
/// for `&'static str`, but pure `&'static` is incompatible with
/// `Deserialize` for runtime-parsed input — Cow is the standard idiom for
/// "static when emitted, owned when parsed".
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: Cow<'static, str>,
    pub message: String,
    pub path: Option<String>,
    pub resource_ref: Option<String>,
    /// True when this finding alone must block apply.
    pub blocking: bool,
    pub suggestion: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_serializes_snake_case() {
        let json = serde_json::to_string(&Severity::Warning).unwrap();
        assert_eq!(json, "\"warning\"");
    }

    #[test]
    fn severity_round_trip_all() {
        for s in [Severity::Info, Severity::Warning, Severity::Error] {
            let json = serde_json::to_string(&s).unwrap();
            let back: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn finding_round_trip() {
        let f = Finding {
            severity: Severity::Error,
            code: Cow::Borrowed("CHV-101"),
            message: "instance app-01 references unknown template small-linux".to_string(),
            path: Some("instances[0].template".to_string()),
            resource_ref: Some("instance/app-01".to_string()),
            blocking: true,
            suggestion: Some("define a template named small-linux".to_string()),
        };

        let json = serde_json::to_string(&f).unwrap();
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn finding_round_trip_minimal() {
        let f = Finding {
            severity: Severity::Info,
            code: Cow::Borrowed("CHV-001"),
            message: "ok".to_string(),
            path: None,
            resource_ref: None,
            blocking: false,
            suggestion: None,
        };

        let json = serde_json::to_string(&f).unwrap();
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }
}
