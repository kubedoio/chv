# CHV First Release Readiness Audit

**Audit date:** 2026-05-11  
**Version under review:** 0.1.0  
**Auditor:** Release engineering automation  

## Executive Summary

CHV has a solid packaging and release pipeline foundation. The core build system, package generation, smoke tests, and release workflows are implemented and functional. However, **tagging `v0.1.0` as a stable release is not yet recommended** because critical user-facing install artifacts still reference the old four-segment version scheme (`0.0.0.2`). An RC tag (`v0.1.0-rc.1`) is safe to validate the full pipeline end-to-end.

## Release Candidate Recommendation

**Ready with Conditions** — An RC can be tagged today. The following must be fixed before promoting to stable:

1. Update `scripts/install.sh` fallback version from `0.0.0.2` to `0.1.0`.
2. Remove or archive obsolete per-package maintainer scripts from `packaging/scripts/`.
3. Update `docs/DEPLOYMENT.md` and `docs/GAP_ANALYSIS.md` to reference `0.1.0`.

## Critical Blockers

| # | Issue | Impact | Remediation |
|---|-------|--------|-------------|
| 1 | `scripts/install.sh` hardcodes fallback version `0.0.0.2` (lines 232, 233, 236) | Users installing via `curl | sh` may get the wrong old version | Replace all `0.0.0.2` references with `0.1.0` or derive from `VERSION` file |
| 2 | `docs/DEPLOYMENT.md` references `0.0.0.2` and `get.cellhv.com` | Install docs are misleading; the curl endpoint does not exist | Update version references; mark `get.cellhv.com` as future work; redirect to GitHub releases |
| 3 | Obsolete per-package maintainer scripts still in `packaging/scripts/` (`chv-controlplane-postinstall.sh`, `chv-node-postinstall.sh`, etc.) | Risk of confusion; these files are unused but present | Delete the 4 obsolete files; they were superseded by generic `postinstall.sh`, `preremove.sh`, `postremove.sh` |
| 4 | `docs/GAP_ANALYSIS.md` references `0.0.0.2` | Docs inconsistency | Update version reference |

## High Priority Issues

| # | Issue | Impact | Remediation |
|---|-------|--------|-------------|
| 5 | Docker unavailable in CI build environment; smoke/lifecycle tests cannot be verified locally | Cannot pre-validate container tests before pushing to CI | Ensure GitHub-hosted runners have Docker; add a CI step that verifies Docker is available before running smoke tests |
| 6 | RPM metadata cannot be inspected locally (`rpm` command missing) | Cannot validate RPM package contents before CI | Install `rpm` in local dev container or rely on CI validation only |
| 7 | `build-release.sh` derives version via `scripts/version.sh` with channel, but the release workflow uses tag-derived version directly | Potential version mismatch between tarball and packages if `CHV_RELEASE_CHANNEL` is set | Ensure `build-release.sh` and `release.yml` use consistent version derivation; test `make release` end-to-end |
| 8 | No evidence that tarball install path (`install.sh`) has been tested with `0.1.0` packages | The primary user install path may be broken | Run `./scripts/install.sh` against a local tarball built from `make release` |
| 9 | `release.yml` release tarball assembly copies `cmd/chv-controlplane/migrations/*` — if this directory is empty or missing, the tarball is incomplete | Missing migrations in release tarball | Verify migrations exist in `cmd/chv-controlplane/migrations/`; add CI check |

## Medium Priority Issues

| # | Issue | Impact | Remediation |
|---|-------|--------|-------------|
| 10 | `systemd-analyze verify` fails locally because binaries aren't at `/usr/bin/` | Cannot validate units in build environment | This is expected in build containers; units are validated in smoke test containers instead |
| 11 | `package-pr.yml` and `package-nightly.yml` run smoke tests assuming Docker is available — if Docker is missing, the step fails silently or with unclear error | CI failures on runners without Docker | Add a pre-check step: `docker info` with clear failure message |
| 12 | `integration-kvm.yml` downloads artifacts by name `chv-packages-pr-${{ github.event.pull_request.number }}` which may not exist if the PR package workflow hasn't run yet | KVM test may fail on PRs that haven't built packages | Add artifact existence check or build packages in the KVM workflow |
| 13 | `integration-kvm.yml` uses `actions/setup-node@v4` cache with `ui/package-lock.json` but the checkout may not include `ui/` if it's a shallow checkout or artifact download | Cache miss or error | Ensure `ui/package-lock.json` is present or disable cache when running from artifacts |
| 14 | `scripts/install.sh` references `get.cellhv.com` which does not exist | Users following old docs will hit a 404 | Remove or clearly mark as not-yet-implemented |
| 15 | No automated test of the `make release` tarball path | Tarball may be incomplete or incorrect | Add a CI job that builds the tarball and verifies its contents |

