# Prompt 03 — Security, Reproducibility, and Repository Governance

Close the remaining engineering-baseline gaps that can make a release non-reproducible, misconfigured, or insufficiently protected.

This prompt is a coordinated campaign, **not a requirement to put all changes in one PR**. Split independent risk domains into narrow PRs.

## Preconditions

- Execute Prompt 00.
- Revalidate #233, #229, #230, #146, #177 and any successor PRs/issues.
- Inspect `SECURITY.md`, `deny.toml`, `Cargo.lock`, CI/Security/Release workflows, CODEOWNERS, current GitHub rulesets/branch protection visible to the available tooling, and current toolchain behavior.

## Goal

Make production startup, compiler/tooling inputs, advisory policy, vulnerability reporting, review requirements, and release workflow behavior explicit and reproducible.

## Workstream A — typed fail-closed startup

Complete #233 or current equivalent.

Requirements:

- invalid production insecure-mode requests return typed startup failure;
- validation happens before listeners/interceptors/services are constructed;
- insecure mode remains compile-time development-only;
- no production runtime switch bypasses peer identity;
- process-level test proves non-zero clean exit without panic/backtrace path;
- error text is actionable and contains no secret material.

Do not weaken the security policy merely to remove the panic.

## Workstream B — pin Rust and generated-code policy

Complete #229 or current equivalent.

Requirements:

- add one authoritative `rust-toolchain.toml` with an exact reviewed toolchain;
- CI, release, KVM workflows, and contributor docs consume that baseline rather than floating independently;
- repository-owned Rust remains strict;
- generated tonic/prost output cannot break unrelated PRs solely because a new compiler introduces a lint;
- generated code is regenerated, never hand-patched;
- document a reviewable toolchain-bump procedure;
- coordinate future tonic/prost migration with MSRV and generator output.

## Workstream C — advisory-policy truth

Reconcile `deny.toml` against the actual lockfile.

Requirements:

- remove stale advisory exceptions;
- retain an exception only when the vulnerable/unmaintained dependency is still present and the repository documents threat-model rationale plus removal condition;
- #146/#177 remain explicit debt until their dependency paths are actually removed/upgraded;
- do not broaden ignores to make Security green;
- full Security workflow passes after cleanup.

## Workstream D — vulnerability reporting

Replace any placeholder security contact in `SECURITY.md` with a real maintained reporting path.

Verify GitHub private vulnerability reporting/advisory instructions are valid for the repository.

Do not publish private keys or unnecessary personal contact information.

## Workstream E — repository protection

Inspect the effective protection model for `main`.

At minimum define and, where authorized, enable/recommend:

- PR-based changes for protected paths;
- required CI/security checks appropriate to risk;
- review expectations for CODEOWNERS/high-risk infrastructure changes;
- prevention of accidental force-push/deletion;
- release-environment approval where secrets/publishing require it.

If tooling cannot read or mutate a repository setting, record that limitation and provide exact manual verification rather than claiming protection exists.

## Workstream F — supply-chain workflow pinning

Review third-party Actions used in security/release-critical workflows.

Prefer immutable commit-SHA pinning for high-risk release/security actions where practical, with comments or dependency automation preserving updateability.

Do not reduce provenance/signing functionality.

## Acceptance criteria

- invalid insecure production config exits cleanly through typed startup handling;
- exact Rust toolchain is repository-controlled;
- generated-code policy is deterministic;
- advisory exceptions match the actual dependency graph;
- Security workflow passes;
- a real vulnerability-reporting path exists;
- effective `main` protection is verified or the unverified/manual gap is explicitly recorded;
- release/security workflow dependencies have a documented pin/update policy;
- contributor instructions match CI/release behavior.

## Forbidden outcomes

- global `allow(warnings)`;
- hand-editing generated Rust;
- keeping stale RustSec ignores "just in case";
- claiming branch protection from CODEOWNERS files alone;
- using a panic as normal operator-configuration validation;
- weakening mTLS or peer identity;
- bundling dependency-major migrations unrelated to the gate.

## Exit gate

Prompt 03 passes when the exact candidate engineering baseline can be rebuilt and reviewed deterministically, invalid security configuration fails safely, advisory policy is truthful, and repository/release governance is verified rather than assumed.
