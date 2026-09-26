# CHV Production Readiness Prompt Pack

This prompt pack turns the current CHV technical-preview state into a bounded campaign for the first credible production candidate.

It does **not** replace the CellHV Core prompt pack. The Core prompts define the architecture and migration direction; this pack starts from the repository's current state and closes the remaining safety, authority, reproducibility, qualification, release, and field-evidence gaps.

## Objective

Move CHV from a feature-rich technical preview to a release candidate whose claims are backed by reproducible evidence on real KVM hosts.

The campaign deliberately prioritizes **trustworthiness over feature growth**.

Do not add NetBox projection, VMware migration, another VMM, another storage backend, broad UI features, Kubernetes integration, or unrelated product expansion while this pack is active unless a blocker proves one is required.

## Source-of-truth rule

Do not trust roadmap status labels without checking current code and current GitHub state.

At the beginning of every prompt:

1. sync to current `main`;
2. inspect relevant current code, issues, PRs, workflows, and specs;
3. state what is already implemented;
4. state what is only documented;
5. state what has real-host evidence;
6. update the prompt's implementation plan if current evidence has changed.

A merged PR is not automatically qualification evidence. A unit test is not KVM evidence. A historical KVM run is not release qualification for a later commit.

## Capability maturity vocabulary

Use these states instead of a single ambiguous "complete" flag:

- **CODED** — implementation exists.
- **CI-VERIFIED** — repository CI/tests prove the intended contract at non-privileged tiers.
- **KVM-VERIFIED** — required behavior passes on a real KVM/Cloud Hypervisor host.
- **MULTI-HOST-VERIFIED** — cross-host behavior passes on the supported topology.
- **FIELD-QUALIFIED** — the capability has run in a reference deployment with operational evidence.
- **RELEASED** — the exact qualified commit is distributed through the supported release channel.

Never promote a capability to a higher state using lower-tier evidence.

## Active execution order

1. [`00-execution-policy.md`](00-execution-policy.md)
2. [`01-network-isolation.md`](01-network-isolation.md)
3. [`02-single-authority-cutover.md`](02-single-authority-cutover.md)
4. [`03-security-reproducibility-governance.md`](03-security-reproducibility-governance.md)
5. [`04-real-host-qualification.md`](04-real-host-qualification.md)
6. [`05-release-candidate.md`](05-release-candidate.md)
7. [`06-reference-deployment.md`](06-reference-deployment.md)

The top-level campaign goal is in [`GOAL.md`](GOAL.md).

## Current risk anchors

Revalidate these before execution; do not assume their state remains unchanged:

- #227 — CHV firewall policy must not impose host-wide default-drop semantics.
- #231 / #185 — legacy lifecycle compatibility must converge on one durable CellHV Core authority.
- #233 — invalid insecure-mode configuration must fail through typed startup validation, not panic.
- #229 — Rust compiler and generated-code lint behavior must be reproducible.
- #230, #146, #177 — advisory policy must reflect the actual dependency graph and documented risk decisions.
- storage migration mTLS wiring may be merged, but real two-stord identity/negative-path qualification is a separate evidence gate.
- release machinery may exist without any published GitHub Release; publication itself must be proven.
- repository rules and security contact information must be verified, not inferred from files.

## Mandatory workflow

- Work on dedicated branches and PRs; do not implement campaign slices directly on `main`.
- Keep safety/authority changes narrow enough to review independently.
- Split a prompt into multiple PRs when one PR would mix unrelated risk domains.
- Every PR includes: scope, non-scope, invariants, rollback, tests, evidence, and residual risk.
- Re-run current repository CI/security/package gates before declaring a slice complete.
- High-risk changes require real-host evidence at the tier stated by the prompt.
- Preserve evidence artifacts or durable evidence documents tied to exact commit SHAs.
- Do not rewrite working architecture merely to make a prompt easier to satisfy.

## Completion rule

This prompt pack is complete only when:

- the supported VM lifecycle has exactly one durable runtime authority;
- CHV network policy cannot mutate unrelated host traffic;
- production configuration fails closed without panic-based operator paths;
- compiler, dependency-policy, and release inputs are reproducible;
- the exact candidate passes real KVM and multi-host qualification;
- an actual RC is published and installed from its public artifacts;
- a clean reference deployment runs the released artifacts and produces field evidence;
- the support matrix states limitations explicitly;
- no deferred product programme was pulled into the release merely to make the release look broader.
