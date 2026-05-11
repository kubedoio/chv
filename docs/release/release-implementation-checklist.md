# CHV Release Engineering — Implementation Checklist

This document is the ordered implementation checklist for `release-engineering-plan.md`. It covers the work required to move CHV from manual tarball releases to a fully automated, tested, signed, and published release pipeline.

**Repository:** `/root/chv`  
**Current version:** `0.1.0`  
**Workspace:** Rust Cargo workspace with binaries `chvctl`, `chv-controlplane`, `chv-agent`, `chv-nwd`, `chv-stord`

---

## Phase 1: Foundation

> Packaging scaffolding, local build tooling, and base metadata.

- [x] `VERSION` file exists at repo root and is the single source of truth for package versions. *(implemented)*
- [x] `VERSION` standardized to SemVer (`0.1.0`). *(implemented)*
- [x] `CHANGELOG.md` follows Keep a Changelog format with `[Unreleased]` section. *(implemented)*
- [x] `scripts/bump-version.sh` updated for SemVer (`major|minor|patch`). *(implemented)*
- [x] `docs/release/versioning-policy.md` — comprehensive version policy covering SemVer rules, release channels, package version mapping, and environment variables. *(implemented)*
- [x] Build metadata injection (`cmd/chvctl/build.rs`) — embeds git SHA, build date, and release channel into the `chvctl` binary. *(implemented)*
- [x] Rich CLI version output — `chvctl version` prints version, commit, build date, and channel. *(implemented)*
- [x] `scripts/version.sh` — runtime version querying helper. *(implemented by parallel subagent)*
- [x] `scripts/smoke-version.sh` — automated version validation smoke test. *(implemented by parallel subagent)*
- [x] CI version validation — `.github/workflows/ci.yml` validates `VERSION` format, `Cargo.toml` sync, and `chvctl --version` output. *(implemented)*
- [x] `cmd/chvctl/build.rs` includes `cargo:rerun-if-changed` for `VERSION` and `cargo:rerun-if-env-changed` for `CHV_RELEASE_CHANNEL` to ensure binaries rebuild when metadata changes. *(implemented)*
- [x] `packaging/nfpm.yaml` — nfpm template config with envsubst placeholders for name, arch, version, depends, contents, scripts. *(implemented)*
- [x] `packaging/systemd/chv-controlplane.service` — systemd unit for control plane. *(implemented)*
- [x] `packaging/systemd/chv-agent.service` — systemd unit for agent. *(implemented)*
- [x] `packaging/systemd/chv-stord.service` — systemd unit for storage daemon. *(implemented)*
- [x] `packaging/systemd/chv-nwd.service` — systemd unit for network daemon. *(implemented)*
- [x] `packaging/scripts/chv-controlplane-postinstall.sh` — creates `chv` user/group, ensures `/var/lib/chv` ownership, runs `daemon-reload`. *(implemented)*
- [x] `packaging/scripts/chv-controlplane-preremove.sh` — stops service on remove (not upgrade), runs `daemon-reload`. *(implemented)*
- [x] `packaging/scripts/chv-node-postinstall.sh` — creates `chv` user, adds to `kvm` group, ensures `/var/lib/chv`, `/var/log/chv`, `/run/chv` ownership, runs `daemon-reload`. *(implemented)*
- [x] `packaging/scripts/chv-node-preremove.sh` — stops agent/stord/nwd on remove (not upgrade), runs `daemon-reload`. *(implemented)*
- [x] `scripts/build-packages.sh` — builds `.deb` and `.rpm` for `chvctl`, `chv-controlplane`, and `chv-node` using nfpm + envsubst. Generates `SHA256SUMS` in `dist/packages/`. *(implemented)*
- [x] `scripts/build-release.sh` — builds release tarball `dist/chv-<VERSION>-linux-amd64.tar.gz` with binaries, UI, migrations, systemd units, nginx config, example configs, and `install.sh`. Generates `.sha256`. *(implemented)*
- [x] `scripts/smoke-packages.sh` — checks release binaries exist, UI build exists, packaging configs are present, optionally builds packages via nfpm, and inspects `.deb`/`.rpm` metadata and contents. *(implemented)*
- [x] `docs/PACKAGING.md` — documents package split, file layout, install/upgrade/uninstall commands, known gaps (no repo, no signing). *(implemented)*
- [ ] **Refine postinstall scripts to handle idempotent user creation and log directory permissions on upgrade.** *(partial — scripts create user but do not explicitly handle upgrade path without re-running destructive operations; test and harden against re-install/upgrade)*
- [ ] **Add `config|noreplace` markers to all config files in `scripts/build-packages.sh` and verify RPM/DEB behavior.** *(partial — controlplane.toml has it; verify agent.toml, stord.toml, nwd.toml also carry the correct nfpm `type: config|noreplace`)*
- [ ] **Add packaging lint step** (e.g., `lintian` for `.deb`, `rpmlint` for `.rpm`) to `scripts/smoke-packages.sh` or as a new `scripts/lint-packages.sh`.
- [ ] **Add multi-arch support template** in `packaging/nfpm.yaml` and `scripts/build-packages.sh` to prepare for `arm64` builds (currently hardcoded to `amd64`).
- [ ] **Add `Provides`, `Conflicts`, and `Replaces` metadata** in nfpm template for smoother upgrades (e.g., `chv-node` replaces any legacy single-package `chv`).

