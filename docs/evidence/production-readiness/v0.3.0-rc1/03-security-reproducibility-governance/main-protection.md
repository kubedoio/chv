# Prompt 03 Workstream E — `main` protection — evidence

> Parent: [plan.md](plan.md) §2-E. Baseline: `main` `df256bc4` (2026-09-30);
> executed after the prompt-03 PR queue merged through `ef4188d6`.

## 1. Findings (before)

- The `protect-main` ruleset (id `17358522`, created 2026-06-06) was **fully
  defined but `enforcement: disabled`** — every rule (deletion, non-fast-forward,
  required signatures, PR reviews with code-owner review, required status
  checks) existed on paper only. The legacy branch-protection API reported
  `main` unprotected.
- The ruleset's required checks (`Rust checks`, `UI checks`, `E2E tests`)
  matched the current `ci.yml` job names — but the **Security workflow's
  checks were not required**.
- The in-repo governance tooling predated the ruleset:
  `apply-branch-protection.sh` used the legacy API with only two required
  checks (`Rust checks`, `UI checks`); `verify-settings.sh` verified the
  legacy API (which 404s when only a ruleset exists → false errors);
  `BRANCH_PROTECTION.md` described a "Rulesets (Recommended Alternative)"
  rather than the mechanism actually in use, and its note claimed `E2E tests`
  was intentionally omitted — no longer true of the live ruleset.
- `security.yml` had a `pull_request` **path filter** (Cargo/deny.toml/workflow
  files only). A path-filtered workflow does not report its checks on PRs
  outside the filter, which would leave required Security checks stuck in
  "Expected" and block those PRs' merges once enforced.

## 2. Commit-signing readiness (the enforcement blocker)

The ruleset includes `required_signatures`. Live check on 2026-09-30
(`GET /repos/kubedoio/chv/commits?per_page=6`):

```text
df256bc4 verified=false reason=unsigned
b95b90fb verified=true  reason=valid
89924321 verified=true  reason=valid
932d59c1 verified=false reason=unsigned
707a1b55 verified=true  reason=valid
1dadd8c8 verified=false reason=unsigned
```

Signing on `main` is **mixed**. Maintainer decision (2026-09-30): merge the
prompt-03 PR queue first (unsigned commits are fine while enforcement is
off), set up commit signing for all committers, then enable enforcement.
Additionally `require_code_owner_review` + 1 approval means a PR author
cannot self-approve — the second CODEOWNERS maintainer must be available to
review.

## 3. Changes (this PR)

1. **`security.yml`**: the `pull_request` path filter is removed so the
   Security checks report on every PR (required-check-safe). Cost: the full
   matrix runs on every PR (~1 minute, parallel jobs).
2. **`apply-branch-protection.sh`** rewritten for the rulesets API:
   - default: idempotently updates the `protect-main` **definition** — adding
     the five Security checks to the required list — while preserving the
     current enforcement state;
   - `--enforce`: sets enforcement `active`, but **refuses** if any of the
     last 10 `main` commits are unsigned (the signing prerequisite);
   - `--audit`: prints the live ruleset without changing anything.
3. **`verify-settings.sh`**: the live-settings section now verifies the
   ruleset (existence, PR-review rule, signature rule, each of the eight
   required checks) instead of the legacy API; a disabled enforcement state is
   reported as a warning with the staging rationale.
4. **`BRANCH_PROTECTION.md`**: rewritten to the ruleset reality — the actual
   rules and all eight required checks, the staged-enforcement status note,
   the path-filter caveat, and the three script modes.

## 4. Live state after this PR

- `apply-branch-protection.sh` (default mode, no `--enforce`) was run on
  2026-09-30: the disabled ruleset's definition now includes all eight
  required checks — an inert change while enforcement is off, keeping the
  live definition identical to the governance-as-code.
- **Enforcement remains disabled.** This is an explicitly recorded gap, not a
  claim: `main` is not yet protected against force-push/deletion/review
  bypass. The activation procedure is one command
  (`apply-branch-protection.sh --enforce`) once commit signing is universal.

## 5. Activation runbook (once signing is set up)

1. Every committer configures signed commits (GitHub → Settings → SSH signing
   keys or GPG keys; `git config commit.gpgsign true` / `gpg.format ssh`).
2. Verify: `gh api repos/kubedoio/chv/commits?per_page=10` shows
   `verified=true` for recent history, and open PRs are rebased/signed.
3. `./scripts/github-setup/apply-branch-protection.sh --enforce` (its built-in
   unsigned-commit check must pass).
4. `./scripts/github-setup/verify-settings.sh` → enforcement `[OK]`.
5. Record the activation here with the date and the verifying API response.

## 6. Residual risk

Until enforcement is active, `main` relies on maintainer discipline only —
the same state the repository has had since the ruleset was created. The gap
is recorded here and in `BRANCH_PROTECTION.md`'s status note; it closes with
the runbook above.
