# D6-(b) Leg 05 — M4.2 / M4.4 / M4.5 smoke re-qualification on Cloud Hypervisor v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [Campaign index](README.md) · [Leg 01 (anchor)](01-anchor-leg.md) · [Leg 02 (serial re-check)](02-serial-console-recheck.md) · [Leg 03 (M4.3 lifecycle)](03-m43-lifecycle.md) · [Leg 04 (M4.6 migration)](04-m46-migration.md)
> Date: 2026-10-03, 22:18–22:56 UTC
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §7); this document is the leg's only repo deliverable
> Matrix definitions: frozen `docs/evidence/production-readiness/v0.3.0-rc1/04-real-host-qualification/{m4.2-clean-install,m4.4-network,m4.5-storage}.md` — this leg runs the **smoke subset**, honestly scoped (§4 for the M4.2 scoping decision)
> Host: the qualification host (16 vCPU AMD EPYC, 31 GiB RAM, `/dev/kvm`, kernel `6.8.0-142-generic`, Ubuntu 24.04 — nested virtualization, as in the frozen campaign)

## Verdict (summary)

| Scenario | v53.0 candidate | v43.0.0 control (same-session A/B) | Verdict |
|---|---|---|---|
| **M4.4 network** (committed scenario, all legs A–F) | **PASS — 92 PASS / 0 errors / 1 warning, rc=0** (namespace arm) | **PASS — 86 PASS / 0 errors / 1 warning, rc=0** (frozen run-4 count exactly) | **PASS both, version-independent on this matrix** |
| **M4.4 host-safety gate** (prompt-01 rebuild) | **PASS** — both root-gated tests, no leaked links/nft tables, 104-line dump (frozen shape) | n/a — VMM-independent by construction (no CH process; nwd code built from this checkout) | **PASS** |
| **M4.5 storage** (committed scenario, all legs A–F incl. LVM) | **PASS — 103 PASS / 0 errors / 2 warnings, rc=0** (namespace arm) | **PASS — 97 PASS / 0 errors / 2 warnings, rc=0** (frozen run-10 count exactly) | **PASS both, version-independent on this matrix** |
| **M4.2 clean-install** (as-is on current main, v43 pin) | **Leg A PASS** (0 errors / 0 warnings, fresh `05046ffa` packages); Leg B + install-sh-leg **environment-blocked** (host inotify exhaustion — §4.2, not CHV/VMM-related); supplementary v43-pin URL check **PASS** | n/a — container-tier, no VMM surface (see §4) | **Packaging contract holds at this commit; boot legs blocked by host co-tenancy, honestly scoped** |