---

## Phase 2: CI/CD

> GitHub Actions workflows for continuous packaging and release automation.

- [x] `.github/workflows/ci.yml` — runs `cargo fmt/check/clippy/test`, UI build, and Playwright E2E tests on push/PR to `main`. *(implemented)*
- [x] `.github/workflows/release.yml` — triggered on `v*` tags or `workflow_dispatch`. Builds binaries and UI, assembles tarball, builds `.deb`/`.rpm`, generates SHA256 checksums, generates SBOM via `anchore/sbom-action`, creates build-provenance attestation via `actions/attest-build-provenance`, and publishes a GitHub Release with assets. *(implemented)*
- [ ] **PR Artifact Build workflow** (`.github/workflows/pr-artifacts.yml`) — builds packages on every PR (without publishing a release), uploads `dist/packages/*.{deb,rpm}` and the tarball as workflow artifacts. Must comment a summary on the PR with artifact links.
- [ ] **Nightly workflow** (`.github/workflows/nightly.yml`) — scheduled via `cron: '0 6 * * *'`. Builds from `main`, tags artifacts with `nightly-YYYYMMDD`, uploads to a dedicated "Nightly" GitHub Release that is overwritten each night (or to a separate nightly artifact bucket). Does **not** create a stable tag.
- [ ] **RC (Release Candidate) workflow** (`.github/workflows/rc.yml`) — triggered on `rc/*` tags (e.g., `v0.0.0.5-rc1`). Builds and packages exactly like a stable release, creates a **draft** GitHub Release pre-populated with "RC" warnings, does **not** publish to apt/dnf repos, and enables extended soak-testing.
- [ ] **Stable release workflow hardening** — refactor `.github/workflows/release.yml` or create `.github/workflows/stable-release.yml` with:
  - [ ] Explicit environment protection (`environment: production` with required reviewers).
  - [ ] Gating step that verifies the tag matches `VERSION`.
  - [ ] Changelog extraction (parse `CHANGELOG.md` for the matching version section and inject into release body instead of generic template).
  - [ ] Post-release job that triggers repository publishing (Phase 4) via repository_dispatch or workflow_call.
- [ ] **Reusable workflow extraction** — extract common "build + package" steps into `.github/workflows/_build-packages.yml` (composite or reusable workflow) so PR, nightly, RC, and stable workflows share one build definition.
- [ ] **CI matrix for Rust/toolchain versions** — test packaging on `ubuntu-latest` and `ubuntu-22.04` to catch glibc linking issues.

---

## Phase 3: Testing

> Automated verification that packages install, upgrade, remove, and behave correctly in clean environments.

- [x] `scripts/smoke-packages.sh` validates that packages can be built and contain expected files/binaries. *(implemented)*
- [ ] **Container-based install test for `.deb`** — create `tests/package/install-deb.sh` (or Docker-based test) that:
  - Spins up an `ubuntu:22.04` / `debian:12` container.
  - Installs built `.deb` packages with `dpkg -i`.
  - Asserts services are enabled (or at least units are present), `chv` user exists, `/var/lib/chv` and `/etc/chv` are created.
  - Runs `systemctl start chv-controlplane` (or validates binary can start with `--help`) in the container.
- [ ] **Container-based install test for `.rpm`** — same as above for `fedora:latest` / `rockylinux:9` using `rpm -i`.
- [ ] **Upgrade test (`.deb`)** — install an older CHV `.deb` (e.g., download `0.1.0` release), then upgrade to the newly built packages. Verify:
  - Config files in `/etc/chv/` are preserved (`config|noreplace`).
  - Services restart (or are at least stopped then started by script logic).
  - Database/data in `/var/lib/chv/` survives.
- [ ] **Upgrade test (`.rpm`)** — same for RPM using `rpm -U`.
- [ ] **Remove/purge test (`.deb`)** — install then `apt remove` / `apt purge`. Verify:
  - Binaries are removed.
  - Config files remain on `remove`, removed on `purge`.
  - `/var/lib/chv/` and `/var/log/chv/` are intentionally retained (documented behavior).
