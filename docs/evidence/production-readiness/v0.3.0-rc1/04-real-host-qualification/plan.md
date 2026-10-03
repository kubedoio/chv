# CHV Production-Readiness — Prompt 04 Real-Host Qualification Plan

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [04-real-host-qualification](/docs/prompts/production-readiness/04-real-host-qualification.md)
> Capability maturity ladder: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Release boundary: [../00-execution-declaration.md](../00-execution-declaration.md) §3 (frozen).

---

## 1. Preconditions revalidated (2026-09-30, live)

| Prompt-04 precondition | Status |
|---|---|
| Prompts 01–03 merged | ✅ 01: merge `731a89cf` (#256); 02: through `b95b90fb` (M2.5 run 10b 62/62); 03: `2d4853ed`–`fdfe9c3d` (#302–#318, post-merge review complete) |
| Exact candidate SHA fixed | ✅ **`fdfe9c3d`** (main, 2026-09-30). CI + Security + Nightly green on it; nightly packages `0.2.0.nightly.20260930.gfdfe9c3` (.deb + .rpm × controlplane/node/chvctl) published with CI provenance |
| Disposable real Linux/KVM hosts | ⚠️ single shared host (below) — same-host evidence, honestly labeled; no second physical host |
| Package-equivalent layout | ✅ nightly `.deb` artifacts exist for the candidate; `install.sh` + `kvm-smoke.sh --packages` shape reusable |

Maturity frame inherited: this prompt can advance capability to
**KVM-VERIFIED** across the claimed matrix on the qualification host.
MULTI-HOST-VERIFIED and above remain unprovable on this infrastructure
(declaration §5) and are reported as unproven.

## 2. Qualification environment (this host, recorded honestly)

| Item | Value |
|---|---|
| Host | **4 vCPU, 7.8 GiB RAM**, 333 GiB free disk, `/dev/kvm`, Ubuntu kernel `6.8.0-139-generic`, x86_64, root |
| vs. M2.5 host | **Different, smaller box** (M2.5 ran on 16 vCPU / 31 GiB). All prompt-04 evidence is generated fresh on this host; M2.5 evidence is not reused as this prompt's proof |
| LVM | `lvm`/`vgcreate`/`losetup` present — LVM storage profile testable via loopback PV |
| Container runtime | none installed; `apt` works (used in prompt 03) — `systemd-nspawn`/`debootstrap` installable for clean-install isolation |
| VMM | cloud-hypervisor **v43.0.0** static release binary (download verified reachable) |
| Firmware | rust-hypervisor-firmware **0.5.0** — repo renamed to `cloud-hypervisor/rust-hypervisor-firmware`; asset URL verified reachable |
| Guest seed | Ubuntu noble server cloud image amd64 (qcow2) — verified reachable |

**Resource constraint (recorded):** 4 vCPU / 7.8 GiB bounds the matrix to
sequential single-guest legs (guest: 2 vCPU / 1 GiB, matching M2.5). No
concurrency, density, or scale claim may be derived from this host; results
are baseline measurements labeled with the exact hardware.

**Host-reboot recovery:** the box is shared and cannot be rebooted. The
"host reboot" leg of the prompt's lifecycle matrix is **not provable here**
and will be recorded as such; the closest provable subset (full four-daemon
stack restart with the VM running, cold-start reconciliation) is exercised
instead and labeled as a subset.

## 3. Workstreams (each a reviewable PR; harness code committed, evidence recorded)

> Unlike M2.5 (host-local harness, quoted in evidence), prompt 04's harnesses
> are **committed to the repo** under `scripts/integration/qual/` — reusable,
> reviewable, and the evidence docs reference exact script + SHA.

### M4.1 — Qualification harness + environment provisioning (PR-1)

`scripts/integration/qual/`:
- `env-preflight.sh` — root, `/dev/kvm`, pinned CH version, firmware/image
  presence + sha256, repo SHA capture, binary staging to an isolated dir
  (release build of the candidate; embedded `--version` strings recorded);
- `deploy.sh` — the M2.5-proven candidate-path deployment as committed code:
  self-signed CA, server cert (SAN localhost/127.0.0.1), real mTLS
  (pre-placed client cert only for the EnrollNode handshake, then the
  control-plane-issued node cert CN=node id), one-time bootstrap token via
  the loopback-only internal endpoint, admin bcrypt seeding, `0700` agent
  runtime dir (Core `validate_paths` contract), stord path allowlist,
  `authority_mode = "core-managed"`, **no `CHV_ALLOW_INSECURE`**;
- shared assert helpers (forbidden-outcome assertions first-class).

Exit criteria: deploy.sh brings up control-plane → token seed → stord/nwd →
agent on this host; enrollment issues the node cert; core API socket up;
TenantReady; teardown cleans everything.

### M4.2 — Clean installation baseline (PR-2)

- Download the candidate's nightly `.deb`s (`gfdfe9c3`) — CI-built,
  package-equivalent, provenance recorded;
- install into an isolated clean container (`systemd-nspawn`/`debootstrap`,
  same physical host — labeled) and on throwaway dirs via
  `kvm-smoke.sh --packages` shape;
- assert: service users, directories + modes, sockets, config validation,
  startup ordering, systemd unit presence from packaging;
- capture component versions + redacted config.

### M4.3 — Lifecycle & recovery on real KVM (PR-3)

Via `chvctl → BFF → control-plane → agent gRPC → Core → CH v43.0` on this
host, single guest (`qual-vm-1`, 2 vCPU / 1 GiB, firmware boot, default
network):

create → start → guest-boot evidence (console.log kernel banner + login) →
reboot (guest-level, same CH process) → stop → start → delete; plus:
- **agent SIGKILL while Running** (S1 replay: CH survives, convergence,
  exactly one CH process, no duplicate state);
- **control-plane restart while Running** (VM unaffected, reconnect);
- **management-plane outage/reconnect** (BFF/API unavailable window,
  deterministic operation history after);
- **full stack restart** (all four daemons, cold reconcile) — the provable
  subset of host-reboot recovery, labeled as such;
- identity + operation-history determinism asserted at every leg.

### M4.4 — Network qualification (PR-4)

- Prompt-01's privileged host-safety suite **rebuilt as a committed script**
  (it was host-local in prompt 01) and executed against the candidate;
- guest path: attach → connectivity → policy allow/deny → detach → cleanup →
  restart/reconcile, with forbidden-outcome assertions on the host stack;
- unsupported coexistence (K8s/CNI, Docker-forwarded, multi-bridge) recorded
  as not claimed — inherited from prompt 01, restated.

### M4.5 — Storage qualification (PR-5)

The prompt's reusable contract
(`validate → provision/consume → attach → guest write/read → restart/interruption → recover → detach → cleanup → repeat`)
for **local file** and **LVM** (loopback PV → VG → LV) — the declared storage
profiles. Ceph RBD / iSCSI: not claimed (declaration §3 non-scope).

### M4.6 — Two-stord mTLS migration (PR-6)

Positive: CA-issued identities for source/destination stord; migration over
mTLS; dirty rounds + paused final sync execute; destination data verified
(digest). Negative (each a dedicated leg, fail-closed asserted): missing TLS
config; wrong CA; wrong destination identity/server name; mismatched
keypair; malformed cert/key/CA; expired certificate (constructed via
short-lived CA); plaintext endpoint/downgrade attempt; interrupted transfer
with deterministic retry/recovery.

### M4.7 — Fault & interruption matrix (PR-7)

On real KVM (reusing M2.4's fault shapes at the process level): service
restart during active operation; migration interruption; control-plane loss;
agent reconnect; storage/network provider restart; repeated cleanup /
idempotent retry. Every leg asserts **forbidden outcomes** (duplicate VM
processes, lost/duplicated authoritative state, orphaned disks/taps), not
merely eventual success.

### M4.8 — Performance & soak baseline (PR-8)

Measure, don't guess, all labeled with the exact hardware and "baseline
measurement, not a scale claim": idle CPU/RSS per daemon; lifecycle
operation latency; bounded concurrent API workload (control-plane latency);
migration throughput on the qualified path; control-plane DB growth; open
fd/socket growth; log/metric cardinality; bounded steady-state soak with
repeated operations (sequential, resource-bounded on this host).

### Evidence + status (PR-9)

Evidence matrix vs. prompt-04 acceptance criteria; status section; residual
risk; campaign declaration updates if any boundary changed.

## 4. Evidence matrix (prompt-04 acceptance → milestone)

| Acceptance criterion | Milestone |
|---|---|
| exact candidate passes real-KVM lifecycle and recovery | M4.3 (host-reboot leg: recorded not-provable, subset exercised) |
| network isolation gate passes on real Linux | M4.4 |
| every advertised storage path has real-system evidence | M4.5 (local file + LVM only) |
| two-stord mTLS migration passes positive + negative identity cases | M4.6 |
| backup claim includes restore validation or is explicitly narrowed | **explicitly narrowed** — excluded from RC claims (declaration §3; no evidence needed, claim absent) |
| interruption/retry does not duplicate or lose authoritative state | M4.7 |
| baseline performance/soak data without unsupported scale claims | M4.8 |
| cleanup leaves no unexplained residue | every milestone's teardown asserts + final sweep |

## 5. Scope & non-scope

**In scope:** the frozen release boundary (declaration §3): CH-only VMM,
deb/rpm host profile, one VXLAN topology, local file + LVM storage,
single-host two-stord mTLS migration, single durable Core authority,
UI/BFF/CLI for the reference deployment.

**Out of scope (unchanged from the declaration):** backup/restore as DR
(broken no-op manager — claim excluded); multi-host migration; Ceph/iSCSI;
K8s/CNI coexistence; NetBox; VMware import; additional VMMs; control-plane
HA; CH newer than v43.0 (the v43 serial-console upstream defect remains the
recorded gate above KVM-VERIFIED; machinery-side remediation is complete
and verified per M2.5 run 10b).

**Stale-docs note (from the declaration):**
`docs/specs/component/live-migration-spec.md:212` claimed dirty rounds are
never sent — contradicted by `sender.rs` (and by the M4.6 scenario's
dirty-round log evidence). Corrected on main by **#398** (docs-only, after the
M4.6 scenario's final run): the spec now records quiescent-volume migration as
the claimed mode and #394's concurrent-write boundary as not claimed — see
[m4.6-migration.md](m4.6-migration.md) §4.1/§6.

## 6. Status

- **PR-0 (plan): merged (`fa2b5724`).**
- **M4.1 (harness + environment): COMPLETE** — `scripts/integration/qual/`
  committed (lib.sh, env-preflight.sh, deploy.sh; shellcheck-clean);
  environment provisioned (CH v43.0.0, firmware 0.5.0 matching the M2.5
  digest, noble guest image); candidate `baa20c0e` staged (code-identical
  to `fdfe9c3d`); deployment smoke passes errors=0 warnings=0 with
  zero-residue teardown. Two findings filed: #320 (chvctl health routes),
  #321 (SQLite WAL unlink hazard). See [m4.1-harness.md](m4.1-harness.md).
- M4.1 findings fix round (post-rc1, 2026-10-01): **COMPLETE** — #320
  fixed (viewer-tier `/v1/health` routes implemented, verified live via
  `deploy.sh --exec` scenario; see m4.1-harness.md finding 4) and #321
  resolved by documentation + corrected mechanism record (re-verification
  showed the external-close unlink does not reproduce while the CP holds
  pool connections; the real loss vector is direct sidecar file
  manipulation; OPERATIONS.md gains "Live Database Access" rules —
  read-only URIs for live reads, stop-windows for manual writes; see
  m4.1-harness.md finding 3).
- M4.2 (clean installation baseline): **COMPLETE** —
  `scripts/integration/qual/clean-install.sh` committed (clean noble
  container via debootstrap + systemd-nspawn, Legs A static + B boot;
  errors=0 warnings=3) plus the `kvm-smoke.sh --packages` host leg
  (PASSED, real /dev/kvm). Static packaging contract holds; control plane
  fails CLOSED on a bare install (packages↔install.sh boundary); agent
  unit holds the /run Core contract. Four findings filed: #323 (stord
  unit holds the /run Core contract. Four findings filed: #323 (stord
  unit user vs storage ownership), #324 (nwd unit /run/netns), #325
  (postinst group-membership grep bug), #326 (packaged legacy authority
  default). Two kvm-smoke harness bugs fixed in-repo (v1 certs, cleanup
  abort). See [m4.2-clean-install.md](m4.2-clean-install.md).
