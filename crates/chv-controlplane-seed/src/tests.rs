//! Unit tests for crate-internal helpers.
//!
//! Repository-touching tests live in `tests/seed_idempotency.rs`;
//! round-trip tests in `tests/fixtures_round_trip.rs`. This file covers
//! the pure-data contract: slug shape, fixture count, name/slug coupling.

use crate::starters::STARTER_FIXTURES;

#[test]
fn starter_fixtures_has_six_entries() {
    // The plan pins the starter set at exactly six. Adding or removing
    // entries is a public-contract change; this test exists to make that
    // intent explicit and surface accidental drift.
    assert_eq!(
        STARTER_FIXTURES.len(),
        6,
        "STARTER_FIXTURES must contain exactly six starters"
    );
}

#[test]
fn starter_slugs_are_kebab_case() {
    for fx in STARTER_FIXTURES {
        let slug = fx.slug;
        assert!(!slug.is_empty(), "slug must be non-empty");
        assert!(
            slug.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "slug '{slug}' must be kebab-case (lowercase letters, digits, hyphens)"
        );
        assert!(
            !slug.starts_with('-') && !slug.ends_with('-'),
            "slug '{slug}' must not start or end with '-'"
        );
        assert!(
            !slug.contains("--"),
            "slug '{slug}' must not contain consecutive hyphens"
        );
    }
}

#[test]
fn starter_names_have_starter_prefix_with_matching_slug() {
    for fx in STARTER_FIXTURES {
        let expected = format!("starter-{}", fx.slug);
        assert_eq!(
            fx.name, expected,
            "fixture name '{}' must match slug-derived form '{}'",
            fx.name, expected
        );
    }
}

#[test]
fn starter_environments_avoid_production() {
    // Per the plan: no starter ships as `production` so no operator can
    // accidentally apply one against a production environment guard.
    for fx in STARTER_FIXTURES {
        assert_ne!(
            fx.environment, "production",
            "starter '{}' must not be tagged production",
            fx.name
        );
    }
}

#[test]
fn starter_yaml_metadata_matches_name() {
    // The fixture's embedded YAML must declare the same name we record on
    // the row, otherwise the seeded `name` column and the `latest_yaml`
    // body disagree. Parsing via the architecture validator (rather than a
    // free-form YAML reader) doubles as a smoke test that every fixture is
    // a valid CHVArchitecture document at the schema level.
    for fx in STARTER_FIXTURES {
        let model = chv_architecture_validate::parse_yaml(fx.yaml)
            .unwrap_or_else(|err| panic!("fixture {} yaml parse: {err}", fx.name));
        assert_eq!(
            model.metadata.name, fx.name,
            "fixture {} metadata.name drift",
            fx.name
        );
    }
}