- [ ] **Remove test (`.rpm`)** — install then `rpm -e`. Same retention assertions.
- [ ] **Dependency resolution test** — verify `chv-node` depends on `chv-controlplane` and that installing `chv-node` alone pulls in `chv-controlplane` (or fails gracefully with a clear message if repo is not configured).
- [ ] **Install-from-tarball test** — run `scripts/install.sh` inside a clean container with `INSTALL_CHV_TARBALL_PATH` set to the built tarball and verify the all-in-one install path still works.
- [ ] **Package test GitHub Actions job** — add a `package-test` job to `.github/workflows/ci.yml` (or a new `package-test.yml`) that runs the container-based install/upgrade/remove tests on every PR that touches `packaging/` or `scripts/build-packages.sh`.

---

## Phase 4: Publishing

> Distribution of signed artifacts via GitHub Releases and package repositories.

- [x] GitHub Release creation with tarball, `.deb`, `.rpm`, `sha256sums.txt`, and `sbom.spdx.json`. *(implemented)*
- [x] Build-provenance attestation via GitHub Actions `actions/attest-build-provenance`. *(implemented)*
- [ ] **Artifact signing with GPG**:
  - [ ] Generate or import a CHV release signing key.
  - [ ] Store private key in GitHub Secrets (`GPG_PRIVATE_KEY`, `GPG_PASSPHRASE`).
  - [ ] Add signing step to release workflow: sign tarball, `.deb`, and `.rpm` packages.
  - [ ] Publish public key to `docs/release/CHV-RELEASE-KEY.asc` and link from release notes.
- [ ] **Debian/Ubuntu APT repository**:
  - [ ] Create `scripts/publish-apt-repo.sh` that uses `reprepro` or `aptly` to build a signed APT repo from `.deb` packages.
  - [ ] Generate `InRelease`, `Release`, and `Packages` files signed with GPG.
  - [ ] Host repository on GitHub Pages (`gh-pages` branch) or S3-compatible storage under a stable URL (e.g., `https://apt.chv.io`).
  - [ ] Add install instructions to docs: `echo "deb [signed-by=/usr/share/keyrings/chv.gpg] https://apt.chv.io stable main" | sudo tee /etc/apt/sources.list.d/chv.list`.
- [ ] **RHEL/CentOS/Fedora DNF/YUM repository**:
  - [ ] Create `scripts/publish-dnf-repo.sh` that uses `createrepo_c` to generate repodata.
  - [ ] Sign repo metadata with GPG.
  - [ ] Host on stable URL (e.g., `https://dnf.chv.io`).
  - [ ] Add install instructions to docs: `.repo` file with `gpgcheck=1` and `gpgkey=`.
- [ ] **Automated publish from CI**:
  - [ ] On stable release, after GitHub Release is created, run `publish-apt-repo.sh` and `publish-dnf-repo.sh` to update the live repositories.
  - [ ] Use a dedicated GitHub Environment (`production-repos`) with manual approval for the publish step.
- [ ] **Nightly artifact retention policy** — configure the nightly workflow to retain only the last N artifacts (e.g., 7 days) to avoid unbounded storage growth.

---

## Phase 5: Enterprise Hardening

> Production-grade behaviors: rollback, config migration, and install validation.

- [ ] **Rollback support**:
  - [ ] Document rollback procedure in `docs/release/rollback.md`: downgrade package version, restart services, and verify DB schema compatibility.
  - [ ] Add `chv-controlplane --db-rollback-check` (or similar CLI flag) that verifies the current DB schema is compatible with the binary version before starting services.
  - [ ] In postinstall scripts, print a warning if a downgrade is detected (compare installed version to previous version).
- [ ] **Config migration**:
  - [ ] Create `scripts/migrate-config.sh` that reads old config schemas and rewrites them to new schema versions (e.g., add missing keys with defaults, rename deprecated keys).
  - [ ] Invoke config migration in postinstall scripts when upgrading (detected via package manager `$1` parameter).
  - [ ] Add a `config_version` field to `controlplane.toml`, `agent.toml`, `stord.toml`, and `nwd.toml` so migration scripts know which schema the file conforms to.
- [ ] **Pre-install validation**:
  - [ ] Add `scripts/chv-controlplane-preinstall.sh` and `scripts/chv-node-preinstall.sh`:
    - Check for required kernel modules (`kvm`, `vhost-net`).
    - Check for `bridge-utils` / `iproute2` availability.
    - Warn if `/dev/kvm` is missing or permissions are wrong.
    - Warn if `chvbr0` bridge is not configured (for `chv-node`).
  - [ ] Wire preinstall scripts into nfpm config (`scripts.preinstall`).
