# Prompt 05 — Publish and Verify the First Real Release Candidate

Use the existing release machinery to publish the exact qualified commit as an actual RC and verify the public consumption path end to end.

## Preconditions

- Prompts 01–04 pass for the exact candidate SHA.
- No unresolved blocker is being hidden by release notes.
- Re-read the current release workflow and install/release documentation.
- Verify the current repository owner/URLs rather than copying historical values.
- Choose the next valid RC version from the current `VERSION`, tags, changelog, and versioning policy; do not hard-code an obsolete target from this prompt.

## Goal

Create one real GitHub Release candidate whose artifacts are the same artifacts used for clean-host verification.

The release is not complete when CI builds packages. It is complete when an external operator can discover, verify, download, install, run, upgrade, and troubleshoot them using the published documentation.

## Required work

### 1. Release preflight

Verify:

- candidate SHA equals the qualified SHA;
- working tree/tag input is immutable;
- required CI, Security, package, and real-host gates pass;
- release notes describe actual support level and known limitations;
- install URLs point to the real repository;
- no placeholder security/contact/version information remains;
- release environment and signing/provenance permissions are configured.

### 2. Publish RC

Use the repository-supported release path.

Expected artifacts should include the supported set of:

- tarball/binaries;
- DEB packages;
- RPM packages;
- checksums/signatures;
- SBOM(s);
- provenance/attestations.

Do not manually substitute different locally built binaries after qualification.

### 3. Public artifact verification

From a clean environment that does not rely on the source checkout:

- discover the release from GitHub;
- download artifacts from the published release;
- verify checksums/signatures/attestations using documented commands;
- inspect package metadata and version;
- install using public documentation;
- start services;
- enroll/run the reference minimum workload;
- uninstall/clean up according to documentation.

Record every stale URL, hidden prerequisite, undocumented file permission, missing package, and manual workaround as a defect.

### 4. Upgrade/rollback path

Using published artifacts:

- install the previous supported/qualified version where one exists;
- upgrade to the RC;
- preserve supported workload identity/state;
- validate schema/config migration behavior;
- perform the documented rollback where supported;
- when rollback is intentionally unsupported after a schema boundary, prove the documented recovery path rather than pretending rollback exists.

### 5. Release metadata truth

Publish an explicit support matrix in the release notes or linked documentation:

- host OS profile;
- Cloud Hypervisor version/profile;
- lifecycle maturity;
- network profile;
- storage profiles and evidence level;
- migration support;
- backup/restore support;
- upgrade path;
- known unsupported features.

Do not label a CODED or CI-only capability as production-qualified.

## Acceptance criteria

- GitHub Release exists for the RC.
- Artifacts are publicly retrievable at documented locations.
- checksum/signature/provenance verification succeeds.
- clean-host install from release artifacts succeeds.
- reference VM lifecycle succeeds from the installed packages.
- supported upgrade path succeeds.
- rollback/recovery behavior matches documentation.
- release notes and support matrix match evidence.
- all public documentation used in the test is reproducible without source-tree knowledge.

## Forbidden outcomes

- publishing before real-host qualification;
- rebuilding different binaries after qualification;
- using unpublished CI artifacts as proof of the public release path;
- claiming stable/production status because an RC was published;
- hiding unsupported rollback or restore behavior;
- leaving dead repository URLs in install/verification documentation.

## Exit gate

Prompt 05 passes when the exact qualified candidate is a real, verifiable GitHub Release candidate and clean hosts can consume it using only supported public artifacts and documentation.
