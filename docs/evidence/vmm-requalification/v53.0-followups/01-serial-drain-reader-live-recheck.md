# v53.0 follow-up leg 01 — Live recheck of #471's drain-continuously serial reattach reader (post-release)

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [v53.0 campaign index](../v53.0/README.md) · [Leg 02 — serial-console defect re-check](../v53.0/02-serial-console-recheck.md)
> Date: 2026-10-04 (UTC)
> Closes: [release declaration §12 decision point 1](../../production-readiness/v0.3.0/00-release-declaration.md) — "#471 (drain-continuously serial reader) is verified at mock/stand-in tier only … A live leg-02-style recheck with the deployed agent on v53.0 is the natural follow-up before/with rollout"
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §6); the evidence PR is the leg's only repo deliverable
> Method: leg-02's E3 shape (late attach after the guest settles — *intended* fully-quiet; as observed the guest instead parked at the bootloader for the whole down window and the OS boot ran only after the watchdog's reboot dispatch, §4.2) re-run **through the deployed CHV stack** (core-managed authority, real mTLS, `deploy.sh` shape), driving the agent's own console surfaces (console.log writer + WS console server) — the exact surface the #471 reader serves, which leg 02 (raw CH, no agent) could not cover

## Verdict

**Mixed. The #471 reader itself behaves as designed wherever it has something to read — but the deployed-stack late-attach after an agent death with serial output in flight is gated *upstream* of the reader by CH v53.0's silent serial-manager thread death (leg-02 check #4). Recovery happened, completely and byte-exactly, via the boot-watchdog layer firing exactly as designed.**

| # | Check | This leg (deployed agent, v53.0) | Verdict |
|---|---|---|---|
| A | late-attach backlog recovery through the agent console path | **0 B for 120.0 s after adoption** (serial manager dead — E4-class, §4.4); boot watchdog fired at exactly `stall_secs=120` and dispatched its remediation; **+1,075 B within ~0.3 s** of the dispatch, then a slow crawl (17,323 B received by T0+192.6, ~84 B/s average — the in-between profile is unobserved, §7). Only **~11 KB was pre-buffered backlog** (bootloader-park output; the ring never held a kernel banner before the jolt): the guest had been **parked at the bootloader for the entire 240 s down window**, and the OS boot began ~2.5 s *after* the reboot RPC (kernel `RTC time: 17:56:37`). The remaining ~64 KB of the 74,972 B stream was live OS-boot output delivered byte-at-a-time (~6.3 KB/s over the ws2 window), **no hard stop, no truncation**; complete receipt between T0+204.7 and T0+207.7 (guest emission finished 17:57:18 = T0+164, cloud-init `finished`, Up 41.34 s) | **RECOVERED — via the watchdog layer, not the reader alone** |
| A′ | WS console client served the recovered stream | ws2: 72,892 B; ws3: 74,972 B = ws2 + 2,080 B tail, byte-prefix-consistent over the full length; complete marker set in the stream; the stream is exactly one OS boot, begun ~2.5 s after the reboot dispatch (§4.2) | **PASS** |
| B | reconnect scrollback recovery | byte-exact scrollback replay (74,972 B in 3 chunks, ~1 ms); scrollback is per-agent-process memory and correctly starts empty after an agent restart | **PASS** |
| C | boot-marker audit (observational) | watchdog fired at **exactly T0+120.00 s** — correct on both counts: the console path was dead (§4.4) *and* the marker genuinely absent (the guest was still parked at the bootloader; the OS boot had not begun); dispatched the CH-api reboot; no second firmware or kernel banner in the stream (exactly one of each; CH pid constant) — the OS boot began ~2.5 s later, causality not established (§4.4); the marker arrived with the recovered stream and the fire count stopped | **FIRED AS DESIGNED — and was the necessary recovery trigger** |

Two new findings came out of this leg, both beyond #471's own claims (§4.4, §4.5): the E4-class serial-manager thread death is reachable through a **plain agent SIGKILL with output in flight** (no `SO_LINGER` tricks needed), and the **console.log writer's silent `Lagged`-skip path is reachable** under v53.0's one-byte-at-a-time flush — this run could not establish whether it actually fired (§4.5).

## 1. What this leg tests and why

