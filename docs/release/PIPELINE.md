# CHV Release Pipeline — LLM Context Document

**Purpose:** This is the single source of truth for the CHV release engineering pipeline. If you are an LLM agent working on releases, packaging, CI/CD, or versioning, **read this file first** before exploring the repository.

**Last updated:** 2026-05-11  
**Version:** 0.1.0  

---

## Pipeline Overview

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Source    │────▶│    Build    │────▶│   Package   │────▶│    Test     │────▶│   Publish   │
│   (git)     │     │   (Rust+UI) │     │  (nFPM)     │     │(smoke/life) │     │(GitHub+repo)│
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │                   │                   │
       │                   │                   │                   │                   │
   VERSION file      build.rs injects     .deb / .rpm         Docker containers    GitHub Release
   scripts/version.sh  version metadata    systemd units       install/upgrade      + apt/yum repo
                       into binaries       config files        remove/reinstall     (optional)
                                           maintainer scripts
```

**Channels:** `stable` (vX.Y.Z) → `rc` (vX.Y.Z-rc.N) → `nightly` (rolling from main) → `pr` (branch builds)

---

## Source of Truth

| What | File | Format |
|------|------|--------|
| Semantic version | `VERSION` | Plain text, e.g. `0.1.0` |
| Per-crate version | `cmd/*/Cargo.toml` | Must match `VERSION` |
| Changelog | `CHANGELOG.md` | Keep a Changelog format |
| Git tag | `vX.Y.Z` or `vX.Y.Z-rc.N` | Must match `VERSION` |

**Rule:** All 5 binary crates (`chv-controlplane`, `chv-agent`, `chv-stord`, `chv-nwd`, `chvctl`) share the same version. CI validates this.

---

## File Map (Who Calls Whom)

### Version Derivation
```
VERSION ──▶ scripts/version.sh ──▶ deb: 0.1.0~rc.1
                              ──▶ rpm: 0.1.0-0.1.rc1
                              ──▶ nightly: 0.1.0~nightly.20260511.gabc1234
```
- Called by: `scripts/build-packages.sh`, CI workflows, Makefile
- Environment override: `CHV_PKG_PRERELEASE` (set by CI for RC builds)

### Build
```
Makefile:build-release ──▶ cargo build --workspace --release
                        ──▶ cd ui && npm ci && npm run build
                        ──▶ tar -czf dist/chv-VERSION-linux-amd64.tar.gz
```
- Version metadata injected via `cmd/*/build.rs` (CHV_VERSION, CHV_GIT_SHA, CHV_BUILD_DATE, CHV_RELEASE_CHANNEL)
- Binaries respond to `--version` with: `chvctl 0.1.0 (commit abc1234, build 2026-05-11, channel stable)`

### Package Generation
```
scripts/build-packages.sh ──▶ nfpm package -f config.yaml -p deb/rpm
   │
   ├── packaging/nfpm/chv-controlplane.yaml  → chv-controlplane_0.1.0_amd64.deb
   ├── packaging/nfpm/chv-node.yaml          → chv-node_0.1.0_amd64.deb
   └── packaging/nfpm/chvctl.yaml            → chvctl_0.1.0_amd64.deb
   └── packaging/scripts/postinstall.sh      → runs on package install
   └── packaging/scripts/preremove.sh        → runs before package removal
   └── packaging/scripts/postremove.sh       → runs after package removal
```
- **Tool:** nFPM v2.41.1 (pinned in CI)
- **Formats:** `.deb` (Debian/Ubuntu) and `.rpm` (RHEL/Rocky/Alma/Fedora)
- **Package `chv-node` depends on `chv-controlplane`**
- Config files marked `config|noreplace` (survive upgrades)
- Services installed but NOT auto-started

### Testing
```
scripts/package/smoke-deb.sh     → installs .deb in clean Debian container, checks binaries
scripts/package/smoke-rpm.sh     → installs .rpm in clean Rocky container, checks binaries
scripts/package/lifecycle-deb.sh → install → upgrade → remove → reinstall with sentinel files
scripts/package/lifecycle-rpm.sh → same for RPM
```
- Sentinel files in `/var/lib/chv/` and `/etc/chv/` prove data survives operations
- Lifecycle tests require Docker

### CI/CD Workflows

| Workflow | Trigger | What it does | Runner |
|----------|---------|--------------|--------|
| `ci.yml` | push/PR to `main` | fmt, clippy, test, version check | `ubuntu-latest` |
| `package-pr.yml` | PR to `main` | build, package, smoke deb/rpm | `ubuntu-latest` |
| `package-nightly.yml` | push to `main`, dispatch | build, package, smoke, lifecycle, publish pre-release | `ubuntu-latest` |
| `release.yml` | tag `v*`, dispatch | full pipeline + SBOM + signing + GitHub Release | `ubuntu-latest` |
| `integration-kvm.yml` | dispatch, PR label, push `main` | host diagnostics, KVM tests, package install | self-hosted `chv-kvm` |

**Workflow dependencies:**
```
ci.yml ──▶ (gates PRs)
package-pr.yml ──▶ produces artifacts (7 day retention)
release.yml:build ──▶ release.yml:package ──▶ release.yml:release ──▶ release.yml:publish-repo
```

### Signing and Trust Artifacts
```
dist/packages/SHA256SUMS ──▶ scripts/release/sign-checksums.sh
   ├── SHA256SUMS.sig       (GPG, if CHV_RELEASE_GPG_KEY secret set)
   └── SHA256SUMS.cosign.sig (Cosign, if CHV_RELEASE_COSIGN_KEY secret set)

SBOM:
   ├── dist/sbom.spdx.json      (anchore/sbom-action)
   └── dist/sbom.cyclonedx.json (anchore/sbom-action)

