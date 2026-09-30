# Prompt 03 Workstream F — Supply-chain workflow pinning — evidence

> Parent: [plan.md](plan.md) §2-F. Baseline: `main` `df256bc4` (2026-09-30).

## Findings (before)

- Every third-party action in every workflow was tag-pinned (`@v7`, `@v2`,
  `@v0`, …) — mutable refs that can be force-moved by a compromised upstream.
- `security.yml` carried explicit unresolved TODOs:
  `SHA: <maintainer: pin to a commit SHA on next review>`.
- `dependabot.yml` already covered the `github-actions` ecosystem (monthly),
  so SHA pinning would not lose updateability.

## Change

All **54** third-party action uses across the seven workflows are now pinned
to immutable commit SHAs, with the moving tag and pin date recorded in a
trailing comment on each `uses:` line (the format Dependabot's
github-actions updater recognizes — it proposes SHA bumps as new versions
release). SHAs were resolved from each action repository's current ref via
the GitHub API on 2026-09-30:

| Action | Ref | Pinned SHA |
|---|---|---|
| actions/checkout | v7 | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| Swatinem/rust-cache | v2 | `6323deb102c322ba6fcbdcafc7e3dddab59af2b6` |
| actions/setup-node | v7 | `820762786026740c76f36085b0efc47a31fe5020` |
| actions/download-artifact | v8 | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` |
| actions/upload-artifact | v7 | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` |
| arduino/setup-protoc | v3 | `c65c819552d16ad3c9b72d9dfd5ba5237b9c906b` |
| softprops/action-gh-release | v3 | `efb35369e0ad2afab669f228072c1b0d510eae64` |
| anchore/sbom-action | v0 | `e22c389904149dbc22b58101806040fa8d37a610` |
| actions/attest-build-provenance | v4 | `4d101475d8b20a2381f78447822ac1eab6504dd8` |
| bufbuild/buf-setup-action | v1 | `a47c93e0b1648d5651a065437926377d060baa99` |
| actions/cache | v6 | `55cc8345863c7cc4c66a329aec7e433d2d1c52a9` |
| EmbarkStudios/cargo-deny-action | v2 | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` |

Per-workflow counts: ci 6, integration-kvm 5, package-nightly 10,
package-pr 5, proto 2, release 20, security 6.

- `security.yml`'s stale header TODO block replaced with the pin policy.
- `.github/dependabot.yml`'s `github-actions` section documents the policy:
  new third-party actions must be SHA-pinned the same way; tag-pinned actions
  are forbidden in security- and release-critical workflows.
- The only remaining third-party action after this PR and PR #303 (toolchain
  pin) merge is none: #303 replaces `dtolnay/rust-toolchain@stable` with the
  in-repo composite action, which is repo-owned and inherently immutable.
  Until #303 merges, the five `dtolnay/rust-toolchain@stable` uses are the
  one known unpinned third-party action.

## Provenance/signing preserved

`anchore/sbom-action`, `actions/attest-build-provenance`, and
`softprops/action-gh-release` are pinned to the SHAs their tags pointed to on
2026-09-30 — the same versions that produced the green baseline runs. No
provenance or signing functionality was reduced or reordered.

## Rollback

Revert; workflows return to tag refs.

## Residual risk

Pinned SHAs can drift behind upstream security fixes until Dependabot's
monthly github-actions pass proposes updates (accepted: monitored channel,
same trade-off as any pinned supply chain). Note some refs are moving major
branches (`v7`, `v2`, `v0`), so the comment tags denote major versions, not
exact releases — Dependabot resolves the exact version on update.
