//! Architecture Designer validator.
//!
//! Three things ship from this crate:
//!
//! 1. [`model`] — strongly-typed `CHVArchitecture` matching the
//!    `chv.kubedo.io/v1alpha1` YAML contract.
//! 2. [`schema`] — JSON-Schema validation against the embedded
//!    `chvarchitecture-v1alpha1.schema.json`.
//! 3. [`static_checks`] — graph-shape checks that the schema cannot express
//!    (cross-reference existence, CIDR overlap, IP scope, secret leakage,
//!    permission allowlist, …).
//!
//! [`validate`] is the entry point. It runs schema validation first and
//! short-circuits on schema failure (running graph-shape checks against a
//! malformed model produces noise). When schema is clean it parses the
//! model and runs every static check.
//!
//! Findings are emitted as
//! [`chv_controlplane_types::architecture::Finding`] so the BFF and the
//! eventual fleet-check / plan layers all share one diagnostic shape.

pub mod codes;
pub mod model;
pub mod parse;
pub mod schema;
pub mod static_checks;

use chv_controlplane_types::architecture::{Finding, Severity};
use serde::{Deserialize, Serialize};

pub use model::CHVArchitecture;
pub use parse::{parse_yaml, to_yaml, ParseError, EXPECTED_API_VERSION, EXPECTED_KIND};
pub use schema::validate_against_schema;
pub use static_checks::run_static_checks;

/// Validation result envelope mirroring the
/// `validation-plan-contract.md` JSON shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub status: ValidationStatusKind,
    pub summary: ValidationSummary,
    pub findings: Vec<Finding>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatusKind {
    Valid,
    Warning,
    Invalid,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
}

impl ValidationSummary {
    pub fn from_findings(findings: &[Finding]) -> Self {
        let mut s = Self::default();
        for f in findings {
            match f.severity {
                Severity::Error => s.errors += 1,
                Severity::Warning => s.warnings += 1,
                Severity::Info => s.info += 1,
            }
        }
        s
    }
}

impl ValidationStatusKind {
    pub fn from_summary(summary: &ValidationSummary) -> Self {
        if summary.errors > 0 {
            Self::Invalid
        } else if summary.warnings > 0 {
            Self::Warning
        } else {
            Self::Valid
        }
    }
}

/// End-to-end YAML validation. Schema first, static checks second; static
/// checks are skipped when schema validation produced any error because
/// graph-shape checks against a structurally invalid model produce noise.
pub fn validate(yaml: &str) -> ValidationResult {
    let schema_findings = validate_against_schema(yaml);
    if !schema_findings.is_empty() {
        let summary = ValidationSummary::from_findings(&schema_findings);
        return ValidationResult {
            status: ValidationStatusKind::from_summary(&summary),
            summary,
            findings: schema_findings,
        };
    }

    // Schema clean — parse and run static checks. A failure here is itself a
    // schema-shaped error (we already passed schema validation, so this is a
    // surprising state); surface it as a single SCHEMA_INVALID rather than
    // panicking.
    let model = match parse_yaml(yaml) {
        Ok(m) => m,
        Err(err) => {
            let f = Finding {
                severity: Severity::Error,
                code: std::borrow::Cow::Borrowed(codes::SCHEMA_INVALID),
                message: format!("model parse failed after schema passed: {err}"),
                path: None,
                resource_ref: None,
                blocking: true,
                suggestion: None,
            };
            let findings = vec![f];
            let summary = ValidationSummary::from_findings(&findings);
            return ValidationResult {
                status: ValidationStatusKind::from_summary(&summary),
                summary,
                findings,
            };
        }
    };

    let findings = run_static_checks(&model);
    let summary = ValidationSummary::from_findings(&findings);
    ValidationResult {
        status: ValidationStatusKind::from_summary(&summary),
        summary,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_minimal_returns_valid() {
        let yaml = r#"
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: t1
"#;
        let r = validate(yaml);
        assert_eq!(r.status, ValidationStatusKind::Valid);
        assert_eq!(r.summary.errors, 0);
    }

    #[test]
    fn schema_invalid_returns_invalid_and_skips_static() {
        let yaml = r#"
apiVersion: chv.kubedo.io/v999
kind: CHVArchitecture
metadata:
  name: t1
"#;
        let r = validate(yaml);
        assert_eq!(r.status, ValidationStatusKind::Invalid);
        assert!(r.findings.iter().all(|f| f.code.as_ref() == codes::SCHEMA_INVALID));
    }

    #[test]
    fn summary_status_table() {
        let s = ValidationSummary {
            errors: 0,
            warnings: 0,
            info: 0,
        };
        assert_eq!(ValidationStatusKind::from_summary(&s), ValidationStatusKind::Valid);
        let s = ValidationSummary {
            errors: 0,
            warnings: 1,
            info: 0,
        };
        assert_eq!(ValidationStatusKind::from_summary(&s), ValidationStatusKind::Warning);
        let s = ValidationSummary {
            errors: 1,
            warnings: 0,
            info: 0,
        };
        assert_eq!(ValidationStatusKind::from_summary(&s), ValidationStatusKind::Invalid);
    }
}