Provenance:
   └── GitHub artifact attestation (actions/attest-build-provenance)
```
- Signing gracefully degrades if secrets are missing
- No signing keys are currently configured

### Publishing
```
GitHub Release (always):
   └── Created by softprops/action-gh-release@v2

Package Repository (optional, requires secrets):
   └── scripts/publish/publish-repo.sh
       ├── apt repository (dpkg-scanpackages + GPG-signed Release/InRelease)
       └── yum repository (createrepo_c + GPG-signed repomd.xml)
       └── Upload: S3 sync OR rsync
```
- Repo publish is dry-run by default (no credentials = prints what it would do)

---

## Exact Commands (Copy-Paste)

### Local Development

```bash
# Build release binaries and tarball
make build-release

# Build packages (requires nfpm + envsubst)
make package-deb    # or: make package-rpm
make package-local  # both formats

# Run smoke tests (requires Docker)
make package-smoke-deb
make package-smoke-rpm

# Run lifecycle tests (requires Docker)
make package-lifecycle-deb
make package-lifecycle-rpm

# Verify everything locally
make check-release-local

# Generate and sign checksums
make sign-checksums
```

### Version Management

```bash
# Bump VERSION file and all Cargo.toml files
./scripts/bump-version.sh 0.1.1

# Derive package versions
./scripts/version.sh --deb        # 0.1.0
./scripts/version.sh --rpm        # 0.1.0
./scripts/version.sh --deb rc 1   # 0.1.0~rc.1
./scripts/version.sh --rpm rc 1   # 0.1.0-0.1.rc1
./scripts/version.sh --deb nightly
./scripts/version.sh --rpm nightly
```

### Release a New Version

```bash
# 1. Bump version
./scripts/bump-version.sh 0.1.1

# 2. Update CHANGELOG.md
# 3. Commit and push
# 4. Tag (triggers release.yml)
git tag v0.1.1
git push origin v0.1.1

# For RC:
git tag v0.1.1-rc.1
git push origin v0.1.1-rc.1
```

---

## Key Decisions and Rationale

| Decision | Why |
|----------|-----|
| **nFPM instead of cargo-deb/cargo-rpm** | Single tool generates both formats; simpler config (YAML); handles maintainer scripts natively |
| **Generic maintainer scripts** | One `postinstall.sh`/`preremove.sh`/`postremove.sh` for all packages instead of per-package scripts. Safer, easier to maintain, no duplication |
| **Services NOT auto-started** | User must configure network/storage first. Prevents broken first-boot states |
| **Config files `config\|noreplace`** | Modified configs survive package upgrades. No silent overwrites |
| **`/var/lib/chv` and `/etc/chv` preserved on remove** | Data safety. Intentional design choice. User can purge manually if needed |
| **No purge script** | Not implemented. Package remove preserves data by design |
| **Self-hosted runner for KVM** | GitHub-hosted runners don't support nested virtualization. KVM tests need bare-metal or dedicated VM |
| **Rolling nightly pre-release** | Single `nightly` tag on GitHub that gets overwritten. Avoids clutter. Users pin to specific nightly versions via exact filename |
| **GPG + Cosign dual signing** | GPG for traditional package manager trust; Cosign for modern Sigstore ecosystem |

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `nfpm: command not found` | nFPM not installed | `go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest` or download binary |
| `envsubst: command not found` | gettext not installed | `sudo apt-get install gettext-base` |
| Smoke tests fail with "Docker not available" | Docker daemon not running | `sudo systemctl start docker` or run in CI |
| `rpm` command not found (on Debian) | Can't inspect RPM metadata locally | Use CI, or install `rpm` package |
| `systemd-analyze verify` fails locally | Binaries not in `/usr/bin/` during build | Expected in build container; verified in smoke tests instead |
| Release workflow fails at "Create GitHub Release" | Missing `contents: write` permission | Check workflow `permissions` block |
| Signing step shows "SIGNING NOT CONFIGURED" | Secrets not set | Add `CHV_RELEASE_GPG_KEY` or `CHV_RELEASE_COSIGN_KEY` to repo secrets |
| `local: can only be used in a function` | Bash `local` outside function | Fix: remove `local` keyword from top-level code |
| RPM version contains `^` character | Invalid RPM version separator | Use `~` instead (valid in both RPM and Debian) |

---

## LLM Agent Quick Reference

**If the user asks you to:**
- "Build packages" → run `make package-local` or `make package-deb` / `make package-rpm`
- "Run smoke tests" → run `make package-smoke-deb` and `make package-smoke-rpm` (requires Docker)
- "Cut a release" → bump VERSION, update CHANGELOG, commit, tag `vX.Y.Z`, push tag
- "Fix the install script" → edit `scripts/install.sh` (not the hosting scripts unless explicitly asked)
- "Update version everywhere" → run `./scripts/bump-version.sh NEW_VERSION`
- "Review release workflow" → read `.github/workflows/release.yml` and `docs/release/PIPELINE.md`
- "Sign artifacts" → check if `CHV_RELEASE_GPG_KEY` or `CHV_RELEASE_COSIGN_KEY` secrets exist; if not, explain graceful degradation

**Before modifying any packaging or release file:**
1. Read this document (`docs/release/PIPELINE.md`)
2. Read the specific file you're changing
3. Check if the change affects other files in the pipeline (use the File Map above)
4. Run `make check-release-local` after changes

**Files you should NOT modify without explicit user approval:**
- `packaging/scripts/postinstall.sh`, `preremove.sh`, `postremove.sh` (run as root on user machines)
- `.github/workflows/release.yml` environment protection rules
- `scripts/install.sh` when it downloads and executes binaries
