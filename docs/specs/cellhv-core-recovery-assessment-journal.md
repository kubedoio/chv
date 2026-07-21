# CellHV Core Recovery Assessment Journal

**Status:** Phase C assessment-only library; production-unwired
**Authority:** `chv-agent` remains the sole VM lifecycle authority

## Purpose

`cellhv-core-recovery` maps one bounded runtime-ownership classification into
an append-only recovery assessment persisted by
`cellhv-core-operations`. This records what was observed after restart without
turning evidence into permission to act.

The mapper depends only on `cellhv-core-operations`,
`cellhv-core-runtime-ownership`, and `serde_json`. It does not depend on the
executor, Cloud Hypervisor runtime, command binary, providers, API transport,
or control plane. `thiserror` supplies only the closed mapper error type. No
production crate may consume the recovery crate in this slice.

## Invariant

Every operation that was `Running` at restart remains `InspectRequired` before
and after assessment. Appending an assessment must not:

- change operation status or result;
- claim, finish, supersede, retry, or replace an execution attempt;
- mint, reveal, rotate, or validate an execution token;
- signal, adopt, stop, launch, or otherwise control a VM process;
- connect to, unlink, replace, or mutate a VM API socket;
- authorize executor admission.

An ownership classification, including `OwnershipMatched`, is evidence only.
It is never a control capability or a terminal recovery decision.

## Mapping Boundary

The public mapper is:

```rust
assessment_for(
    classification: Classification,
    expected_assessment_revision: u64,
    evidence: serde_json::Value,
) -> Result<RecoveryAssessment, RecoveryMappingError>
```

It performs a closed mapping from every runtime-ownership classification to a
typed assessment kind. `expected_assessment_revision` binds the later append to the latest
recovery-assessment revision observed by the caller; a mismatch must fail rather
than append against a changed assessment stream. The active attempt token
separately fences the operation being assessed. The operations/store layer owns assessment identity,
ordering, evidence fingerprinting, revision validation, and the append
transaction. The mapper does not compute a journal identity or write storage
directly.

Evidence must be a bounded JSON object before append. Assessment rows are
immutable and append-only; distinct observations produce additional ordered
evidence rather than overwriting history, while an exact retry replays its
existing row and event. Persistence failure returns an error
and cannot be treated as a successful assessment.

Reopen validation retains token correlation without exposing the token: an
assessment must belong either to a running operation with the matching active
attempt or to a terminal operation with the matching completed attempt.
Accepted operations and token mismatches fail closed. This permits later
token-fenced terminalization to preserve assessment history without relaxing
the assessment-only behavior of this slice.

## Deferred Decisions

This slice deliberately provides no transition from `InspectRequired`. A later
reviewed recovery-resolution protocol must define token-fenced state changes,
identity-bound control capabilities, ambiguity policy, and crash/replay
semantics before any retry, adoption, or terminalization is allowed.

T0 dependency guards and T1 mapper/store tests are evidence for assessment
recording only. They are not T2 process recovery, T3 real-KVM recovery, or
permission to change production VM launch and management behavior.