- M4.2 fix round (post-rc1, 2026-10-01): **COMPLETE** — all four M4.2
  findings fixed on main (stord storage ownership → chv runtime user with
  the model documented on the unit; /run/netns via packaged tmpfiles;
  per-entry membership guards; shipped configs default core-managed
  authority) and re-qualified with the same harness against locally
  rebuilt packages: Legs A+B errors=0 warnings=0 (stord/nwd units now
  active; agent core-managed on a bare install; supervisor defers to the
  unit daemons), kvm-smoke host leg PASSED, upgrade-path residue
  corrected in place. One new finding filed during the fix round: #328
  (netns creation under the unit needs CAP_SYS_ADMIN — maintainer
  decision, M4.4). See [m4.2-clean-install.md](m4.2-clean-install.md) §8.
- #328 follow-up (2026-10-01): **RESOLVED on main** — the capability
  decision was made: `chv-nwd.service` grants `CAP_SYS_ADMIN` (ambient +
  bounding) with the ProtectSystem tradeoff documented on the unit and
  `RestrictAddressFamilies` added as a compensating control. Proven on
  the real host via a transient unit with the exact final security
  context (netns add/list/del as `chv`), and the clean-install legs
  re-run errors=0 warnings=0 against packages rebuilt from the fix
  commit. The agent-supervised fallback nwd remains bootstrap-only
  (documented limitation). Issue #227 (firewall host-safety) was also
  verified as already fixed by the prompt-01 work (PR #256 + hardening)
  and closed with evidence. See
  [m4.2-clean-install.md](m4.2-clean-install.md) §9.
- Fix round 2 (2026-10-01, review-driven): **COMPLETE** — a comprehensive
  review of the prompt-04 diff range found the install.sh path had
  drifted (embedded stale pre-#323/#328 systemd units; pre-#323 storage
  chown in `start_services()`; missing bcrypt apt dependency; minor doc
  staleness). All fixed; the tarball now ships the canonical units +
  tmpfiles entry which install.sh installs verbatim (fail-closed, no
  embedded copies anywhere); new permanent harness leg
  `qual/install-sh-leg.sh` covers the install.sh path on a clean
  container (30 assertions + 7 host-side source-of-truth parity
  checks, errors=0 warnings=0) so it cannot silently
  drift again. A round-3 fresh-eyes pass over that fix hardened the leg
  (real exit gate, source-of-truth parity vs packaging/) and provisioned
  the credential encryption key on both install surfaces (#335 — report
  finding H-7, previously claimed fixed but never wired: S3 credentials
  were plaintext on every default install; now AES-256-GCM with the key
  minted create-if-absent and asserted by both legs). Final: clean-install
  Legs A+B and install-sh-leg both errors=0 warnings=0. See
  [m4.2-clean-install.md](m4.2-clean-install.md) §10.
  - Round 4 (same day) closed the last silent-plaintext path (empty-key
    mint/write guard on both surfaces + daemon warns on unset OR empty;
    tmpfiles apply failure fatal) and re-qualified both legs green.
  - Round 5 verified the round-4 fixes correct and swept the wider repo:
    no MAJOR findings; fixed remaining doc-drift surfaces (DEPLOYMENT.md
    dead nwd.toml keys + missing authority_mode + missing encryption-key
    step; legacy docker-compose.prod.yml and examples/bootstrap.sh dead
    keys; stale assertion count here). See
    [m4.2-clean-install.md](m4.2-clean-install.md) §10 round-5 note.
- M4.3 (2026-10-01): **COMPLETE** — lifecycle & recovery on real
  nested-KVM, six legs in one scenario
  (`qual/m4.3-lifecycle.sh` via `deploy.sh --exec`): full lifecycle
  (create-and-boot, guest-level reboot same-CH-pid, graceful stop,
  re-spawn), S1 agent-SIGKILL adoption, CP restart, 60 s
  management-plane outage, four-daemon cold restart (the labeled
  host-reboot subset), stop → delete — identity + operation-history
  determinism asserted at every leg. Final run: 111 PASS, 1 error =
  the recorded product finding #345 (graceful stop of an adopted VM
  wedges the VMM; detected + remediated in-run per the documented
  operator escape), teardown clean. Four product defects found and
  filed with reproductions — #339 (image import → vm create chain,
  harness works around via absolute-path `--image`), #341 (graceful
  stop leaks an unreaped VMM zombie), #343 (agent restart with
  unflushed deferred reports bricks startup — Leg E gated on the
  drain), #345 (the wedge above) — each with a narrow fix PR on main
  (#340, #342, #344, #346). Harness defects found and fixed along the
  way: `pgrep -x` comm-truncation (every prior CH residue check was
  vacuously green — lib.sh + deploy.sh), missing `chvbr0`/nft teardown
  fallbacks, the deployment-error gate reset. A post-milestone
  comprehensive review of the merged range produced three hardening
  PRs (#348 stop-path SIGKILL verification, #349 image-chain lookup
  termination/canonicalization, #350 harness robustness), the CI
  timeout guard #352, and one tracked follow-up (#351, delete-path
  kill refusal) — see m4.3-lifecycle.md §6. See
  [m4.3-lifecycle.md](m4.3-lifecycle.md).
- M4.4 — COMPLETE (network qualification): prompt-01's host-safety gate
  rebuilt as a committed script (`host-safety.sh`; candidate-identity
  guarded) and re-proven green on the candidate's nwd code; the guest
  path qualified end-to-end (attach → connectivity incl. cloud-init
  seed/DHCP reservation/ping/ARP → policy attempt → nwd hard-kill with
  supervisor recovery and idempotent re-attach → stop/start with stable
  tap/IP → second-network fallback-CIDR collision → cleanup) with
  host-stack forbidden-outcome assertions and an all-green teardown
  (final run: 0 errors, 8 warnings, 77 assertions, rc=0).
  Four candidate defects recorded with issues — #354 (N1
  name↔network_id split with fallback-CIDR subnet collision), #355 (N2
  no operator-reachable policy path / deployed nft table bare), #356
  (N4 orphaned nic rows blocking network delete + N5 network delete
  performs no host teardown); harness gains:
  dnsmasq+genisoimage preflight, dnsmasq/nwd teardown fallbacks, run
  logs clean of job-control noise.
  **Re-qualification on post-fix main (close-out, §7 of the evidence
  doc):** after #358/#361/#359/#362/#363 fixed N1–N5 and #364
  truth-updated the scenario, the re-run found and fixed two NEW
  defects — N6 absent-gateway → L2-only bridge on operator networks
  (#365) and N7 a UI-dialect firewall ruleset terminally bricking VM
  creates at attach (#369, with save-time validation against the
  engine vocabulary defined once in `chv_common::firewall`) — plus
  three observability fixes (#365 effector-failure logging, #366
  supervisor daemon stdio, #367 `CHV_QUAL_LOG_LEVEL`) and one harness
  truth-update (#370, nwd's host-perspective policy direction
  semantics). Final run (binaries `3e9bbcc3`, scenario `0d91291f`):
  **0 errors, 1 warning (the deliberate N2-era record), 86 assertions,
  rc=0**, host-safety gate green on the same build, teardown all-green.
  Open follow-ups: #368 (transient effector failure terminally wedges
  a journaled create — no re-drive) and #355's UI store unification.
  See [m4.4-network.md](m4.4-network.md) §7.
- **M4.5 — storage qualification: COMPLETE** (scenario PR #382, squash
  `1463e5d6`; evidence [m4.5-storage.md](m4.5-storage.md)). Final run
  (binaries `b7e03e6f` = post-review-fixes main): **97 passes, 0 errors,
  2 warnings (both Leg E's deliberate M2.5-retention records), rc=0**;
  host-safety (loop/LV/VG residue) all-green on the same build. The milestone
  found and fixed three product defects — all-digit VM ids crash cloud-init's
  NoCloud datasource (#374/#375), the supervisor-respawned stord silently loses
  its path confinement and relocates runtime_dir (#376/#377), and volume clone
  could never succeed because the target volume row was never created
  (#380/#381). A post-merge comprehensive review round fixed two more
  (#387: clone targets were ownerless — admin-only in the BFF — plus
  `{:?}`-as-TOML in the supervisor's respawn config; #388: scenario
  hardening — LVM cleanup on every exit path, delete-intent and
  owner-inheritance assertions) and recorded three issues (#384
  physical-table upserts unguarded + clone TOCTOU, #385 respawn drops
  operator stord.toml keys, #386 ownerless import/template volumes), and
  corrected the evidence doc's run-progression record. Boundaries recorded
  as issues: snapshot/clone accepted-then-fails-closed on core-managed
  nodes (#378, accepted-then-silent dispatch-retry UX gap) and LVM
  unreachable from the VM lifecycle (#379, design decision;
  LVM is qualified at the stord layer only, 7/7 root-gated real-LVM tests).
  Open follow-ups carried: #368, #355, #384, #385, #386, plus #378/#379
  residuals noted in the evidence doc.
- **M4.6 — two-stord mTLS migration: COMPLETE** (scenario + evidence PR; evidence
  [m4.6-migration.md](m4.6-migration.md)). Final run (run 5, on the post-review
  artifact with the vendored grpcurl checksums asset; binaries `6a1dfa06`,
  code-identical to the main of the runs `80afd8db` for crates/proto — #403,
  merged after run 5, is comment-only in `sender.rs` and behavior-identical): **135 passes, 0 errors,
  0 warnings, rc=0**, teardown all-green.
  One scenario (Leg P + N1–N9 via `deploy.sh --exec`): the positive path
  (seeded 4 GiB volume → BULK_COPY → dirty-round machinery → pause handshake →
  resume → COMPLETED) with harness-level digest + byte-compare of source vs
  destination, and a nine-case negative matrix — missing TLS config (trigger-time
  refusal + startup-exit half-config variants), wrong CA, wrong server name,
  wrong destination identity, mismatched keypairs, malformed material, expired
  certificate, plaintext/downgrade (http:// force-upgraded to https://, no
  plaintext path exists), and interrupted transfer with deterministic recovery
  (mid-BULK_COPY destination SIGKILL → create_new refusal → documented operator
  recovery → COMPLETED + digest). Every identity-rejection leg asserts
  fail-closed: task FAILED, zero bytes transferred, no receiving volume on the
  destination. The scenario+evidence PR itself changed no product code (campaign
  rule) — the milestone's product fixes landed ahead of it as #393 (mTLS
  receiver listener), #396 (ack protocol) and #397 (finalize digest), with
  #403 (comment-only) after the runs; three product characteristics recorded
  in the evidence doc: the #394
  concurrent-write boundary (dirty-round evidence is quiescent-source only —
  dirty-block transfer is proven at protocol level by the in-repo e2e test
  alone), destination-only (receiver-only) stords are not expressible
  (`enabled=true` makes the client identity mandatory — **issue #401**), and
  mTLS rejection observability (client-side rejections collapse to one
  transport-error text; server-side rejections surface as an opaque
  race-dependent form and the destination logs nothing — **issue #402**). The
  stale `live-migration-spec.md` "Critical Implementation Gaps" section was
  corrected on main by #398, and the sibling `disk-migration-protocol-spec.md`
  by #403 (both docs-only, post-run) — the specs now record quiescent-volume
  migration as the claimed mode, matching the #394 boundary above.
- **M4.7 — COMPLETE** (final green run 5: 226 pass / 0 errors / 5 warn,
  rc=0, candidate `5870a4a5`; log `/tmp/opencode/m4.7-run5.log`):
  `m4.7-faults.sh` (preamble + legs F1–F7 + final sweep) kills each
  service *inside an operation's execution window* — CP mid-CreateVm dispatch,
  agent mid-StartVm CH-spawn, stord mid boot-volume provision (the only
  operator-reachable volume path on a core-managed node; standalone
  attach/detach fails closed there — disclosed deviation), nwd mid
  tap-provision, source stord mid-BULK_COPY (extends M4.6 N9 to the source
  side), CP mid-BULK_COPY (the direct stord↔stord path must survive it) —
  plus the idempotent-cleanup leg (re-delete, double-stop,
  delete-vs-in-flight-create). Every leg asserts the forbidden outcomes
  (duplicate VM processes, double-spawned daemons, lost/duplicated state,
  orphaned disks/taps) with converges-OR-fails-cleanly disjunctions (#368
  terminally-failed-but-clean is a PASS with disclosure); shared checkers
  written once (CH-process counts, tap counts, row↔backing bijection,
  both-journals-terminal, one-CreateVm-row-per-VM). Evidence:
  `m4.7-faults.md` (status COMPLETE). **Three product findings filed and
  fixed from the run series — #405 (delete-after-restart tap/session leak;
  fixed #407), #406 (retried-delete 500; fixed #408), #409 (agent death
  with serial traffic in flight wedges the VM half-booted while everything
  reports Running; fixed #410 — live-validated via the wedge-then-heal
  console artifact in run 5, after the F2 trigger was tightened twice to
  target the serial-RST condition deterministically: submit-level op
  success → firmware output → kernel-spew)**. #368 boundary disclosed
  (F3/F4), not gated. Not claimed: CP-orchestrated
  migration, host reboot, multi-node, M2.5 delete retention (warned, not
  gated).
- **M4.8 — IN PROGRESS** (`m4.8-perf-soak.sh` + `m4.8-perf-soak.md` skeleton
  committed; live run pending): prelude + idle baseline (P1) → sequential
  soak, N=6 cycles of create→start→stop→delete with zero-residue assertions
  per cycle and lifecycle latency distributions measured on the cycles (P2) →
  idle-after-soak fd/socket leak verdict vs the pre-registered thresholds
  (P3) → bounded concurrent API workload, 4 readers + 2 writers for 90 s,
  labeled bounded-by-this-host (P4) → migration throughput on the M4.6
  two-stord mTLS path, 4 GiB seed (P5) → final sweep (P6). Measurements are
  records, not gates; leak/forbidden-outcome checks are assertions, with
  thresholds pre-registered in the script and evidence doc before the run.
- M4.9: not started.
