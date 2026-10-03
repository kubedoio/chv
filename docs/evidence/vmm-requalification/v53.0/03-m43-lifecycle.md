# D6-(b) Leg 03 — M4.3 lifecycle & recovery re-qualification on Cloud Hypervisor v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [Campaign index](README.md) · [Leg 01 (anchor)](01-anchor-leg.md) · [Leg 02 (serial re-check)](02-serial-console-recheck.md)
> Date: 2026-10-03, 21:47–22:06 UTC (v53 arm 21:49:35–21:56:52; v43 control 21:57:48–22:05:15)
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §7); this document is the leg's only repo deliverable
> Matrix definition re-run: frozen `docs/evidence/production-readiness/v0.3.0-rc1/04-real-host-qualification/m4.3-lifecycle.md` (run 6, the evidence run), via the CHV stack (`chvctl → BFF → control-plane → agent gRPC → CellHV Core → CH`), unlike leg 02's raw-CH method
> Host: the qualification host (16 vCPU AMD EPYC, 31 GiB RAM, `/dev/kvm`, kernel `6.8.0-142-generic`, Ubuntu 24.04 — nested virtualization, as in the frozen campaign)

## Verdict

**PASS — full tier (real guests through the full CHV stack), zero errors.**
The complete frozen M4.3 lifecycle & recovery matrix passes on Cloud
Hypervisor v53.0 with the current main CHV stack: **119 PASS / 0 errors /
2 warnings** (both benign-known). The frozen campaign's single recorded
error — the #345 post-adoption stop wedge — **does not occur at v53.0**, and
a same-session **v43.0 control arm reproduces it exactly** (111 PASS / 1
error / 3 warnings, same wedge, same leg, same remediation), giving clean
version attribution. **Nothing from this leg blocks the pin move.**

