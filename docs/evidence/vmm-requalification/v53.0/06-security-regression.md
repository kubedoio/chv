# D6-(b) Leg 06 — Security-regression re-qualification: Cloud Hypervisor v43.0.0 → v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [Campaign index](README.md) · [Leg 01 (anchor)](01-anchor-leg.md) · [Leg 02 (serial re-check)](02-serial-console-recheck.md) · [Leg 03 (M4.3 lifecycle)](03-m43-lifecycle.md) · [Leg 04 (M4.6 migration)](04-m46-migration.md) · [Leg 05 (smokes)](05-smokes.md)
> Date: 2026-10-03, 23:00–23:16 UTC
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §7); this document is the leg's only repo deliverable
> Scope: verify the two known upstream CVEs in the pinned v43.0.0 close at v53.0, that no new advisory affects v53.0, and that the move introduces no new security regressions — at records tier (advisories, fix-commit containment) and runtime tier (same-session v43/v53 A/B at raw-CH and full-stack tiers)
> Host: the qualification host (16 vCPU AMD EPYC 9554P, 31 GiB RAM, `/dev/kvm`, kernel `6.8.0-142-generic`, Ubuntu 24.04)

## Verdict (summary)

| Question | Answer |
|---|---|
| Do the two known CVEs close at v53.0? | **YES — both.** CVE-2026-27211: **proven at runtime** on this host, full attack chain reproduced at v43.0.0 under the exact CHV invocation shape and refused (fail-closed) at v53.0 on every entry point. CVE-2026-45782: closed at **records tier** (v53.0 > fix versions; fix commit verified in the v53.0 tag); no practical runtime repro exists without a malicious guest virtio driver — honestly tier-labeled below. |
| Any NEW advisories affecting v53.0? | **NO.** Upstream has exactly 3 published advisories; the third is CVE-2023-30612 (affected v30.0/v31.0 only). Nothing new affects v53.0 (released 2026-07-12, tag `9ed824d`). **No campaign-stopping finding.** |
| New security regressions at v53.0? | **None found** on every surface swept (process/thread hardening, socket/attack-surface inventory, CHV-side sandbox posture, serial/seccomp signatures). One security **improvement** confirmed (disk-image locking, v46.0 #6974 — enforced at v53, absent at v43). One availability-relevant behavioral delta recorded (§4.6). The leg-02 defects (silent serial-manager thread death; #8322 reconnect stall) remain the pin move's known live cost — unchanged by this leg, not intersected by any check here. |
| What blocks the pin move? | **Nothing from this leg.** Recorded deferrals (all already on record from legs 04/05): install-path supply chain (install.sh checksum gap), the Landlock posture decision, and the §9 disclosure removal — all pin-move-PR items, §6. |

## 1. Advisory research (records tier)

### 1.1 CVE-2026-27211 / GHSA-jmr4-g2hv-mjj6 — "Host File Exfiltration via QCOW Backing File Abuse"

- **Component:** virtio-block disk backend / qcow2 image opening (`op=open` device-manager path).
- **Attack vector:** a malicious guest overwrites its own raw-backed disk header with a crafted QCOW2 structure whose backing-file path points at an arbitrary host file. On the next VM boot or disk scan, image-format auto-detection parses the header and **serves the host file's contents to the guest as disk storage**. Guest-initiated reboots are sufficient to trigger the re-scan and do not exit the VMM process — no management-stack interaction needed. Precondition: the backing image is guest-writable or from an untrusted origin (CHV's qualified storage profile: local-file raw volumes, guest write access).
- **Severity:** upstream High 7.2 (CVSS:4.0, AV:L/AC:L/AT:P/PR:N/UI:N, CWE-73); **NVD scores it Critical, CVSS 3.1 = 10.0** (`AV:N/AC:L/PR:N/UI:N/S:C/C:H/I:H/A:N`).
- **Affected versions:** v34.0 – v50.0. **Patched:** v50.1, v51.0. The pinned **v43.0.0 is in the affected range** (released-version impact tier: host files that are valid qcow2 images, ≥64 KB — the raw-backing and small-file escalations were never in a release).
- **Fix commits** (from the advisory's revision table): `509832298b68…` (2026-02-10, mitigation `backing_files=false` enabled by default) and `a63315df54e0…` (2026-02-19, hardening: explicit image typing via the UI + prevent sector-zero writes for auto-detected raw images). **Both verified ancestors of the v53.0 tag** (GitHub compare `v53.0…<sha>` → status `behind`, ahead_by 0).
- **v53.0 > v51.0** — in the patched range.
- **Advisory mitigation besides upgrading:** Landlock sandboxing (`--landlock`). CHV exposes the knob (`hv.landlock_enable`) but the agent's default tuning sends `"landlock": false` (`crates/chv-agent-runtime-ch/src/core_runtime.rs:972`, `process.rs:3298`) and it has never been qualified — disclosed as an option in DEPLOYMENT-ARCHITECTURE §9, not enabled. Pin-move-PR consideration (§6).

### 1.2 CVE-2026-45782 / GHSA-f47p-p25q-83rh — "Use-after-free in virtio-block Async I/O Completion"

- **Component:** virtio-block async I/O completion path (io_uring / aio).
- **Attack vector:** a guest submits two virtio-block descriptor chains that **reuse the same `head_index`** while asynchronous block I/O is enabled (the default). If the kernel completes the duplicate operation before the original, the completion path frees a bounce buffer the kernel is still using (bounce buffers are in play when the guest descriptor is unaligned) → memory-corruption primitive in the VMM process, escalatable to arbitrary code execution / full guest-to-host escape. **Async I/O on virtio-block is the default configuration.**
- **Severity:** upstream High 8.9 (CVSS:4.0, CWE-416).
- **Affected versions:** v21.0 – v51.1. **Patched:** v51.2, v52.0. The pinned **v43.0.0 is in the affected range.**
- **Fix:** upstream PR #8220, commit `1314ac883c64…` (remediation by dgreid/Meta). **Verified ancestor of the v53.0 tag** (compare → behind_by 609, ahead_by 0).
- **v53.0 > v52.0** — in the patched range.
- **Advisory workaround besides upgrading:** `_disable_io_uring=on` + `_disable_aio=on` per virtio-block device. The CHV agent sets **neither** (grep over `crates/` `cmd/` — no async-I/O keys anywhere); live `vm.info` on both arms confirms `disable_io_uring: false, disable_aio: false`, and `io_uring` anon-inode fds are present in both arms' CH processes. I.e., **the CHV usage shape is the affected default on both arms**; the exposure at v43 is real, and no config mitigation exists in the stack today.

### 1.3 New-advisory check for v53.0 — NEGATIVE (campaign-stopping check)

Sources checked 2026-10-03: the upstream repo's advisory list (exactly **3** advisories total), OpenCVE's vendor CVE list (CVE-2026-45782, CVE-2026-27211, CVE-2023-30612 — nothing newer), and NVD. The third advisory is **CVE-2023-30612 / GHSA-g6mw-f26h-4jgp** (malicious HTTP requests closing arbitrary fds via the API socket; affected v30.0/v31.0 only, patched v30.1/v31.1, Moderate 4.0) — **not applicable to either the v43.0.0 pin or v53.0** (both past v31.1; the restricted-socket posture is unchanged, §4.5). **No advisory affects v53.0. No campaign-stopping finding.**

### 1.4 What advisories alone prove vs what needed runtime evidence

Advisories + tag containment prove v53.0 contains both fixes (records tier). They do **not** prove the binaries behave as patched at the CHV invocation points, nor that no *new* surface appeared. That is what §3/§4 add.

## 2. Provenance and pin-safety

| Artifact | Version | sha256 (observed = expected) | Notes |
|---|---|---|---|
| `cloud-hypervisor-static` (candidate) | v53.0 | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` — **MATCH** | reused from the legs 03–05 staging after re-verification; originally upstream release asset |
| `ch-remote-static` (candidate) | v53.0 | `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7` — **MATCH** | same staging, re-verified |
| System pin `/usr/bin/cloud-hypervisor` | v43.0.0 | `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496` | **verified before and after every run** (each raw-CH run logs its own before/after pair; stack arms bracketed by runner checks; final sweep re-verified) — never written, mounted over only inside the v53 namespace arm, executed read-only for all v43 controls |
| CHV binaries | 0.2.0, commit `b6d6ad50`, build 2026-10-03 | reused from `/var/lib/chv/qual/bin` | reuse condition verified live: `git log b6d6ad50..origin/main -- crates/ cmd/` is **empty** (main moved `b6d6ad50` → `aba53949`, all docs-only: PRs #463/#464/#465; PR #466 also docs-only). No rebuild needed; no new SHA. |
| Guest image / firmware | noble-qual-patched / rust-hypervisor-fw 0.5.0 | `37f7c340…` / `4a0a1e97…` | read-only, digests verified, never written |

Isolation model (campaign standard): raw-CH runs invoked each binary **by path** (v43 = the pin, read-only; v53 = the staged candidate); the stack v53 arm ran inside `unshare --mount` with the candidate bind-mounted over `/usr/bin/cloud-hypervisor` namespace-locally (digest-verified inside the namespace before any harness code ran); the stack v43 control ran the pin read-only on the host, no namespace. The stack arms reused `deploy.sh --exec` (same shape as legs 03–05).

Environment constraint honored: **no nspawn-based scenarios were used**; the co-tenant kube-apiserver fleet was not touched (54 processes at leg start and end — the fleet is dynamic, not modified by this leg) and **no sysctls were changed**. No systemd-booted containers were needed by any check in this leg.

## 3. Verification matrix

| # | Check | Tier | v43.0.0 evidence | v53.0 evidence | Verdict |
|---|---|---|---|---|---|
| 1 | CVE-2026-27211: backing-file chain followed on fresh boot, **autodetect** (no `image_type`) | runtime (raw-CH) | boots; **canary (undeclared host file) opened O_RDONLY**, served as guest storage | `vm.boot` refused: `"Cannot open disk path" / "Unsupported feature (path=… op=open)" / "Maximum disk nesting depth exceeded"`; canary never opened | **v43 VULNERABLE / v53 FIXED (fail-closed)** |
| 2 | CVE-2026-27211: same with **`image_type: Raw` declared (the CHV agent's exact shape)** | runtime (raw-CH) | boots; **canary opened O_RDONLY** (strace: `open("…canary.qcow2", O_RDONLY\|O_LARGEFILE\|O_CLOEXEC) = 65` after `open("…evil.qcow2", O_RDWR…) = 64`) | same refusal as #1; canary never opened | **v43 VULNERABLE even with explicit typing / v53 FIXED** — new empirical finding about v43 (§4.2) |
| 3 | CVE-2026-27211: **reboot re-scan** (header lands in a guest-writable raw disk after boot, then `vm.reboot`) | runtime (raw-CH) | after reboot: fd 62 `victim.raw` O_RDWR + **fd 63 `canary.qcow2` O_RDONLY** — the re-scan follows the chain with no management interaction beyond the guest's own reboot | `vm.reboot` refused with the same backing-file error; canary never opened; CH alive, API responsive | **v43 VULNERABLE (full chain) / v53 FIXED (fail-closed)** |
| 4 | CVE-2026-27211: fix presence in v53.0 | records | v43.0.0 in affected range v34.0–v50.0 | fix commits `5098322`, `a63315d` are ancestors of tag v53.0; live `vm.info` shows `"backing_files": false` in effect | **CLOSED** |
| 5 | CVE-2026-45782: fix presence in v53.0 | records | v43.0.0 in affected range v21.0–v51.1; async I/O (io_uring) confirmed active in the CHV shape | v53.0 > v52.0; fix commit `1314ac8` (PR #8220) is an ancestor of tag v53.0; async I/O still default (fixed path in use) | **CLOSED at records tier** (runtime repro not feasible — §4.3) |
| 6 | New advisories affecting v53.0 | records | n/a | upstream advisory list = 3 total; third is CVE-2023-30612 (v30/v31 only); OpenCVE/NVD show nothing newer | **NEGATIVE — no stop finding** |
| 7 | Disk-image file locking (v46.0 #6974 / v50.0 #7494) | runtime (raw-CH + stack) | host fcntl write-lock probe **acquires** the CH-held disk (no lock); stack arm: volume + seed ISO both acquirable | probe **REFUSED (EAGAIN)** on data.raw, on the stack volume `26e73e9f.img` and on `seed.iso`; live config shows `"lock_granularity": "ByteRange"` | **security improvement at v53** (§4.4) |
| 8 | Process/thread hardening from /proc | runtime (raw-CH + stack) | main thread Seccomp=0; worker threads Seccomp=2, filters 1–2; NoNewPrivs=0; CapEff/CapBnd full (root); 10–11 threads | identical shape; serial-manager filters **2** (v43: 1) — the leg-02 signature, seccomp applied in-thread and functioning; no syscall-kill events in any run | **UNCHANGED / functioning** (no regression, no new exposure) |
| 9 | Socket / attack-surface inventory of a running CH | runtime (raw-CH + stack) | only AF_UNIX (api socket + serial socket); no TCP listeners; serial.sock `srwx------` root-only in a 0700 runtime dir; fd count 107–144 | same shape; no TCP listeners; serial.sock `srwx------`; fd count 115–160; new `vm.sock.lock` file (API-socket lockfile, new surface at v53 — root-only, removed with the runtime dir) | **NO new exposed surface** |
| 10 | CHV-side sandbox posture (deploy.sh / agent spawn shape) | runtime (stack) | CH spawned as `cloud-hypervisor --api-socket <runtime-dir>/vm.sock`, no `--seccomp=false`, no `--landlock`; config via REST `vm.create`; agent default sends `"landlock": false`; disks declared `image_type: Raw` | identical (verified live via `vm.info`: serial Socket mode, console Off, disks Raw) | **UNCHANGED** — no external sandbox exists in the harness; CH's own seccomp carries both arms (see #8); Landlock off on both (§6) |
| 11 | Leg-02 signatures (silent serial thread death; #8322 stall; in-thread seccomp silent-exit edge) | carried / runtime corroboration | m2.5 defect set (frozen) | not triggered by any check in this leg (no reconnect-then-marker-wait, agent's draining reader used); serial-manager alive with filters=2 in every booted run; the **in-thread seccomp-apply silent-exit edge remains a v53-specific widening** of the thread-death defect (not triggered, carried forward) | **carried, unchanged** (§5) |

## 4. Findings in detail

### 4.1 CVE-2026-27211 runtime A/B — method and full-fidelity chain (runtime tier, raw-CH)

Artifacts (campaign workdir): `canary.qcow2` (a valid qcow2 "secret" image, 1.25 MB file, 16 MiB virtual, content = 0x44 pattern, backing chain readback verified with `qemu-io`), `evil.qcow2` (196 KB overlay whose **backing file is the absolute host path of the canary**), `data.raw` (plain raw disk). VM shape per leg 02 / m2.5 e-series: 2 vCPU, 512 MiB, firmware `rust-hypervisor-fw 0.5.0`, no NIC, per-run api/serial sockets, draining serial reader (leg-02 console discipline). The stack-shape cases replicate the agent's exact `vm.create` payload (CLI: `--api-socket` only; serial Socket mode in JSON; console Off; disks with `image_type: Raw`).

**v43.0.0 (qualified pin, executed read-only):**

- Fresh boot, autodetect: `vm.create` + `vm.boot` succeed; after boot the CH process holds an open fd on `canary.qcow2` (`canary-open: 1`). The undeclared host file is being served to the guest as disk storage — the exfiltration primitive.
- Fresh boot, **`image_type: Raw` declared**: identical result — plus syscall-level proof (strace, preserved in the workdir): the declared disk is opened `O_RDWR` (fd 64) and then the **undeclared backing file is opened `O_RDONLY` (fd 65)**. **Explicit image typing does not protect v43** — see §4.2.
- Reboot re-scan (full chain): boot with a clean raw `victim.raw` (declared Raw), then write the overlay's bytes into the disk file *after boot* (the byte-identical equivalent of the guest overwriting its own disk header), then `vm.reboot` → the fd table afterwards shows `victim.raw` O_RDWR (fd 62) **and `canary.qcow2` O_RDONLY (fd 63)**. The advisory's key exploitability claim ("guest-initiated reboots are sufficient; no management-stack interaction") is reproduced on this host: the exposure is continuous across the VM's life, not boot-only.

**v53.0 (candidate, staged by path):**

- Every entry point into the backing chain fails closed, with the identical API error on `vm.boot` and on `vm.reboot`:
  `["Error from API","The VM could not boot","Error from device manager","Cannot open disk path","Unsupported feature (path=… op=open)","Maximum disk nesting depth exceeded"]`
- The canary file is **never opened** (`canary-open: 0` in all v53 arms); the CH process stays alive and API-responsive through the refusal (fail-closed, not a crash).
- Live `vm.info` through the CHV stack shows the mitigation's default in effect: `"backing_files": false` on every disk, alongside the agent's `"image_type": "Raw"`.

Honest scoping note: the guest-written-header step was simulated host-side (the same bytes land in the guest-writable file through the same open fd the guest writes through); no in-guest driver was needed. The VMM-side code path (header sniff → qcow2 parse → backing-file open) is identical either way, and the reboot-re-scan step (the only guest-timed element) was exercised live via `vm.reboot`.

### 4.2 New finding (v43-side, refines the §9 disclosure) — explicit `image_type: Raw` does not mitigate CVE-2026-27211 at v43

The CHV agent has pinned `image_type` since before the frozen campaign (`crates/chv-agent-runtime-ch/src/process.rs:3194-3199`, introduced in `691132cc`, pre-rc1): `Raw` for every non-`.qcow2` path. One might have read #448's "the candidate should pin `image_type=raw` … (also the CVE-2026-27211 hardening)" as implying the current stack is thereby protected at v43. **It is not**: run #2 above proves v43.0.0 opens the backing file even when the disk is declared Raw — v43's declared type does not gate the qcow2 sniff/backing-file open. Consequences: (a) the §9 disclosure's severity claim for CHV ("maps directly onto CHV's raw-disk + guest-reboot profile") is fully vindicated empirically; (b) the *only* thing that closes this exposure is the version move (or Landlock, unqualified); (c) at v53 the declared Raw + `backing_files=false` default together make the agent's existing shape fail-closed — no CHV code change is required for the CVE to close, though the `image_type` pinning remains good defense-in-depth and matches upstream's deprecation of autodetection (v52.0 #8219).

### 4.3 CVE-2026-45782 — why records tier, and the exposure shape (runtime-observable context)

A faithful reproduction requires a **malicious guest**: two virtio-block descriptor chains reusing the same `head_index` with unaligned descriptors (to force the bounce-buffer path) while async I/O is enabled. The qualification guest is a stock patched noble image; crafting a guest-side virtio driver PoC is outside this leg's scope and would add nothing beyond the advisory's own analysis. Recorded instead, with tier labels:

- **Records tier:** v43.0.0 ∈ affected range (v21.0–v51.1); v53.0 > both fix versions (v51.2, v52.0); fix commit `1314ac8` (PR #8220) verified inside tag v53.0.
- **Runtime-observable exposure shape (both arms):** the CHV stack runs the affected configuration — `disable_io_uring: false`, `disable_aio: false` in live `vm.info`, `io_uring` anon-inode fds present in the CH process fd table, `iou-wrk-*` threads present. I.e., at v43 the CHV deployment model (any running guest with default async block I/O) is exposed exactly as the advisory describes; at v53 the same configuration runs the fixed completion path.

### 4.4 Disk-image locking — security improvement confirmed (runtime tier)

Upstream v46.0 #6974 (file-level locking) + v50.0 #7494 (byte-range locks) — flagged by #448 as an interaction point and by this leg's brief as a known v53 delta worth recording. Confirmed at **both** tiers with the same host-side probe (attempt to acquire a conflicting POSIX write lock on a CH-held disk):

- v43: probe **acquires** (no lock) — raw-CH arms and the stack arm (volume `2ba15782.img`, `seed.iso`).
- v53: probe **REFUSED (EAGAIN)** — raw-CH arm (`data.raw`) and the stack arm (volume `26e73e9f.img`, `seed.iso`); live config reports `"lock_granularity": "ByteRange"`.

Security relevance: a locked disk cannot be silently opened writable by a second process (e.g., a concurrent stord migration target or an operator error) while a VM holds it — this is the hardening that the M4.6 leg's #394 boundary discussion touches, enforced by the VMM at v53.

### 4.5 No-new-regressions sweep (runtime tier, raw-CH + CHV stack)

**Process/thread hardening** (`/proc/<pid>/status`, per-thread `task/*/status`), identical security shape on both arms, verified at raw tier and at stack tier under the agent:

| Field | v43 | v53 |
|---|---|---|
| Main thread `Seccomp` | 0 (no filter on the process leader) | 0 |
| Worker threads `Seccomp` | 2 (filters 1–2) | 2 (filters 1–2; serial-manager **2** vs v43's 1) |
| `NoNewPrivs` | 0 | 0 |
| `CapEff`/`CapBnd` | full (root — the agent spawns CH as root) | full (unchanged) |
| Threads (booted VM, no NIC) | 11 | 10 |
| Threads (stack VM, 1 NIC) | ~16 | ~16 |

**Socket/attack-surface inventory** of a running CH under the agent: both arms expose **only** the two AF_UNIX sockets (`vm.sock` API + `serial.sock`), both inside the agent's 0700 per-VM runtime dir, serial socket `srwx------` root-only; **no TCP listeners** on either arm (`ss -tlnp` shows only stack/co-tenant ports). `/proc/<pid>/fd` exposure is comparable (v43 107–144 fds, v53 115–160; both dominated by eventfds; the v53 delta is the `vm.sock.lock` lockfile + slightly more epoll/eventfd plumbing — no new file or socket *classes* exposed). The v43 serial socket shows 3 CH fds per accepted client vs v53's 2 (the v43 dup'd-fd pattern leg 02 documented — cosmetic here, one client, no reboot cycling).

**CHV-side sandbox posture:** `deploy.sh` and the agent apply **no external sandbox** (no systemd-run/cgroup/seccomp wrapper; CH spawned as a plain child with `--api-socket` only) — the CHV-side boundary is the 0700 runtime dir + root-only unix sockets + CH's own self-applied seccomp, identical on both arms. This is unchanged from the frozen v43 campaign posture (DEPLOYMENT-ARCHITECTURE §2/§4 trust model: the guest is untrusted, the CH process is root-privileged and trusted, and the serial/api surfaces are node-local). No regression, no improvement — recorded as-is.

**Leg-02 known v53 deltas re-verified at stack level:** serial-manager thread name `serial-manager` (comm-scan heal remains structurally valid), per-thread seccomp applied and functioning (filters 2), no seccomp-kill events; graceful stop of the sweep VM exited CH cleanly on both arms (0 CH processes after stop, deploy teardown all-PASS).

### 4.6 One availability-relevant behavioral delta (not a security regression)

At v53, a disk whose header has been overwritten with qcow2 magic **fails the next boot/reboot** (the §4.1 refusal) instead of silently exfiltrating. A malicious guest can therefore make its own VM un-bootable by writing a qcow2 header to its raw disk (it could corrupt its disk anyway — the data was always guest-writable). The failure is loud: `vm.boot`/`vm.reboot` return a device-manager error the agent/API surfaces, and the CH process stays healthy. Recorded as the intended fail-closed trade-off of the a63315d hardening, with the operational note that support/debug flows will see "Cannot open disk path / Unsupported feature / Maximum disk nesting depth exceeded" for guest-corrupted disks.

## 5. Known-defect interactions (leg-02 signatures, carried)

| Leg-02 defect | Interaction with this leg | Evidence |
|---|---|---|
| Silent serial-manager thread death (RST with queued unread data; upstream-unfixed at v53) | **Not triggered** — every serial client in this leg was a continuously-draining reader (the leg-02 mitigation, and the agent's own shape); no abortive closes were performed. Remains the pin move's live cost, carried by #284/#292/#410. | all runs; serial-manager alive in every booted run |
| #8322 pre-connect buffering stall (reconnect to idle VM delivers ~278 B then stalls) | **Not triggered** — no reconnect-then-marker-wait in any check; fresh-CH waits only. | run designs above |
| **In-thread seccomp-apply silent-exit edge (v53-specific)** | **Carried as a security-relevant v53 observation**: v53 applies the serial-manager's seccomp filter inside the thread (filters=2 observed, functioning on this host); a filter-application failure would exit the thread through the same panic-only `catch_unwind` silent discard — a new early-exit path for the thread-death defect. Not triggered in any run (no filter-apply failure is known on this kernel). The #410 heal's comm-scan detector covers this path too (thread death is thread death), but the silent channel remains. | leg 02 §4.4/§4.6; this leg's per-thread seccomp tables |
| #345-class exit wedge | **Not reproduced at v53** (legs 02/03); the sweep VM's graceful stop exited CH cleanly on both arms. | §4.5 |

## 6. Recorded deferrals (not silently dropped; none introduced by this leg)

1. **Install-path supply chain** (leg 05 §4.3): install.sh downloads the CH binary with `curl -fsSL` and **no digest check** (v43 or v53). Adding the campaign digests belongs in the pin-move PR, together with the pin move itself (`scripts/install.sh:413`, `43.0` → `53.0`) and the unit wiring.
2. **Landlock posture** (#448 asks for a decision as part of the new qualified tuple): the advisory's non-upgrade mitigation for CVE-2026-27211 is Landlock; CHV exposes `hv.landlock_enable` but sends `false` by default and it has never been qualified. This leg records the knob's state and the advisory's recommendation; the decision (qualify it, or keep it off and rely on `backing_files=false` + `image_type` pinning) is the maintainer's, in the pin-move PR or a follow-up leg.
3. **§9 CVE disclosure removal** (DEPLOYMENT-ARCHITECTURE §9 / D6 row): the pinned-VMM CVE exposure paragraph is removed only when the new qualified tuple closes the exposure — this leg supplies the evidence that it does; the edit itself belongs to the pin-move PR, which also adds the D6 resolution line.
4. **image_type autodetection deprecation (v52.0 #8219)**: the agent already pins `image_type` per disk (extension-based). No CHV change is required for the CVE to close (proven in §4.1/§4.2); whether to also handle `.qcow2`-extension volumes explicitly (the only path where the agent sends `Qcow2`) is a pin-move-PR review note — no qcow2 volumes exist in the qualified storage profiles (local-file raw + LVM).
5. **M4.2 boot legs** remain environment-blocked by the host co-tenancy (leg 05 §4.2) — unrelated to this leg; nothing here requires them.

## 7. Host-cleanliness proof and repo isolation

- **Qualified pin:** `/usr/bin/cloud-hypervisor` sha256 `a250a934…` (v43.0.0) — verified before and after **every** run (per-run log lines in each run's `run.log`; runner checks on both stack arms; final sweep 23:15:47 UTC). Never written; bind-mounted over only inside the v53 namespace arm, which is gone.
- **Candidate staging** still matches the campaign digests (`448af3d4…`, `13f32ba9…`) at the final sweep.
- **Frozen records:** full sha256 manifest of `/var/lib/chv/qual` (29,500 files) taken pre-leg and post-leg — **byte-identical**. No m4.x scenario was run; nothing under `/var/lib/chv/qual` was opened for write. Read-only assets (image, firmware) digest-verified, never written.
- **Final sweep:** zero cloud-hypervisor / chv-* / dnsmasq processes (the two `pgrep` hits in the sweep log are the sweep shell's own argv self-match — known artifact of the residue pattern); host links exactly baseline (`lo eth0 ens19`); no nft tables; no listeners on :8080/:8443/:9100/:8444/:51052/:51053; no `/tmp/chv-qual-*` dirs (both stack arms' test dirs removed by their clean teardowns; the two earlier failed-attempt dirs from runner-invocation defects were removed after verification); `/run/chv/{agent,core,stord}` = the pre-existing empty M4.2-era dirs, `nwd` removed by teardown; no process holds the candidate bind mount; zero leftover sockets in the workdir.
- **Co-tenant:** 54 kube-apiserver processes at start and end — untouched; no sysctls changed; no nspawn used.
- **Repo:** `git status --porcelain` = 0 at leg start and end; no commits, branches, PRs, or file writes under the repo by this leg.
- Two sub-runs stopped early on **runner-invocation defects of the leg's own scripts** (missing `+x`; a wrapper argv that self-matched deploy.sh's CH-residue pattern) — captured, classified as not-a-v53-regression / not-a-product-defect, cleaned up, and re-run cleanly; both failed attempts' preserved test dirs were verified empty of processes and removed.

## 8. Judgment

**Do the CVEs close at v53.0?** Yes. CVE-2026-27211 closes **with runtime proof on this host**: the complete attack chain (guest-writable raw disk, header overwrite, reboot re-scan, undeclared host file opened O_RDONLY and served as guest storage) reproduces on the qualified v43.0.0 under the exact CHV invocation shape — including the case where the agent's `image_type: Raw` pinning is applied, which this leg newly shows does **not** protect v43 — while v53.0 refuses every entry into the backing chain, fail-closed, with the mitigation's default (`backing_files: false`) visible in the live stack config. CVE-2026-45782 closes at **records tier** (v53.0 contains the fix; the CHV stack runs the affected-by-default async configuration on both arms, so the version move is the closure), with the practical-repro limitation honestly labeled: it needs a malicious guest virtio driver this campaign does not have.

**Are there new security regressions?** None found. The no-new-regressions sweep (process/thread hardening, socket and fd attack-surface inventory, CHV-side sandbox posture, serial/seccomp signatures — at both raw-CH and full-stack tiers, same-session A/B) shows an unchanged security shape, one confirmed **improvement** (disk-image locking enforced at v53, absent at v43), and one recorded availability trade-off (fail-closed refusal on guest-corrupted disk headers, §4.6). The leg-02 defects remain upstream-unfixed at v53 and carried by the existing CHV containment; the in-thread seccomp silent-exit edge is carried as a v53-specific widening of the thread-death defect. No new advisory affects v53.0.

**What blocks the pin move?** Nothing from this leg. The remaining pin-move-PR items are the recorded deferrals of §6 (install.sh checksum + pin move, Landlock decision, §9 disclosure edit) — all already on record from legs 04/05; the security motivation for the move is now positively evidenced rather than advisory-only.

## 9. Not landed by this leg (separate work, not silently decided)

- The pin-move decision itself and the pin-move PR (campaign close), including every §6 deferral.
- The Landlock posture decision (maintainer's, per §6.2).
- No CHV-side adaptation work and no upstream reports — the leg-02 mitigation requirements remain recorded pin-move conditions.

## Artifacts (campaign workdir, ephemeral)

`/tmp/opencode/d6b/sec/`: this report's primary record (`security-regression-report.md`); `runs/` (six raw-CH runs with full logs, configs, the strace capture, per-run pin checks); `stack/v53/`, `stack/v43/` (stack-arm sweep logs incl. deploy output); `artifacts/` (canary/overlay/raw disks); `logs/` (fix-commit containment, frozen-record manifests pre/post, final sweep). Load-bearing excerpts embedded in this document.
