# CHV v0.3.0 Stable-Release Evidence Declaration

> Capability maturity vocabulary: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Evidence root for this release: `docs/evidence/production-readiness/v0.3.0/` (this document).
> Supersedes — never rewrites — the frozen RC declaration:
> [v0.3.0-rc1/00-execution-declaration.md](../v0.3.0-rc1/00-execution-declaration.md)
> (baseline `020e2b22a523b6e7e697a48bc2c020088bbf90e4`).

**Status of this document:** drafted as PR evidence for maintainer approval and a
separate independent review. This PR creates no tag, publishes no release, and
modifies no frozen tree (`docs/evidence/**` outside this new directory,
`docs/plans/**`, accepted ADRs, released CHANGELOG). The release cut itself —
the `VERSION` bump, the `v0.3.0` tag, and the GitHub Release — is a maintainer
action that follows approval of this declaration.

**Release line:** `v0.3.0` — the first **stable** release, cut from current
`main`. The maintainer decision (2026-10-04) is to cut now; the host-OS support
list is recorded in §10 (maintainer sign-off 2026-10-04).

---

## 1. Release frame (HEAD `9332f9b8`)

| Item | Value | Source |
|---|---|---|
| Release HEAD `main` SHA | `9332f9b853914d0df97639d359e8439d6eb5588a` (2026-10-04) | `git log` |
| `VERSION` at HEAD | `0.2.0` — **must be bumped to `0.3.0` in the release cut** (single source of truth; `docs/release/PIPELINE.md`) | `VERSION` |
| Git tags | `v0.1.0-mvp1`, `v0.2.0`, `nightly` (moving; advanced by the nightly packaging workflow) | `git tag` |
| GitHub Releases | only the `nightly` prerelease — **no stable release exists** | `gh release list` |
| CI at HEAD | **green** (2026-10-04 run 37208621319, 6 m 24 s) | `gh run list` |
| Security workflow at HEAD | **success** (push 37208621287 + scheduled 37198895242) | `gh run list` |
| Nightly Packages workflow | **success** at HEAD (the rc1-era recurring failure was fixed during the campaign window) | `gh run list` |
| Branch protection | `protect-main` ruleset (id 17358522) and `protect-tags` ruleset (id 24270232), both **enforced: active** | `gh api /rulesets` |
| Advisory policy | cargo-deny + real `cargo audit` in CI; `unused-ignored-advisory = deny`; every ignore documented with removal condition | `deny.toml`, Security workflow |
| Open PRs against `main` | none | `gh pr list` |
| Developers | @zoorpha @senolcolak | `.github/CODEOWNERS` |

## 2. Relationship to the frozen rc1 declaration

The rc1 execution declaration (Prompt 00, frozen at baseline `020e2b22`)
declared the RC production-ready **within a strict boundary** and listed seven
blockers before any stable-production claim (its §4). Since that baseline,
`main` has accumulated **184 reviewed commits** (`git log --oneline
020e2b22..9332f9b8`). This document reconciles that delta: it carries the rc1
boundary forward (§3), reconciles each rc1 blocker against the current world
(§4), classifies what the 184 commits actually did and at what verification
tier (§5), and maps every capability claim to the evidence that covers it at
the release HEAD (§6). Where the world changed since rc1 — materially, the
Cloud Hypervisor pin move v43.0 → v53.0 — this declaration states the change
and its evidence; it does not rewrite rc1 history.

## 3. Release boundary (carried from rc1 §3; changes marked)

The boundary below is the rc1 §3 boundary carried forward. **The only
material change is the VMM pin** (marked **[CHANGED]**); every other line is
carried verbatim in substance from rc1 §3, with precision records that
already existed in the frozen m4.9 §4 close-out.

