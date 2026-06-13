//! Verifies the canonical example file in `docs/examples/` validates clean.

#[test]
fn canonical_example_validates_clean() {
    let yaml = include_str!("../../../docs/examples/chvarchitecture-example.yaml");
    let result = chv_architecture_validate::validate(yaml);
    assert_eq!(
        result.summary.errors, 0,
        "canonical example should validate without errors; findings: {:#?}",
        result.findings
    );
    // Warnings are allowed (the example doesn't currently emit any, but
    // future diagnostics may add advisory checks). We assert only the
    // status to remain forward-compatible.
    assert_ne!(
        result.status,
        chv_architecture_validate::ValidationStatusKind::Invalid,
        "canonical example should not be Invalid"
    );
}

#[test]
fn round_trip_canonical_example() {
    let yaml = include_str!("../../../docs/examples/chvarchitecture-example.yaml");
    let model = chv_architecture_validate::parse_yaml(yaml).expect("parse");
    let emitted = chv_architecture_validate::to_yaml(&model).expect("emit");
    let model2 = chv_architecture_validate::parse_yaml(&emitted).expect("re-parse");
    assert_eq!(model, model2);
}