- [ ] **Post-install health check**:
  - [ ] Add `scripts/chv-health-check.sh` (or binary subcommand) that validates:
    - All expected services are loaded by systemd.
    - `/var/lib/chv` is writable by `chv` user.
    - `/etc/chv/*.toml` files are readable and parseable.
    - Control plane can bind to its API port (if not already running).
  - [ ] Optionally run this in CI package tests and suggest it to admins in `docs/PACKAGING.md`.
- [ ] **Graceful shutdown ordering**:
  - [ ] Verify systemd unit dependencies: `chv-agent.service` should stop before `chv-stord.service` and `chv-nwd.service` during node shutdown (or define explicit `After=` / `Before=` ordering if needed).
- [ ] **Backup hook on upgrade**:
  - [ ] In postinstall scripts, create a timestamped backup of `/etc/chv/` to `/etc/chv/backups/` and `/var/lib/chv/controlplane.db` to `/var/lib/chv/backups/` before upgrading (if database file exists).

---

## Phase 6: Documentation

> Administrator-facing guides for installing, upgrading, and troubleshooting CHV via packages.

- [x] `docs/PACKAGING.md` — high-level packaging overview, quick install/upgrade/uninstall commands. *(implemented)*
- [ ] **Admin Install Guide** (`docs/release/admin-install-guide.md`):
  - [ ] Pre-requisites: supported OS versions (Ubuntu 22.04/24.04, Debian 12, RHEL 9, Rocky 9, Fedora 40+), hardware requirements (KVM, bridge interface), TLS cert generation.
  - [ ] Step-by-step install from APT repo (add key, add source, `apt update`, `apt install chv-controlplane chv-node chvctl`).
  - [ ] Step-by-step install from DNF repo (add `.repo` file, `dnf install chv-controlplane chv-node chvctl`).
  - [ ] Step-by-step install from GitHub Release (manual `.deb`/`.rpm` download).
  - [ ] Step-by-step tarball install using `install.sh`.
  - [ ] First-boot configuration: editing `controlplane.toml`, `agent.toml`, `stord.toml`, `nwd.toml`; generating mTLS certs; creating `chvbr0`; starting and enabling services.
  - [ ] Firewall and SELinux notes (if applicable).
- [ ] **Upgrade Guide** (`docs/release/upgrade-guide.md`):
  - [ ] Standard upgrade path: `apt upgrade` / `dnf upgrade`.
  - [ ] Manual upgrade path: download new packages, `dpkg -i` / `rpm -U`.
  - [ ] Pre-upgrade checklist: backup DB and configs, review changelog for breaking changes.
  - [ ] Post-upgrade verification: service status, health check command, UI accessibility.
  - [ ] Rollback instructions (link to `docs/release/rollback.md`).
- [ ] **Troubleshooting Guide** (`docs/release/troubleshooting.md`):
  - [ ] Common install failures: missing `kvm` group, `chvbr0` not found, port conflicts, permission denied on `/var/lib/chv`.
  - [ ] How to read systemd logs (`journalctl -u chv-controlplane`).
  - [ ] How to verify package integrity (`sha256sum -c`, `dpkg -V`, `rpm -V`).
  - [ ] How to manually run preinstall/postinstall scripts for debugging.
  - [ ] Uninstall and clean-up procedure (full wipe including `/var/lib/chv`).
- [ ] **Release Process Documentation** (`docs/release/release-process.md`):
  - [ ] Document the release checklist for maintainers: version bump (`make bump-version`), update `CHANGELOG.md`, tag `v*`, verify CI passes, approve production deploy.
  - [ ] Document branching/tagging conventions (`main` always releasable, `rc/*` tags for candidates).
  - [ ] Document secrets required in GitHub (GPG key, production environment reviewers).
- [ ] **Update `README.md`** at repo root to point to the new install guides instead of (or in addition to) the raw tarball instructions.

---

## Appendix: Quick-Reference File Paths

| Path | Purpose |
|------|---------|
| `VERSION` | Source of truth for release version |
| `CHANGELOG.md` | Human-readable release notes |
| `packaging/nfpm.yaml` | nfpm package template |
| `packaging/systemd/*.service` | systemd units shipped in packages |
| `packaging/scripts/*-{postinstall,preremove}.sh` | Package lifecycle scripts |
| `scripts/build-packages.sh` | Local `.deb`/`.rpm` builder |
| `scripts/build-release.sh` | Local tarball builder |
| `scripts/smoke-packages.sh` | Local package smoke tests |
| `scripts/install.sh` | All-in-one tarball installer |
| `.github/workflows/ci.yml` | Rust + UI CI |
| `.github/workflows/release.yml` | Tag-based release workflow |
| `docs/PACKAGING.md` | Existing packaging overview |

---

*When this checklist is complete, CHV will have a fully automated release pipeline: PR artifacts → nightly builds → RC drafts → signed stable releases published to APT/DNF repositories, with containerized install/upgrade/remove tests and comprehensive admin documentation.*