| Arm | VMM | Result | Errors | Warnings | Duration |
|---|---|---|---|---|---|
| Candidate (mount namespace) | **v53.0** (`448af3d4…`) | **PASS** | 0 | 2 (both benign-known) | 7 m 17 s |
| A/B control (system pin, read-only, no namespace) | v43.0.0 (`a250a934…`) | FAIL — the frozen run-6 signature | 1 (#345 wedge, Leg F) | 3 | 7 m 27 s |

## 1. Candidate and CHV-build provenance

| Artifact | Version | sha256 (observed = expected) | Source |
|---|---|---|---|
| `cloud-hypervisor-static` | v53.0 | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` — **MATCH** | `…/releases/download/v53.0/cloud-hypervisor-static` |
| `ch-remote-static` | v53.0 | `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7` — **MATCH** | `…/releases/download/v53.0/ch-remote-static` |

Both executed on kernel `6.8.0-142-generic` (amd64); `--version` →
`cloud-hypervisor v53.0` / `ch-remote v53.0`. Downloaded fresh this leg; the
runner aborts if the namespace view does not expose `448af3d4…` before any
harness code runs.

**CHV binaries** (the five the qual harness deploys, per `deploy.sh:85`):
built **fresh from main HEAD `b6d6ad50ea23bcb362fd0de5ca7127321c0f2c48`**
(clean tree, 0 dirty paths; the target cache was wiped first — full
dependency recompile, `Finished release in 1m 23s`) via
`env-preflight.sh --stage-binaries`, namespace-external. Staged into
`/var/lib/chv/qual/bin`; all daemons report `0.2.0 (commit b6d6ad50, build
2026-10-03, channel stable)`.

Campaign-decision compliance verified structurally before execution:
`no_shutdown`/`--no-shutdown` appears **nowhere** in the workspace
(Option B: default VMM lifetime), and the PR #286 machinery is present
(`adopt_running_vms`, `respawn_vmm` in `crates/chv-agent-runtime-ch/src/process.rs`).

**Repo state during the leg** (disclosed for completeness): main moved
`b6d6ad50` → `05046ffa` (PR #463, the leg-02 evidence doc — docs-only) at
21:57:16 UTC via a merge from another session; this leg never wrote to the
repo (`git status --porcelain` = 0 at start and end). The v53 arm (ended
21:56:52) ran entirely at `b6d6ad50`; the v43 control ran at `05046ffa`, but
the harness scripts are byte-identical across that range — both arms executed
identical harness code.

## 2. Pin-safety proof (the qualified v43.0.0 pin was never written)

System pin `/usr/bin/cloud-hypervisor`, expected sha256
`a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`:

| Point in the leg | sha256 | Version |
|---|---|---|
| Host baseline, before anything (21:47) | `a250a934…` | v43.0.0 |
| Host, after binary staging run (21:49) | `a250a934…` | v43.0.0 |
| Host, after the v53 namespace run (21:56:52) | `a250a934…` | v43.0.0 |
| Host, after the v43 control run (22:05:15) | `a250a934…` | v43.0.0 |
| Host, final sweep (22:05:49) | `a250a934…` | v43.0.0 |

Isolation model (campaign-standard, per the [campaign index](README.md)): the
v53 arm executed inside `unshare --mount` with the candidate bind-mounted
over `/usr/bin/cloud-hypervisor` namespace-locally (the qual harness hardcodes
that path — `deploy.sh:82` existence check, `deploy.sh:408` `chv_binary_path`).
Namespace-local view verified `448af3d4…` / `v53.0` before any harness code
ran. After the run, no process on the host held the bind mount (checked every
`/proc/*/mountinfo`) — the mount vanished with the namespace.

**env-preflight version-gate interaction (the #459 residual-risk note),
verified live:** inside the namespace with `CHV_VERSION=v53.0`, the gate at
`env-preflight.sh:117-136` read the bind-mounted candidate, matched `v53.0`,
and passed — with **zero writes** through the bind mount: both the namespace
view and the staged candidate file still hashed `448af3d4…` after the
preflight. The host-side staging run used the default `CHV_VERSION=v43.0`,
which passes against the installed pin without writing.

The v43 control arm executed the pin **read-only, on the host, no namespace**
(identical matrix).

## 3. The matrix (frozen M4.3 scope, mirrored exactly)

Per the frozen m4.3: single guest `qual-vm-1` (2 vCPU / 1 GiB, firmware boot,
default network), Legs A–F, identity + operation-history determinism asserted
at every leg. **Scope note (drift refused):** the leg brief's
recovery-matrix enumeration mentioned "CH kill -9 while VM runs" — the frozen
M4.3 contains **no such leg** (CH-kill scenarios are M4.7 faults scope);
this leg mirrors the frozen M4.3 exactly: agent restart (Leg B), CP restart
(Leg C), management-plane outage (Leg D), four-daemon cold restart (Leg E,
the labeled host-reboot subset), stop→delete (Leg F). Host reboot remains
not-provable on this topology, as in the frozen campaign.

### 3.1 Result table — v53.0 candidate arm (119 PASS / 0 errors / 2 warnings)

| Leg (frozen definition) | Key assertions | v53.0 result | v43.0 control | Verdict |
|---|---|---|---|---|
| **A — full lifecycle** | create-and-boot; kernel banner + logind evidence ×3 boots; reboot = guest-level, **same CH process**; graceful stop → CH exits; second start re-spawns fresh CH; spec honored; exactly one CH process at every point | all PASS (console: boot1 71,687 B, boot2 166,498 B cumulative, boot3 90,691 B incl. `ubuntu login:` prompt) | all PASS | **PASS both**; v53 stop path clean |
| **B — S1 agent SIGKILL while Running** | CH survives crash; restarted agent re-adopts same pid; exactly one CH; identity constant; ops history unchanged | all PASS (re-adopted; ops=5 events=15) | all PASS (ops=5 events=15) | **PASS both** |
| **C — control-plane restart while Running** | CH unaffected, same pid across restart; agent reconnect; identity; ops unchanged | all PASS | all PASS | **PASS both** |
| **D — 60 s management-plane outage** | API refuses for full window; guest keeps executing; same CH pid; ops unchanged; outage-heal drain gate | all PASS except cputime warn (idle guest — `qual_warn`, not a gate); drain gate passed 0→0 | same shape: cputime warn | **PASS both**; cputime warn is version-independent (both arms) — an idle guest, not a regression |
| **E — four-daemon cold restart** | CH orphaned but alive; cold reconcile re-adopts; node Healthy; identity; ops unchanged | all PASS | all PASS | **PASS both** |
| **F — stop (graceful, logind-gated) → delete → absence** | graceful stop of the **adopted** VM; CH exits; no CH process remains; Stopped desired state; delete accepted; post-delete absence + determinism | **all PASS — clean exit, no wedge** (Leg A fresh-spawn stop and Leg F adopted stop both exited with the guest) | **FAIL — the #345 wedge fired exactly as frozen**: CH alive with dead API after the stop reported success; harness 3-strike detector remediating with SIGKILL | **v53 PASS / v43 FAIL — the frozen campaign's single error is gone at v53** |
| Forbidden outcomes (every leg + teardown) | no orphaned CH processes after stop/delete; no new host links; no new nft tables; no nwd dnsmasq; identity/ops determinism | all PASS (teardown residue all clean) | all PASS after SIGKILL remediation | **PASS both** |

Warnings recorded (v53 arm), both benign-known:
1. `Leg D: guest cputime did not advance (14 → 14) — idle guest?` — the
   harness's own non-gating observation; **identical in the v43 control**
   (15→15), so version-independent. (The frozen run 6 saw cputime advance —
   guest idleness varies; the frozen doc's timing-margins caveat covers this.)
2. `BFF list still renders the VM after delete` — the documented M2.5
   deferred-scope delete retention, re-observed identically in the frozen run
   (`qual_warn`, not a gate).

### 3.2 Assertion-count reconciliation (119 vs 111, and vs frozen run 6)

v53 = 119 PASS; v43 control = 111 PASS — the diff is fully explained:
+6 env-preflight lines (run inside the v53 namespace as the version-gate
proof; the control didn't need it), +1 `Leg F stop: guest down (CH process
exited)` (at v43 the same point produced the wedge FAIL), +1 `scenario
exited 0`. The v43 control's 111 PASS / 1 error / 3 warnings reproduces
frozen run 6 (111 PASS / 1 error / 2 warnings) with the same single error
(#345 wedge, Leg F) — the third warning is the idle-cputime observation
absent in the frozen run.

## 4. Known v53-defect interactions (leg 02 signatures) vs new findings

Leg 02 established: (a) reconnecting to an **idle** booted VM delivers ~278 B
then stalls (#8322 buffering defect); (b) an abortive client close (RST with
unread data) permanently and silently kills the serial-manager thread; (c)
clean EOF is handled well; (d) all v53 exit paths are clean. Cross-referenced
against this leg:

| Leg-02 defect | Did it interact with the M4.3 matrix? | Evidence |
|---|---|---|
| #8322 pre-connect buffering stall | **No assertion touched it; exposure noted.** Every console-marker wait in M4.3 (kernel banner, logind gates) occurs on a **fresh CH process**, where the agent's serial client connects at spawn — no pre-connect backlog; all three boots delivered the full stream (71,687 / 166,498 / 90,691 B, login prompt reached). The matrix has **no reconnect-then-marker-wait** assertion. The reconnects that do occur (agent restart, Legs B/E, on an idle VM) broke nothing. The agent's client is a continuously-draining reader by design (the console-log writer), which is leg 02's recorded mitigation; the crawl-aware-timeout mitigation is not needed by any current M4.3 assertion. | §3.1 Leg B/E rows; artifact sizes |
| Silent serial-manager thread death (RST) | **Not triggered.** The agent's close discipline is drain-then-close (#292 machinery: `drain_serial_receive_queue` forces an empty receive queue before close → FIN, not RST; `abandon_serial_connection` half-closes and drains). The SIGKILLed agent (Leg B) died with a drained queue → clean EOF, which v53 handles well (leg 02 check c). | `process.rs` drain-then-close paths; §3.1 |
| #345-class `GuestExit` wedge | **Not reproduced at v53 through the CHV stack — corroborates leg 02 check 5.** Both graceful stops (fresh-spawned Leg A, adopted Leg F) exited with the guest, API responsive until exit, no SIGKILL needed. The v43 control reproduced the wedge in the same leg with the same remediation, on the same CHV build — clean A/B attribution. | §3.1 Leg F row; §5 |

**New findings at v53.0: none.** Zero errors, zero unexpected warnings, no
new behavior outside the frozen matrix's expected shapes (console rotation on
graceful stop → 0-byte after-stop saves, identical to the frozen artifacts;
cumulative console across guest-level reboot; delete retention). No failure
occurred that required classification against leg-02 signatures beyond the
above.

## 5. The v43 control's wedge: why the harness, not the agent, cleared it

Included because it affects how the A/B should be read. The current main
agent carries the #346/#348 containment on this exact stop path
(`process.rs:3855+`: post-loop `prove_exited` → warn + SIGKILL +
`confirm_vmm_death`). The v43 control's preserved agent log shows the Leg F
stop ran the graceful path and logged **no** containment warn — the harness's
own 3-strike wedge detector (2 s polls) fired inside the ~1 s window between
the agent's stop loop ending on the dead-socket premise and its post-loop
`prove_exited` remediation, and SIGKILLed the process first. The wedge itself
— guest powers down, v43 control loop hangs, process alive with dead API,
stop reports success — is the frozen run-5/6 signature. Conclusion unchanged
either way: **v43 wedges on this path, v53 does not**; at v53 no remediation
of any kind was needed.

## 6. Harness drift vs the frozen M4.3

| Component | Frozen era (run 6, candidate `baa20c0e`) | This leg | Assessment |
|---|---|---|---|
| `m4.3-lifecycle.sh` | as of #350 | **unchanged** (last commit touching it: #350) | zero scenario drift |
| `deploy.sh` | as of #350 | +`CHV_QUAL_LOG_LEVEL` (#367), +stord `path_allowlist` propagation (#377) | robustness only; default log level `info` = evidence shape |
| `env-preflight.sh` | as of #357 | unchanged for qual (#459 touched kvm-smoke only); the version-gate reinstall hazard remains as disclosed — handled by the namespace model, verified in §2 | as designed |
| CHV stack | `baa20c0e` (rc1 boundary) | `b6d6ad50` — carries the post-rc1 fixes the frozen run recorded as findings: #342 (zombie reap), #344 (cache import), #346/#348 (stop-wedge containment), #334 (core-managed authority) | **intended drift**: this leg re-qualifies the *current* stack against v53.0, not the frozen candidate. This is why the frozen run's product findings (#339/#341/#343/#345) do not recur in either arm |
| Guest image / firmware | `noble-qual-patched.img` (`37f7c340…`), rust-hypervisor-fw 0.5.0 (`4a0a1e97…`) | same artifacts, same digests, read-only (stord seeds volumes by `fs::copy`; the shared image is never written) | identical |

## 7. Host-cleanliness proof and repo isolation

Final sweep (22:05:49 UTC, after both arms + post-mortem relocation):

- `/usr/bin/cloud-hypervisor`: sha256 `a250a934…`, v43.0.0 — **unchanged
  throughout** (§2).
- No `cloud-hypervisor`, `chv-*`, or `dnsmasq` processes (argv[0]-anchored
  patterns; the anchored form cannot self-match).
- Host links exactly the baseline (`ens19 eth0 lo`); **no new
  bridges/taps**; **no nft tables** remain.
- No listeners on :8080/:8443/:9100/:8444.
- No `/tmp/chv-qual-*` dirs (the v43 control's preserved post-mortem was
  copied into the campaign workdir and the host copy removed).
- `/run/chv/nwd` (created by the runs) removed by teardown. Pre-existing
  empty `chv:chv`-owned dirs `/run/chv/{agent,core,stord}` (stamped
  2026-09-30, M4.2 clean-install era) were **not** created by this leg and
  were left untouched.
- **No process holds the candidate bind mount** — the namespace is gone.
- Repo: `git status --porcelain` = 0 at leg start and end; no commits,
  branches, or edits made by this leg. The one HEAD movement during the leg
  was the PR #463 merge from another session — docs-only, and the harness
  scripts are byte-identical across it.

## 8. Judgment

**Does the lifecycle matrix pass at v53.0?** Yes — at **full tier**: this is
not a harness-tier-only result like leg 01; the matrix booted real guests
through the complete CHV chain and asserted guest-facing evidence (kernel
banners, logind, orderly shutdowns, power-state transitions) plus every
forbidden-outcome residue check. 119 PASS / 0 errors / 2 benign-known
warnings.

**Version attribution (the A/B):** with the CHV build, harness scripts, host,
image, and firmware held identical, v43.0.0 reproduces the frozen campaign's
single lifecycle error (#345 post-adoption stop wedge, Leg F) and v53.0 does
not — both stops exit cleanly with the guest. This corroborates leg 02's
check 5 (the `GuestExit` refactor holds) at the CHV-stack tier. The upgrade
strictly improves the lifecycle leg's worst recorded failure mode.

**What blocks the pin move?** Nothing from this leg. Residual considerations
for the maintainer (none introduced by this leg, all already on record):

1. **Leg-02 defects remain the pin move's live cost**: the silent
   serial-manager thread death (deterministic, upstream-unfixed) and the
   #8322 reconnect stall. This leg shows the current M4.3 assertion set does
   not intersect them (no reconnect-then-marker-wait; agent drains before
   close; reconnects on idle VMs broke nothing), and the CHV containment
   (#284/#292 machinery + #410 heal) carried the load. Any future lifecycle
   assertion that waits for console markers **after an agent/CP restart on an
   idle VM** would hit the ~278 B stall and needs leg 02's mitigations
   (continuously-draining reader — already the agent's shape — and
   crawl-aware ~1 KB/s timeouts).
2. **n=1 per arm.** The frozen M4.3 was iterated six times; this leg ran the
   matrix once per arm. The A/B contrast is clean and the passing arm had no
   near-misses (no wedge-detection strikes, no timeout recoveries), but a
   maintainer wanting repetition evidence can re-run cheaply (~7 min/arm,
   scripts and provenance in the campaign workdir).
3. **Nested virtualization only** (inherited frozen-campaign limit); the
   single-VM/serial-lifecycle and timing-margins caveats carry over
   unchanged.
4. The v53 arm's test dir was removed on success (standard harness
   behavior), so no agent log survives for it; the v43 control's full
   post-mortem is preserved in the campaign workdir. The full assertion run
   log (all 119 assertions) is preserved and is this document's record.

## 9. Not landed by this leg (separate work, not silently decided)

- The pin-move decision and the remaining campaign legs (per the
  [campaign index](README.md) leg table).
- Any CHV-side adaptation work — the leg-02 mitigation requirements remain
  recorded as pin-move conditions, not implemented here.

## Artifacts (campaign workdir, ephemeral)

Full run logs (both arms, all assertions), pass lists, pin-safety sweeps,
CHV build log, candidate provenance, per-leg console artifacts for both arms,
the v43 control's complete post-mortem (all four daemon logs, configs, DBs),
the frozen campaign's preserved m4.3 artifact backup, and the leg's runner
scripts. The load-bearing excerpts (verdict tables, assertion counts,
pin-safety proofs, A/B reconciliation) are embedded in this document.
