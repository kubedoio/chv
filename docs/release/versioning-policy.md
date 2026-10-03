# CHV Versioning Policy

> Status: Active  
> Applies to: All CHV binaries, packages, and releases  
> Source of truth: `VERSION` file at repository root  

---

## 1. Overview

CHV uses **Semantic Versioning 2.0.0** (SemVer) with the format:

```
MAJOR.MINOR.PATCH
```

Example: `1.2.3`

In addition to stable releases, CHV publishes builds through **release channels** (`stable`, `rc`, `nightly`, `pr`). Channel information is embedded in package versions and CLI build metadata but does not alter the core SemVer identity of a release.

---

## 2. Source of Truth

The `VERSION` file at the repository root is the single source of truth for the project version.

All other version references are derived from it:

- `Cargo.toml` — crate `version` fields (all binary and library crates)
- `ui/package.json` — `version`
- Package filenames (`.deb`, `.rpm`)
- Git tags (`v<VERSION>`)
- CLI `--version` output

---

## 3. Release Channels

| Channel | Purpose | Example Version String |
|---------|---------|------------------------|
| `stable` | Production-ready releases | `<version>` |
| `rc` | Release candidates for soak-testing | `<version>~rc.1` (Debian), `<version>-0.1.rc1` (RPM) |
| `nightly` | Automated builds from `main` | `<version>~nightly.<date>.g<sha>` (Debian and RPM, as shipped by CI) |
| `pr` | Builds from pull requests | `<version>~pr<N>.<date>.g<sha>` (Debian and RPM, as shipped by CI) |

### 3.1 Package Version Mapping

The following table shows how `scripts/version.sh` maps a single SemVer to Debian and RPM version strings per channel. Note: CI currently derives one string with `version.sh --deb` (plus `CHV_PKG_PRERELEASE`) and stamps it on both formats, so the **as-shipped** RPM versions are the Debian-column strings.

| Channel | SemVer | Debian Version | RPM Version (`version.sh --rpm` channel path) |
|---------|--------|----------------|-------------|
| `stable` | `X.Y.Z` | `<version>` | `<version>` |
| `rc` | `X.Y.Z` | `<version>~rc.1` | `<version>-0.1.rc1` |
| `nightly` | `X.Y.Z` | `<version>~nightly.<date>.g<sha>` | `<version>^nightly.<date>.g<sha>` |
| `pr` | `X.Y.Z` | `<version>~pr<N>.<date>.g<sha>` | `<version>^pr<N>.<date>.g<sha>` |

> **Notes**
> - Debian uses `~` to ensure prereleases sort **before** the stable release in `dpkg --compare-versions`.
> - RPM carries RC in the release segment: `<version>-0.1.rc1` sorts before the stable `<version>-1`.
> - The `^` entries are the `version.sh --rpm` channel-argument output. In RPM version comparison, `^` is the post-release operator and sorts **after** the stable release with the same base version. **No workflow uses this path today** — CI stamps the Debian-derived `~` string on the RPM, which sorts before stable. If the pipeline ever adopts per-format version strings, RPM nightly ordering flips to above-stable (tracked issue).

### 3.2 Git Tags

| Channel | Tag Format | Example |
|---------|-----------|---------|
| `stable` | `v<VERSION>` | `v<version>` |
| `rc` | `v<VERSION>-rc.N` | `v<version>-rc.1` |
| `nightly` | Rolling `nightly` tag, overwritten on every build | `nightly` |
| `pr` | No tag; packages are uploaded as workflow artifacts | — |

---

## 4. SemVer Rules

### 4.1 Version Components

| Component | When to Bump | Examples |
|-----------|-------------|----------|
| **MAJOR** | Incompatible API or database schema changes | Dropping a gRPC method, removing a DB column, changing protobuf wire format |
| **MINOR** | New features, backward-compatible | Adding a backend-for-frontend (BFF) endpoint, a new Cloud Hypervisor (the VMM) VM flag, a new UI page |
| **PATCH** | Bug fixes, backward-compatible | Fixing a race condition, correcting a SQL query, UI layout fix |

### 4.2 Pre-1.0 Exception

While `MAJOR` is `0`, MINOR releases **may** contain breaking changes. Breaking changes during the `0.x` series **must** be documented in:

- `CHANGELOG.md` under a `Changed` or `Removed` subsection with a **"Breaking:"** prefix
- Release notes with a prominent migration note

Once `1.0.0` is reached, breaking changes are restricted to MAJOR bumps only.

---

## 5. When to Bump

| Event | Bump |
|-------|------|
| Every merged bug fix | `PATCH` |
| Every merged feature | `MINOR` |
| Breaking change (post-1.0) | `MAJOR` |
| Release candidate cut | Append `-rc.N` to the tag; package prerelease suffix handled by CI |

Bump the version **before** cutting a release tag, not after.

---

## 6. Changelog Rules

CHV follows the **[Keep a Changelog](https://keepachangelog.com/)** format.

### Structure

```markdown
## [Unreleased]

### Added
- ...

### Changed
- ...

### Deprecated
- ...

### Removed
- ...

### Fixed
- ...

### Security
- ...
```

### Workflow

1. **During development**: Accumulate changes under `[Unreleased]` in the appropriate category.
2. **On release**:
   - Rename `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD`.
   - Add a new empty `[Unreleased]` section at the top.
   - Ensure the date is the day the release tag is pushed.

---

## 7. Environment Variables

The build and packaging pipeline respects the following environment variables:

| Variable | Values | Description |
|----------|--------|-------------|
| `CHV_RELEASE_CHANNEL` | `stable` (default), `rc`, `nightly`, `pr` | Selects the release channel. Affects package prerelease suffixes and CLI build metadata. |
| `CHV_PKG_PRERELEASE` | Any valid prerelease string | Overrides the auto-generated prerelease suffix in package filenames. Used by CI when cutting RCs or nightlies. |

### Example: Nightly Build

```bash
export CHV_RELEASE_CHANNEL=nightly
export CHV_PKG_PRERELEASE="nightly.$(date +%Y%m%d).g$(git rev-parse --short HEAD)"
./scripts/build-packages.sh
```

---

## 8. Version Validation

CI enforces version consistency on every pull request and push to `main`:

1. `VERSION` must match `^\d+\.\d+\.\d+$`.
2. All `Cargo.toml` `version` fields must equal `VERSION`.
3. `chvctl --version` must contain the expected version, git SHA, build date, and channel.

These checks run in `.github/workflows/ci.yml` after `cargo test`.

---

## 9. Migration from Legacy 4-Segment Versions

CHV previously used a four-segment scheme (`0.0.0.4`). All releases from `0.1.0` onward use strict SemVer (`MAJOR.MINOR.PATCH`). Historical releases retain their original version numbers in `CHANGELOG.md`.
