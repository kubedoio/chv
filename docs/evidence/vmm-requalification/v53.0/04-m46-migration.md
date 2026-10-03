# D6-(b) Leg 04 — M4.6 scoped-migration re-qualification on Cloud Hypervisor v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [Campaign index](README.md) · [Leg 01 (anchor)](01-anchor-leg.md) · [Leg 02 (serial re-check)](02-serial-console-recheck.md) · [Leg 03 (M4.3 lifecycle)](03-m43-lifecycle.md)
> Date: 2026-10-03, 22:10–22:16 UTC (v53 arm 22:10:54–22:13:42; v43 control 22:14:09–22:16:35)
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §6); this document is the leg's only repo deliverable
> Matrix definition re-run: frozen `docs/evidence/production-readiness/v0.3.0-rc1/04-real-host-qualification/m4.6-migration.md` (run 5, the evidence run), mirrored exactly via `scripts/integration/qual/m4.6-migration.sh` (Legs P + N1–N9) on a `deploy.sh --exec` deployment (core-managed authority, real mTLS, guest image `noble-qual-patched.img`)
> Host: the qualification host (16 vCPU AMD EPYC, 31 GiB RAM, `/dev/kvm`, kernel `6.8.0-142-generic`, Ubuntu 24.04 — nested virtualization, as in the frozen campaign)

## Verdict

**PASS — the frozen M4.6 two-stord mTLS migration matrix passes on Cloud
Hypervisor v53.0 with zero errors and zero warnings, and a same-session v43.0
A/B control reproduces the frozen run-5 result exactly (135 PASS / 0 / 0).**
The v53 arm records **141 PASS / 0 errors / 0 warnings** — the +6 over the
frozen 135 is fully reconciled (§3.2: the env-preflight band run inside the
namespace as the version-gate proof, the same +6 leg 03 recorded). Per-leg
pass counts are **identical to the frozen run 5 in every band**. **Nothing
from this leg blocks the pin move.**

| Arm | VMM | Result | Errors | Warnings | Duration |
|---|---|---|---|---|---|
| Candidate (mount namespace) | **v53.0** (`448af3d4…`) | **PASS** | 0 | 0 | 2 m 48 s (incl. env-preflight + deploy) |
| A/B control (system pin, read-only, no namespace) | v43.0.0 (`a250a934…`) | **PASS** | 0 | 0 | 2 m 26 s |

**Scope truth (stated up front, mirroring the frozen matrix exactly):** the
frozen M4.6 is a **stord-layer** matrix — the migration runs between two
standalone stord daemons over a harness-seeded, **quiescent** 4 GiB volume;
**no VM is booted anywhere in the scenario** and no guest touches either
volume. A VM writing through the CH-held fd is precisely the disclosed,
unmitigated **#394** concurrent-write hazard, which the frozen matrix
explicitly does not claim and this leg did not gate on (drift refused, as
leg 03 did for its recovery-matrix enumeration). Consequently the
version-attributable VMM surface in this leg is the deployed stack's
configuration (agent `chv_binary_path`, namespace-locally the v53.0
candidate) and the env-preflight version gate's execution of the candidate —
**no guest-facing VMM behavior is exercised by this matrix, on either arm.**
The A/B therefore proves the migration result is version-independent on this
matrix, not that v53.0's guest surface changed it.

## 1. Provenance

