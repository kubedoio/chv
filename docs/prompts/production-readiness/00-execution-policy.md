# Prompt 00 — Production Readiness Execution Policy

You are executing the CHV production-readiness campaign in `kubedoio/chv`.

Do not implement feature expansion while executing this prompt. Establish the current truth and the evidence frame that every later prompt must use.

## Mandatory reading

Read:

- `docs/prompts/production-readiness/README.md`
- `docs/prompts/production-readiness/GOAL.md`
- `docs/prompts/cellhv-core/00-execution-policy.md`
- current `README.md`
- `docs/ARCHITECTURE.md`
- `PHASED_IMPLEMENTATION_PLAN.md`
- `docs/GAP_ANALYSIS.md`
- `docs/production-readiness-report.md` if present
- `SECURITY.md`
- current CI, Security, KVM integration, package, and release workflows
- current open issues and PRs relevant to #227, #231, #233, #229, #230 and any successor/follow-up work.

Inspect the current code paths; do not infer current behavior solely from documents.

## Goal

Create an exact implementation declaration for the campaign based on current `main`, and identify contradictions between code, roadmap claims, issue state, and available evidence.

## Required work

### 1. Freeze the evidence frame

Record:

- exact `main` SHA;
- `VERSION`;
- relevant open/merged PRs;
- relevant issue states;
- CI/security status for the exact SHA;
- latest real-KVM evidence SHA/date;
- whether a GitHub Release exists for the current line;
- whether the release artifacts referenced by install docs actually exist.

### 2. Reconcile capability claims

For lifecycle, network, storage, migration, backup/restore, packaging, release, and Designer:

- identify current implementation;
- identify current automated tests;
- identify real-host evidence;
- assign one maturity state from the prompt-pack vocabulary;
- flag stale/contradictory documents.

Do not "fix" stale documentation by blindly choosing the most optimistic claim. Use code + executable evidence.

### 3. Define the release boundary

Write the candidate scope explicitly.

Prefer the smallest supportable product:

- Cloud Hypervisor only;
- current CellHV Core runtime in `chv-agent`;
- only storage/network paths that can be qualified in the available lab;
- existing control-plane/UI required for the reference deployment;
- no deferred integration presented as supported.

### 4. Evidence location

Create or select a durable evidence root such as:

```text
docs/evidence/production-readiness/<candidate-or-date>/
```

Every later prompt must tie evidence to exact commit SHAs and environment identity.

### 5. Required implementation declaration

Before coding later prompts, maintain:

```markdown
## CHV production-readiness declaration

- Baseline main SHA:
- Candidate release line:
- Supported host OS/profile:
- VMM:
- Network profile:
- Storage profile(s):
- Lifecycle authority:
- Current maturity by capability:
- Current blockers:
- Required real-host topology:
- Release artifact path:
- Explicit non-scope:
- Evidence root:
- Residual unknowns:
```

## Stop conditions

Stop and report rather than guessing when:

- current code contradicts the issue description materially;
- the claimed supported path has no executable implementation;
- required KVM/multi-host infrastructure is unavailable;
- a change would create a second runtime authority;
- a fix would weaken mTLS, ownership, idempotency, or isolation;
- success requires inventing unsupported product behavior.

## Exit gate

Prompt 00 passes when the campaign has one exact baseline, one bounded release scope, one evidence hierarchy, and a documented list of current blockers grounded in current code rather than historical status labels.
