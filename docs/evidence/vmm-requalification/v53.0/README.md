# D6-(b) VMM Re-qualification — Cloud Hypervisor v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (decision D6, option (b)).
> Capability maturity vocabulary: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Evidence root for this campaign: `docs/evidence/vmm-requalification/v53.0/`
> Supersedes (never rewrites) the qualified v43.0 records under `docs/evidence/production-readiness/v0.3.0-rc1/`.

The pin does not move until this campaign produces evidence; nothing unqualified
may be claimed shipped in between. The v43.0 records and the D6 row's option-(a)
history are never rewritten — a pin move adds a new resolution line.

## Candidate tuple

| Artifact | Version | sha256 |
|---|---|---|
| `cloud-hypervisor-static` | v53.0 | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` |
| `ch-remote-static` | v53.0 | `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7` |

Source: upstream `v53.0` release (asset naming identical to v43.0). Every leg
re-downloads and pins by these digests; a digest mismatch fails the leg.

## Campaign decisions (recorded before execution)

- **VMM lifetime (up-front, per #448):** Option B — keep the PR #286
  persisted-payload re-spawn machinery, do **not** adopt `--no-shutdown`.
  Follow-up questions for a future option: #457.
- **Isolation standard:** the system pin `/usr/bin/cloud-hypervisor` (qualified
  v43.0.0, sha256 `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`)
  is never written. Harness-driven legs execute inside a private mount namespace
  (`unshare --mount`) with the candidate bind-mounted over the system path
  namespace-locally; raw-CH legs invoke the staged candidate by path. Every leg
  records the system pin's sha256 before and after its runs.

## Host

The qualification host (m4.1-class after resize: 16 vCPU AMD EPYC, 31 GiB RAM,
`/dev/kvm`, kernel `6.8.0-142-generic`, Ubuntu 24.04). The host was **resized
relative to the m2.5/v43.0 campaign era**; performance figures from the v43.0
records are not comparable and are re-baselined by this campaign's own legs
where measured, not compared.

## Legs

| Leg | Evidence | Status |
|---|---|---|
| Anchor (kvm-smoke matrix on the candidate) | [01-anchor-leg.md](01-anchor-leg.md) | PASS at harness tier (after #458/#459 harness fix) |
| Serial-console re-check (m2.5 e-series against v53.0) | [02-serial-console-recheck.md](02-serial-console-recheck.md) | **Mixed** — 2 FIXED, 1 BROKEN (#8322 buffering stalls in the reconnect scenario), 1 STILL PRESENT (silent thread death), #345 wedge NOT REPRODUCED, thread/seccomp unchanged. Material input to the pin-move decision — see the leg's §5 |
| #345/#409-verification thread coupling | — | pending |
| M4.3 lifecycle | — | pending |
| M4.6 scoped migration | — | pending |
| M4.2/M4.4/M4.5 smoke | — | pending |
| Security regression | — | pending |
| Pin-move PR (campaign close) | — | pending — all class-(i) v43.0 references move together |

## Standing rules

- Legs execute and report; fixes are separate narrow, independently reviewed
  PRs. Frozen evidence, released history, old plans, and analysis documents are
  never rewritten.
- Capability claims follow the repository's tier vocabulary. A merged PR is not
  automatically qualification evidence; a harness-tier pass makes no
  guest-facing claim.
