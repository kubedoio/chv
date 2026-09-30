# CHV Production-Readiness — Prompt 03 Security / Reproducibility / Governance Plan

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [03-security-reproducibility-governance](/docs/prompts/production-readiness/03-security-reproducibility-governance.md)
> Capability maturity: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Issue anchors: #229 (toolchain + generated-code policy), #230 (advisory truth),
> #146 / #177 (advisory debt), #233 (closed — typed startup validation, PR #253).

---

## 1. Baseline freeze (grounded revalidation, 2026-09-30)

All facts below were re-derived from current `main` and the live GitHub state on
2026-09-30 — not from roadmap labels (Prompt 00 source-of-truth rule).

| Item | Value |
|---|---|
| Baseline `main` SHA | `df256bc4` (M2.5 run 10/10b evidence follow-up) |
| `VERSION` | `0.2.0` (campaign target line: `v0.3.0-rc1`) |
| CI on baseline | success (`df256bc4`, 2026-09-29) |
| Security workflow on baseline | success (`df256bc4`, 2026-09-29 and 2026-09-30 scheduled run) |
| Nightly Packages on baseline | success (`df256bc4`) |
| GitHub Releases | **none** — only the `nightly` pre-release (2026-09-28); no versioned release exists |
| Latest real-KVM evidence | M2.5 run 10b, 62 PASS / 0 FAIL on `b95b90fb` (`02-single-authority-cutover/m2.5-kvm-qualification.md`) |
| Toolchain used by CI | floating `dtolnay/rust-toolchain@stable`; **no `rust-toolchain.toml` exists** |
| Maturity of this prompt's scope | all workstreams CODED or NOT-STARTED as itemized below; nothing here requires KVM tiers — this prompt is CI-VERIFIED tier by nature |

Issue-anchor revalidation:

- **#233 — CLOSED** (2026-09-26) by PR #253 (`8c04bf50`): typed
  `validate_security_mode` + `validate_tls` in `chv-controlplane-service`, called
  from `build_service` **before** any listener/interceptor is constructed;
  `PeerIdentityInterceptor::new` fallible; unit tests present.
- **#229 — OPEN.** Confirmed current facts: no `rust-toolchain.toml`;
  `CONTRIBUTING.md` line 9 says "latest stable via rustup"; all five Rust
  workflows (`ci.yml`, `integration-kvm.yml`, `package-nightly.yml`,
  `package-pr.yml`, `release.yml`) select floating stable; the generated crates'
  `#![allow(clippy::result_large_err)]` wrapper mechanism (see §2-B) exists.
