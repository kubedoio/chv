# Prompt 03 — post-merge comprehensive review — evidence

> Parent: [plan.md](plan.md). Review executed 2026-09-30 after the prompt-03
> queue merged through `b176e023` (plan §6 status update). Method: verify
> every merged claim against live state (CI, issues, rulesets, repo
> settings), run the governance tooling end-to-end, sweep for leftover
> defects, then fix findings in a follow-up PR. Iterate until clean.

## 1. Verified clean (no action needed)

- **CI on final `main`** (`b176e023`): CI, Security, Nightly Packages all
  `success` — the combined toolchain pin + startup smoke + SHA pins + real
  cargo-audit stack works together.
- **Issue hygiene**: #229, #230, #146 closed by the merged PRs as claimed;
  #177 (quick-xml) and #235 (tonic/prost) correctly still open as tracked
  debt.
- **Action pinning**: zero unpinned third-party `uses:` in workflows or
  composite actions (grep over `@40-hex` pins).
- **Workflow permissions**: all 7 workflows carry an explicit top-level
  `permissions:` block (default `contents: read`, job-level raises where
  needed, e.g. `issues: write` for rustsec/audit-check).
- **Ruleset definition**: `protect-main` (id 17358522) carries all eight
  required checks; PR-review and signature rules present.

## 2. Findings and fixes (this review round)

| # | Finding | Severity | Fix |
|---|---------|----------|-----|
| F1 | **Tag protection did not exist at all** — `apply-tag-protection.sh` used the legacy tags/protection API (and had the `.git`-in-slug regex bug fixed elsewhere but missed here); `verify-settings.sh` checked the legacy API and warned; `BRANCH_PROTECTION.md` documented the legacy Settings→Tags UI. Unprotected `v*` tags could trigger `release.yml`. | High (unprotected release trigger) | Script rewritten for the rulesets API (`protect-tags`, target tag, `refs/tags/v*`, creation/update/deletion restricted, create-or-replace, `--enforce`/`--audit` modes, slug fixed). **Live: ruleset created and enforced** (id 24270232). Doc + verify checks updated to match. Safe to enforce immediately: no workflow creates tags — verified by grep; release.yml only reacts to tag pushes; both committers are admins. |
| F2 | **Default workflow token permissions were `write`** (repo inherited the org default; verify-settings warned). | Medium (token blast radius) | Set to `read` via `PUT /repos/kubedoio/chv/actions/permissions/workflow`. Inert for current workflows — every workflow has an explicit permissions block. |
| F3 | **Actions SHA pinning was policy-only** — `sha_pinning_required` was `false`, so nothing prevented adding a tag-pinned action later. | Medium (lock-in of workstream F) | Set `sha_pinning_required=true` via `PUT /repos/kubedoio/chv/actions/permissions`. All current uses are already SHA-pinned (F-verified), so no workflow breaks. |
| F4 | **Dependabot PR #309 (vitest 3.2.6→5.0.2) failed CI** — double-major test-framework migration mid-RC. | Medium (broken open PR / update churn) | Closed with rationale; `dependabot.yml` now ignores vitest semver-major version updates (security updates still come through); upgrade tracked as a post-RC issue. |
| F5 | Dependabot PRs #308 (`@types/node` patch) and #310 (sbom-action 0.24.2, updating the pinned SHA) were green but unreviewed. | Low | Reviewed and merged — #310 also proves the SHA-pin + Dependabot flow works end-to-end. |
| F6 | `task_plan.md` at the repo root was a stale June working plan (Designer Phase 6, merged in #128). | Low (clutter) | Archived to `docs/analysis/2026-06-13-designer-phase-6-task-plan.md`. |

## 3. Live state after this round

- `verify-settings.sh`: **all checks `[OK]`** except the single intentional
  `[WARN]` — `protect-main` enforcement staged pending universal commit
  signing (maintainer runbook in `main-protection.md` §5).
- Rulesets: `protect-main` (id 17358522, enforcement staged),
  `protect-tags` (id 24270232, **enforced**).
- Actions permissions: `sha_pinning_required=true`, default workflow token
  permissions `read`, GitHub Actions enabled.

## 4. Residual risk

Unchanged from the recorded baseline: `protect-main` enforcement remains
staged on the commit-signing prerequisite (one maintainer action:
`apply-branch-protection.sh --enforce`), and #177/#235 dependency debt stays
open with removal conditions. Nothing new was introduced by this round; all
changes are reversible (revert the PR, re-run the scripts with the previous
definitions, or flip the two Actions-permission settings back).

## 5. Review round 2 (same day, after #314 merged)

Second full pass with fresh eyes over the merged prompt-03 surface.

**Verified clean:** CI/Security/Nightly green on `5921f523` (including under
the new platform settings from round 1 — SHA-pin requirement and read-only
token default); zero unpinned third-party actions; all 7 workflows carry
explicit permissions blocks; issue hygiene correct (#177/#235/#315 open as
tracked debt, everything else closed); no Dependabot PR backlog.

| # | Finding | Severity | Fix |
|---|---------|----------|-----|
| F7 | **The controlplane's dev-build operator contract was impossible to satisfy** — `InsecureModeLockedOut` (from #233/#253) tells operators to "rebuild with: cargo build --features dev", but `cmd/chv-controlplane` had no `dev` feature: it existed only on the `chv-controlplane-service` dependency and was never forwarded, so the documented command fails. Nothing in the repo ever compiled the dev path for the controlplane. | High (operator contract broken; dev-mode guidance dead end) | `cmd/chv-controlplane` now forwards the feature (`dev = ["chv-controlplane-service/dev"]`); ci.yml gained a compile guard (`cargo check -p chv-controlplane -p chv-agent --features dev`) so the forwarding cannot silently break again; CHANGELOG entry added. Verified locally: dev-feature build with `CHV_ALLOW_INSECURE=1` passes `validate_security_mode` and `validate_tls` and proceeds into bootstrap (no lockout markers), while the production-build smoke S1/S2/S3 still passes. |
| F8 | `REPOSITORY_HARDENING.md` references and `docs/plans/*` references to the archived `task_plan.md`. | Informational | Left as-is: the hardening checklist is mechanism-agnostic and still accurate ("branch protection rule or ruleset"); the plan docs are historical records of June work and are not updated retroactively. |
| F9 | Dependabot reruns on `main` after the ignore rule landed. | None (expected) | Confirmed no new vitest-major PR was opened; backlog empty. |

No further defects found in this round: the agent gate (main.rs:586) is
placed before all runtime wiring and its enrolled-mTLS re-check (~line 774)
is unreachable in production builds (defense-in-depth, consistent with the
controlplane's interceptor-level repeat); the controlplane gate
(`peer_identity.rs::validate_security_mode`) is the same shape and is now
actually reachable in dev builds.