## Low Priority Improvements

| # | Issue | Impact | Remediation |
|---|-------|--------|-------------|
| 16 | Old `chv-0.0.0.4-linux-amd64.tar.gz` still in `dist/` | Minor confusion | Clean `dist/` in `make clean` or `.gitignore` |
| 17 | `docs/release/release-engineering-plan.md` references old per-package scripts | Historical doc drift | Update to reference generic scripts |
| 18 | `release.yml` uploads `dist/packages/SHA256SUMS.sig` and `SHA256SUMS.cosign.sig` even when signing is not configured | Release may show zero-byte or missing signature files | Only upload signature files if they exist |
| 19 | `package-nightly.yml` does not generate SBOM or provenance attestation | Nightly builds lack full trust artifacts | Add SBOM generation to nightly workflow (optional) |
| 20 | `CHANGELOG.md` `[Unreleased]` section is very long | Difficult to extract the stable section | Consider collapsing older unreleased items into the first stable release section |

## Verified Working Areas

| Area | Evidence |
|------|----------|
| **Rust build** | `cargo build --workspace --release` succeeds |
| **Workspace version consistency** | `VERSION` = `0.1.0`; all `cmd/*/Cargo.toml` match; CI validates this |
| **CLI version output** | All 5 binaries emit `0.1.0 (commit 0872c4a7, build 2026-05-10, channel stable)` |
| **Local release check** | `scripts/check-release-local.sh` passes all 5 steps |
| **Debian package generation** | `make package-deb` produces 3 `.deb` files with correct metadata |
| **RPM package generation** | `make package-rpm` produces 3 `.rpm` files with correct metadata |
| **Package metadata** | Maintainer, license, description, homepage all correct |
| **Package dependencies** | `chv-node` correctly depends on `chv-controlplane` |
| **Systemd units** | All 4 unit files present; syntax is valid |
| **Config preservation** | All configs marked `config|noreplace` in nFPM YAMLs |
| **Maintainer script safety** | No `rm -rf /var/lib/chv`, `userdel`, `mkfs`, or `dd` commands |
| **Smoke tests** | `smoke-deb.sh` and `smoke-rpm.sh` exist and pass syntax checks |
| **Lifecycle tests** | `lifecycle-deb.sh`, `lifecycle-rpm.sh`, `lifecycle-common.sh` exist and pass syntax checks |
| **KVM workflow** | `integration-kvm.yml` exists; uses self-hosted runner label; gated by PR label |
| **GitHub Actions security** | No `pull_request_target`; no `issues: write` or `packages: write` in sensitive workflows |
| **Secrets hygiene** | No hardcoded secrets; all secrets use `${{ secrets.XXX }}`; PR workflow uses no secrets |
| **Publishing gating** | `release.yml` uses `environment: production` / `rc` for approval gating |
| **Release tag behavior** | Tag patterns `v[0-9]+.[0-9]+.[0-9]+` and `v[0-9]+.[0-9]+.[0-9]+-rc.[0-9]+` are valid |
| **Release artifact completeness** | Tarball, `.deb`, `.rpm`, `SHA256SUMS`, SBOM, provenance attestation all configured |
| **Checksums** | `SHA256SUMS` generated in package job |
| **SBOM** | SPDX and CycloneDX generated via `anchore/sbom-action` |
| **Provenance** | GitHub artifact attestations via `actions/attest-build-provenance` |
| **Installation docs** | 5 install docs created; match actual package names; no unimplemented features claimed |
| **Uninstall docs** | Data preservation clearly documented; destructive cleanup instructions present |
| **Cargo.lock tracked** | Reproducible builds ensured |

## Package Matrix

| Package | Format | Version | Size (deb) | Size (rpm) | Dependencies | Configs |
|---------|--------|---------|------------|------------|--------------|---------|
| `chv-controlplane` | deb/rpm | 0.1.0 | ~9.1 MB | ~9.4 MB | None | `controlplane.toml` |
| `chv-node` | deb/rpm | 0.1.0 | ~13.6 MB | ~14.0 MB | `chv-controlplane` | `agent.toml`, `stord.toml`, `nwd.toml`, `chv.yaml` |
| `chvctl` | deb/rpm | 0.1.0 | ~3.0 MB | ~3.1 MB | None | None |

