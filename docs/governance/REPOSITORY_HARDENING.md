# Repository Hardening Checklist

This checklist covers repository-level protections for the CHV project. Items marked **(file)** are enforced by files in the repo. Items marked **(admin)** require a repository admin to configure in GitHub settings.

---

## 1. Access Control

| # | Control | Status | How |
|---|---------|--------|-----|
| 1.1 | Default branch is `main` | ✅ | Already configured |
| 1.2 | Direct pushes to `main` are blocked | **(admin)** | Branch protection rule or ruleset |
| 1.3 | PR requires at least 1 approval | **(admin)** | Branch protection rule or ruleset |
| 1.4 | CODEOWNERS review is required | **(admin)** | Enable in branch protection; see `.github/CODEOWNERS` |
| 1.5 | Stale approvals are dismissed on new commits | **(admin)** | Branch protection rule or ruleset |
| 1.6 | All conversations must be resolved before merge | **(admin)** | Branch protection rule or ruleset |
| 1.7 | Signed commits are required | **(admin)** | Branch protection rule or ruleset |
| 1.8 | Administrators are subject to the same rules | **(admin)** | "Include administrators" toggle |
| 1.9 | Force pushes are blocked on `main` | **(admin)** | Branch protection rule or ruleset |
| 1.10 | Branch deletion is blocked on `main` | **(admin)** | Branch protection rule or ruleset |

---

## 2. CI / Workflow Security

| # | Control | Status | How |
|---|---------|--------|-----|
| 2.1 | Workflows have minimal explicit permissions | ✅ | `permissions: contents: read` in all workflows that do not publish; elevated permissions only in publishing jobs |
| 2.2 | `GITHUB_TOKEN` has restricted default permissions | **(admin)** | Settings → Actions → Workflow permissions → Read repository contents and packages |
| 2.3 | Only actions from GitHub-verified creators are allowed | **(admin)** | Settings → Actions → Allow kubedoio, and select non-GitHub-verified actions (review monthly) |
| 2.4 | Self-hosted runner labels are restricted | **(admin)** | `integration-kvm.yml` uses `chv-kvm`; ensure only trusted runners advertise this label |
| 2.5 | Workflow `workflow_dispatch` inputs are validated | ✅ | `release.yml` validates version format; `package-nightly.yml` has dry-run mode |

### Workflow permission audit

| Workflow | Workflow-level permissions | Job-level elevation |
|----------|---------------------------|---------------------|
| `ci.yml` | `contents: read` | None |
| `proto.yml` | `contents: read` | None |
| `security.yml` | `contents: read`, `issues: write`, `checks: write` | None |
| `package-pr.yml` | `contents: read` | None |
| `package-nightly.yml` | `contents: read` | `publish-github`: `contents: write`; `publish-repo`: environment-gated |
| `integration-kvm.yml` | `contents: read` | None |
| `release.yml` | `contents: read` | `release`: `contents: write`, `attestations: write`, `id-token: write` (environment-gated); `publish-repo`: environment-gated |

---

## 3. Release / Tag Safety

| # | Control | Status | How |
|---|---------|--------|-----|
| 3.1 | Only tags trigger releases | ✅ | `release.yml` triggers on `push: tags:` only |
| 3.2 | Version tags (`v*`) are protected | **(admin)** | Tag protection rule or ruleset |
| 3.3 | Release publishing requires environment approval | ✅ | `release` job uses `environment: production` or `rc`; configure required reviewers in Settings → Environments |
| 3.4 | Package repo publishing requires separate environment | ✅ | `publish-repo` job uses `production-repo` or `rc-repo` |
| 3.5 | Changelog is required for stable releases | ✅ | `release.yml` validates CHANGELOG.md before building |
| 3.6 | Smoke and lifecycle tests gate publishing | ✅ | `release.yml` package job runs smoke/lifecycle tests before release job starts |
| 3.7 | Artifacts are checksummed and signed | ✅ | `SHA256SUMS` + GPG + Cosign (gracefully degrades if secrets absent) |
| 3.8 | SBOM and provenance attestations are generated | ✅ | `anchore/sbom-action` + `actions/attest-build-provenance` |

---

## 4. Secret Leak Prevention

