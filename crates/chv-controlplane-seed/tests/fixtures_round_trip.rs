//! YAML round-trip test for every starter fixture.
//!
//! For each of the six starters:
//!
//! 1. `parse_yaml(include_str!(...))` succeeds.
//! 2. `run_static_checks(&model)` returns zero error-severity findings.
//! 3. `to_yaml(&model)` produces a string that re-parses to a structurally
//!    equal `CHVArchitecture` (`assert_eq!`). This guards against schema
//!    drift: a future change to the model that breaks emission would also
//!    break the seeder.
//!
//! Running this against `STARTER_FIXTURES` (rather than reading files by
//! path) means the test exercises the same `include_str!` payload the
//! seeder ships with, not whatever happens to be on disk.

use chv_architecture_validate::{parse_yaml, run_static_checks, to_yaml};
use chv_controlplane_seed::STARTER_FIXTURES;
use chv_controlplane_types::architecture::Severity;

#[test]
fn every_starter_round_trips() {
    for fx in STARTER_FIXTURES {
        let model_a =
            parse_yaml(fx.yaml).unwrap_or_else(|err| panic!("starter '{}' parse: {err}", fx.name));

        let findings = run_static_checks(&model_a);
        let errors: Vec<_> = findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "starter '{}' has {} error-severity finding(s): {:#?}",
            fx.name,
            errors.len(),
            errors
        );

        let emitted =
            to_yaml(&model_a).unwrap_or_else(|err| panic!("starter '{}' to_yaml: {err}", fx.name));

        let model_b = parse_yaml(&emitted)
            .unwrap_or_else(|err| panic!("starter '{}' re-parse: {err}\n---\n{emitted}", fx.name));

        assert_eq!(
            model_a, model_b,
            "starter '{}' round-trip drift; emitted YAML follows:\n{emitted}",
            fx.name
        );
    }
}
