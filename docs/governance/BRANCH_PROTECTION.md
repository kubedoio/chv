# Branch and Tag Protection

This document describes the required GitHub repository settings for the CHV project. These settings **cannot be expressed as files in the repository**; they must be configured via the GitHub UI, the `gh` CLI, or the rulesets API.

> **Status (2026-09-30, prompt 03 workstream E):** the `protect-main` **ruleset**
> (id 17358522) exists and matches the specification below, including all eight
> required status checks. **Enforcement is staged, not active**: the ruleset
> includes `required_signatures`, and recent `main` history mixes signed and
> unsigned commits — enabling enforcement before commit signing is universal
> for all committers would block legitimate PRs. Once signing is set up, run
> `./scripts/github-setup/apply-branch-protection.sh --enforce`.

---

## Branch Ruleset: `protect-main`

Applied to the default branch (`main`) via **Settings → Rules → Rulesets** (the
mechanism actually in use — the legacy branch-protection API is not).

| Rule | Value | Rationale |
|---------|-------|-----------|
| **Restrict deletions** | ✅ Enabled | Prevents accidental branch deletion |
| **Block force pushes** | ✅ Enabled (non-fast-forward) | Prevents history rewriting |
| **Require signed commits** | ✅ Enabled | Cryptographic provenance for every commit |
| **Require a pull request before merging** | ✅ Enabled | No direct pushes to `main` |
| **Required approvals** | `1` minimum | At least one human review |
| **Dismiss stale PR approvals when new commits are pushed** | ✅ Enabled | Prevents approval hijacking |
| **Require review from CODEOWNERS** | ✅ Enabled | Enforces the ownership model in `.github/CODEOWNERS` |
| **Require conversation resolution before merging** | ✅ Enabled | Ensures all review threads are addressed |
| **Extra approval for unattributed changes** | ✅ Enabled | Changes pushed by someone other than the PR author need a second look |
| **Require status checks to pass** | ✅ Enabled | CI and Security must be green |
| **Status checks that are required** | `Rust checks`, `UI checks`, `E2E tests`, `cargo audit`, `cargo deny (advisories)`, `cargo deny (bans)`, `cargo deny (licenses)`, `cargo deny (sources)` | Gates from `.github/workflows/ci.yml` and `.github/workflows/security.yml` |
| **Allowed merge methods** | merge, squash, rebase | Matches current conventions |

> **Note:** every required check must report on **every** PR. For that reason
> `security.yml` has no `pull_request` path filter — a path-filtered workflow
> leaves its checks unreported ("Expected") on PRs outside the filter and
> blocks their merge once the check is required.

---

## Tag Protection Rule

Apply via **Settings → Tags → Add rule**.

| Setting | Value | Rationale |
|---------|-------|-----------|
| **Tag name pattern** | `v*` | Protects all version tags |
| **Restrict creations** | ✅ Enabled | Only maintainers/admins can create version tags |

This prevents accidental or malicious tag creation that could trigger the release workflow (`release.yml`).

---

## Automated Application

Run the helper scripts (requires `gh` CLI and repo admin access):

```bash
# Update the protect-main ruleset definition (keeps current enforcement state)
./scripts/github-setup/apply-branch-protection.sh

# Activate enforcement (gated on commit-signing readiness — see the status note above)
./scripts/github-setup/apply-branch-protection.sh --enforce

# Print the live ruleset without changing anything
./scripts/github-setup/apply-branch-protection.sh --audit

# Apply tag protection
./scripts/github-setup/apply-tag-protection.sh

# Verify current settings
./scripts/github-setup/verify-settings.sh
```

See the script source for the exact API payloads.

---

## Related

- [`.github/CODEOWNERS`](../../.github/CODEOWNERS) — ownership mapping
- [`REPOSITORY_HARDENING.md`](./REPOSITORY_HARDENING.md) — full hardening checklist
