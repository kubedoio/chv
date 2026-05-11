# CHV Release Channels

CHV distributes packages through multiple channels. Choose the channel that matches your risk tolerance and use case.

## Channel comparison

| Channel | Stability | Use case | Source |
|---------|-----------|----------|--------|
| **Stable** | Production-ready | Production deployments, long-term evaluation | GitHub Release (tagged) |
| **RC** | Pre-release | Validation before stable, integration testing | GitHub Pre-release (tagged) |
| **Nightly** | Unstable | Development, feature preview, CI integration | GitHub Nightly pre-release |
| **PR** | Experimental | Testing specific changes before merge | GitHub Actions artifacts |

## Stable

Stable releases are tagged with SemVer versions: `v0.1.0`, `v0.2.0`, etc.

- **Quality:** Full CI pipeline passes, container smoke tests pass, lifecycle tests pass, changelog entry required.
- **Artifacts:** `.deb`, `.rpm`, tarball, checksums, SBOM, build provenance attestation.
- **Support:** Best-effort community support. Security fixes are backported to the latest stable minor version.
- **Upgrade path:** Forward upgrades to newer stable versions are safe. Persistent data and configs are preserved.

### Install stable

```bash
VERSION="0.1.0"
BASE_URL="https://github.com/chv-project/chv/releases/download/v${VERSION}"
curl -sLO "${BASE_URL}/chv-controlplane_${VERSION}_amd64.deb"
curl -sLO "${BASE_URL}/chv-node_${VERSION}_amd64.deb"
curl -sLO "${BASE_URL}/chvctl_${VERSION}_amd64.deb"
sudo dpkg -i chv-controlplane_${VERSION}_amd64.deb chv-node_${VERSION}_amd64.deb chvctl_${VERSION}_amd64.deb
```

Full instructions: [Debian / Ubuntu](debian-ubuntu.md) or [RHEL / Rocky / AlmaLinux](rhel-rocky-alma.md)

## RC (Release Candidate)

RC releases are tagged as `v0.1.0-rc.1`, `v0.1.0-rc.2`, etc.

- **Quality:** Same CI pipeline as stable, but may contain unfinished edge cases.
- **Artifacts:** Same as stable.
- **Support:** Community support. RCs are intended for validation, not production.
- **Upgrade path:** Can upgrade to the final stable release with the same minor version.

### When to use RC

- You need a specific fix or feature that is not yet in stable.
- You are validating CHV in a staging environment before a stable release.
- You are a contributor testing the release pipeline.

### Install RC

Download from the GitHub Pre-release page. The install command is identical to stable.

## Nightly

Nightly packages are built automatically from every merge to `main`.

- **Quality:** Automated tests pass, but the code may contain regressions, breaking changes, or incomplete features.
- **Artifacts:** `.deb`, `.rpm`, checksums. SBOM is generated. Provenance attestation may be generated.
- **Support:** No support guarantee. File issues against the specific commit if you find bugs.
- **Upgrade path:** Can upgrade to RC or stable. Nightly versions sort before RC and stable in package manager version ordering.

### Version format

```text
0.1.0~nightly.20260510.g0872c4a7   (Debian)
0.1.0^nightly.20260510.g0872c4a7   (RPM)
```

The version includes the date and git short SHA, making every nightly build uniquely identifiable.

### When to use nightly

- You want to test the latest changes.
- You are developing integrations against CHV and need bleeding-edge APIs.
- You are a contributor validating a fix on real hardware.

### Install nightly

Download from the rolling [CHV Nightly](https://github.com/chv-project/chv/releases/tag/nightly) GitHub pre-release.

> **Warning:** Do not use nightly packages in production. Use them only on disposable test hosts or VMs.

## PR artifacts

Every pull request to `main` triggers a package build. The packages are uploaded as GitHub Actions artifacts with 7-day retention.

- **Quality:** The PR's CI passes, but the code is unmerged and may be rejected or revised.
- **Artifacts:** `.deb`, `.rpm`, checksums.
- **Support:** No support. These artifacts are for manual testing by reviewers and contributors.
- **Retention:** 7 days.

### When to use PR artifacts

- You are reviewing a PR and want to test the changes on real hardware.
- You are a contributor sharing a build with a reviewer.

### Install PR artifacts

1. Go to the PR's GitHub Actions page.
2. Find the "PR Packages" workflow run.
3. Download the artifact (`chv-packages-pr-N`).
4. Install the `.deb` or `.rpm` files manually.

## Version precedence

Package managers order versions from oldest to newest:

```text
nightly < RC < stable
```

Examples:
- `0.1.0~nightly.20260510.g0872c4a7` < `0.1.0~rc.1` < `0.1.0`
- `0.1.0-0.1.rc1` < `0.1.0` (RPM)

This means upgrading from nightly → RC → stable is always a forward upgrade.

## Switching channels

### Nightly → Stable

Install the stable release over the nightly package:

```bash
# Debian/Ubuntu
sudo dpkg -i chv-controlplane_0.1.0_amd64.deb chv-node_0.1.0_amd64.deb chvctl_0.1.0_amd64.deb

# RHEL/Rocky/Alma
sudo rpm -U chv-controlplane-0.1.0-1.x86_64.rpm chv-node-0.1.0-1.x86_64.rpm chvctl-0.1.0-1.x86_64.rpm
```

### Stable → Nightly (not recommended)

Downgrading from stable to nightly is possible but not recommended. The package manager may require `--force` flags.

## Repository publishing (future)

Once apt and yum repositories are configured, you will be able to install CHV using standard package manager commands:

```bash
# apt (future)
sudo apt install chv-controlplane chv-node chvctl

# dnf (future)
sudo dnf install chv-controlplane chv-node chvctl
```

See [Nightly Packages](../release/nightly-packages.md) for repository configuration details.

## See also

- [Release Process](../release/release-process.md) — how releases are built and published
- [Verify Release Artifacts](../release/verify-release-artifacts.md) — checksum and signature verification
- [Nightly Packages](../release/nightly-packages.md) — nightly build internals
