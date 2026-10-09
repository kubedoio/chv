# Prompt 03 Workstream C — Advisory-policy truth — evidence

> Parent: [plan.md](plan.md) §2-C. Issue anchors: #230 (this workstream),
> #146 (proc-macro-error2 debt — **resolved by lockfile drift, closed**),
> #177 (quick-xml debt — **remains, kept explicit**).
> Baseline: `main` `df256bc4` (2026-09-30). Tool: cargo-deny 0.20.2
> (`cargo deny --all-features check advisories`).

## 1. Method

Every `deny.toml` advisory ignore was revalidated against:

1. the current `Cargo.lock` (package present? which version?);
2. the actual resolved dependency graph (`cargo tree -i <crate>` — default,
   `--all-features`, and `--all-features --target all`);
3. the upstream RustSec advisory record (affected/patched version ranges);
4. cargo-deny's own verdict on whether the ignore matches anything
   (`advisory-not-detected` warnings).

## 2. Reconciliation table

| Advisory | Crate (lockfile) | Resolved graph | Upstream range | Verdict |
|---|---|---|---|---|
| RUSTSEC-2023-0071 (rsa Marvin Attack) | `rsa` 0.9.10 | **unreachable** (not in the graph under default, `--all-features`, or `--all-features --target all` resolution; lockfile entry is stale residue) | no upstream patch | **ignore removed** — cargo-deny: `advisory-not-detected` |
| RUSTSEC-2026-0173 (proc-macro-error2 unmaintained) | **absent from lockfile** | n/a | n/a | **ignore removed** — `tabled` 0.22 no longer pulls a `proc-macro-error*` helper; #146's removal condition is met by dependency drift, no code change was needed. **#146 closed.** |
| RUSTSEC-2025-0134 (rustls-pemfile 2.x unmaintained) | `rustls-pemfile` 2.2.0 | via `tonic` 0.12.3 → production crates (agent, controlplane, errors, …) | folded into `rustls-pki-types` ≥ 1.9; tonic 0.13+ migrated | **ignore kept** — still applicable; removal condition unchanged (tonic 0.12 → 0.14 upgrade, tracked separately) |
| RUSTSEC-2026-0204 (crossbeam-epoch pointer deref) | `crossbeam-epoch` **0.9.21** | via `rayon` → `criterion` — **dev-dependency only** (`chv-architecture-validate` benches/tests; never shipped) | **patched ≥ 0.9.20** — 0.9.21 is not affected | **ignore removed** — the advisory no longer matches the installed version |
| RUSTSEC-2026-0194 (quick-xml O(N²) attr check) | `quick-xml` 0.38.4 | via `rust-s3` 0.37.2 → `chv-controlplane-service` (**production**) | patched ≥ 0.41.0 — 0.38.4 **affected** | **ignore kept** — justification tightened: quick-xml parses S3 responses from the operator-configured endpoint, not untrusted input; removal condition made concrete (quick-xml ≥ 0.41.0 via rust-s3 update, #177) |
| RUSTSEC-2026-0195 (quick-xml NsReader unbounded alloc) | `quick-xml` 0.38.4 | same path | patched ≥ 0.41.0 — 0.38.4 **affected** | **ignore kept** — same rationale and removal condition (#177) |

## 3. Policy hardening

`[advisories] unused-ignored-advisory = "deny"` was added: an ignore that no
longer matches any advisory in the actual graph now **fails** the Security
workflow instead of warning, so stale exceptions cannot silently accumulate
again. A dependency removal/upgrade that resolves an advisory must remove its
ignore in the same change — the gate enforces it.

## 4. Verification

- `cargo deny --all-features check advisories` → `advisories ok`, **zero**
  `advisory-not-detected` warnings (previously 3).
- `cargo deny --all-features check` → `advisories ok, bans ok, licenses ok,
  sources ok`.
- The Security workflow on this PR is the CI-tier proof.

## 5. Residual debt (explicit, per the prompt's forbidden outcomes)

- **#177 (quick-xml)**: 0.38.4 is genuinely affected by two DoS-class
  advisories; non-applicability rests on the S3-response trust boundary. The
  real cure is rust-s3 moving to quick-xml ≥ 0.41.0 (or CHV moving off
  rust-s3). Kept as documented debt — not broadened, not hidden.
- **rustls-pemfile**: unmaintained crate in the production gRPC path until the
  tonic 0.12 → 0.14 migration lands (tracked in the ignore's removal
  condition and the deferred #235-adjacent work).
- No ignore was kept "just in case"; no ignore was broadened to make Security
  green.

## 6. Addendum (2026-09-30) — reconciliation with senolcolak's deeper audit (PR #301)

PR #301 (rebased onto this queue) found what the cargo-deny-only view above
missed, and its findings supersede parts of §1–§4:

1. **The `cargo audit` CI job never ran cargo-audit** — it ran cargo-deny a
   second time, and `.cargo/audit.toml` was dead config whose header falsely
   claimed the workflow consumed it. The job now runs the real
   `rustsec/audit-check@v2` reading `.cargo/audit.toml`. This is not
   redundant with cargo-deny: cargo-audit walks the raw lockfile with no
   reachability filtering.
2. **event-listener RUSTSEC-2026-0221** (unsound `StackSlot` `Send`/`Sync`,
   patched 5.4.2) was invisible to cargo-deny 0.20.2 but reported by
   cargo-audit; fixed by a single-package lockfile bump to 5.4.2.
3. **The two tools have intentionally different ignore sets** — graph
   reachability (deny.toml) vs raw lockfile (audit.toml). Notably
   RUSTSEC-2023-0071 (`rsa`) is correctly absent from deny.toml (unreachable)
   but retained in audit.toml (present in Cargo.lock). §2's "ignore removed"
   verdict for rsa is therefore deny.toml-scoped only. Reconcile each file
   against its own tool; the old "mirrored verbatim" comments were untrue.
4. cargo-deny's `db-path = "~/.cargo/advisory-db"` tilde non-expansion cloned
   the advisory DB into a literal `./~` tree; the override was removed.

Verification on the reconciled tree: `cargo deny --all-features check
advisories` → ok; `cargo audit` → clean (exit 0).

## 7. Addendum (2026-10-08) — rustls-pemfile residual debt resolved (#235)

The §5 residual-debt line for `rustls-pemfile` is resolved, via the two-PR
plan tracked in issue #235:

1. **PR 1 (#574)**: the tonic 0.12 → 0.14.6 / prost 0.13 → 0.14.4 stack bump
   removed the transitive edge (tonic 0.14 uses `rustls-pki-types` directly);
   the crate became a direct dependency of `chv-stord-core` only.
2. **PR 2**: the three remaining direct sites (the migration receiver TLS
   config in `chv-stord-core/src/server.rs` plus the two migration mTLS test
   files) migrated to `rustls-pki-types`' folded-in PEM parser (reached via
   the `rustls::pki_types` re-export), and the dependency was dropped. The
   crate is absent from `Cargo.lock` entirely.

Consequently the `RUSTSEC-2025-0134` ignore was removed from both `deny.toml`
and `.cargo/audit.toml` (dated removal notes in each, per the truth
discipline). The §1–§6 snapshot above is untouched — this addendum records
the resolution, it does not rewrite the baseline. Verification on the
resolved tree: `cargo deny check` → ok; `cargo audit` → clean (exit 0).