[#471](https://github.com/kubedoio/chv/pull/471) replaced the serial reattach
reader's poll-style loop with a drain-continuously per-connection reader
(`drain_console_endpoint`: blocking `read()` on `spawn_blocking`, no awaits
between reads) plus crawl-aware boot-marker waits. It is the **only Rust-code
delta** between the v53.0-qualified stack (`b6d6ad50`) and the v0.3.0 release
HEAD (`a36d9588`), and at release time it carried mock/stand-in-tier
verification only (declaration §5.1/§12 decision point 1).

Leg 02 established, at raw-CH tier, that v53.0's pre-connect buffering
(upstream #8322) is lossless but delivery-stalled: a late client on a
**fully quiet** guest receives exactly **278 B** (one flush session, hard
stop) unless it keeps the socket drained; a trickling guest crawls at
~1 KB/s on a poll-style reader; and a tight reader that keeps the socket
empty gets the whole backlog in one pass when guest output re-triggers a
flush session (E3f: ~72.7 KB in 0.4 s). Separately, leg-02 check #4 (E4)
proved v53.0's serial-manager thread **dies silently** on an abortive client
close with queued unread data (`SO_LINGER 0` → RST): a fresh client then
connects (kernel backlog) but is **never served** — 0 B, no EOF, no log line.

This leg re-runs the E3 *shape* with the deployed agent in the loop: the guest
boots with the agent's serial connection gone (agent SIGKILLed early in
boot), then the agent restarts and adopts the running
VM — the late-attach path — and the recovered stream is measured at both
console surfaces the agent owns. The fully-quiet premise **did not hold as
run**: the guest never completed its boot during the down window — it sat
parked at the bootloader (§4.2), and the OS boot ran only after the
watchdog's reboot dispatch. The consequences for each E3 comparison are
carried through §4.3 and §8.

## 2. Provenance and environment

| Asset | Source | sha256 (observed) | Expected | Note |
|---|---|---|---|---|
| CHV stack (CP/agent/stord/nwd/chvctl) | release build from a clean worktree at `a36d9588` (= v0.3.0 release HEAD = #471 merge) | `a36d9588…` (candidate SHA recorded by `deploy.sh`) | `a36d9588…` | agent logs `version 0.3.0, commit a36d9588` |
| `cloud-hypervisor` (candidate under test) | v53.0 static release, staged by path | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` | `448af3d4…` ([campaign index](../v53.0/README.md)) — **MATCH** | never installed to `/usr/bin` |
| `/usr/bin/cloud-hypervisor` (system pin) | qualified v43.0.0 pin | `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496` — identical before and after **all three attempts** | `a250a934…` — **MATCH, unchanged** | **never executed, never written** (§3) |
| Guest image | `/var/lib/chv/qual/images/noble-qual-patched.img` | `37f7c34075044e8c78f3c9bd8987f4f063bddd91a67b6408ef8daae53dabd22a` — identical before and after | `37f7c340…` — **MATCH, unchanged** | shared qcow2 never written; the VM's disk is a private copy |

| Component | Version / shape |
|---|---|
| Host | qualification host: 16 vCPU AMD EPYC 9554P, 31 GiB RAM, `/dev/kvm`, Ubuntu 6.8.0-142-generic; no chv units/processes before or after (§6) |
| Deploy | committed `scripts/integration/qual/deploy.sh` (core-managed authority, real mTLS, 0700 runtime dir) |
| VM shape | 2 vCPU, 1 GiB, private disk copy of the noble qualification image, network `default` (10.200.0.0/24) — deployed-stack shape, unlike leg 02's no-NIC raw-CH shape |
| Client surface | the agent's WS console server (`chv-agent-core/src/console_server.rs`, `GET /vms/<vm_id>/console?token=…` on console_bind 127.0.0.1:8444, scrollback-first-then-live); stdlib-RFC6455 instrument client with per-message timeline, byte-exact stream dump, and scheduled input pokes |
| Instruments | `ws_console.py` (WS client + HS256 token minter), `logsize_poller.py` (console.log size timeline, 0.2 s poll), scenario driver `leg471.sh`; all in the leg scratch dir, none in the repo |

## 3. Deploy shape (safety-relevant choice, disclosed) and attempt history

`deploy.sh` hardcodes `chv_binary_path = "/usr/bin/cloud-hypervisor"` in the
generated `agent.toml`. Executing the v43 system pin would have invalidated
the leg's premise, and overwriting or mounting over `/usr/bin/cloud-hypervisor`
was forbidden. The leg therefore used the #458 kvm-smoke private-staging shape
transplanted onto the committed `deploy.sh`: `deploy.sh` brings the stack up
normally with **no VM existing** (the system pin is never executed); the
scenario then stops the deploy-time agent, rewrites the generated
`agent.toml`'s `chv_binary_path` to the digest-verified staged v53.0 candidate
(by path), appends the `[watchdog]` section (Scenario C), and restarts the
agent. Every cloud-hypervisor process in this leg is the v53.0 static binary
executed by path from the leg's staging dir. The `agent.toml` edit is a
scratch config in the throwaway test dir. One host-side note: the agent
auto-generates and reuses `/etc/chv/jwt_secret` (pre-existing on this host,
64 bytes); the leg read it to mint WS console tokens and did not modify it.

### 3.1 Attempt history (harness iterations, not product findings)

Two earlier attempts failed for scenario-harness reasons; both are recorded
because they are **not** product results and must not be read as such:

1. **Attempt 1 — health-gate bug (no VM was ever created).** The scenario's
   post-restart health gate invoked `qual_chvctl_ok`, a helper defined in the
   m4.3/m4.7 scenario scripts but **not** in `lib.sh` (which this leg's script
   sources) — `wait_for` polled a nonexistent command for 60 s and the leg
   aborted before Scenario A. A diagnostic re-deployment proved the agent
   itself restarts cleanly (reconnect within ~0.1 s; `chvctl node list`
   returns RC=0 from t+5 s, and even with the agent fully down — the
   control-plane serves node state from its DB). Deploy teardown was clean.
2. **Attempt 2 — kill-timing bug: the VM never started (invalid run).** The
   scenario SIGKILLed the agent on seeing `ch.pid` + `serial.sock` (2 s
   dwell). But CH spawns in `Created` state with both already present, and
   the agent issues the actual boot RPC only after pushing the VM config
   (agent log: `spawning cloud-hypervisor` → `chv serial socket connected` →
   6.6 s later `booting vm via ch api`) — the kill landed before it. The VM
   sat in `Created` for the entire 240 s window (CH cputime 0 s → 0 s,
   `vm.info` = `Created`, 0 serial bytes ever, no serial-manager thread).
   Every downstream "failure" in that run is explained by the VM never
   booting; the run is **invalid as a test of the reader**. Notably, even in
   this state the stack behaved sanely: CH survived the agent SIGKILL,
   adoption re-attached without error, `vm stop` shut the Created-state VM
   down cleanly, teardown was residue-free.

Attempt 3 (§4–§6) fixes the gate and kills the agent only after the VM is
demonstrably `Running` — CH's own `vm.info` **and** the agent's recorded
state (the m4.3 S1-proven shape).

## 4. Scenario A — late-attach backlog recovery (E3 shape, agent in the loop)

### 4.1 Procedure

1. `chvctl vm create` + `vm start` (2 vCPU / 1 GiB / private disk copy);
2. wait for the VM to be demonstrably running (CH `vm.info` = `Running`, then
   the agent's recorded state converges), then **SIGKILL the agent** —
   13.65 s after `vm start` acceptance, in the firmware/shim bootloader
   phase (the measured 5,684 B pre-kill prefix is firmware output, the shim
   MOK/TPM failure sequence, and the start of the bootloader's NUL-dominated
   filler — §4.2; measured, not assumed);
3. confirm the CH process survives and probe its `serial-manager` thread
   (leg-02's comm scan);
4. **240 s agent-down window** (intended E3 regime: the guest finishes
   booting and settles to fully-quiet while CH's SharedSerialBuffer
   accumulates the stream) — as observed, the guest instead parked at the
   bootloader for the whole window (§4.2);
5. restart the agent → `adopt_running_vms` re-attaches the serial socket and
   respawns the broadcaster + console.log writer (the #471 reader);
6. measure: console.log size timeline (0.2 s poller, 120 s), a first WS
   console client (scrollback + live, 30 s, Enter-poke at +8 s), then — after
   the recovery settles — two further WS clients (Scenario B).

### 4.2 Observed timeline (attempt 3, wall-clock UTC)

| Time | Event |
|---|---|
| 17:50:07.98 | agent restarted (Phase 1: v53.0 by path, watchdog on) |
| 17:50:15.45 | `spawning cloud-hypervisor` (VM `Created`) |
| 17:50:15.50 | agent connects the serial socket |
| 17:50:22.06 | `booting vm via ch api` (the actual VM start) |
| 17:50:29.1 | `vm.info` = `Running` (+13.62 s after `vm start` accepted); chvctl state converged; **agent SIGKILLed** (+13.65 s); console.log prefix = **5,684 B** |
| 17:50:31 / :39 | serial-manager comm scan: **thread NOT alive** (+2 s and +10 s after the SIGKILL) |
| 17:50:29 → 17:54:34 | 240 s down window: CH survives (pid constant), `vm.info` = `Running` throughout, guest cputime advances only 5 s → 6 s — the guest is **parked at the bootloader**, not booting: the pre-kill stream ends in the shim MOK/TPM failure sequence (`Could not create MokListRT …`, `import_mok_state() failed`, `TPM logging failed` — the sequence appears twice, offsets ~1.4–3.4 KB) followed by ~7.7 KB of NUL-dominated menu filler; **no kernel banner ever arrives before the jolt** (the ring grows 5,684 → ~11,110 B of bootloader output); console.log stays at 5,684 B |
| 17:54:34.06 | agent restarted → **adoption**: `adopted running vm after agent restart; console capture resumed` (same CH pid) — **T0** |
| T0 → T0+120 | **zero delivery**: poller 600 samples, size constant at 5,684 B; WS client 1 (T0+15 → T0+45): **0 B**, Enter-poke at T0+23 never echoed (connection accepted by the kernel backlog, never served; note these console-side signals alone cannot discriminate a dead serial manager from a parked guest — §4.4 anchors the thread-death on the comm scans) |
| T0+120.00 | **boot watchdog fired**: `console stalled mid-boot without the boot-complete marker; rebooting to recover (this also re-creates the serial manager and restores console capture)` → `rebooting vm via ch api` → `recovery reboot dispatched` |
| ~T0+120.3 | serial delivery revived: console.log 5,684 → 6,759 B (+1,075) within ~0.3 s of the reboot dispatch; the ring then drains slowly — 17,323 B received in total by T0+192.6 (~84 B/s average; no size samples in between, the poller had ended — §7) |
| T0+122.5 | **OS boot begins** — kernel `PM: RTC time: 17:56:37` at kernel-ts 0.346 s, i.e. **~2.5 s after the reboot RPC** (dispatched T0+120.03); the pre-jolt ring held only ~11 KB of bootloader output |
| 17:56:36.6 → 17:57:18 | guest OS boots to completion (cloud-init `init-local` 17:56:40 / Up 3.37 s → `modules:config` 17:56:45 / Up 8.18 s → `modules:final` + `finished` 17:57:18 / Up 41.34 s); **guest emission complete at T0+164** |
| 17:59:46.7 (T0+192.6) | WS client 2 connects (the ~192 s shell-side delay before its launch is unexplained, §7): scrollback served = **17,323 B** (one <32 KiB chunk; 27,116 B cumulative within 0.5 s including early live bytes), then **55,569 B live over 8.8 s (~6.3 KB/s; 45,699 of 45,736 live-window messages exactly 1 byte)** — the buffered stream plus already-emitted boot output draining through the reader; the byte 0.34 s after the Enter-poke is replayed boot output, **not** an echo (§4.3) |
| 18:00:01.8 (T0+207.7) | WS client 3 connects: the **complete 74,972 B** stream served as scrollback in 3 chunks (~1 ms) — receipt had completed in the ~3 s since ws2 ended; the 2,080 B tail includes the 17:57:18 cloud-init finish, so ws3's connect is definitively after T0+164 |
| 18:00:09.99 | `vm stop` — graceful (ACPI honored; logind evidence per the run-log grep, §4.5); CH exits |
| 18:00:11.50 | agent **truncates console.log on graceful stop** (by design — fresh log per lifecycle) |

### 4.3 The reader's own performance (once there was something to read)

After the watchdog jolt, the #471 drain reader delivered the **entire**
stream — a mix of ~11 KB pre-buffered bootloader output and live OS-boot
output — through both agent console surfaces:

- **Completeness**: the recovered WS stream is whole —
  firmware banner (`[INFO] Setting up 4 GiB identity mapping …`), the
  bootloader-park region (shim failures + NUL filler, offsets 0 → 11,110),
  then exactly one
  `Linux version` kernel banner (stream offset 11,110 — the first OS-boot
  byte, emitted ~2.5 s after the reboot RPC; kernel `RTC time: 17:56:37`),
  `systemd-logind`
  ×2 (offset 69,822), the `login:` prompt (offset 72,941), ending at
  cloud-init final (`modules:final` → `finished`, Up 41.34 s, emitted
  17:57:18). 74,972 B total —
  matching leg-02's healthy full-boot sizes (E3c-F file sink 74,228 B;
  E3f full flush ~72.7 KB).
- **No hard stop observed — but the quiet-guest E3 comparison was never in
  play**: the frozen E3 signature (278 B hard stop) is a **quiet-guest**
  result; here the stream ran against a **live-booting guest** whose own
  output kept re-triggering flush sessions, so that comparison is drained
  of its premise — the reader's no-hard-stop property on a fully quiet
  guest was **not exercised live** by this leg (its coverage remains the
  mock-tier E3f-shaped stand-in test). What this run does show: a mixed
  backlog + live stream delivered byte-at-a-time (~6.3 KB/s over the ws2
  window), never stalling, no truncation.
- **Byte-consistency across reconnects**: ws3's scrollback replay is
  byte-identical to ws2's received stream over ws2's entire 72,892 B length,
  plus a 2,080 B tail that arrived between the two clients (§5).
- **Poke fate indeterminate**: the byte arriving 0.34 s after ws2's
  Enter-poke was replayed boot output, **not** an echo — the getty
  `login:` prompt sits 49 B past ws2's end of stream (never served to it),
  and ws3's complete snapshot contains no post-poke getty response. No
  echo evidence exists either way; the run's earlier "interactive again"
  reading was wrong.

### 4.4 New finding 1 — E4-class serial-manager thread death via a plain agent SIGKILL

The serial-manager thread was dead within 2 s of the agent SIGKILL and stayed
dead through the 240 s window — established by the **comm scans** (leg-02's
`/proc/<pid>/task/*/comm` probe: no `serial-manager` thread at +2 s and
+10 s) together with the pre-kill delivery (5,684 B flowed before the kill,
so the manager was alive until then). The post-restart adoption connected
cleanly but was **never served**. Note the evidentiary weighting: the
console-side signals of the down window — 0 B for 120.0 s and the unacked
poke — are *consistent with* the dead manager, but they would equally be
produced by a healthy console on a silent or bootloader-parked guest, and
this run's guest was in fact parked at the bootloader (§4.2); on their own
they cannot discriminate. The thread-death claim therefore rests on the
comm scans, not on the 0 B / unacked-poke signature. As an E4 signature this
is leg-02 check #4's — but leg-02 needed an explicit
`SO_LINGER 0` abortive close at raw-CH tier, while this run reached it with a
**production-shaped trigger**: SIGKILL of the agent while boot output was
flowing (unread data in the dying client's receive queue ⇒ the kernel's close
is abortive from CH's side ⇒ silent `ECONNRESET` thread death). The #471
drain reader narrows the unread-data window but cannot eliminate it — with
output actively flowing at death time, the trigger fires.

Corollaries, stated carefully:

- m4.3's S1 leg (agent SIGKILL while `Running`) passes at v53.0 (campaign leg
  03, 119 PASS) because it kills on a **quiet, fully-booted** guest — clean
  close, thread survives. The discriminator is output-in-flight at death,
  not the SIGKILL itself.
- The m2.5-era `abandon_console_endpoint` clean-close discipline (v43
  motivation) does not apply to a SIGKILL — no user-space close runs at all.
- **No reader-side fix exists for this shape**: the serial manager is the
  sender. Recovery requires re-creating it, which is exactly what the boot
  watchdog's CH-api reboot does (its log message says so, and delivery
  revived within ~0.3 s of the dispatch). Whether CH executed or rejected
  the reboot call is indeterminate from outside (the agent itself records
  that `reboot_vm` "reports Ok for non-2xx responses too"). What is
  observed: no second firmware or kernel banner appears in the stream
  (exactly one of each), CH's pid stayed constant, and the parked
  bootloader's OS boot **began ~2.5 s after the dispatch** — coincidence or
  causation is not establishable from outside, and this doc does not claim
  the RPC rebooted, reset, or resumed the guest.

### 4.5 New finding 2 — console.log completeness is unproven under the one-byte flush regime (silent `Lagged`-skip path reachable)

Both WS surfaces carry the complete stream (scrollback + live,
byte-prefix-consistent, §5); the persisted console.log could be shown neither
complete nor incomplete. What the artifacts actually establish, and when:

- **T0+120.2 — preserved snapshot (the source of markers.json)**: console.log
  = 6,759 B — the 5,684 B pre-kill prefix plus 1,075 B of post-restart ring
  replay. `kernel_banner_full=false`, `systemd_logind_recovered=false` —
  **timing-explained**: the agent had received only ~1 KB of the replay; the
  kernel banner (stream offset 11,110) and the logind lines had not been
  received by *any* surface yet. This snapshot is not evidence of a skip.
- **T0+120.3 — run-log grep, un-archived**: the scenario's boot-count check
  grepped this same 6,759 B file — `Linux version` count = 0
  (`[QUAL][FAIL] boot count = 0`, reconciled in §5) — same timing
  explanation. **No banner grep ever ran on the final file.**
- **~T0+215.9 — run-log grep, un-archived**: `systemd-logind` count = **2**
  on the live console.log, after receipt was complete (T0+207.7) — the log
  writer was alive and had processed at least the stream's logind region
  (~75.5 KB into the file). No final size was recorded.

So between the last two observations the writer demonstrably kept up through
most of the stream, and nothing in this run shows a gap. The skip concern is
a **code-path reachability** result, not an observed defect:
`drain_console_endpoint` fans out to a tokio broadcast channel (capacity
4096 messages); `spawn_console_log_writer` does one async `write_all` **per
message** and on `RecvError::Lagged` simply continues — i.e., it **skips the
lagged messages**. Under v53.0's one-byte-at-a-time flush (up to ~20k
messages/s in the ws2 burst window; 45,699 of 45,736 live-window messages
were exactly 1 byte), an fs-writing consumer doing one syscall per byte can
fall past the 4096-message window, and the loss would be silent. Whether it
did so in this run is **not established**: the final console.log is
unrecoverable (the agent truncates console.log on graceful stop by design,
and `vm delete` removed the runtime dir before the scenario's final copy —
a script flaw, §7). The first draft of this finding reported an *observed*
divergence ("scrollback 27,116 B within ~3 s of the jolt vs console.log
+1,075 B"; "banner absent after full recovery") — both premises fail
re-verification against the artifacts (the 27,116 B observation is from
T0+192.6, not ~3 s post-jolt; the banner-absent grep ran at T0+120.3, before
the banner had been received by anyone) and are **retracted**. The
WS/scrollback surface is the completeness-proven one (the scrollback is
pushed synchronously in the drain thread; the WS relay kept up — proven by
the ws2/ws3 byte-prefix consistency). Recorded as a follow-up: make the
persisted audit surface completeness-checkable (or batch its writes) in this
regime.

## 5. Scenarios B and C

**B — reconnect scrollback recovery: PASS.** WS client 2 (12 s, poke at +6 s)
received 72,892 B; WS client 3 (8 s, no poke) received the full 74,972 B as
pure scrollback in 3 chunks (32,768 + 32,768 + 9,436) within ~1 ms of
connecting. `ws3.startswith(ws2)` holds over ws2's entire length; the 2,080 B
difference is the stream tail that arrived between the two connections. The
scrollback is per-agent-process memory: after the agent restart it correctly
starts empty (WS client 1, connecting 15 s after adoption, was served 0 B —
there was nothing in memory yet and nothing live to relay), and it is not
back-filled from console.log.

**C — boot-marker audit: the watchdog fired, and that was the recovery.** The
watchdog (enabled for the whole leg: `stall_secs=120`, marker
`systemd-logind`, `max_reboots=1`) fired at **exactly T0+120.00 s** after the
adopting agent's start — correct on both counts: the console path was dead
(§4.4) *and* the marker genuinely had not arrived (the guest was still
parked at the bootloader; the OS boot had not begun, §4.2) — logged the
stall, dispatched the remediation reboot, and delivery revived within
~0.3 s. No second fire ("standing down" absent); exactly one firmware banner
and one kernel banner in the recovered stream with CH's pid constant (the
OS boot began ~2.5 s after the dispatch — causality not established, §4.4);
and the marker arrived with the recovered stream. One harness
reconciliation: the scenario's own boot-count check printed
`[QUAL][FAIL] boot count = 0` because it grepped the console.log snapshot at
T0+120.2 — pure bootloader-park content, no kernel banner yet (the same
timing-explained absence as markers.json, §4.5); the recovered WS stream
contains exactly one `Linux version` banner. In this
scenario the watchdog is not a false positive to explain away — it is the
containment layer doing precisely its documented job, and this leg is the
first live evidence of the full chain (reader stalls on a dead serial
manager → watchdog fires → reboot dispatched → serial manager re-created
and delivery revives → the parked guest's OS boot proceeds → the drain
reader delivers the complete stream).

## 6. Teardown and residue proof

- `deploy.sh` teardown after attempt 3: **no cloud-hypervisor processes, no
  new host links, no new nft tables, no nwd-spawned dnsmasq** (all PASS);
  the test dir was removed (successful-run path).
- Digests, before vs after **all three attempts** (each attempt verified
  before and after): `/usr/bin/cloud-hypervisor`
  `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496`
  (unchanged; never executed — §3); shared guest image
  `37f7c34075044e8c78f3c9bd8987f4f063bddd91a67b6408ef8daae53dabd22a`
  (unchanged; never written — the VM used a private disk copy).
- No chv processes/units on the host at leg end; no `tap-*`/`br-*` links; no
  `chv-*` nft tables.
- Remaining host artifacts (intentional): the leg scratch dir
  (`/tmp/opencode/leg471/` — binaries, instruments, evidence incl.
  `attempt1/` and `attempt2/` archives) and `/tmp/chv-qual-failed-deploy`
  (attempt-1 post-mortem preserved by `deploy.sh`'s failure path; its logs
  are archived under `evidence/attempt1/`). `/etc/chv/jwt_secret` is
  agent-created host state that pre-dated the leg and was only read.

## 7. Limitations (stated plainly)

- Single host, one valid run; no soak, no repetition.
- No **pre-kill** serial-manager comm scan was taken in the valid run; the
  thread-death attribution rests on 5,684 B of pre-kill delivery (the manager
  must have been alive to deliver), leg-02's verification of the
  `serial-manager` thread name on this exact v53.0 binary, and the dead-at-+2 s
  scans. A pre-kill scan would have made it airtight.
- The final console.log content is unrecoverable (truncate-on-graceful-stop
  by design, then `vm delete` removed the dir before the scenario's final
  copy — a script flaw), and no banner grep ever ran on the final file (the
  banner-absent observations are from T0+120.2–120.3, before the banner had
  been received by anyone). Finding 2 (§4.5) therefore rests on code-path
  reachability plus one un-archived late grep (logind = 2), not on any
  observed skip or any preserved complete file.
- Whether CH executed or rejected the watchdog's `reboot_vm` call is
  indeterminate from outside; only its effects are observed (serial delivery
  revived within ~0.3 s; the parked bootloader's OS boot began ~2.5 s later).
- **The scenario's central premise did not hold as designed**: the guest
  never booted during the 240 s down window — it sat parked at the
  bootloader (cputime +1 s; shim MOK/TPM failure sequence, then NUL-dominated
  filler), and the OS boot began ~2.5 s after the watchdog's reboot dispatch,
  completing at 17:57:18 (T0+164). Consequences carried in §4.3: only ~11 KB
  was pre-buffered backlog, the rest of the stream was live boot output, and
  the quiet-guest 278 B hard-stop comparison was never exercised live.
  Whether the reboot RPC *caused* the boot to proceed is not established —
  no second firmware or kernel banner appears (no classic reset is visible)
  and CH's pid was constant.
- **An unexplained ~192 s shell-side delay** between the mid-run analysis
  (markers.json written 17:56:34.2) and the ws2 launch (client start
  ≈17:59:46.6): every intervening script command is sub-second by
  inspection. It shifted the ws2/ws3 observation points — their connect
  times (T0+192.6 and T0+207.7) are derived from evidence-file mtimes plus
  client-measured durations — but it affects no agent-side measurement.
- **The delivery-rate profile between T0+120.3 and T0+192.6 is unobserved**
  (the size poller ended at T0+119.9): the ~84 B/s figure is a window
  average between the +1,075 B observation and ws2's 17,323 B scrollback
  snapshot; the ~6.3 KB/s figure is measured only across ws2's live window.
- WS client 1's byte-exactness check is vacuous (0 B on both sides); the
  meaningful byte-exactness evidence is the ws2/ws3 prefix consistency and
  the complete marker set.
- The scenario's `peak_bytes_per_s` timeline statistic was computed wrongly
  (summed poll samples rather than deltas) and is ignored; the poller's
  actual observation — 600 samples, size constant at 5,684 B — is unaffected.
- E4-class interaction beyond the SIGKILL shape (e.g., RST-thread-death
  races during WS client churn) was not exercised; the watchdog's
  crawl-regime behavior was exercised via a genuine dead-console stall, not a
  synthetic slow marker.

## 8. Conclusion

For declaration §12 decision point 1 — "is #471's drain-continuously reader
live-verified at v53.0?" — this leg's answer is:

1. **The reader's own claims hold live, with one property untested**: wherever
   CH's serial manager is alive, the #471 reader delivers the late-attach
   stream completely and losslessly through the deployed agent's console
   path — here a mix of ~11 KB pre-buffered bootloader output and live
   OS-boot output, 74,972 B total, every boot marker, byte-consistent across
   reconnects, sustained through a 55,529-message one-byte flood without
   stalling or truncating. The **quiet-guest no-278 B-stop property was not
   exercised live** (the guest was booting throughout the delivery window,
   so its own output kept re-triggering flush sessions); that property's
   coverage remains the mock-tier E3f-shaped stand-in test.
2. **But the deployed-stack late-attach after an agent death mid-output is
   gated upstream of the reader** by CH v53.0's silent serial-manager thread
   death (E4-class), now shown reachable through a plain agent SIGKILL with
   output in flight. No reader can fix a dead sender; in this shape the
   release's layered containment — not #471 alone — is what recovers: the
   boot watchdog fired at exactly its `stall_secs` and dispatched the CH-api
   reboot (which its own log message records as re-creating the serial
   manager); delivery revived within ~0.3 s and the drain reader then
   delivered everything.
   The system-level outcome (complete console recovery within ~3.5 minutes
   of agent restart — bounded by the watchdog's `stall_secs=120` plus drain
   time; no second boot visible in the stream; no operator action) is the
   behavior v0.3.0's containment story promised.
3. **Recorded follow-ups** (beyond this leg's scope): (a) adoption-level
   detection of a never-served serial connection — console-side signals
   (0 B, unacked poke) cannot discriminate a dead serial manager from a
   quiet **or bootloader-parked** guest, so detection needs a
   thread-liveness or CH-api probe — with proactive serial-manager revival,
   instead of waiting out the watchdog's full `stall_secs`; (b) the console.log
   writer's `Lagged`-skip under one-byte flush floods (§4.5) — the persisted
   audit surface's completeness in this regime is **unproven** (this run
   could not audit the final file; the skip path is reachable in code but
   was not observed to fire), while the WS/scrollback surface is
   completeness-proven; (c) the upstream E4 report can now cite a
   production-shaped trigger (plain SIGKILL mid-output), not just
   `SO_LINGER 0`; (d) v53-era runbooks/procedures that SIGKILL agents
   (m4.3-S1-style) should note the quiet-guest caveat — kill during active
   console output kills the serial manager.