| Artifact | Version | sha256 (observed = expected) | Source |
|---|---|---|---|
| `cloud-hypervisor-static` | v53.0 | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` — **MATCH** | reused from the M4.3 leg's staging after re-verifying digests; originally `…/releases/download/v53.0/cloud-hypervisor-static` |
| `ch-remote-static` | v53.0 | `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7` — **MATCH** | same staging, same re-verification |

Both executed on kernel `6.8.0-142-generic` (amd64); `--version` →
`cloud-hypervisor v53.0` (`Migration Protocol Versions: 0`) / `ch-remote
v53.0`. The runner aborts if the namespace view does not expose `448af3d4…`
before any harness code runs (it did).

**CHV binaries** (the five the qual harness deploys): reused from
`/var/lib/chv/qual/bin`, built by the M4.3 leg from main
`b6d6ad50ea23bcb362fd0de5ca7127321c0f2c48`; all daemons report
`0.2.0 (commit b6d6ad50, build 2026-10-03, channel stable)`. Reuse
condition verified live this leg: `git log b6d6ad50..origin/main -- crates/
cmd/` is **empty** (main moved only in docs/evidence since). No rebuild
needed; no new SHA to record.

**Read-only qualification assets** (digests verified, never written): guest
image `/var/lib/chv/qual/images/noble-qual-patched.img`
(`37f7c34075044e8c78f3c9bd8987f4f063bddd91a67b6408ef8daae53dabd22a` —
identical to legs 02/03), firmware `/var/lib/chv/hypervisor-fw`
(`4a0a1e97…`, matches the M2.5-recorded prefix).

**grpcurl** v1.9.3 — pinned and checksum-verified at runtime by the scenario
itself against the checked-in checksums asset, exactly as in the frozen run.

## 2. Pin-safety proof (the qualified v43.0.0 pin was never written)

System pin `/usr/bin/cloud-hypervisor`, expected sha256
`a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`:

| Point in the leg | sha256 | Version |
|---|---|---|
| Host baseline, before anything (22:10:50) | `a250a934…` | v43.0.0 |
| Host, before the v53 namespace run (22:10:54) | `a250a934…` | v43.0.0 |
| Host, after the v53 namespace run (22:13:42) | `a250a934…` | v43.0.0 |
| Host, before the v43 control run (22:14:09) | `a250a934…` | v43.0.0 |
| Host, after the v43 control run (22:16:35) | `a250a934…` | v43.0.0 |
| Host, final sweep (22:16:42) | `a250a934…` | v43.0.0 |

Isolation model (campaign-standard, per the [campaign index](README.md)): the
v53 arm executed inside `unshare --mount` with the candidate bind-mounted
over `/usr/bin/cloud-hypervisor` namespace-locally (the qual harness hardcodes
that path — `deploy.sh:82` existence check, `deploy.sh:408`
`chv_binary_path`). Namespace-local view verified `448af3d4…` / `v53.0`
before any harness code ran. After the run, no process on the host held the
bind mount (checked every `/proc/*/mountinfo`) — the mount vanished with the
namespace.

**env-preflight version-gate interaction (the #459 residual-risk note),
re-verified live** (leg 03 first proved it): inside the namespace with
`CHV_VERSION=v53.0`, the gate at `env-preflight.sh:117-136` read the
bind-mounted candidate, matched `v53.0`, and passed — with **zero writes**
through the bind mount: both the namespace view and the staged candidate
file still hashed `448af3d4…` after the preflight (6 PASS lines, including
"cloud-hypervisor pinned version: cloud-hypervisor v53.0"). The v43 control
arm did not invoke env-preflight (deploy.sh does not call it) and executed
the pin **read-only, on the host, no namespace**.

**Frozen-evidence protection:** the frozen campaign's persistent artifact
record `/var/lib/chv/qual/m4.6-artifacts/` (13 files, the frozen run-5 era)
was backed up read-only to the campaign workdir, then **moved aside**
(renamed, never modified) for the duration of both arms so the scenario's
append-only evidence writes could not commingle with the frozen record. Each
arm's artifacts were moved out separately, and the frozen dir was restored
to its original path — `diff -r` against the pre-leg backup proves the
restored content **byte-identical**. The repo's frozen evidence documents
were never opened for write.

## 3. The matrix (frozen M4.6 scope, mirrored exactly)

Same deploy shape as the frozen run (`deploy.sh --exec`, core-managed
authority, real mTLS, node TenantReady/Healthy) plus the scenario's two
standalone stords (SRC/DST, deliberately not agent-supervised) and the N8
plaintext echo listener. Scenario list, assertions, and forbidden outcomes
are the frozen run-5 matrix, unchanged (§5: zero harness drift).

### 3.1 Result table — per frozen leg

| Leg (frozen definition) | Key assertions | v53.0 | v43.0 control | Verdict |
|---|---|---|---|---|
| **P** — positive migration (24) | seeded 4 GiB → trigger → BULK_COPY polled → dirty-round machinery converged at 0 dirty blocks (quiescent source, #394 boundary) → PAUSED_FINAL_SYNC `needs_vm_pause=true` → `ResumeDiskMigration{vm_paused:true}` → COMPLETED → sha256 + byte-compare src vs dst | all PASS (digest `ceaf40138120dcc8…`, byte-identical 4 GiB) | all PASS (same shape) | **PASS both** |
| **N1** — missing TLS config (8) | trigger on the deployed stord (no `[migration]`) fails with the mTLS-required precondition naming the keys; 2 startup-exit variants for `enabled=false` half-configs | all PASS (`status: FailedPrecondition`) | all PASS | **PASS both** |
| **N2** — wrong CA (6) | task FAILED at the handshake, zero bytes, no receiving volume on DST | all PASS (`failed to connect to peer with mTLS: transport error`) | all PASS (same form) | **PASS both** |
| **N3** — wrong server name (7) | same fail-closed outcome; SRC log pins `dest_domain=wrong.example` | all PASS | all PASS | **PASS both** |
| **N4** — wrong destination identity (6) | DST rejects the rogue-CA client leaf; task FAILED, zero bytes, no receiving volume | all PASS (surfaced form: `status: Cancelled`) | all PASS (surfaced form: `status: Unknown`) | **PASS both** — form difference is the frozen §4.3 TLS 1.3 alert race, not a version effect (§4) |
| **N5** — mismatched keypairs (4) | client half + server half exit at startup, "does not match" | all PASS | all PASS | **PASS both** |
| **N6** — malformed material (8) | garbage cert/key/CA + empty CA bundle → startup exits naming the cause | all PASS | all PASS | **PASS both** |
| **N7** — expired client leaf (6) | handshake-time rejection; zero bytes; no receiving volume | all PASS (`Cancelled` form) | all PASS (`Unknown` form) | **PASS both** — same §4.3 race note |
| **N8** — plaintext/downgrade (5) | `http://` endpoint force-upgraded to `https://` (SRC log) → handshake fails against the non-TLS echo listener; listener logged the dial | all PASS | all PASS | **PASS both** — no plaintext migration path, with N1 |
| **N9** — interruption + recovery (14) | SIGKILL DST mid-BULK_COPY → task FAILED with error → restart re-binds listener → retry hits the create_new refusal ("refusing to truncate") → remove partial (operator step) → re-trigger → COMPLETED + digest/byte match | all PASS (post-recovery digest `ceaf4013…`) | all PASS | **PASS both** |
| Close-out (12 + 5 deploy-level) | sessions closed, scenario stords SIGTERM-stopped cleanly, both ports freed, no scenario processes, resource dir removed, deployed stack stord untouched, scenario exited 0, deploy teardown clean | all PASS | all PASS | **PASS both** |
| Preamble (10) | identity generation (EKU/expiry/chain preconditions), grpcurl pin + checksum, port-free gates, 4 GiB patterned seed | all PASS | all PASS | **PASS both** |
| **Totals** | | **141 PASS / 0 errors / 0 warnings, rc=0** | **135 PASS / 0 errors / 0 warnings, rc=0** | **PASS both** |

Forbidden outcomes (frozen set): no data corruption (digest + byte-compare
on both completed migrations, per arm), no orphaned processes (scenario
pkill fallback + deploy teardown assertions), no leaked sockets/ports
(51052/51053 freed, UDS dirs removed with TEST_DIR), teardown residue clean
(links/nft/dnsmasq/CH processes) — **all green on both arms**.

### 3.2 Assertion-count reconciliation (141 vs the frozen 135)

- v53 arm: 141 = **135 (the frozen run-5 shape)** + **6 env-preflight lines**
  (run inside the namespace as the version-gate proof; the control arm, like
  the frozen runs, invokes no env-preflight). The identical +6 was recorded
  by leg 03 for the same reason.
- v43 control: 135 = the frozen run-5 count **exactly**, same per-band
  split: deploy 20, preamble 10, P 24, N1 8, N2 6, N3 7, N4 6, N5 4, N6 8,
  N7 6, N8 5, N9 14, close-out 12, deploy-level 5.
- v53 arm per-band: identical to the control in every scenario band; the
  only differences are the 6 preflight lines and nothing else.

### 3.3 Same-session A/B discipline

Both arms ran within 6 minutes of each other on the same host, same CHV
build (`b6d6ad50`), same harness scripts (byte-identical), same image and
firmware, same scenario ports and volume shape. The only intended
difference: the VMM visible at `/usr/bin/cloud-hypervisor` (v53.0 candidate
in a private mount namespace vs the qualified v43.0.0 pin executed
read-only) — the campaign-standard A/B pattern leg 03 used.

## 4. Known v53-defect interactions (leg-02 signatures) vs new findings

The M4.6 matrix contains **no serial-console, no VM boot, no VMM lifecycle
event** — the surface leg 02 characterized is **structurally unreachable**
in this scenario: no `cloud-hypervisor` process is ever spawned on either
arm (the deploy teardown's "no cloud-hypervisor processes" assertion is
trivially green). There was therefore **no interaction** with any leg-02
defect, and none was expected — consistent with leg 03's finding that the
M4.x storage/network scenarios do not intersect the serial chain.

| Leg-02 defect | Intersected this matrix? | Evidence |
|---|---|---|
| #8322 pre-connect buffering stall | No — no console client, no VM | no CH process on either arm |
| Silent serial-manager thread death (RST) | No — no serial connection exists | same |
| #345-class `GuestExit` wedge | No — no guest, no VMM exit path | same |

**New findings at v53.0: none.** Zero errors, zero warnings, no assertion
near-miss, no timeout recovery on either arm.

One **expected, non-defect** observation worth recording: the N4/N7
server-side rejection surfaced as `status: Cancelled, "operation was
canceled"` on the v53 arm and as `status: Unknown, "transport error"` on the
v43 control. This is precisely the frozen campaign's §4.3 finding — in
TLS 1.3 the alert races the client's in-flight request, and **both forms
were observed on the same binary within the frozen campaign itself**
(`Cancelled` in run 2, `Unknown` in run 3, same candidate). The scenario
asserts the disjunction of observed forms plus the fail-closed invariants
(task FAILED, zero bytes, no receiving volume); both arms satisfied it.
Issue #402 (rejection observability) remains the recorded, unfixed
observability finding — unchanged by this leg, version-independent.

The frozen campaign's other recorded findings carry forward unchanged and
were re-observed exactly as documented (they are product truths of the CHV
stack, not VMM-dependent): #394 (dirty-block transfer under concurrent
writers unprovable at this layer — the quiescent-source boundary is asserted
in the SRC log lines this run too), #401 (destination-only stord not
expressible — the scenario's DST still carries the structurally required
client identity), #402 (rejection observability, above).

## 5. Harness drift vs the frozen M4.6

| Component | Frozen era (run 5, scenario `d9542ca4`) | This leg | Assessment |
|---|---|---|---|
| `m4.6-migration.sh` | as of #400 | **byte-identical** — `git diff d9542ca4 HEAD -- scripts/integration/qual/` touches only `m4.7-faults.sh`/`m4.8-perf-soak.sh`; zero commits touch the m4.6 scenario since #400 | **zero scenario drift** |
| `deploy.sh` / `lib.sh` / `env-preflight.sh` | as of run 5 | **byte-identical** (same diff; empty for all three) | zero shared-harness drift |
| CHV stack | `6a1dfa06` (frozen candidate) | `b6d6ad50` — current main; carries the post-rc1 fixes (#334/#342/#344/#346/#348) plus everything through the leg-02/03 era; verified no runtime-code changes since `b6d6ad50` | **intended drift**: this leg re-qualifies the *current* stack, per the campaign's purpose |
| Guest image / firmware | `noble-qual-patched.img` (`37f7c340…`), rust-hypervisor-fw 0.5.0 (`4a0a1e97…`) | same artifacts, same digests, read-only | identical |
| grpcurl pin | v1.9.3, checked-in checksums asset | same pin, verified at runtime | identical |
| Host | 16 vCPU / 31 GiB (resized vs the frozen campaign's record era) | same | performance figures not compared ([campaign index](README.md) rule); both arms same-session |

No harness defects were encountered; nothing to classify as v53 regression
vs pre-existing; no fix PRs needed from this leg.

## 6. Host-cleanliness proof and repo isolation

Final sweep (22:16:42 UTC, after both arms, evidence relocation, and
frozen-dir restore):

- `/usr/bin/cloud-hypervisor`: sha256 `a250a934…`, v43.0.0 — **unchanged
  throughout** (§2, six checkpoints).
- Candidate staged binaries still match the campaign digests
  (`448af3d4…`, `13f32ba9…`).
- No `cloud-hypervisor`, `chv-*`, or `dnsmasq` processes (argv[0]-anchored
  patterns).
- Host links exactly the baseline (`ens19 eth0 lo`); **no new
  bridges/taps**; **no nft tables**.
- No listeners on :8080/:8443/:9100/:8444/:51052/:51053.
- No `/tmp/chv-qual-*` dirs (both arms' test dirs removed on success —
  standard harness behavior; the full assertion logs and per-leg stord logs
  are preserved in the campaign workdir).
- `/run/chv/nwd` (created by the runs) removed by teardown. Pre-existing
  empty `chv:chv`-owned dirs `/run/chv/{agent,core,stord}` (stamped
  2026-09-30, M4.2 clean-install era — the same end state leg 03 recorded)
  were not created by this leg and were left untouched.
- **No process holds the candidate bind mount** — the namespace is gone.
- Frozen `/var/lib/chv/qual/m4.6-artifacts/` restored to its original path,
  `diff -r`-verified byte-identical to the pre-leg backup.
- Repo: `git status --porcelain` = 0 at leg start and end; no commits,
  branches, PRs, or file writes under the repo by this leg.
- Disk unchanged (~237 GiB free on /; both arms' ~12 GiB transient
  footprints reclaimed by teardown).

## 7. Judgment

**Does the scoped-migration matrix pass at v53.0?** Yes — the complete
frozen M4.6 matrix (positive quiescent-source migration + all nine
negative/failure legs) passes on the candidate with **141 PASS / 0 errors /
0 warnings**, and the same-session v43.0 control reproduces the frozen
run-5 result exactly (135 / 0 / 0, identical per-band counts). Data
integrity holds (both completed migrations digest- and byte-identical),
the paused-final-sync handshake behaves identically, every identity and
plaintext case fails closed with zero bytes leaving the source, interruption
recovery is deterministic, and teardown residue is zero on both arms.

**Version attribution (honest scope):** this matrix boots no VM — mirroring
the frozen matrix exactly — so it exercises the VMM only through the
deployed stack's configuration surface and the version gate. The A/B shows
the migration contract is **version-independent across v43.0 → v53.0** on
this matrix (as expected: the migration path is CHV stord code over
loopback mTLS, with no VMM involvement). The disk-locking interaction the
campaign flagged for this leg (upstream v46.0 #6974 file-level locking) is
likewise not exercised at the VMM layer by this matrix — the volumes are
stord-owned files, never CH-backed disks; the lock-relevant surface was the
create_new refusal (N9), which behaved identically on both arms.

**What blocks the pin move?** Nothing from this leg. Residual
considerations for the maintainer (none introduced here, all already on
record):

1. **The leg-02 defects remain the pin move's live cost** (silent
   serial-manager thread death; #8322 reconnect stall) — this leg adds no
   new intersection: the M4.6 matrix never spawns a VMM process, so the
   serial chain is structurally out of scope here. The #284/#292/#410
   mitigation conditions recorded by legs 02/03 are unchanged.
2. **#394 (open)** — concurrent-write migration remains unclaimed and
   untested as a gate, per the frozen boundary and this leg's instructions;
   the quiescent-source dirty-round machinery is the claimed mode and
   passed identically on both arms.
3. **Recorded product findings** #401 (destination-only not expressible)
   and #402 (rejection observability, incl. the N4/N7 error-form race
   re-observed in both forms across this leg's arms) — unchanged,
   version-independent, already filed.
4. **n=1 per arm** (plus the frozen campaign's five-run progression);
   re-runnable in ~3 min/arm with the scripts and provenance in the
   campaign workdir.
5. Both arms' test dirs were removed on success (standard harness
   behavior); the preserved record is the full assertion logs, per-leg
   stord logs, digests, and failed-migration records for both arms in the
   campaign workdir.

## 8. Not landed by this leg (separate work, not silently decided)

- The pin-move decision and the remaining campaign legs (M4.2/M4.4/M4.5
  smoke, security regression, pin-move PR) per the [campaign index](README.md).
- No CHV-side adaptation work and no upstream reports — the leg-02
  mitigation requirements remain recorded pin-move conditions.

## Artifacts (campaign workdir, ephemeral)

`/tmp/opencode/d6b/m46/`: full assertion logs for both arms (141 / 135
PASS), host baseline + final sweep, wrapper console log, each arm's complete
`m4.6-artifacts` record (`failed-migrations.jsonl` — 8 negative-leg terminal
statuses each, digests, grpcurl provisioning, per-leg stord logs), the
read-only pre-leg snapshot of the frozen record, and the runner scripts.
The load-bearing excerpts (verdict tables, count reconciliation, pin-safety
proofs, frozen-record protection) are embedded in this document.