- **#230 — OPEN.** `deny.toml` carries six documented advisory ignores; the
  Security workflow passes on the baseline SHA, so policy and lockfile are
  currently consistent. The remaining work is per-entry revalidation and honest
  debt labeling (#146, #177), not green-by-broadening.
- **#146 / #177 — OPEN** (explicit debt, referenced from `deny.toml`).

## 2. Workstream findings and plan

### A — Typed fail-closed startup (#233 successor work)

**Already delivered by #253:** typed `ControlPlaneServiceError::InsecureModeLockedOut`
returned from `validate_security_mode`, invoked before `validate_tls` and before
listener/interceptor construction; fallible interceptor as defense-in-depth;
unit tests for all three modes.

**Gaps against the prompt's requirements (the remaining scope):**

1. **Process-level test.** #253 proved the typed path with unit tests only. The
   prompt requires a process-level test proving *non-zero clean exit without a
   panic/backtrace path*. Plan: spawn the `chv-controlplane` binary (non-`dev`
   build) with `CHV_ALLOW_INSECURE=1` and no TLS config; assert exit code != 0,
   stderr contains the greppable `CHV_ALLOW_INSECURE` guidance, and contains no
   `panicked at` / backtrace. Follow the existing process-smoke conventions in
   `ci.yml` ("Build chv-agent process smoke binary" / "Test core-native
   chv-agent process lifecycle").
2. **Agent-side insecure switch assessment.** `cmd/chv-agent/src/main.rs` (~line
   753) reads `CHV_ALLOW_INSECURE` as a pure **runtime** env switch (typed
   `Err` return — clean exit, no panic) with **no `dev` feature gating**, unlike
   the controlplane's compile-time lockout. The prompt requires "insecure mode
   remains compile-time development-only" and "no production runtime switch
   bypasses peer identity". Assessment needed: either gate the agent's insecure
   path behind the `dev` feature like the controlplane, or document why the
   enrollment-bootstrap context makes the runtime switch safe — with tests. The
   decision must not weaken mTLS on enrolled nodes.
3. **Error-text hygiene.** The process test asserts actionable text and absence
   of secret material (paths/keys) in the failure output.

### B — Pin Rust and generated-code policy (#229)

Facts: no `rust-toolchain.toml`; five workflows on `dtolnay/rust-toolchain@stable`
(floating — CI behavior can change without any repository commit, the exact #229
reproducibility complaint); generated crates under `gen/rust/*/src/lib.rs` are
repo-owned wrappers carrying `#![allow(clippy::result_large_err)]`, which is
already the narrowest generated-code lint mechanism (tonic-build output is
included under a crate root we own; generated files are never hand-patched).

Plan (closes #229):

1. Add `rust-toolchain.toml` pinning the exact reviewed toolchain —
   `1.98.1` (2026-09-01), the stable the baseline SHA's CI runs actually used
   (CI/Security/Nightly green on it).
2. Switch the five workflows from `dtolnay/rust-toolchain@stable` to the
   action's `@master` ref (which reads `rust-toolchain.toml`) — one selection
   source. Coordinate with Workstream F: the `@master` ref is then SHA-pinned
   there, so the two edits land in a defined order (B first, F on top).
3. `CONTRIBUTING.md` / `AGENTS.md`: replace "latest stable" guidance with the
   file-driven toolchain (rustup picks `rust-toolchain.toml` automatically).
4. Document the toolchain-bump procedure (reviewable PR bumping the pin; CI is
   the verification; cadence and MSRV coordination with the deferred tonic
   0.12→0.14 migration per `deny.toml`'s rustls-pemfile removal condition).
5. Verify handwritten code stays `-D warnings` and the generated crates keep the
   narrow crate-root allow (no global `allow(warnings)` — forbidden outcome).

### C — Advisory-policy truth (#230)

Facts: Security workflow green on baseline → `deny.toml` matches the lockfile
today. Six ignores, each with (a)/(b)/(c) documentation:

| Advisory | Crate | Path | Removal condition |
|---|---|---|---|
| RUSTSEC-2023-0071 | `rsa` | sqlx 0.8 top-level mysql dep (not compiled in) | sqlx makes sqlx-mysql/-postgres optional |
| RUSTSEC-2026-0173 | `proc-macro-error2` | `tabled_derive` → chvctl (build-time only) | chvctl tabled migration (#146) |
| RUSTSEC-2025-0134 | `rustls-pemfile` | tonic 0.12 client TLS | tonic 0.12 → 0.14 upgrade |
| RUSTSEC-2026-0204 | `crossbeam-epoch` | rayon-core/criterion (dev/bench only) | criterion/rayon update |
| RUSTSEC-2026-0194 | `quick-xml` | transitive | next quick-xml update (#177) |
| RUSTSEC-2026-0195 | `quick-xml` | transitive | next quick-xml update (#177) |

Plan: revalidate each entry against the current `Cargo.lock` (crate present?
justification still true? removal condition still tracked?); remove any ignore
whose dependency path is gone; record the reconciliation as evidence here.
#146/#177 remain explicit debt — do **not** broaden ignores to keep Security
green (forbidden outcome). Full Security workflow must pass after cleanup.

### D — Vulnerability reporting

Facts (live GitHub state, 2026-09-30):

- `SECURITY.md`'s alternative email channel is a **placeholder**
  (`security@<your-domain>` with a `MAINTAINER: replace` comment) — not a real
  reporting path.
- **GitHub private vulnerability reporting is disabled**
  (`GET /private-vulnerability-reporting` → `{"enabled":false}`) — so the
  *preferred* channel named by `SECURITY.md` does not currently work either.

Plan: enable private vulnerability reporting on the repository (API change,
evidence recorded here); replace the placeholder email with a real maintained
address or drop the email channel in favor of the (now-working) GitHub advisory
path — maintainer decision; verify the reported instructions in `SECURITY.md`
match the actual enabled mechanisms; publish no keys/personal data.

### E — Repository protection

Facts (live GitHub state, 2026-09-30):

- Ruleset `protect-main` (id `17358522`, created 2026-06-06, updated
  2026-06-13) is **fully defined but `enforcement: disabled`**. Its rules:
  deletion, non-fast-forward, required signatures, pull-request reviews
  (1 approval, code-owner review, stale-review dismissal, review-thread
  resolution, extra approval for unattributed changes; merge/squash/rebase
  allowed), and required status checks.
- The ruleset's required status checks — `Rust checks`, `UI checks`,
  `E2E tests` — **match the current `ci.yml` job names** (verified against
  `ci.yml` jobs `rust`/`ui`/`e2e`).
- Gaps: the **Security workflow's checks are not required**; and the classic
  branch-protection API reports `main` unprotected because the (disabled)
  ruleset is the only protection object.
- Operational prerequisite: the `required_signatures` rule means every commit
  on `main` must carry a verified signature once enforcement is on — verify
  commit-signing works for all committers before enabling, or scope the rule.

Plan: add the Security workflow's check (`cargo audit`, and the `cargo deny`
matrix jobs) to the required list; then set the ruleset to `active`; record the
verification (API responses) as evidence. **Sequenced last** among the PRs
below — once enforcement is on, every subsequent campaign PR needs the full
review/CI/signature gauntlet, including ours.

### F — Supply-chain workflow pinning

Facts: every action in every workflow is tag-pinned (`@v7`, `@v2`, `@v0`, …),
not commit-SHA pinned; `security.yml` explicitly carries
`SHA: <maintainer: pin to a commit SHA on next review>` TODO comments;
`dependabot.yml` already covers the `github-actions` ecosystem (monthly), so
SHA pinning does not lose updateability.

Plan: pin the security/release-critical actions to immutable commit SHAs with
the tag recorded in a comment — priority order: `release.yml`
(`anchore/sbom-action`, `actions/attest-build-provenance`,
`softprops/action-gh-release`, `actions/download-artifact`,
`actions/upload-artifact`, `arduino/setup-protoc`, `actions/checkout`) and
`security.yml` (`EmbarkStudios/cargo-deny-action`), then the remaining
workflow actions. Provenance/signing functionality must not be reduced
(forbidden outcome).

## 3. PR sequence (narrow, independent risk domains)

| # | Branch | Workstream | Content |
|---|---|---|---|
| PR-0 | `p03/security-reproducibility-governance` | — | this plan + baseline evidence (docs only) |
| PR-1 | `p03/toolchain-pin` | B | `rust-toolchain.toml`, workflow consumption, docs, bump procedure (closes #229) |
| PR-2 | `p03/fail-closed-startup-process-test` | A | process-level #233 test + agent insecure-switch assessment |
| PR-3 | `p03/advisory-truth` | C | per-ignore lockfile revalidation evidence (+ removals if any) (closes #230 if clean) |
| PR-4 | `p03/vulnerability-reporting` | D | enable private vulnerability reporting + `SECURITY.md` real path |
| PR-5 | `p03/actions-sha-pinning` | F | SHA-pin security/release-critical actions |
| PR-6 | `p03/main-protection` | E | required Security checks + ruleset enforcement + verification evidence |

Ordering rationale: B before F (F pins the `@master` ref B introduces); E last
(enforcement changes merge mechanics for every later PR, including ours).

## 4. Evidence matrix (Prompt 03 acceptance)

| Criterion | Evidence |
|---|---|
| Invalid insecure production config exits cleanly through typed startup handling | #253 (merged) + PR-2 process test |
| Exact Rust toolchain is repository-controlled | PR-1 (`rust-toolchain.toml` + workflow consumption) |
| Generated-code policy is deterministic | PR-1 (verified narrow allow; bump procedure doc) |
| Advisory exceptions match the actual dependency graph | PR-3 reconciliation + green Security workflow on the merged SHA |
| Security workflow passes | baseline `df256bc4` green; re-run on each PR SHA |
| A real vulnerability-reporting path exists | PR-4 (enabled private reporting; `SECURITY.md` truth) |
| Effective `main` protection is verified or the gap is explicitly recorded | PR-6 (ruleset active + API evidence) — until then: **recorded gap — ruleset defined but enforcement disabled** |
| Release/security workflow dependencies have a documented pin/update policy | PR-5 + existing dependabot `github-actions` config |
| Contributor instructions match CI/release behavior | PR-1 (`CONTRIBUTING.md`/`AGENTS.md` toolchain truth) |

## 5. Scope and non-scope

- **In scope:** the six workstreams above, at CI-VERIFIED tier; repository
  settings changes (private reporting, ruleset enforcement) executed and
  evidenced via API responses.
- **Non-scope:** the tonic 0.12→0.14 migration and the #146/#177 dependency
  removals themselves (separate engineering, tracked); any KVM/multi-host
  qualification (Prompt 04); release publication (Prompt 05); no second
  runtime authority; no feature expansion (campaign non-goal).
- **Residual risks:** enforcing `required_signatures` may block committers
  without signing configured (mitigated by pre-enabling verification and the
  maintainers' direct-push discipline); SHA pinning depends on the actions'
  upstream repos retaining tagged commits (accepted, dependabot-monitored).

## 6. Status

**Prompt 03 is functionally complete.** All six workstreams merged to `main`
(2026-09-30).

- **PR-0 (#302, `2d4853ed`)** — this plan + baseline evidence.
- **Workstream A (#304, `30ec55ad`)** — agent insecure-mode dev gating (#253
  mirror) + process-level fail-closed startup smoke (S1/S2/S3).
- **Workstream B (#303, `43e436e4`)** — `rust-toolchain.toml` pin (1.98.1) +
  composite `setup-rust` action; closed #229.
- **Workstream C (#305, `5ca3af8a`)** — advisory-truth cleanup: removed 3
  stale ignores, `unused-ignored-advisory = "deny"`; closed #230, #146.
  **Superseded in part by #312** (see below) — #312 found the `cargo audit`
  CI job had never run cargo-audit and fixed `event-listener` 5.4.2
  (RUSTSEC-2026-0221); evidence addendum in `advisory-truth.md` §6.
- **Workstream D (#306, `d678e478`)** — GitHub-only vulnerability reporting;
  private reporting enabled on the repo; placeholder email removed.
- **Workstream E (#311, `2ab969ae`)** — ruleset-based `main` protection:
  SHA/required-check manifest, `security.yml` path filter removed, rewritten
  `apply-branch-protection.sh` (rulesets API, `--audit`/`--enforce`, guard
  on unsigned commits) + `verify-settings.sh`. **Live:** the `protect-main`
  ruleset definition now requires all eight checks; **enforcement remains
  staged/disabled** pending universal commit signing (recorded gap —
  see `main-protection.md`).
- **Workstream F (#307, `ef4188d6`)** — all third-party actions SHA-pinned.
- **Reconciled #301 → #312 (`3de5069a`)** — senolcolak's deeper advisory
  truth (real `cargo audit` job, `event-listener` 5.4.2, audit.toml/deny.toml
  semantic separation) rebased onto the merged queue with authorship
  preserved; closes #230, #146 on the record.

**Workstream E final step (open, maintainer action):** enable enforcement once
commit signing is configured for all committers —
`./scripts/github-setup/apply-branch-protection.sh --enforce` — and record the
verification in `main-protection.md`. Prompt 03's evidence links (§1 Evidence
matrix) all point to the merged artifacts above.
