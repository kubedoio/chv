# Branch and Tag Protection

This document describes the required GitHub repository settings for the CHV project. These settings **cannot be expressed as files in the repository**; they must be configured via the GitHub UI or the `gh` CLI.

> **Status:** These rules are the intended configuration. They must be applied by a repository admin.

---

## Branch Protection Rule: `main`

Apply to the default branch (`main`) via **Settings → Branches → Add rule**.

| Setting | Value | Rationale |
|---------|-------|-----------|
| **Branch name pattern** | `main` | Protects the default branch |
| **Require a pull request before merging** | ✅ Enabled | No direct pushes to `main` |
| **Require approvals** | `1` minimum | At least one human review |
| **Dismiss stale PR approvals when new commits are pushed** | ✅ Enabled | Prevents approval hijacking |
| **Require review from CODEOWNERS** | ✅ Enabled | Enforces the ownership model in `.github/CODEOWNERS` |
| **Require status checks to pass** | ✅ Enabled | CI must be green |
| **Status checks that are required** | `Rust checks`, `UI checks` | Gates from `.github/workflows/ci.yml` |
| **Require branches to be up to date before merging** | ✅ Enabled (recommended) | Prevents merge skew |
| **Require conversation resolution before merging** | ✅ Enabled | Ensures all review threads are addressed |
| **Require signed commits** | ✅ Enabled | Cryptographic provenance for every commit |
| **Include administrators** | ✅ Enabled | Admins follow the same rules |
| **Allow force pushes** | ❌ Disabled | Prevents history rewriting |
| **Allow deletions** | ❌ Disabled | Prevents accidental branch deletion |

> **Note:** The `E2E tests` job is intentionally omitted from required checks. It has a 15-minute timeout and depends on the UI job; making it required would slow down merges without adding material safety beyond the build checks.

---

## Tag Protection Rule

Apply via **Settings → Tags → Add rule**.

| Setting | Value | Rationale |
|---------|-------|-----------|
| **Tag name pattern** | `v*` | Protects all version tags |
| **Restrict creations** | ✅ Enabled | Only maintainers/admins can create version tags |

This prevents accidental or malicious tag creation that could trigger the release workflow (`release.yml`).

---

## Rulesets (Recommended Alternative)

If the repository has access to GitHub Rulesets (preferred over legacy branch protection):

Create a **Branch ruleset** named `Protect main`:
- Targets: `main`
- Restrict deletions: ✅
- Require signed commits: ✅
- Require pull request: ✅ (1 approval, dismiss stale, CODEOWNERS, resolve conversations)
- Require status checks: ✅ (`Rust checks`, `UI checks`)
- Block force pushes: ✅
- Require merge queue (optional): enables batched merges for high-velocity periods

Create a **Tag ruleset** named `Protect version tags`:
- Targets: `v*`
- Restrict creations: ✅ (roles: admin, maintain)
- Restrict updates: ✅
- Restrict deletions: ✅

---

## Automated Application

Run the helper script (requires `gh` CLI and repo admin access):

```bash
# Apply branch protection
./scripts/github-setup/apply-branch-protection.sh

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
