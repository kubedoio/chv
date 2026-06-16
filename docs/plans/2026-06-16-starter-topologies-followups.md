# Starter-topologies follow-ups

Tracked from the Stage A+B+C reviewer pass on PR #132. None of these block ship; all are quality-of-life or coverage-tightening for a future PR.

## 1. Clone double-click race

**Reviewer:** test-analyzer (Valuable #5)

The Stage C clone button does not pin single-flight behavior. A user
double-clicking quickly could fire two `/v1/architectures/create` calls;
the BFF accepts both with distinct short-id suffixes, producing two clones.

**Recommended fix:** disable the clone button while a clone is in flight,
add a Playwright test that clicks twice rapidly and asserts
`createSpy.length === 1`.

**Why deferred:** the clone surface is small and the double-clone outcome
is benign (operator deletes the duplicate). Worth fixing; not urgent.

## 2. Service-level integration test for starter seed

**Reviewer:** test-analyzer (Important #3)

Plan §6 cited
`cargo test -p chv-controlplane-service --test starter_seed_integration`
as an acceptance gate. The crate-level idempotency tests already cover the
seeder's contract end-to-end, but a service-level test that boots
`build_service`, queries `/v1/architectures/list`, and asserts six rows
would close the operator-facing loop one layer up.

**Why deferred:** would require either (a) a new dev-dep edge from
`chv-controlplane-service` to a TopologyRepository helper, or (b) a
service-level harness that doesn't exist today. Either way, scope-creep
beyond the reviewer pass for PR #132.

## 3. Round-trip emit-stability + on-disk vs embedded fixture audit

**Reviewer:** test-analyzer (existing-issue #2 — partially closed by PR #132's
`fixture_yaml_emitter_is_stable` test). The follow-up bit is a per-fixture
**byte-equality** assertion against a checked-in canonical file, which
would let CI catch a silent emitter-version bump in `serde_yaml`.

**Why deferred:** today's `to_yaml(parse_yaml(yaml)) == to_yaml(parse_yaml(to_yaml(...)))`
test catches non-fixed-point emitters but not version bumps that
canonically reorder maps. A `cargo insta`-style snapshot test would close
this; cost ≈ 1 hour, not in scope for #132.

## 4. SeedError → ControlPlaneServiceError dedicated variant

**Reviewer:** language-specialist (N2)

Today `bootstrap.rs` stringifies `SeedError` into
`ControlPlaneServiceError::Internal`. A dedicated `Seed(SeedError)` variant
preserves the structure for ops dashboards.

**Why deferred:** the `tracing::error!(?err, ...)` line added in PR #132
already structurally logs the error to operators before the conversion;
the variant addition is a minor follow-up.

## 5. Documented opt-out / re-seed procedures in OPERATIONS.md

**Reviewer:** docs review

The plan documents the opt-out / re-seed procedures (set sentinel before
boot; re-flip and restart) but they live only in
`docs/plans/2026-06-16-starter-topologies-and-auto-seed.md`. A short
operator-facing section in `docs/OPERATIONS.md` under "Architecture
Designer day-2 operations" would be discoverable from the documented
operator runbook.

**Why deferred:** docs-only; better to wait until the implementation lands
on `main` so the OPERATIONS.md change references a real merged commit.

## 6. SemVer bump for the embedded YAML schema

The 6 starter YAMLs are versioned by the implementation `chv-controlplane-seed`
crate version. If the embedded `chv_architecture_validate::model::CHVArchitecture`
adds a new required field, every starter has to be updated atomically. A
release-note checklist item ("when bumping the architecture model schema,
re-run `cargo test -p chv-controlplane-seed --test fixtures_round_trip` and
update fixtures if it fails") prevents drift.

**Why deferred:** procedural / release-engineering, not a code change.
