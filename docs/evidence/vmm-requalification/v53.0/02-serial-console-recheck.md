# D6-(b) Leg 02 — Serial-console defect re-check on Cloud Hypervisor v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [Campaign index](README.md) · [Leg 01 (anchor)](01-anchor-leg.md)
> Date: 2026-10-03 (evening UTC)
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §7); the evidence PR is the leg's only repo deliverable
> Method: m2.5 e-series isolation experiments re-run against the v53.0 candidate, driving CH **directly** (no CHV agent, no kvm-smoke, no CHV units), scoped to v53's refactored `GuestExit`/serial code paths
> Frozen references: m2.5 evidence §5 runs 6–10b, §6 GUEST-PLATFORM BLOCKER; m4.9 §3 (#345, #409/#410 gates)

## Verdict

**Mixed — this leg is a material input to the pin-move decision, not a pass/fail gate it clears.**

| # | Check (m2.5 e-series) | v43.0 (pin) | v53.0 (candidate) | Verdict |
|---|---|---|---|---|
| 1 | pty output gate (passive reader gets output) | closed (0 B for 300 s until input) | first byte +0.904 s, banner +3.587 s | **FIXED** (upstream #7502) |
| 2 | socket fd leak across `vm.reboot` | +1 fd/reboot, no EOF on old conn, blocking-write vCPU stall | fd flat (130) across 10 reboots, EOF 10/10, reboots 19–43 ms | **FIXED** (upstream #7502) |
| 3 | pre-connect buffering (late client gets boot log) | n/a (feature absent — output dropped) | lossless but stalls: **278 B hard stop on a quiet guest**, ~1 KB/s crawl otherwise | **BROKEN** (upstream #8322 defect; mechanism proven, §4.3) |
| 4 | silent serial-manager thread death | m2.5 defect | **dead at t+0.0 on RST with queued unread data, zero log lines, probe never served, guest healthy** | **STILL PRESENT** |
| 5 | #345-class `GuestExit`/control-loop wedge | **reproduced twice** in this leg's controls (SIGKILL required, sockets left) | 5 exit paths exercised, all rc=0, API responsive, sockets cleaned; 14/14 clean teardowns | **NOT REPRODUCED** |
| 6 | thread name / per-thread seccomp | `serial-manager`, filters 1 | name unchanged, filters 2, applied in-thread, functioning | **UNCHANGED / FUNCTIONING** (new silent-exit edge, §4.4) |

## 1. Candidate provenance

| Asset | Source | sha256 (observed) | Expected | `--version` |
|---|---|---|---|---|
| `cloud-hypervisor-static` | `…/releases/download/v53.0/cloud-hypervisor-static` | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` | `448af3d4…` ([campaign index](README.md)) — **MATCH** | `cloud-hypervisor v53.0` |
| `ch-remote-static` | `…/releases/download/v53.0/ch-remote-static` | `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7` | `13f32ba9…` ([campaign index](README.md)) — **MATCH** | `ch-remote v53.0` |

Both executed on kernel `6.8.0-142-generic` (amd64) throughout. Staged in the
campaign workdir and invoked **by path**; `/usr/bin/cloud-hypervisor` (the
qualified v43.0.0 pin, sha256 `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`)
was never written, mounted over, or replaced — it was executed read-only for
the v43 A/B control arms, exactly as the m2.5 e-series did (digest verified
before and after all runs, §7).

v53.0 sources (tag tarball) and v43.0 sources were downloaded for the
code-path scoping; the v43↔v53 diff of `vmm/src/serial_manager.rs` grounds
the source-level verdicts (§3).

## 2. Experiment environment (mirrors m2.5 e-series)

| Component | Version / shape |
|---|---|
| Host | qualification host: 16 vCPU AMD EPYC 9554P, 31 GiB RAM, `/dev/kvm`, Ubuntu 6.8.0-142-generic |
| VMM candidate | cloud-hypervisor **v53.0** (static release, digests above) |
| VMM control | cloud-hypervisor **v43.0.0** (qualified pin, executed by path) |
| Guest | the qualification patched noble image `/var/lib/chv/qual/images/noble-qual-patched.img` (sha256 `37f7c34075044e8c78f3c9bd8987f4f063bddd91a67b6408ef8daae53dabd22a`, grub-patched `root=/dev/vda1`, no initrd — the same image every m2.5/m4.x campaign leg used), converted per-experiment to a private raw copy (the shared qcow2 is never written) |
| Firmware | rust-hypervisor-fw 0.5.0 at `/var/lib/chv/hypervisor-fw` (read-only) — the m2.5 e-series boot shape |
| VM shape | 2 vCPU, 1 GiB, single disk, **no NIC** (m2.5 e2–e5 single-disk shape; also the strictest host-isolation posture — no bridges, no taps, no user-net sockets) |
| Serial | per-experiment `--serial pty` / `--serial socket=<workdir path>`; unique api-socket and serial-socket paths per experiment |

**Scripts** (reconstructed cleanly from the m2.5 frozen experiment
descriptions; the original ad-hoc scripts no longer exist on this host):
`e_common.py` (harness), `e1_pty_gate.py`, `e2_fd_leak.py`,
`e3_preconnect_buffer.py`, `e4_thread_death.py`, `e5_guest_exit.py`,
`run_all.sh`, plus this leg's follow-up discriminators `e3b_staggered.py`,
`e3c_discriminate.py`, `e3d_thread_vs_sink.py`, `e3e_seccomp_off.py`,
`e3f_flush_probe.py` and the corrected v43 control `e2b_fd_leak_v43.py`.
Each experiment: setup → run → observe → teardown with residue assertion; all
timelines logged to per-experiment results JSON + CH logs.

## 3. Source scoping (v43.0 → v53.0, `vmm/src/serial_manager.rs` + friends)

The diff confirms exactly the changes the campaign predicted, and nothing more:

1. **Pty initial-flush gate (fixed, #7502):** v43's timeout-flush branch was
   guarded `if matches!(in_file, ConsoleOutput::Socket(_)) && num_events == 0`
   — dead code for ptys (the m2.5 root cause). v53 guards it
   `matches!(transport, ConsoleTransport::Pty(_))` — the inverted guard, so a
   passive pty reader gets the 500 ms-timeout flush.
2. **Socket fd leak (fixed, #7502):** v43's accept path cloned the accepted fd
   for epoll registration and never closed it. v53 keeps the reader as an
   owned `UnixStream`, shuts down any previous reader on a new connection,
   and drops the reader when the thread exits — the old client sees EOF.
3. **Pre-connect buffering (new, #8322):** `SocketConsole`/`SharedSerialBuffer`
   — a persistent `SerialBuffer` ring (1 MiB cap) installed as the serial
   device's output sink while detached; `attach_client()` retargets it and
   flushes (replays) on connect.
4. **Non-blocking client sockets (new):** the accepted client is set
   `O_NONBLOCK`; `SerialBuffer::write` re-buffers on `WouldBlock` — this
   closes the *slow-client* half of the v43 stall family.
5. **Silent thread death (STILL PRESENT):** v53 `serial_manager.rs` File
   branch, non-`WouldBlock` read error (ECONNRESET after a client RST):
   `Err(e) => return Err(Error::ReadInput(e))` — the closure's `Err` return
   exits the epoll loop and is **silently discarded** by
   `catch_unwind(…).map_err(|_| error!("serial-manager thread panicked")…)` —
   which fires only on *panics*. No log, no `set_out(None)`/`detach_client()`,
   no exit event. Byte-for-byte the v43 defect.
6. **EPIPE→`thr_empty()` skip (STILL PRESENT):** `devices/src/legacy/serial.rs`
   `handle_write` still does `out.write_all(&[v])?` **before**
   `self.thr_empty()?`, and `SerialBuffer::write` propagates non-`WouldBlock`
   write errors — so after a silent thread death the guest's
   interrupt-driven tty TX path can still starve (the v43 three-defect
   chain's legs 2 and 3).
7. **Per-thread seccomp (new in v53):** the serial-manager thread applies its
   filter *inside* the thread; a filter-application failure returns `Err` — a
   **new early-exit path with the same silent discard**. The thread name is
   unchanged: `.name("serial-manager")`.
8. **`GuestExit` refactor:** v43 funneled guest poweroff through
   `EpollDispatch::Exit`; v53 splits it into a dedicated
   `EpollDispatch::GuestExit`: guest exit event → (default,
   `no_shutdown=false`) `vmm_shutdown()` + loop break. The external
   `vmm.shutdown` API keeps the `Exit` dispatch. The control loop still both
   serves the API and drives `vmm_shutdown`, so the #345 wedge *shape* (API
   dead while process lives) remains structurally possible if the loop wedges.

## 4. Experiments — method, observations, verdicts

All experiments drive CH directly with the §2 shape. Every run ends with an
escalating teardown (shutdown-vmm → SIGTERM → SIGKILL) and a residue
assertion. Full timelines are preserved in the campaign workdir
(`logs/<exp>-results.json` + CH logs; raw console captures per experiment).

### 4.1 E1 — pty output gate (check #1)

**Method.** `--serial pty`; passive reader opens the slave (`O_RDWR`, never
writes) immediately at spawn; observe whether output flows without the client
ever writing (v43's gate only opened on input).

| Arm | First byte after slave open | Kernel banner | Verdict |
|---|---|---|---|
| **v53.0** | **+0.904 s** | +3.587 s (11,064 B total) | **gate open** |
| v43.0 control | none in 300 s | only after a newline was written (+300.2 s) | gate closed (m2.5 repro) |

**Check #1: FIXED at v53.** The v43 control reproduces the m2.5 defect exactly.

### 4.2 E2 — socket-backend fd leak across `vm.reboot` (check #2)

**Method.** `--serial socket=…`; one client connected from before boot; 10
`vm.reboot` cycles; after each: `/proc/<pid>/fd` count, EOF-on-old-connection,
kernel-banner-verified boot.

**v53.0:** fd count **flat at 130 across all 10 reboots** (fd_growth 0); EOF
observed on the pre-reboot connection **10/10**; 11 banner-verified boots;
every reboot returned in 19–43 ms; serial-manager thread alive throughout;
clean exit at teardown.

**v43.0 controls.** The first control attempt wedged before any reboot
completed: `ch-remote reboot` blocked 30 s because the v43 serial-manager's
*blocking* socket writes stalled the vCPU while the client stopped draining —
then shutdown-vmm timed out, SIGTERM was ineffective, SIGKILL was required
(an incidental #345-class wedge, and a repro of the v43 slow-client stall
family from #410). The corrected re-run (dedicated draining reader thread,
immediate reconnect after each reboot) completed all 10 cycles: **fd count
121 → 131 (exactly +1 fd leaked per reboot, 10/10; fd_growth +10)**; **EOF on
the pre-reboot connection: never (0/10)** — the m2.5 signature; 11
banner-verified boots; clean teardown.

**Check #2: FIXED at v53.** The v43 control reproduces both the leak and the
blocking-write stall family.

### 4.3 E3 — pre-connect output buffering (check #3, the #8322 feature)

This check consumed most of the leg: the initial repro was followed by six
discriminator experiments (E3b–E3f) plus a host-level socket
characterization, because the failure mode is subtle and unlike any single
m2.5 defect.

#### 4.3.1 Repro (E3, E3b)

| Run | Guest output window | Client | Delivered |
|---|---|---|---|
| E3 | boot + 240 s settle, **fully quiet** guest | 20 ms-poll reader, 60 s | **278 B**, no banner, no live output |
| E3b-B | boot + 90 s settle (trickling) | 20 ms-poll reader, 10 s | **7,265 B** byte-exact prefix, cut mid-GRUB; cascade client after press: 0 B |
| E3d late1 | same | 20 ms-poll, 10 s | **8,119 B**; 0 live bytes while attached; late2 (after detach): **9,288 B** |
| E3e late1/2 | same, `--seccomp=false` | 20 ms-poll, 12 s + 8 s | **10,314 B** then **6,265 B** — a byte-exact *continuation*; press adds 4,958 B ending mid-kernel-log |

The healthy stream for this image (file sink, §4.3.2) is ≈74 KB by +90 s:
firmware INFO (~1.2 KB) → GRUB/EFI blob (~9.5 KB) → kernel banner at byte
~11,051 → dmesg replay (~43 KB) → systemd to ~62 KB by +12 s → 117–200 B/s
trickle.

**Key discriminator results:**

- **E3c-F (file sink, `--serial file=`)** — fully healthy: 74,228 B by +90 s,
  banner+systemd+login present, press honored 1.2 s. The guest and the serial
  device produce the complete stream; nothing is lost before the sink.
- **E3c-P (pty, slave opened at +90 s)** — **62,464 B backlog replayed** (first
  byte +0.902 s), then live cascade. The buffering itself works and the pty
  flush path drains the whole backlog.
- **E3d (thread vs sink)** — serial-manager thread alive through the whole
  run; late1 accepted; late2 re-attach re-serves. Not a thread death, not a
  dead listener.
- **E3e (seccomp off)** — identical defect. Not seccomp.
- **Byte-offset proof (e3e):** late1 + late2 + press bytes end at reference
  offset **exactly 21,537 = 10,314 + 6,265 + 4,958** — a perfect in-order
  continuation. The buffer holds the **full** guest stream (the guest is
  fully booted — it honors the ACPI press in 1.2 s while the delivered stream
  is still at `[0.284 s]` kernel-log content, which a booted guest cannot be
  emitting live). So: **no data loss, no guest stall — the backlog exists;
  delivery of it stalls.**

#### 4.3.2 Mechanism (E3f strace + host-level characterization)

E3f (`--seccomp=false` for ptrace friendliness; defect proven identical)
attached `strace` to the serial-manager TID before a late (+90 s) attach,
with a tight `select()`-based reading client and `ss -xp` socket sampling.

**strace, syscall by syscall:**

```
accept4(25, ...) = 128                       # client accepted
fcntl(128, F_DUPFD_CLOEXEC) = 129            # writer clone
epoll_ctl(…, EPOLL_CTL_ADD, 128, {EPOLLIN})  # input only — never EPOLLOUT
write(129, "[", 1) = 1                       # flush begins: ONE byte per
write(129, "I", 1) = 1                       #   write() syscall
…  332 one-byte writes in ~21 ms …
write(129, "N", 1) = -1 EAGAIN               # flush loop breaks HERE
( zero further syscalls on this thread for 68 s — blocked in the epoll wait;
  all subsequent backlog bytes were written by the vCPU thread )
```

**Host-level characterization** (workdir test, `accept()`-shaped socket pair,
non-blocking, 1-byte sends): with `SO_SNDBUF = wmem_default = 212,992`, a
1-byte-per-send writer gets **EAGAIN after exactly 278 bytes** — AF_UNIX
sender accounting charges each 1-byte skb ~766 B of truesize
(212,992 / 278 ≈ 766). Large writes fit 233,152 B.

**The complete defect chain:**

1. `SocketConsole::attach_client()` flushes the backlog through
   `SerialBuffer::flush()`, which pops **one byte per `write_all()` syscall**
   to the **O_NONBLOCK** client socket.
2. The socket accepts only ~278–330 one-byte skbs before **EAGAIN** (truesize
   accounting), regardless of the nominal 212 KB `SO_SNDBUF`.
3. The flush loop **breaks on the first error — silently**: the EAGAIN is
   never surfaced anywhere; `flush()` still returns `Ok`.
4. **No retry path exists**: the serial-manager epoll registers the client
   socket for `EPOLLIN` only — there is no `EPOLLOUT` (writable) watch — and
   with a quiet guest the thread simply sleeps in `epoll_wait` (strace: 68 s
   of silence).
5. Backlog delivery resumes **only when the vCPU thread writes new guest
   output**: the device path retries the backlog on every guest byte.
   Delivery rate is therefore governed by guest output rate and client drain
   speed:
   - **quiet guest (idle VM):** one flush session, **hard stop at ~278 B** —
     E3's replay of exactly 278 B is the fingerprint;
   - **trickling guest + 20 ms-poll reader:** ~0.6–1.3 KB/s crawl;
   - **tight continuously-draining reader:** the first vCPU-triggered session
     pushes the whole backlog — E3f delivered **75,281 B in 0.4 s**, full
     86,379 B (backlog + live trickle, ending at the `ubuntu login:` prompt)
     over the 68 s window.
6. Live output after a late attach is stuck **behind** the stalled backlog
   (in-order, stale-first): during the e3e press the client received
   3-minute-old kernel-log lines while the actual shutdown cascade was
   produced; only after the backlog drains does live output flow.

Upstream test gap (why #8322's CI passed): the verification used a
kernel-panic guest (constant `printk` = constant flush triggers) and a
continuously-draining reader — the combination that masks the stall. The
detached-through-real-boot + quiet-guest scenario was never covered; v53.0 is
the latest release, no fix exists yet.

**Check #3: BROKEN at v53 for its purpose.** The #8322 buffer is lossless and
in-order, but backlog delivery stalls after ~one socket-fill (~280–330 B) and
only creeps forward on new guest output; for the canonical CHV scenario —
reconnecting to an **idle** booted VM — the client receives ~278 B of stale
firmware output and no live console. It works only with a continuously
draining fast reader while the guest is producing output.

### 4.4 E4 — silent serial-manager thread death (check #4)

**Method.** Client connected from spawn, reads through the kernel banner,
then stops reading for 1 s (output queues unread), then abortive close
(`SO_LINGER 0` → RST with unread data in flight). Sample thread liveness,
vm.info, CH log; probe client 15 s; power-button press at +60 s.

| Arm | serial-manager after RST | CH log | Probe client | Press |
|---|---|---|---|---|
| **v53.0** | **dead at t+0.0 s** (every sample) | **no line at all** about the RST | connect OK (kernel backlog) but **0 B in 15 s, never served, no EOF** | honored — ACPI cascade, CH exit rc=0 in 1.4 s (guest healthy) |
| v43.0 control | alive (this run's RST landed on the graceful-EOF branch: `Remote end closed serial socket` logged) | EOF line logged | served (2,296 B) | cascade +1.4 s, **but CH never exited: SIGTERM ineffective, SIGKILL required, sockets left — #345-class wedge** |

v53 source confirms the m2.5 defect byte-for-byte (§3.5). The thread never
restarts: serial console service is dead for the lifetime of that CH process
while the guest stays healthy — the worst combination for a headless host.

Two v53 refinements vs the m2.5 picture:

- The **graceful-EOF path is clean** at v53: a cleanly closed client gets the
  EOF log line, the thread **stays alive**, and a fresh client is served.
- **New early-exit path, same silent discard:** v53 applies the per-thread
  seccomp filter *inside* the thread — a filter-application failure would
  exit the thread through the same panic-only `catch_unwind` discard. Not
  triggered in any run, but it widens the defect's surface.

**Check #4: STILL PRESENT at v53** — deterministic silent thread death on RST
with queued unread data (the m2.5 e4 signature, unchanged).

### 4.5 E5 — `GuestExit` / control-loop wedge (check #5, #345 class)

**Method.** Three arms at v53: (a) first-boot `power-button`; (b) `vm.reboot`
then `power-button`; (c) SIGTERM to a booted VMM. A console client watches
the shutdown cascade; API-socket liveness and process exit are timed; socket
cleanup verified.

| Arm | Press→cascade | Press→CH exit | Exit code | Sockets cleaned |
|---|---|---|---|---|
| E5-a first boot | +0.00 s | +26.31 s (API dead only at exit) | 0 | yes |
| E5-b post-reboot | +0.00 s | +33.01 s | 0 | yes |
| E5-c SIGTERM | — | 0.11 s | 0 | yes |

The 26–33 s press→exit is the *guest's own* systemd shutdown; the VMM control
loop and API stayed responsive throughout. Across the whole leg, every v53
exit path (press, SIGTERM, shutdown-vmm, post-RST press) terminated rc=0 with
no residue — **14/14 v53 runs tore down cleanly**. The v43 controls wedged
twice (E4 control, first E2 attempt — both #345-class, SIGKILL required).

**Check #5: NOT REPRODUCED at v53** — the `GuestExit` refactor holds on every
exercised exit path, with clean A/B contrast against v43.

**Caveat, clearly labeled:** one socket-mode run (a client detached mid-boot
at +8.9 s) had the guest stall at `multipathd.service` from ~+9 s — no
further console output, probe ignored, press ignored for 90 s — while
`shutdown-vmm` still worked in 31 ms (control loop healthy). This is a
**guest-side** stall, not a VMM control-loop wedge; the serial layer is
exonerated (the graceful EOF does not kill the serial-manager; 7 of the leg's
8 socket-mode boots had healthy, ACPI-responsive guests — the sole exception
is this run). Recorded as an unresolved one-off guest anomaly, not counted
against check #5.

### 4.6 E6 — serial-manager thread name and per-thread seccomp (check #6)

Folded into the other runs (thread enumeration in E2/E4/E5, liveness
sampling, strace):

- **Thread name unchanged:** `serial-manager` (`/proc/<pid>/task/*/comm`) in
  both v43 and v53 — observability/annotation assumptions hold.
- **Per-thread seccomp applied and functioning:** serial-manager shows
  `Seccomp: 2, Seccomp_filters: 2` (v43: filters 1); no syscall-kill events in
  any default-seccomp run; `--seccomp=false` changes nothing observable about
  the serial paths; the filter set covers the new buffered-output syscalls.
- New (theoretical) exposure: a filter-apply failure is a silent early exit
  (§4.4).

**Check #6: name unchanged; per-thread seccomp applied and functioning.**

## 5. Implications for the pin-move decision (#448) and CHV machinery

**Net assessment.** Of the m2.5 GUEST-PLATFORM BLOCKER's serial legs: the two
#7502 fixes (pty gate, fd leak) are real and verified with clean v43
controls; the #345-class exit wedge did not reproduce at v53 (and *did*
reproduce in v43 controls on this host). But check #4's silent thread death
is unchanged and **deterministic**, and the new #8322 buffering feature — the
one upgrade this pin-move was partly counting on for console robustness — is
defective precisely in CHV's reconnect scenario.

- **#284 (console rotation / reattach):** an agent reconnecting to an
  **idle** VM gets ~278 B of stale firmware output and then a dead console.
  Rotation logic that waits for a marker (login prompt, health banner) in
  replayed output will time out. Mitigation if the pin moves anyway: the
  reattach client **must drain continuously and promptly** — a fast reader
  recovers the full backlog in ~0.5 s while the guest is producing output —
  and marker waits need crawl-aware timeouts (~1 KB/s with a poll-style
  reader).
- **#292 (drain-then-close):** vindicated at v53 and now *load-bearing*:
  graceful close is handled cleanly (thread survives, re-accumulation works),
  while an abortive close (RST) with unread queued data **permanently kills
  serial service** for that CH process, silently. Any CHV code path that
  drops a console connection without draining bricks the console until the
  VMM is restarted.
- **#410 (heal):** detection of the dead-serial state cannot key on CH logs
  (there are none). Behavioral signature (E4): a fresh client connects
  successfully (kernel accept backlog) but receives 0 bytes and no EOF within
  ~15 s while `vm.info` reports the guest running. A `/proc/<pid>/task`
  comm-scan for `serial-manager` is a cheap, reliable detector and the
  natural trigger for a VMM-restart heal.
- **Upstream:** the E3 mechanism (byte-at-a-time non-blocking flush + silent
  EAGAIN break + no EPOLLOUT retry vs AF_UNIX skb-truesize accounting) and
  the E4 silent-`Err`-discard are both reportable upstream; v53.0 is the
  latest release, so no fix can be picked up today — any fix would require a
  re-qualification leg.
- **Decision frame:** v53 fixes 2.5 of the 3 blocker legs but keeps the worst
  failure mode (silent, permanent serial death on client crash) and adds a
  fragile buffering feature. If the pin moves, the #284/#292 machinery needs
  the reader/timeout adaptations above and #410 needs the liveness probe; if
  it does not, the v43 path keeps the exit wedge (#345 class, reproduced
  twice in controls here) and the known v43 serial set.

## 6. Not landed by this leg (separate work, not silently decided)

- Any CHV-side adaptation (#284 reader/timeout changes, #410 behavioral
  detector) — implementation decisions for the maintainer, informed by this
  evidence.
- Upstream defect reports for the E3/E4 mechanisms — prepared from this
  evidence when the maintainer chooses.
- The pin-move decision itself — #448's remaining legs and the final
  maintainer decision.

## 7. Host cleanliness and repo isolation proof

- **Qualified pin untouched:** `/usr/bin/cloud-hypervisor` sha256
  `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`
  (v43.0.0) — verified **before the leg** and **re-verified after all
  runs**; only ever executed read-only by path.
- **Candidate unchanged:** staged binaries still match the campaign digests
  (`448af3d4…`, `13f32ba9…`).
- **Read-only inputs unchanged:** qualification image and firmware digests
  verified; the shared qcow2 was never written (per-experiment raw copies
  only).
- **No host resources shared or left behind:** no bridges/taps/NICs (all VMs
  single-disk, no network); every experiment's teardown asserted
  `residue=none` (14/14 v53 runs, including the corrected v43 control; the
  two v43 *wedge* runs required SIGKILL and left their inert socket files
  behind, as documented in §4; those were removed post-run). **Final sweep
  after all runs: zero cloud-hypervisor processes, zero socket files under
  the workdir.**
- **Repo untouched during execution:** no `git` operations, no file writes
  under the repo — the only deliverable is the report this evidence document
  freezes, plus the campaign-workdir artifacts.