**Nothing found by this leg blocks the pin move.** The known leg-02 defects
(silent serial-manager thread death; #8322 reconnect stall) did not intersect
any assertion in either scenario — structurally, per the leg-03 analysis: every
console-marker wait in m4.4/m4.5 occurs on a fresh CH process with the agent's
continuously-draining console-log reader attached at spawn; neither scenario
contains a reconnect-then-marker-wait assertion. Console artifacts are
full-stream (~70–90 KB per boot, the frozen-era shape), not the ~278-B stall
signature.

## 1. Provenance

| Artifact | Version | sha256 (observed = expected) | Source |
|---|---|---|---|
| `cloud-hypervisor-static` | v53.0 | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` — **MATCH** | reused from `/tmp/opencode/d6b/m43/cand/` (M4.3/M4.6 staging) after re-verification |
| `ch-remote-static` | v53.0 | `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7` — **MATCH** | same staging, same re-verification |

**CHV binaries** (the five the qual harness deploys): reused from
`/var/lib/chv/qual/bin`, built by the M4.3 leg from main
`b6d6ad50ea23bcb362fd0de5ca7127321c0f2c48` (all daemons report
`0.2.0 (commit b6d6ad50, build 2026-10-03, channel stable)`). Reuse condition
verified live this leg: `git log b6d6ad50..origin/main -- crates/ cmd/` is
**empty** — main moved `b6d6ad50` → `05046ffa` (PR #463) at leg start and
`05046ffa` → `aba53949` (PRs #464/#465) during the leg, **all docs-only**
(harness scripts byte-identical across the whole range; see §7). No rebuild
needed; no new SHA to record. Repo at leg start (`05046ffa`) and end
(`aba53949`): `git status --porcelain` = 0.

**Read-only qualification assets** (digests verified, never written): guest
image `/var/lib/chv/qual/images/noble-qual-patched.img`
(`37f7c34075044e8c78f3c9bd8987f4f063bddd91a67b6408ef8daae53dabd22a` —
identical to legs 02/03/04), firmware `/var/lib/chv/hypervisor-fw`
(`4a0a1e97…`).

**M4.2 artifacts** (this leg's fresh build, §4): release tarball
`dist/chv-0.2.0-linux-amd64.tar.gz`
(`0c036ad09d90cf3e76b1614670314469ddabecb0ab5559b3c444711602594202`, binaries
report `commit 05046ffa`) and `.deb`s (`f0f7ef8c…` controlplane, `7b1a302e…`
node, `e23bdf4f…` chvctl) — built from clean `05046ffa` via `make release` +
`make package-deb` (repo tree still clean afterwards). The frozen-era `dist/`
contents (Oct 1) were backed up to the campaign workdir before the rebuild.

## 2. Pin-safety proof (the qualified v43.0.0 pin was never written)

System pin `/usr/bin/cloud-hypervisor`, expected sha256
`a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`:

| Point in the leg | sha256 | Version |
|---|---|---|
| Host baseline, before anything (22:18) | `a250a934…` | v43.0.0 |
| After M4.2 artifact build (22:23) | `a250a934…` | v43.0.0 |
| After M4.4 v53 namespace arm (22:26) | `a250a934…` | v43.0.0 |
| After M4.4 v43 control (22:37) | `a250a934…` | v43.0.0 |
| After host-safety gate (22:39) | `a250a934…` | v43.0.0 |
| After M4.5 v53 namespace arm (22:46) | `a250a934…` | v43.0.0 |
| After M4.5 v43 control (22:55) | `a250a934…` | v43.0.0 |
| After M4.2 legs (22:55) | `a250a934…` | v43.0.0 |
| Host, final sweep (22:56) | `a250a934…` | v43.0.0 |

Isolation model (campaign-standard, per the [campaign index](README.md)): every
v53 arm executed inside `unshare --mount` with the candidate bind-mounted over
`/usr/bin/cloud-hypervisor` namespace-locally (the qual harness hardcodes that
path). Each runner re-verified the candidate digest, then the namespace-local
view (`448af3d4…` / `v53.0`), **before any harness code ran**, and aborted on
mismatch. After each run, no process on the host held the bind mount (checked
every `/proc/*/mountinfo`). The v43 control arms executed the pin read-only,
on the host, no namespace.

**env-preflight version-gate interaction** (the #459 residual-risk note),
re-verified live in each v53 arm (leg-03/04 pattern): inside the namespace
with `CHV_VERSION=v53.0`, the gate at `env-preflight.sh:117-136` read the
bind-mounted candidate, matched, and passed — with **zero writes** through
the bind mount (both the namespace view and the staged candidate file still
hashed `448af3d4…` after the preflight).

## 3. M4.4 network smoke — result

Scope honesty: the committed `m4.4-network.sh` is **monolithic** (no
leg-subset switch exists), so each arm ran the full six-leg matrix — which
subsumes the requested smoke subset (local-bridge topology = Leg A; the
host-safety gate ran separately, below). Zero scenario drift vs the frozen
evidence run: the last commit touching `m4.4-network.sh` is `0d91291f` (#370)
— the exact scenario commit the frozen re-qual run 4 used.

| Arm | VMM | Result | Errors | Warnings | Duration |
|---|---|---|---|---|---|
| Candidate (mount namespace) | **v53.0** (`448af3d4…`) | **PASS** | 0 | 1 (deliberate N2-era bare-table record) | ~6 min |
| A/B control (system pin, read-only, no namespace) | v43.0.0 (`a250a934…`) | **PASS** | 0 | 1 (same) | ~6 min |

- v53 arm: **92 PASS** = the frozen re-qual run-4 count (86) + **6
  env-preflight lines** (run inside the namespace as the version-gate proof;
  the control, like the frozen runs, invokes no env-preflight) — the
  identical +6 reconciliation legs 03/04 recorded.
- v43 control: **86 PASS** — the frozen run-4 count and warning exactly
  (0 errors, 1 warning: the retained N2-era bare-table record for
  dual-version truth).
- Legs exercised (both arms, identical): A attach + connectivity (bridge
  `br-<net>` with defaulted gateway, tap enslaved, nwd dnsmasq with MAC→IP
  reservation, cloud-init seed, host→guest ping, ARP REACHABLE); B policy
  attempt (validated at save, `policy_application: pending`, table
  unchanged); C nwd SIGKILL → supervisor restart → re-attach + attach-time
  policy materialization; D stop/start (tap quiesces, deterministic tap name
  + IPAM IP, connectivity restored); F second network with its own cidr (no
  fallback-CIDR collision); E cleanup (last-detach host teardown, network
  delete unblocked). Teardown all-green on both arms (no CH processes, no
  new links, no new nft tables, no nwd dnsmasq).
- **Host-safety gate** (`host-safety.sh`, VMM-independent — no
  cloud-hypervisor process; nwd firewall code built from this checkout,
  identity guard green `crates/chv-nwd-core identical at b6d6ad50 and HEAD`):
  `confines_policy_to_chv_owned_traffic` PASS,
  `exposure_survives_firewall_apply` PASS, no leaked links, no leaked nft
  tables; ruleset dump 104 lines — the prompt-01/frozen shape. Run once, not
  per arm (nothing in it executes the VMM).

### 3.1 Leg-02 defect interactions (M4.4)

| Leg-02 defect | Intersected? | Evidence |
|---|---|---|
| #8322 pre-connect buffering stall | No — every console wait (kernel banner, logind) is on a fresh CH process (vm create/start) with the agent's draining console-log reader attached at spawn | console artifacts 70–90 KB per boot (full stream, frozen-era shape); zero timed-out waits in either arm's log |
| Silent serial-manager thread death (RST) | No — the agent's close discipline is drain-then-close (#292 machinery); no abortive client exists in this scenario | no serial-related anomalies in any log |
| #345-class `GuestExit` wedge | No — graceful stop (Leg D) exits CH cleanly on both arms | Leg D teardown assertions PASS both arms |

No new findings at v53.0 in this scenario: zero errors, the single expected
warning, no unexpected shapes.

## 4. M4.2 clean-install smoke — scoping decision and result

### 4.1 The scoping decision (recorded before execution)

The M4.2 VMM surface is install.sh's pinned download
(`scripts/install.sh:413`, `chv_version="43.0"` → upstream release assets →
`/usr/local/bin/cloud-hypervisor` + `/usr/bin` symlink). **That pin cannot
become v53.0 before the pin-move PR** — the campaign's own rule (the pin does
not move without evidence, and install.sh's pin moves only in the pin-move
PR, together with every other class-(i) reference). No install.sh
modification was made (repo untouched).

What this leg ran instead, per the leg brief:

- **(a) The M4.2 clean-install smoke as-is on current main (v43 pin)** —
  fresh packages + tarball built from clean `05046ffa`
  (`make release` + `make package-deb`; the frozen-era `dist/` contents were
  backed up first), run through the committed harness with a **private
  `CHV_QUAL_ROOT`** so no frozen record under `/var/lib/chv/qual` was written
  (the cached debootstrap base tarball was reused read-only via symlink; the
  frozen `nspawn-*`/`installsh-*` transcripts and `packages/` were never
  opened for write).
- **(b) The v53 install-path items recorded as pin-move-PR verification
  items** (below).

### 4.2 Result

**Leg A — static packaging contract: PASS, errors=0 warnings=0** on the
fresh `05046ffa` packages. Full green: service users (`chv`, `chv-stord`) +
groups/memberships (incl. `chv-stord`→`chv`, the #325 fix), directories with
exact modes/ownership (agent 0700, storage 0770 `chv:chv-stord`), all four
units + CP hardening keys, tmpfiles, **encryption.env minted 0600 root:root
with a 64-hex key (#335)**, all five binaries, conffiles, 56 migrations, and
versions reporting `commit 05046ffa`. This is the CI-regression value the
campaign wanted: **the packaging contract holds at this commit.**

**Leg B (container boot) and install-sh-leg.sh (tarball path):
environment-blocked — stopped per the stop-don't-improvise rule, fully
classified:**

- Symptom (both legs, identical): the nspawn container's systemd dies at
  manager allocation — `Failed to create control group inotify object: Too
  many open files` → `Failed to allocate manager object` → PID 1 exit 255 —
  **before any chv unit starts**. Boot logs and container roots preserved in
  the campaign workdir for post-mortem.
- Root cause (proven live): host `fs.inotify.max_user_instances` = **128**,
  and uid 0 currently holds **136** inotify instances — dominated by a
  co-tenant fleet of **52 `kube-apiserver` processes** (3 instances each)
  started 2026-10-03 20:15:16 UTC, i.e. before this leg (22:18) and after
  the frozen M4.2 runs (Oct 1), which is why the frozen legs passed and
  these did not. The shell's own rlimits are not the constraint
  (`ulimit -n` = 1048576).
- Classification: **not a v53 regression** (no cloud-hypervisor and no CHV
  code is involved — the container dies at systemd startup); **not a product
  or packaging defect** (Leg A is fully green); **not a harness defect**
  (both legs failed loudly, preserved the roots, and did not report a false
  pass). It is a host co-tenancy condition.
- Not remediated by this leg, deliberately: bumping the sysctl or touching
  the co-tenant processes are host-state changes outside this leg's mandate
  (the same discipline leg 01 applied to the umask workaround). The legs are
  trivially re-runnable (`m42/legs.sh` is idempotent) once the co-tenant
  load clears or the maintainer bumps `fs.inotify.max_user_instances`.
- What *did* pass in install-sh-leg before the boot: host port-80 pre-flight,
  one-shot install service, and the **5/5 source-of-truth parity assertions**
  (tarball `systemd/*` + `tmpfiles/` byte-match `packaging/`).

**Supplementary read-only pin check (outside the harness, labeled as such):**
the upstream v43.0 release URL that install.sh pins
(`…/releases/download/v43.0/cloud-hypervisor-static`) was fetched fresh and
hashes to **exactly the qualified pin digest `a250a934…`**; the
`ch-remote-static` URL serves (HTTP 200). The current pin's download path is
verified fetchable and byte-correct today.

### 4.3 Deferred to the pin-move PR (not pre-move smokes)

1. **install.sh's pin move itself** (`43.0` → `53.0` at
   `scripts/install.sh:413`) with the v53 download URL + unit wiring
   verified against the new binary. Supporting facts already on record:
   v53.0 release asset names are identical to v43.0 (legs 01–04 staged and
   digest-pinned both assets), and the full CHV-stack behavior with v53 was
   exercised by leg 03 and this leg's M4.4/M4.5 arms via the namespace
   model.
2. **Checksum verification of the CH download**: install.sh currently
   performs `curl -fsSL` with **no digest check** on the cloud-hypervisor
   download (v43 or v53). Recorded as a pin-move-PR consideration — adding
   the campaign digests at the same time the pin moves would close a real
   supply-chain gap (this leg's supplementary check shows the v43 asset
   still matches the qualified digest today).
3. **The staged-v53 installed-unit smoke**: not cleanly possible via the
   committed harness — `clean-install.sh` installs no cloud-hypervisor at
   all (the debs don't package one), and `install-sh-leg.sh` has no
   binary-injection hook (its flow unpacks the base tarball and boots
   immediately). install.sh's own already-installed branch
   (`cmd_exists cloud-hypervisor`) would keep a pre-staged binary, but using
   it would require a harness variant this leg would have to improvise —
   and the container boots are environment-blocked this session regardless.
   Deferred with the reasoning that its evidentiary value is small: the
   units are VMM-version-agnostic (CH is invoked by path at VM runtime, not
   at install time), so the installed-unit shape with v53 adds little beyond
   what legs 03/m4.4/m4.5 already proved through the live stack.

## 5. M4.5 storage smoke — result

Scope honesty: as with M4.4, the committed `m4.5-storage.sh` is
**monolithic**, so each arm ran the full six-leg matrix — which subsumes the
requested smoke subset (local-file profile = Legs A–E; LVM profile = Leg F).
Zero scenario drift vs the frozen evidence run: the last commit touching
`m4.5-storage.sh` is `b7e03e6f` (#388) — the frozen run-10 candidate commit.

| Arm | VMM | Result | Errors | Warnings | Duration |
|---|---|---|---|---|---|
| Candidate (mount namespace) | **v53.0** (`448af3d4…`) | **PASS** | 0 | 2 (both the deliberate Leg-E M2.5-residual records) | ~5 min |
| A/B control (system pin, read-only, no namespace) | v43.0.0 (`a250a934…`) | **PASS** | 0 | 2 (same) | ~5 min |

- v53 arm: **103 PASS** = the frozen run-10 count (97) + **6 env-preflight
  lines** (namespace version-gate proof) — the same +6 as legs 03/04 and
  this leg's M4.4 arms.
- v43 control: **97 PASS / 0 errors / 2 warnings** — the frozen run-10
  result exactly (same two Leg-E M2.5-residual records: volume row + backing
  file retained on VM delete).
- Legs exercised (both arms, identical): A provision → attach → guest writes
  the marker (seed conversion, stord session row, kernel banner + logind,
  marker on vda); B stop → start → guest reads the SAME marker back
  (`m45-101319-persist` written and read back byte-identically); C stord
  SIGKILL under the running VM → supervisor respawn → allowlist parity →
  fresh VM boots + deletes cleanly; D snapshot/clone truth (#378: accepted +
  journaled + fail-closed dispatch, no side-effect files, clone target owner
  inheritance, delete-snapshot intent journaled); E cleanup (sessions
  closed, no leaked qemu-img, M2.5 retention recorded); **F LVM (stord
  layer)** — loopback PV → VG → LVs, **7/7 root-gated real-LVM tests**
  (open/export, block write/read, COW snapshot, clone, resize, read-only
  policy, health), zero loop/LV/VG residue.
- Teardown all-green on both arms (no CH processes, no new links, no new nft
  tables, no nwd dnsmasq, no loop devices).

### 5.1 Leg-02 defect interactions (M4.5)

Same structural analysis as M4.4/leg 03: every console wait is on a fresh CH
process (vm create / vm start after stop) with the agent's draining
console-log reader attached at spawn; the stop→start cycle (Leg B) is a fresh
CH process, not a reconnect. Console artifacts are full streams (73–90 KB)
and the marker read-back (the load-bearing guest-facing assertion) passed on
both arms. No reconnect-then-marker-wait assertion exists in the scenario; no
serial anomalies in any log; both graceful stops exited CH cleanly.

**New findings at v53.0: none** — zero errors, the two expected warnings, no
unexpected shapes, LVM residue zero.

## 6. Harness drift vs the frozen docs

| Component | Frozen era | This leg | Assessment |
|---|---|---|---|
| `m4.4-network.sh` | `0d91291f` (#370, re-qual run 4) | **unchanged** (last commit #370) | zero scenario drift |
| `m4.5-storage.sh` | `b7e03e6f` (#388, run 10) | **unchanged** (last commit #388) | zero scenario drift |
| `clean-install.sh` / `install-sh-leg.sh` | frozen-era commits (`97114c08`, `ede89f46`) | unchanged since | zero drift |
| `deploy.sh` | as of frozen runs | +`CHV_QUAL_LOG_LEVEL` (#367), +stord `path_allowlist` propagation (#377) — the same intended drift legs 03/04 recorded | robustness only; default log level `info` = evidence shape |
| `env-preflight.sh` | as of frozen runs | unchanged for qual (#459 touched kvm-smoke only); version-gate reinstall hazard handled by the namespace model, re-verified per arm | as designed |
| CHV stack | frozen candidates | `b6d6ad50` — current main (post-rc1 fix era) | **intended drift**: this leg re-qualifies the *current* stack against v53.0 |
| Guest image / firmware | same artifacts | same digests, read-only | identical |

## 7. Host-cleanliness proof and repo isolation

Final sweep (22:56 UTC, after all arms, the M4.2 legs, evidence relocation,
and frozen-dir restore):

- `/usr/bin/cloud-hypervisor`: sha256 `a250a934…`, v43.0.0 — **unchanged
  throughout** (§2, nine checkpoints).
- Staged candidate binaries still match the campaign digests
  (`448af3d4…`, `13f32ba9…`).
- No `cloud-hypervisor`, `chv-*`, nwd `dnsmasq`, or `lvm_real` processes.
- Host links exactly the baseline (`ens19 eth0 lo`); **no new
  bridges/taps**; **no nft tables**.
- No listeners on :8080/:8443/:9100/:8444/:51052/:51053.
- **No loop devices, no VGs** (M4.5 Leg F residue zero).
- No `/tmp/chv-qual-*` dirs (all four successful arms removed their test
  dirs on success — standard harness behavior; the full assertion logs are
  preserved in the workdir).
- `/run/chv/nwd` (created by the runs) removed by teardown. Pre-existing
  empty `chv:chv`-owned dirs `/run/chv/{agent,core,stord}` (stamped
  2026-09-30, M4.2 clean-install era — the same end state legs 03/04
  recorded) were not created by this leg and were left untouched.
- No nspawn machines remain; **no process holds the candidate bind mount**.
- **Frozen-record protection (leg-04 pattern), verified:** the frozen
  `/var/lib/chv/qual/m4.4-artifacts/` (28 files) and `m4.5-artifacts/`
  (29 files) were backed up read-only, moved aside for the duration, and
  restored to their original paths — `diff -r` against the pre-leg backups
  proves both **byte-identical**, and a full sha256 manifest taken pre-leg
  and post-leg is **identical**. No other file under `/var/lib/chv/qual`
  was written this leg (mtime scan; the M4.2 legs ran under a private
  `CHV_QUAL_ROOT`; the frozen `installsh-root`, `nspawn-*` transcripts and
  `packages/` were never opened for write).
- **Repo untouched during execution:** `git status --porcelain` = 0 at leg
  start and end; no commits, branches, PRs, or file writes under the repo by
  this leg. Build outputs during the M4.2 artifact build landed only in
  gitignored paths. HEAD moved `05046ffa` → `aba53949` during the leg via
  PRs #464/#465 (the leg-03/04 evidence docs merging from other sessions) —
  **docs-only**; the harness scripts are byte-identical across that range,
  so all arms executed identical harness code.
- Disk ~235 GiB free on / (all transient footprints reclaimed; the two
  preserved M4.2 post-mortem container roots are retained in the workdir as
  failure evidence).

## 8. Judgment

**What passed:**

- **M4.4 network (full committed matrix) at v53.0: PASS** — 92 PASS /
  0 errors / 1 deliberate warning, rc=0, with a same-session v43 control
  reproducing the frozen re-qual run-4 result exactly (86 / 0 / 1). All six
  legs green, teardown residue zero, and the **host-safety gate PASS**
  (both root-gated nwd tests, no leaked links/nft tables, 104-line dump).
  The guest network path through the CHV stack is **version-independent
  across v43.0 → v53.0** on this matrix.
- **M4.5 storage (full committed matrix) at v53.0: PASS** — 103 PASS /
  0 errors / 2 deliberate warnings, rc=0, with the v43 control reproducing
  the frozen run-10 result exactly (97 / 0 / 2). Local-file VM-integrated
  profile (provision → attach → guest write → restart → read-back →
  stord-kill recovery → fail-closed snapshot/clone truth → cleanup) and the
  LVM stord-layer profile (**7/7 root-gated real-LVM tests**, zero
  loop/VG/LV residue) both hold, version-independent.
- **M4.2 clean-install Leg A at current main (`05046ffa`): PASS** — the
  deb packaging contract (users/groups/modes/units/hardening/tmpfiles/
  encryption key/binaries/conffiles/migrations/versions) holds at this
  commit, on packages built fresh this leg. The v43 pin's upstream download
  URL verified byte-correct (supplementary read-only check).
- **Assertion-count reconciliation (all +6s explained):** every v53
  namespace arm = the frozen-era count + exactly 6 env-preflight lines (the
  version-gate proof); every control arm = the frozen-era count exactly —
  the same reconciliation legs 03/04 recorded.

**What's deferred to the pin-move PR (recorded, not silently decided):**

- install.sh's pin move (`43.0`→`53.0`) with the v53 download-URL, unit
  wiring, and (recommended) checksum-verification items — §4.3. The
  staged-v53 installed-unit smoke is deferred with rationale (no clean
  harness mode; low evidentiary value given legs 03/M4.4/M4.5).
- The M4.2 boot-dependent legs (Leg B, install-sh-leg end-to-end) are
  **blocked by host co-tenancy** (uid-0 inotify instance exhaustion from a
  kube-apiserver fleet that started 20:15 UTC today) — not a CHV or VMM
  issue, re-runnable in minutes via `m42/legs.sh` once the host recovers;
  flagged to the maintainer rather than worked around.

**What blocks the pin move: nothing from this leg.** The M4.4 and M4.5
matrices — the VMM-attributable part of this smoke leg — pass identically
on v53.0 and v43.0 with clean A/B attribution, zero errors, and zero new
findings. The pin move's live cost remains what legs 02/03 already recorded:
the silent serial-manager thread death and the #8322 reconnect stall, which
this leg confirms do **not** intersect the M4.4/M4.5 assertion sets
(structurally: fresh-CH console waits + the agent's draining reader; no
reconnect-then-marker-wait; console artifacts are full ~70–90 KB streams,
not the ~278-B stall signature). Combined with legs 01/03/04, every
campaign leg through this one is green at its tier except the leg-02
defects, which are upstream-unfixed and carried by the existing CHV
containment (#284/#292/#410).

**Residual considerations for the maintainer (none introduced by this
leg):**

1. n=1 per arm per scenario (plus the frozen campaigns' multi-run
   progressions); re-runnable in ~5–6 min/arm with the scripts and
   provenance in the workdir.
2. Nested virtualization only; single-network (M4.4) and local-file/LVM
   (M4.5) scope caveats carry over from the frozen docs unchanged.
3. The recorded product truths re-observed identically on both arms (the
   N2-era bare-table warning in M4.4; the M2.5 volume-retention warnings
   and #378 fail-closed truth in M4.5) — version-independent, already
   filed, unchanged.
4. The host inotify exhaustion (§4.2) will block any future nspawn-based
   leg until cleared — worth a maintainer look independent of this
   campaign.

## 9. Not landed by this leg (separate work, not silently decided)

- The M4.2 Leg B / install-sh-leg re-run once the host co-tenancy clears
  (re-runnable in minutes; would upgrade this leg's M4.2 result from
  "Leg A PASS + boot legs environment-blocked" to the full frozen shape).
- The pin-move decision and the remaining campaign legs (security
  regression, pin-move PR) per the [campaign index](README.md).
- No CHV-side adaptation work and no upstream reports — the leg-02
  mitigation requirements remain recorded pin-move conditions.

## Artifacts (campaign workdir, ephemeral)

`/tmp/opencode/d6b/smokes/`: full assertion run logs for all four arms
(92 / 86 / 103 / 97 PASS), host-safety gate log + 104-line ruleset dump,
per-arm artifact records (console logs, nft dumps, dnsmasq conf, LVM test
logs, host-state snapshots), M4.2 build digests + frozen-era dist backup +
private qualroot with the two preserved failure container roots and boot
logs + the v43 URL-check binary, frozen-dir manifests (pre-leg/post-leg
sha256), pin-safety sweeps at nine checkpoints, final sweep, and the runner
scripts reproducing every arm. The load-bearing excerpts (verdict tables,
count reconciliations, pin-safety proofs, frozen-record protection) are
embedded in this document.