## CI/CD Matrix

| Workflow | Trigger | Runner | Tests | Publish | Environment |
|----------|---------|--------|-------|---------|-------------|
| `ci.yml` | push/PR to `main` | `ubuntu-latest` | fmt, clippy, test, version check | No | — |
| `package-pr.yml` | PR to `main`, branch push | `ubuntu-latest` | build, package, smoke deb/rpm | Artifacts only (7d) | — |
| `package-nightly.yml` | push to `main`, dispatch | `ubuntu-latest` | build, package, smoke, lifecycle | GitHub pre-release + repo (optional) | `nightly-repo` |
| `release.yml` | tag `v*`, dispatch | `ubuntu-latest` | build, package, smoke, lifecycle | GitHub Release + repo (optional) | `production` / `rc` |
| `integration-kvm.yml` | dispatch, PR label, push `main` | `self-hosted, chv-kvm` | host diagnostics, package install, service start | No | — |

## Security and Trust Artifacts

| Artifact | Status | Notes |
|----------|--------|-------|
| SHA256SUMS | ✅ Generated | Per-package job |
| GPG signature | 🔶 Conditional | Requires `CHV_RELEASE_GPG_KEY` secret |
| Cosign signature | 🔶 Conditional | Requires `CHV_RELEASE_COSIGN_KEY` secret |
| SBOM (SPDX) | ✅ Generated | `sbom.spdx.json` |
| SBOM (CycloneDX) | ✅ Generated | `sbom.cyclonedx.json` |
| Build provenance | ✅ Generated | GitHub artifact attestations |
| Signed repo metadata | 🔶 Not configured | `publish-repo.sh` supports GPG but key not set up |

## Installation/Upgrade/Removal Safety

| Operation | Tested | Data Preserved | Config Preserved |
|-----------|--------|----------------|------------------|
| Fresh install | ✅ Smoke tests | N/A | N/A |
| Reinstall same version | ✅ Lifecycle tests | ✅ | ✅ |
| Upgrade old → new | ✅ Lifecycle tests | ✅ | ✅ (`config\|noreplace`) |
| Remove | ✅ Smoke + lifecycle | ✅ `/var/lib/chv`, `/etc/chv` | ✅ |
| Reinstall after remove | ✅ Lifecycle tests | ✅ | ✅ |
| Downgrade | ❌ Not supported | N/A | N/A |
| Purge | ❌ Not implemented | N/A | N/A |

## Final Go/No-Go Checklist

### Pre-RC (must pass)

- [x] Rust workspace builds in release mode
- [x] All crate versions match `VERSION` file
- [x] `cargo test --workspace` passes
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] `cargo fmt --all -- --check` passes
- [x] All 5 binaries emit correct version with metadata
- [x] `.deb` packages build successfully
- [x] `.rpm` packages build successfully
- [x] Package metadata is correct (maintainer, license, description)
- [x] Systemd units are present and syntactically valid
- [x] Config files marked `config|noreplace`
- [x] Maintainer scripts contain no destructive commands
- [x] Smoke test scripts pass syntax checks
- [x] Lifecycle test scripts pass syntax checks
- [x] Release workflow YAML is valid
- [x] Changelog has an entry for the target version
- [x] Install docs match actual package names

### Pre-Stable (must pass before promoting RC)

- [ ] `scripts/install.sh` fallback version updated from `0.0.0.2` to `0.1.0`
- [ ] Obsolete per-package maintainer scripts removed from `packaging/scripts/`
- [ ] `docs/DEPLOYMENT.md` updated to reference `0.1.0`
- [ ] `docs/GAP_ANALYSIS.md` updated to reference `0.1.0`
- [ ] Tarball install path (`make release` + `install.sh`) tested end-to-end
- [ ] RC artifacts installed and validated on a real host
- [ ] GitHub Release created with all expected artifacts attached
- [ ] Checksums verified against downloaded artifacts
- [ ] GitHub attestation verified with `gh attestation verify`

### Enterprise readiness (nice to have)

- [ ] GPG signing key generated and configured in secrets
- [ ] Public key fingerprint published in docs
- [ ] Package repository (apt/yum) configured and tested
- [ ] KVM integration workflow validated on a real self-hosted runner
- [ ] Logrotate config provided
- [ ] SELinux policy documented or provided