| # | Control | Status | How |
|---|---------|--------|-----|
| 4.1 | No secrets committed to the repo | ✅ | Regular review; no credential files in source |
| 4.2 | GitHub secret scanning is enabled | **(admin)** | Settings → Security → Secret scanning → Enable |
| 4.3 | Push protection for secrets is enabled | **(admin)** | Settings → Security → Secret scanning → Push protection |
| 4.4 | No `secrets.` values are logged in workflows | ✅ | Reviewed in all workflows |
| 4.5 | Self-hosted runners do not persist state between runs | **(admin)** | KVM runners must clean workspace after each job |

### Pre-commit hook recommendation

Install locally to catch accidents before push:

```bash
# Using detect-secrets (Yelp) or git-secrets (AWS)
pip install detect-secrets
detect-secrets scan > .secrets.baseline
detect-secrets hook --baseline .secrets.baseline
```

Or add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/Yelp/detect-secrets
    rev: v1.5.0
    hooks:
      - id: detect-secrets
        args: ['--baseline', '.secrets.baseline']
```

> **Note:** CHV does not currently use `pre-commit`. This is a recommended optional step for contributors.

---

## 5. Dependency / Supply Chain

| # | Control | Status | How |
|---|---------|--------|-----|
| 5.1 | Dependabot is enabled for Cargo, npm, and GitHub Actions | ✅ | `.github/dependabot.yml` |
| 5.2 | Security updates are grouped separately from version updates | ✅ | `cargo-security`, `npm-security`, `actions-security` groups |
| 5.3 | `cargo audit` runs on every PR touching Cargo files | ✅ | `.github/workflows/security.yml` |
| 5.4 | `cargo deny` runs on every PR touching Cargo files | ✅ | `.github/workflows/security.yml` |
| 5.5 | `cargo deny` is configured (`deny.toml`) | ✅ | At repo root |
| 5.6 | Action versions are pinned to tags or SHAs | ✅ | All `uses:` lines use `@vN` or `@vN.M.P`; comment blocks document recommended SHA pinning |

---

## 6. CODEOWNERS and Review Governance

| # | Control | Status | How |
|---|---------|--------|-----|
| 6.1 | CODEOWNERS file exists and is valid | ✅ | `.github/CODEOWNERS` |
| 6.2 | CODEOWNERS review requirement is enabled | **(admin)** | Branch protection rule |
| 6.3 | Teams referenced in CODEOWNERS exist and have write access | **(admin)** | Create at `https://github.com/orgs/kubedoio/teams` |
| 6.4 | Admin-only paths (`.github/`, `SECURITY.md`) require admin team review | ✅ | `.github/CODEOWNERS` |

### Team inventory

Create these teams in the `kubedoio` organization before enabling CODEOWNERS enforcement:

| Team | Purpose | Approximate size |
|------|---------|-----------------|
| `chv-admins` | Repository governance, CI, security policy | 2–3 |
| `chv-maintainers` | Rust backend, control plane, protobuf | 3–5 |
| `chv-frontend` | SvelteKit UI, BFF integration | 2–3 |
| `chv-ops` | Deployment, packaging, install scripts | 2–3 |
| `chv-architecture` | ADRs, specs, design docs | 2–3 |

---

## 7. Audit and Monitoring

| # | Control | Status | How |
|---|---------|--------|-----|
| 7.1 | Audit log is retained (GitHub Enterprise / Organization) | **(admin)** | Organization-level setting |
| 7.2 | Failed login / access attempts are monitored | **(admin)** | Organization-level security insights |
| 7.3 | Self-hosted runner health is monitored | **(admin)** | Runner admin dashboard; `integration-kvm.yml` uploads logs on failure |

---

## Quick Start for Admins

1. Create the GitHub teams listed above.
2. Apply branch protection: run `./scripts/github-setup/apply-branch-protection.sh`.
3. Apply tag protection: run `./scripts/github-setup/apply-tag-protection.sh`.
4. Enable secret scanning and push protection in repository settings.
5. Restrict default `GITHUB_TOKEN` permissions to read-only.
6. Verify: run `./scripts/github-setup/verify-settings.sh`.

---

## Related Documents

- [`BRANCH_PROTECTION.md`](./BRANCH_PROTECTION.md) — detailed branch/tag protection rules
- [`.github/CODEOWNERS`](../../.github/CODEOWNERS) — code ownership mapping
- [`SECURITY.md`](../../SECURITY.md) — vulnerability reporting and severity classification
- [`docs/release/PIPELINE.md`](../release/PIPELINE.md) — release engineering details