- **VMM [CHANGED]:** Cloud Hypervisor only (`chv-agent` runtime CH), pinned
  **v53.0** — moved from the rc1-era v43.0 by the D6 option-(b) re-qualification
  campaign (#448; pin-move PR #468), with download digest verification in
  `scripts/install.sh` (qualified sha256 for both `cloud-hypervisor-static`
  and `ch-remote-static`; abort on mismatch). Evidence:
  [vmm-requalification/v53.0/](../../vmm-requalification/v53.0/README.md).
- **Host OS profile:** Linux x86_64; `.deb` (Debian/Ubuntu) + `.rpm` packages.
  Qualified on Ubuntu noble amd64 (nested KVM); `.rpm` untested in prompt-04.
  **The exact host-OS support list is recorded in §10 (maintainer sign-off
  2026-10-04).**
- **Network profile:** one CHV-owned overlay topology, host-safe after
  Prompt 01 (must not affect unrelated host/container/SSH/forwarded traffic).
  Precision record (frozen m4.9 §4.5, unchanged): the qualified topology is a
  single-host local Linux bridge (`chvbr0`/`br-<id>`) + taps; the multi-host
  VXLAN fabric is unproven.
- **Storage profile(s):** local file + LVM only. LVM is qualified at the stord
  layer only — not reachable from the VM lifecycle (#379, a recorded design
  decision). Ceph RBD / iSCSI are not claimed.
- **Migration:** single-host qualified migration only — quiescent-volume
  (single-writer) disk migration over mTLS between two stord instances on one
  host. **Multi-host migration remains unproven** and is stated as such. The
  concurrent-write hazard #394 is a disclosed boundary, not a claimed mode.
- **Lifecycle authority:** exactly one durable CellHV Core authority per node;
  legacy/control-plane paths are compatibility adapters only (default
  `authority_mode = "core-managed"` everywhere since #334).
- **Backup/restore:** **excluded from the supported matrix** — carried from
  rc1 §3 verbatim in substance. A backup without restore validation is not
  DR; backup/restore management is labeled **unsupported** in v0.3.0. The
  agent-layer CHV-API snapshot/restore surface exists separately and is not
  promoted by this release.
- **UI/BFF/CLI:** supported for the reference deployment, scoped to the
  exercised surfaces (m4.9 §4.6: all scenario evidence is chvctl/BFF/API-driven;
  the UI itself was not re-qualified in prompt-04).
- **Scale:** no concurrency, density, throughput, or requests-per-second
  claim. M4.8 numbers are baselines on the qualification host only.
- **Hardware minimums:** no qualified minimum (DEPLOYMENT.md's "4 cores /
  8 GB" is an unevidenced documentation claim).

## 4. rc1 §4 blocker reconciliation

The rc1 declaration listed seven blockers before a stable-production claim.
Current state, with primary references:

| # | rc1 blocker | Current state | Primary references |
|---|---|---|---|
| 1 | Host-global default-drop network policy (#227) | **CLOSED.** CHV firewall/NAT policy confined to CHV-owned guest traffic; host-safety gate qualified (Prompt 01, single-host KVM-VERIFIED) and re-verified 2026-10-03 by campaign leg 05 (VMM-independent by construction — both root-gated tests, no leaked links/nft tables). | #227 closed; #256; [01-network-isolation.md](../v0.3.0-rc1/01-network-isolation.md); [leg 05](../../vmm-requalification/v53.0/05-smokes.md) §2/§3 |
| 2 | Dual lifecycle authority + uninvoked Core executor (#231/#185) | **CLOSED.** Prompt-02 completed the single-authority cutover (M2.1–M2.4 evidence set); every prompt-04 and #448-campaign stack leg ran `core-managed` authority with real mTLS; #334 flipped the default config (install.sh, packaged `agent.toml`, reference `chv.yaml` after #441/#424). | #231, #185 closed; [02-single-authority-cutover/](../v0.3.0-rc1/02-single-authority-cutover/) (m2.1a–m2.5); #334; #441 |
| 3 | No durable real-KVM evidence (stub docs) | **CLOSED.** The full real-host evidence tree exists under the frozen rc1 root (m2.5 KVM qualification 48/48; m4.3–m4.9 real-host legs, in-stack, forbidden-outcome-asserted, KVM-VERIFIED), and the guest-facing legs were **re-qualified at the v53.0 pin** by the #448 campaign (M4.3 119/0, M4.6 141/0, M4.4 92/0, M4.5 103/0 — §6). | [04-real-host-qualification/](../v0.3.0-rc1/04-real-host-qualification/); [vmm-requalification/v53.0/](../../vmm-requalification/v53.0/README.md) |
| 4 | Backup/restore management broken no-op + record-only — no DR claim possible | **NOT RESOLVED — carried as a boundary exclusion, not a cleared blocker.** No backup/restore work is in the 184 commits; v0.3.0 keeps rc1's exclusion (§3 above) and makes **no DR claim**. A stable release within this boundary is consistent with rc1 §3, which already fenced backup/restore out of the supported matrix. | rc1 §3/§4; this §3; m4.9 §4 item 3 |
| 5 | No published GitHub Release — RELEASED tier unproven | **CLOSED BY THIS RELEASE.** #440 is the blocker; #454 made the pipeline publish the tarball at the exact URL `install.sh` constructs (shared assembler `scripts/release/assemble-tarball.sh`); pushing the `v0.3.0` tag triggers `release.yml` to publish tarball + `.sha256` + `.deb`/`.rpm` + SHA256SUMS. The release cut (§9) closes #440. | #440 open (closed by the release); #454; §9 below |
| 6 | Toolchain/generated-code reproducibility gap (#229) | **CLOSED.** Rust toolchain pinned via `rust-toolchain.toml` (#303); CI and release packaging consume the same pin through `.github/actions/setup-rust`; toolchain bumps are reviewable pin-only PRs. | #229 closed; #303; `rust-toolchain.toml`; `docs/release/PIPELINE.md` |
| 7 | Stale advisory ignores + placeholder security contact + no branch protection | **CLOSED.** (a) Advisory truth: 3 stale ignores removed (#305), real `cargo audit` wired into CI with reconciled ignores (#312), `unused-ignored-advisory = deny` — the Security workflow is green at HEAD. (b) Vulnerability reporting: real GitHub-only path, placeholder email removed (#306). (c) Protection: ruleset-based `main` protection with required security checks (#311) plus the tag ruleset (#314 review round) — both rulesets verified enforced today (§1). | #230/#146 closed via #305/#312; #306; #311; `gh api /rulesets` |

**Honest summary:** five of seven blockers are closed with primary evidence;
blocker 4 (backup/restore) is carried as an explicit unsupported-boundary
exclusion rather than a fix, exactly as rc1 §3 provided; blocker 5 is closed
by the act of publishing this release, which is the maintainer action this
declaration enables.

## 5. The 184 commits: classification and verification tiers

`git log --oneline 020e2b22..9332f9b8` = **184 commits**. Classified by file
footprint (code = touches `crates/`, `cmd/`, `ui/`, `scripts/`, `tests/`,
`.github/`, packaging, or lockfiles; evidence = touches only
`docs/evidence/` or `docs/plans/`; docs = other docs-only):

| Class | Count | Content |
|---|---|---|
| Code (with per-PR verification) | **124** | The waves below |
| Evidence / plans (frozen records) | **41** | m2.x–m4.9 evidence, campaign legs, plan docs |
| Living docs only | **19** | Documentation-standard adoption (#417), cluster accuracy passes (#419–#421, #433–#438), ADR-022 (#455/#456), docs index (#460/#461), D-register correction (#470), packaging/UI docs (#446/#449/#450) |

(Counts independently recounted at review; the load-bearing claims —
the `b6d6ad50..HEAD` Rust diff being exactly one file, and the wave
memberships below — are exact regardless of bucket boundaries.)

### 5.1 Code waves and their verification tier

| Wave | Commits / PRs | Verification tier of the wave |
|---|---|---|
| Prompt-01 network isolation | #256 | Real-host host-safety gate (single-host KVM-VERIFIED); frozen evidence |
| Prompt-02 single-authority (M2.1–M2.4) | #257–#261, #269–#272, hardening R1/R2 | Unit + integration at the time; KVM qualification at M2.5 (48/48); all frozen |
| ADR-021 fabric | #275–#287 (+ fabric pin bumps v0.1.1→v0.1.5) | Unit/CI per PR; multi-host fabric explicitly **unproven** (not in the supported boundary) |
| M2.5 serial/lifecycle fixes | #282, #284, #286, #290–#297, #299–#300 | Real-KVM re-qualification runs 8e–10b (47/47 → 48/48), frozen |
| Prompt-03 security/reproducibility/governance | #303–#318 | CI (toolchain pin, actions SHA pinning, advisory truth, rulesets); live fail-closed startup proof #304 |
| Prompt-04 qualification harness + M4.2 packaging | #322, #327, #329, #330, #332, #334, #337 | Real-host clean-install legs (container tier) + CI; #334 shipped canonical units with a new install-sh qualification leg |
| Prompt-04 fixes (M4.3–M4.6 legs + fix rounds) | #340–#370, #372–#382, #385–#388, #392–#397, #400, #404 | Each fix PR: unit/mock/integration per-PR tests; each milestone: real-host in-stack qualification leg (frozen) |
| M4.7 / M4.8 / M4.9 | #411, #412, #413 | Real-host in-stack qualification (frozen); **at the v43.0 pin — not re-run at v53.0** (§6, §12) |
| Correctness/security fix wave | **#441–#446, #449–#454** | Per-PR verification only — tier per PR in §5.2 |
| #448 campaign code changes | **#459** (kvm-smoke harness fix), **#468** (pin move, scripts/docs only — no Rust) | #459: real-host kvm-smoke runs (v43 + v53 override) on the qualification host; #468: `bash -n` + CI; fresh-download digest verification **not yet exercised in a container leg** (§12) |
| Serial-console mitigation layer 3 | **#471** (drain-continuously reader + crawl-aware boot-marker waits) | **Mock/stand-in tier only** (127 tests in `chv-agent-runtime-ch`, 1,564 workspace, E3f-shaped stand-in test); explicitly **not** live-KVM re-qualified (PR Residual risk) — §7, §12 |

### 5.2 The fix wave (#441–#446, #449–#454) — per-PR verification tier

These are **per-PR verifications, not real-host re-qualifications** unless
stated. None of them invalidates a frozen qualification leg (the campaign
stack that re-ran the real-host legs at v53.0 includes this entire wave —
§5.3), but their own verification is at the tiers below:

| PR | Change | Verification tier (from the PR's Tests section) |
|---|---|---|
| #441 | five small correctness fixes (chv.yaml authority mode, GITHUB_REPO default, agent.toml paths, load-test port, kvm-smoke asset) | `bash -n` + python packaging tests (config-assertion tier); no Rust |
| #442 | pin installer to qualified v43.0 (superseded by #468) | `bash -n` + grep sweep; script/docs only |
| #443 | RPM `~` pre-release suffix in version.sh | `smoke-version.sh` ordering proofs on the real RPM engine; script only |
| #444 | remove dead `chvctl upgrade` surface | `cargo test -p chvctl` (5), workspace check/clippy/fmt — unit/CI |
| #445 | define consumed-but-undefined CSS custom properties | `npm run check/test/build` — CI |
| #446 | docs note (packaged UI not served) | docs only |
| #449 | D3 interim docs (example proxy conf) | docs + packaging configs: also touched `scripts/package/smoke-common.sh` (presence assertion), nfpm configs, and `packaging/nginx/chv-example.conf` — CI |
| #450 | v53.0 upstream gap + pinned-version CVE disclosure docs | docs only |
| #451 | remove dead firewall-rules editor/CRUD (+ migration 0056 drop) | full workspace tests (80 targets; 283 BFF/store), UI 245 tests — unit/CI |
| #452 | guard legacy `set_firewall_policy` against empty rulesets (Closes #360) | workspace 1,564 tests incl. 4 new mock-nwd tests — unit/CI |
| #453 | fix + CI-gate the packaging security-parity test (Closes #439) | python suite 117 tests, now CI-gated — CI |
| #454 | release tarball attachment + installer fail-fast (Refs #440) | shellcheck, mocked-curl success/failure paths, assembler dry-run, live fail-fast against the real API — script/CI tier |

### 5.3 What stack did the v53.0 campaign actually qualify?

The #448 campaign legs 03–06 executed against CHV binaries **built fresh from
`b6d6ad50`** (leg 03; legs 04–06 reused them after verifying
`git log b6d6ad50..origin/main -- crates/ cmd/` empty); leg 05's M4.2 packaging
leg used packages built from **`05046ffa`** (docs-only past `b6d6ad50`).
Both commits **include the entire fix wave** (#441–#454), #455/#456, #459,
and #460/#461.

The delta from the qualified stack to the release HEAD is exactly:

| Commit | Content | Rust code? |
|---|---|---|
| #463–#467 (`05046ffa`…`87232ecc`) | campaign evidence docs | no |
| #468 (`451c7871`) | VMM pin move v43.0 → v53.0 + install.sh digest verification + harness defaults | **no** — scripts and docs only |
| #470 (`acb4ee51`) | D-register tracker correction | no |
| #471 (`9332f9b8`) | serial reattach reader (drain-continuously + crawl-aware waits) | **yes** — `crates/chv-agent-runtime-ch/src/process.rs` |

**Therefore the only Rust-code difference between the v53.0-qualified stack
and the release HEAD is #471**, whose verification is mock/stand-in tier
(§5.1). This is stated plainly and carried as a maintainer decision point
(§12, item 1) — it is not papered over by claiming the campaign legs cover
HEAD.

## 6. Capability → evidence mapping at the release HEAD

Tier vocabulary per the repo's discipline. "(a)" = frozen rc1 evidence at
baseline `020e2b22` (v43.0 pin); "(b)" = #448 campaign evidence at v53.0;
"(c)" = per-PR verification (unit/mock/integration/CI) — **not** a
real-host re-qualification unless the PR says so.

| Capability | Evidence at HEAD | Tier | Classification |
|---|---|---|---|
| **KVM qualification / VM lifecycle & recovery** | rc1 M4.3 six-leg matrix (a); campaign leg 03 at v53.0: **119 PASS / 0 errors** through the full CHV stack, with a same-session v43 control reproducing the frozen #345 wedge (b, stack `b6d6ad50`). Post-`b6d6ad50` delta = #471 only (c, mock tier). | KVM-VERIFIED (single host) | (a)+(b), with (c) for the #471 delta |
| **Single-authority cutover** | Prompt-02 evidence set (a); core-managed authority exercised by every prompt-04 leg and campaign legs 03–06 (b); default-config flip #334/#441 (c). | KVM-VERIFIED (single host) | (a)+(b)+(c) |
| **M4.4 network (guest path + host safety)** | rc1 M4.4 six-leg scenario + host-safety gate (a); campaign leg 05 at v53.0: **92/0/1** vs frozen run-4 count 86 exactly on the v43 control; host-safety gate PASS (VMM-independent) (b, stack `b6d6ad50`). Fix-wave network changes #451/#452/#361-era included in that stack. | KVM-VERIFIED (single host) | (a)+(b) |
| **M4.5 storage (local file + LVM)** | rc1 M4.5 scenario incl. root-gated real-LVM tests (a); campaign leg 05 at v53.0: **103/0/2** vs frozen run-10 count 97 exactly on the control (b, stack `b6d6ad50`). LVM stord-layer only (#379). | KVM-VERIFIED (single host, stord layer for LVM) | (a)+(b) |
| **M4.6 migration (two-stord, mTLS)** | rc1 M4.6 matrix (a); campaign leg 04 at v53.0: **141/0/0**, v43 control reproduces frozen run-5 (135) exactly — **version-independent by construction** (stord-layer, quiescent volume, no guest) (b). Boundary: #394 concurrent-write hazard disclosed. | KVM-VERIFIED (single host, stord layer) | (a)+(b) |
| **M4.7 fault & interruption matrix** | rc1 M4.7 (a) — **real-host, in-stack, at the v43.0 pin; not re-run at v53.0** (the #448 campaign's leg set did not include it). No fix-wave change is known to alter the fault-injection surface, but this is inference from review, not a re-qualification. | KVM-VERIFIED at v43.0 baseline only | (a) — **decision point, §12** |
| **M4.8 performance & soak baseline** | rc1 M4.8 (a) — same status as M4.7; numbers are host baselines, no scale claim either way. Note: the qualification host was **resized** after the v43.0 era; the campaign re-baselined only where its own legs measured. | KVM-VERIFIED at v43.0 baseline only | (a) — **decision point, §12** |
| **M4.2 packaging / clean install** | rc1 M4.2 + #334 install-sh leg (a); campaign leg 05 at v53.0: **Leg A PASS on fresh `05046ffa` packages** (0 errors/0 warnings); Legs B + install-sh-leg **environment-blocked** (host inotify co-tenancy — not CHV/VMM; left recorded per maintainer decision 2026-10-04) (b). install.sh checksum gap closed by #468's digest verification (c; fresh-download path not yet container-exercised). | CONTAINER-VERIFIED (Leg A at `05046ffa`); boot legs blocked | (a)+(b, partial)+(c) |
| **Serial console** | §7 below — defects upstream-unfixed at v53.0; three CHV mitigation layers merged; contained, not cured. | Contained within KVM-VERIFIED; gate above KVM-VERIFIED survives | (b) for defect truth; (c) for #471 |
| **Security (VMM CVEs, advisories, supply chain)** | Campaign leg 06 (b): CVE-2026-27211 closed **with runtime proof** (full chain reproduced at v43 under the CHV shape incl. `image_type: Raw`, fail-closed at v53 on every entry point); CVE-2026-45782 closed at **records tier**; no new advisories affect v53.0 (checked 2026-10-03); no new regressions; disk-image locking gained. Advisory policy + Security CI green at HEAD (c). | Runtime-proof (27211) / records (45782) | (b)+(c) |
| **Packaging artifacts / RELEASED tier** | `release.yml` + shared assembler (#454) produce `chv-0.3.0-linux-amd64.tar.gz` + `.sha256` + `.deb`/`.rpm` + SHA256SUMS at the exact URL `install.sh` constructs (§9). Becomes **RELEASED** when the GitHub Release is published. | RELEASED (upon publication) | (c) + the release act |
| **UI / BFF / CLI** | CI-VERIFIED (a carried forward; #444/#445/#451/#452 within it); scenario evidence is chvctl/BFF/API-driven; UI not re-qualified in prompt-04 (m4.9 §4.6). | CI-VERIFIED (reference-deployment supported) | (a)+(c) |
| **Multi-host anything** | **Unproven on this infrastructure** (rc1 §5, unchanged — single physical host). Capability capped at KVM-VERIFIED. | Not claimed | — |

## 7. The serial-console story at v53.0

**Defect truth (campaign leg 02, raw-CH A/B with v43 controls):** at the
v53.0 pin, of the m2.5 GUEST-PLATFORM BLOCKER's serial legs — the pty output
gate and the socket fd leak are **fixed** (upstream #7502); the #345-class
exit wedge **did not reproduce** at v53 (and did reproduce in v43 controls);
but:

1. **Silent serial-manager thread death — upstream-unfixed.** Deterministic,
   byte-for-byte the v43 defect: a client RST with queued unread data kills
   the serial-manager thread silently (no log line, no exit event); console
   service is dead for the life of that CH process while the guest stays
   healthy. Reported upstream as
   [cloud-hypervisor#8998](https://github.com/cloud-hypervisor/cloud-hypervisor/issues/8998)
   (open).
2. **#8322 pre-connect buffering stalls in the reconnect scenario —
   upstream-unfixed.** The backlog is lossless and in-order but delivery
   stalls after ~one socket fill (~278–330 B on an idle guest; ~1 KB/s crawl
   otherwise); only new guest output pushes it forward. Reported upstream as
   [cloud-hypervisor#8997](https://github.com/cloud-hypervisor/cloud-hypervisor/issues/8997)
   (open).

**CHV mitigation layers (all merged):**

- **#284 — rotation:** force-rotate the serial connection across `vm.reboot`,
  preserving the drain-then-close discipline (#292, vindicated and
  load-bearing at v53: graceful close is clean, abortive close is the killer).
- **#410 — heal:** behavioral detection of the dead-serial state and
  VMM-restart heal (reboot adopted VMs whose serial-manager thread died).
- **#471 — drain-continuously reader + crawl-aware boot-marker waits:** the
  reattach reader issues back-to-back blocking reads with no inter-read
  awaits, keeping the socket empty through CH's flush sessions (the E3f
  one-pass property); the boot watchdog's stall window is progress-based, so
  a crawling-but-alive console is waited out. Verified at mock/stand-in tier
  (the #410-era test tier); the CH-side mechanism is live-proven by leg 02's
  E3/E3f host-level measurements. **Not re-qualified on live KVM with the
  deployed agent — recorded, not silently dropped (§12, item 1).**

**What v0.3.0 claims here:** serial console is **supported within the
containment envelope** — rotation, drain-then-close discipline, behavioral
heal, and a continuously-draining reader — at KVM-VERIFIED tier. What it does
**not** claim: a cure. The two upstream defects are present at v53.0; the
recorded gate above KVM-VERIFIED survives the pin move (the defect exists at
both pins; re-verify at any future Cloud Hypervisor upgrade). Known residual
behavior inside the envelope: an adopted, marker-less, fully quiet guest can
still look stalled to the boot watchdog and take its bounded recovery reboot
(inherent to #8322, not agent-fixable); the console of a VM whose
serial-manager died is recovered by the heal path's VMM restart, not
transparently.

## 8. Security posture

- **CVE-2026-27211** (qcow2 backing-file host-file exfiltration; NVD 10.0):
  **closed at the v53.0 pin with runtime proof** — the full attack chain was
  reproduced at v43.0.0 under the exact CHV invocation shape (including
  `image_type: Raw`, proven **not** to protect v43), and v53.0 refuses
  fail-closed on every entry point (boot, reboot re-scan) with the canary
  never opened. Leg 06 §4.1/§4.2.
- **CVE-2026-45782** (virtio-block async-I/O UAF): **closed at records tier**
  (v53.0 > both fix versions; the v52.0 release merge bundling the fixes is
  verified an ancestor of tag v53.0). No runtime repro was attempted — a
  faithful one requires a malicious guest virtio driver; honestly
  tier-labeled. Leg 06 §4.3.
- **No new advisories affect v53.0** as of 2026-10-03 (upstream advisory list,
  OpenCVE, NVD; the only other advisory, CVE-2023-30612, affects v30/v31
  only). No new security regressions found on every swept surface; one
  improvement gained (disk-image byte-range locking, enforced at v53).
- **Landlock stays off** — recorded in D6: the CVE closes via the version move
  alone (`backing_files=false` + the agent's existing `image_type` pinning);
  Landlock has never been qualified and is disclosed as an option, not
  enabled.
- **Supply chain:** `scripts/install.sh` verifies the qualified sha256 digests
  of both VMM downloads and aborts on mismatch (#468); third-party Actions
  are pinned to immutable commit SHAs (#307); the Rust toolchain is pinned
  (#303); cargo-deny + cargo audit run in CI with `unused-ignored-advisory =
  deny` and documented ignores. The release tarball carries a `.sha256`
  sidecar and SHA256SUMS cover the packages.
- **Repository governance:** `protect-main` and `protect-tags` rulesets
  enforced (§1); the GitHub-only vulnerability-reporting path is real (#306).

## 9. Release mechanics (what closes #440)

1. **Version:** bump `VERSION` `0.2.0` → `0.3.0` (reviewable release-cut PR;
   `scripts/version.sh` derives every packaged form from it).
2. **Changelog:** the same release-cut PR adds the `## [0.3.0]` section to
   `CHANGELOG.md` — `release.yml`'s `Validate changelog` step runs
   `scripts/release/extract-changelog.sh 0.3.0` and **fails stable builds
   without it** (fail-closed gate; `[Unreleased]` alone does not satisfy it).
3. **Tag:** `v0.3.0` on the release commit (protect-tags ruleset governs it).
4. **GitHub Release:** `release.yml` fires on the `vX.Y.Z` tag and publishes
   the Release with assets assembled by the shared
   `scripts/release/assemble-tarball.sh` (#454): the tarball, its `.sha256`
   sidecar, the `.deb` and `.rpm` packages, and `SHA256SUMS`.
5. **Tarball path — verified against the code:** `scripts/install.sh`
   (`download_release`) constructs
   `https://github.com/${GITHUB_REPO}/releases/download/v${INSTALL_CHV_VERSION}/chv-${INSTALL_CHV_VERSION}-linux-amd64.tar.gz`
   with `GITHUB_REPO=kubedoio/chv` (default since #441/#425), i.e. for this
   release exactly:

   ```
   https://github.com/kubedoio/chv/releases/download/v0.3.0/chv-0.3.0-linux-amd64.tar.gz
   ```

   `scripts/release/assemble-tarball.sh` (the single source of truth for
   layout and naming, used by both `scripts/build-release.sh` and
   `release.yml`) emits `dist/chv-<version>-linux-amd64.tar.gz` and documents
   that name as matching the URL install.sh constructs. The two were
   reconciled by #454 precisely so the first stable release publishes a
   tarball the installer accepts (the pre-#454 workflow assembly would have
   shipped a tarball missing `tmpfiles/chv-node.conf` — install.sh fails
   closed without it).
6. **With the Release published**, `install.sh`'s from-GitHub path resolves
   `latest` → `0.3.0`, downloads, verifies, and installs; **#440 closes**.
7. **Host-OS support list:** recorded in §10 (maintainer sign-off
   2026-10-04); this declaration does not invent it.

## 10. Host-OS support list (maintainer sign-off recorded 2026-10-04)

Derived from the install docs, the packaging pipeline, and the
qualification evidence; signed off by the maintainer on 2026-10-04
(release decision: cut v0.3.0 stable). Every line carries its evidence
tier.

| Dimension | Support statement | Tier / evidence |
|---|---|---|
| Architecture | **x86_64 (amd64) only.** The release tarball is `linux-amd64`; all published packages are x86_64. `install.sh` *accepts* `arm64`/`aarch64` at the script level, but no released artifact exists for it — script-level acceptance is **not** a support claim. | Artifact inventory (`build-release.sh` `ARCH="linux-amd64"`; nightly assets x86_64-only) |
| `.deb` distros | **Debian 12, Ubuntu 24.04** | CI tier: package smoke + upgrade/downgrade lifecycle tests in `debian:12` and `ubuntu:24.04` containers (`scripts/package/smoke-deb.sh`, `lifecycle-deb.sh`), run in the release pipeline before publishing |
| `.rpm` distros | **Rocky Linux 9** | CI tier: same smoke + lifecycle coverage in `rockylinux:9` (`smoke-rpm.sh`, `lifecycle-rpm.sh`) |
| glibc floor | **≥ 2.35** — release binaries are built on the pinned `ubuntu-22.04` runner specifically so they load on the oldest matrix distro (Debian 12 = glibc 2.36) | `release.yml` build-pin rationale |
| Kernel / virtualization | Linux with `/dev/kvm` (VT-x / AMD-V) required | Qualification tier: the qualified host ran Ubuntu 24.04, kernel `6.8.0-142-generic`, 16 vCPU AMD EPYC 9554P, 31 GiB |
| Hardware minimums | 4 cores / 8 GB RAM / 50 GB disk — **documented guideline only, explicitly unevidenced**; no scale claims are derivable from the qualified host | `docs/DEPLOYMENT.md` (self-labeled) |
| Source installs | Ubuntu/Debian build host, Rust toolchain (`rustup`), Node.js 22+ and `npm` | `docs/DEPLOYMENT.md` build prerequisites |
| VMM (pinned dependency) | Cloud Hypervisor **v53.0** — downloaded by `install.sh` with sha256 digest verification; rust-hypervisor-firmware 0.5.0 | Qualification tier: the #448 re-qualification campaign (`docs/evidence/vmm-requalification/v53.0/`) |

Honesty notes carried into the release notes: (1) the distro matrix is
**CI container tier**, not per-distro real-host qualification — the
real-host qualified deployment was Ubuntu 24.04 amd64; (2) the hardware
minimums remain labeled as an unevidenced guideline; (3) arm64 is not
supported in v0.3.0.

## 11. Rollback / withdrawal of the release

The release is withdrawable without touching `main`:

- **Delete the GitHub Release** (removes all downloadable assets; #440 would
  reopen — `install.sh` fails fast with actionable diagnostics, which #454
  guarantees).
- **Delete the `v0.3.0` tag.** Both are pure publication artifacts.
- The code is already on `main` regardless; withdrawal is a distribution
  action, not a code revert. A corrected re-release uses a new tag
  (`v0.3.1`) per the versioning policy — `VERSION`-derived package ordering
  (`nightly < rc < stable`, #443) keeps channels coherent.
- Installed hosts are unaffected by withdrawal except that re-installation
  from GitHub fails fast with the documented alternatives (nightly packages
  or build-from-source via `make build-release` + `INSTALL_CHV_TARBALL_PATH`).
  Existing hosts keep their installed v43.0/VMM or v53.0 pin as-is (the
  already-installed branch never downgrades the VMM; upgrade semantics are
  ADR-007/D8 territory).

## 12. Claims that could NOT be cleanly mapped to evidence at HEAD (maintainer decision points)

Listed explicitly, not papered over:

1. **#471 (drain-continuously serial reader) is verified at mock/stand-in
   tier only.** It is the only Rust-code delta between the v53.0-qualified
   stack (`b6d6ad50`) and the release HEAD. The campaign legs did not and
   could not cover it. A live leg-02-style recheck with the deployed agent on
   v53.0 is the natural follow-up before/with rollout (recorded in #471's own
   Residual risk). The maintainer accepts mock-tier coverage for the release
   or holds it — this declaration only records the state.
2. **M4.7 (fault & interruption matrix) and M4.8 (performance & soak) were
   not re-qualified at v53.0.** The #448 campaign's leg set did not include
   them; their evidence remains the frozen v43.0-pin records. The pin-move
   decision (#448, maintainer 2026-10-03) accepted this scope; the capability
   table (§6) labels them accordingly.
3. **M4.2 boot legs (Leg B + install-sh-leg) are environment-blocked** (host
   inotify co-tenancy, not CHV/VMM) and were left recorded per the
   maintainer decision of 2026-10-04. Leg A (package install/contract) passed
   on fresh `05046ffa` packages.
4. **The install.sh fresh-download path with digest verification has not
   been exercised in a container leg** (the #468 recorded follow-up; blocked
   by the same host co-tenancy). The digest logic itself is
   campaign-evidence-grounded (every leg re-verified the same digests).
5. **Backup/restore remains excluded** (rc1 blocker 4) — a boundary exclusion,
   not a fix; no DR claim is made.
6. **The serial-console upstream defects (#8997, #8998) are unfixed
   upstream**; v0.3.0 ships containment, not a cure, and the gate above
   KVM-VERIFIED survives.
7. **Multi-host, FIELD-QUALIFIED, and bare-metal tiers remain unproven on
   this infrastructure** (single physical host, nested KVM) — unchanged from
   rc1 §5.
8. **The host-OS support list** is recorded in §10 (maintainer sign-off
   2026-10-04).

## 13. Non-scope (carried from rc1 §6, plus additions)

Carried verbatim in substance from rc1 §6: NetBox projection; VMware
import/migration; additional VMMs; external Ceph/iSCSI qualification;
another storage backend; Kubernetes/operator machinery (incl. any
CNI-coexistence claim); speculative control-plane HA; database replacement;
major UI redesign; broad OpenStack compatibility claims; Designer expansion;
frontend/tonic major toolchains (#234/#235).

Added for v0.3.0:

- **Landlock sandboxing stays off** (recorded in D6; unqualified, not needed
  for the CVE closure at v53.0).
- **#447 (ServeDir-from-disk UI serving, D3 target) remains open** — the
  packages ship an example reverse-proxy conf only (D3 interim, #449);
  serving-from-packages is CODE-SUPPORTED, UNQUALIFIED.
- **The leg-02 upstream serial defects** (§7) — no CHV-side cure claimed.
- **M4.2 boot legs left environment-blocked** (maintainer decision
  2026-10-04; §12 item 3).
- **`--no-shutdown` VMM-lifetime option** — not adopted (campaign decision
  Option B; #457 remains open as a future simplification candidate).
- **Backup/restore** (§3, §12 item 5).
- **Node-upgrade orchestration / ADR-007 presentation** — D8 open; the dead
  `chvctl upgrade` surface was removed (#444).

---

*Prepared as a docs-only PR. No tag created, no release published, no frozen
tree modified. Maintainer approval and a separate independent review precede
the release cut.*
