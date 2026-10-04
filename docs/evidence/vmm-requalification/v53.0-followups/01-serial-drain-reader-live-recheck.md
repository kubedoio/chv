# v53.0 follow-up leg 01 — Live recheck of #471's drain-continuously serial reattach reader (post-release)

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [v53.0 campaign index](../v53.0/README.md) · [Leg 02 — serial-console defect re-check](../v53.0/02-serial-console-recheck.md)
> Date: 2026-10-04 (UTC)
> Closes: [release declaration §12 decision point 1](../../production-readiness/v0.3.0/00-release-declaration.md) — "#471 (drain-continuously serial reader) is verified at mock/stand-in tier only … A live leg-02-style recheck with the deployed agent on v53.0 is the natural follow-up before/with rollout"
> Execution: subagent, execution + reporting only — repo untouched during the leg (proof in §6); the evidence PR is the leg's only repo deliverable
> Method: leg-02's E3 shape (late attach to a fully-quiet guest) re-run **through the deployed CHV stack** (core-managed authority, real mTLS, `deploy.sh` shape), driving the agent's own console surfaces (console.log writer + WS console server) — the exact surface the #471 reader serves, which leg 02 (raw CH, no agent) could not cover

## Verdict

**Mixed. The #471 reader itself behaves as designed wherever it has something to read — but the deployed-stack late-attach after an agent death with serial output in flight is gated *upstream* of the reader by CH v53.0's silent serial-manager thread death (leg-02 check #4). Recovery happened, completely and byte-exactly, via the boot-watchdog layer firing exactly as designed.**

| # | Check | This leg (deployed agent, v53.0) | Verdict |
|---|---|---|---|
| A | late-attach backlog recovery through the agent console path | **0 B for 120.0 s after adoption** (serial manager dead — E4-class); boot watchdog fired at exactly `stall_secs=120` and dispatched its remediation; within ~1 s of that, delivery revived and the **complete 74,972 B backlog** (firmware → kernel → systemd → logind ×2 → login prompt → cloud-init final; exactly one boot) flowed through the #471 drain reader — byte-at-a-time at 5,485 B/s, **no 278 B hard stop, no truncation**; full recovery ≤ ~18 s after the watchdog jolt | **RECOVERED — via the watchdog layer, not the reader alone** |
| A′ | WS console client served the recovered stream | ws2: 72,892 B; ws3: 74,972 B = ws2 + 2,080 B tail, byte-prefix-consistent over the full length; complete marker set in the stream | **PASS** |
| B | reconnect scrollback recovery | byte-exact scrollback replay (74,972 B in 3 chunks, ~1 ms); scrollback is per-agent-process memory and correctly starts empty after an agent restart | **PASS** |
| C | boot-marker audit (observational) | watchdog fired at **exactly T0+120.00 s** (marker unseen — correctly: the console was dead), dispatched the CH-api reboot; the guest did **not** reboot (single boot in the stream, CH pid constant, kernel timestamps continuous); the marker arrived with the recovered backlog and the fire count stopped | **FIRED AS DESIGNED — and was the necessary recovery trigger** |

Two new findings came out of this leg, both beyond #471's own claims (§4.4, §4.5): the E4-class serial-manager thread death is reachable through a **plain agent SIGKILL with output in flight** (no `SO_LINGER` tricks needed), and the **console.log writer silently skips bytes** under v53.0's one-byte-at-a-time backlog flush.

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

This leg re-runs the E3 shape with the deployed agent in the loop: the guest
boots with the agent's serial connection gone (agent SIGKILLed early in
boot), settles to fully-quiet, then the agent restarts and adopts the running
VM — the late-attach path — and the recovered stream is measured at both
console surfaces the agent owns.

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
   13.65 s after `vm start` acceptance, in the GRUB/early-boot phase
   (the pre-kill prefix captured by the agent is measured, not assumed);
3. confirm the CH process survives and probe its `serial-manager` thread
   (leg-02's comm scan);
4. **240 s agent-down window**: the guest finishes booting and settles to
   fully-quiet (the E3 regime); CH's SharedSerialBuffer accumulates the stream;
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
| 17:50:29 → 17:54:34 | 240 s down window: CH survives (pid constant), `vm.info` = `Running` throughout, guest cputime 5 s → 6 s, guest boots to completion (cloud-init final at kernel-ts 41.2 s) and goes quiet; console.log stays at 5,684 B |
| 17:54:34.06 | agent restarted → **adoption**: `adopted running vm after agent restart; console capture resumed` (same CH pid) — **T0** |
| T0 → T0+120 | **zero delivery**: poller 600 samples, size constant at 5,684 B; WS client 1 (T0+15 → T0+45): **0 B**, Enter-poke at T0+23 never echoed (connection accepted by the kernel backlog, never served — leg-02 E4's exact signature) |
| T0+120.00 | **boot watchdog fired**: `console stalled mid-boot without the boot-complete marker; rebooting to recover (this also re-creates the serial manager and restores console capture)` → `rebooting vm via ch api` → `recovery reboot dispatched` |
| ~T0+120.3 | delivery revived: console.log 5,684 → 6,759 B (+1,075) |
| T0+~123 | WS client 2 connects: 27,116 B of scrollback replayed in <0.5 s, then live stream at **5,485 B/s** (45,699 of 45,736 live messages are exactly 1 byte); Enter-poke echoed after 0.34 s (guest console interactive again) |
| T0+~138 | WS client 3 connects: **full 74,972 B** of scrollback replayed in 3 chunks (~1 ms) |
| 18:00:09.99 | `vm stop` — graceful (ACPI honored; logind evidence present); CH exits |
| 18:00:11.50 | agent **truncates console.log on graceful stop** (by design — fresh log per lifecycle) |

### 4.3 The reader's own performance (once there was something to read)

After the watchdog jolt, the #471 drain reader delivered the **entire**
buffered stream through both agent console surfaces:

- **Completeness**: the recovered WS stream is a whole, single boot —
  firmware banner (`[INFO] Setting up 4 GiB identity mapping …`), exactly one
  `Linux version` kernel banner (stream offset 11,110), `systemd-logind`
  ×2 (offset 69,822), the `login:` prompt (offset 72,941), ending at
  cloud-init final (`modules:final`, kernel-ts 41.2 s). 74,972 B total —
  matching leg-02's healthy full-boot sizes (E3c-F file sink 74,228 B;
  E3f full flush ~72.7 KB).
- **No hard stop, no truncation**: unlike the frozen E3 signature (278 B hard
  stop on a quiet guest) the stream never stalled; it ran in the byte-at-a-time
  flush regime (~5.5 KB/s — ~5× the old poll-reader's ~1 KB/s crawl, but not
  the E3f one-pass burst, which requires guest output to re-trigger a flush
  session; the guest here was quiet, as designed).
- **Byte-consistency across reconnects**: ws3's scrollback replay is
  byte-identical to ws2's received stream over ws2's entire 72,892 B length,
  plus a 2,080 B tail that arrived between the two clients (§5).
- **Interactivity restored**: the poke echo returned (0.34 s) as soon as the
  stream was live.

### 4.4 New finding 1 — E4-class serial-manager thread death via a plain agent SIGKILL

The serial-manager thread was dead within 2 s of the agent SIGKILL and stayed
dead through the 240 s window; the post-restart adoption connected cleanly
but was **never served** (0 B for 120.0 s; the poke never reached the guest).
This is leg-02 check #4's E4 signature — but leg-02 needed an explicit
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
  revived within ~1 s of the dispatch). In this run the guest itself did
  **not** reboot (single boot in the stream, CH pid constant, kernel
  timestamps continuous, ACPI stop honored later) — whether CH executed or
  rejected the reboot call is indeterminate from outside (the agent itself
  records that `reboot_vm` "reports Ok for non-2xx responses too").

### 4.5 New finding 2 — console.log silently skips bytes under the one-byte flush regime

Mid-recovery the two agent surfaces diverged: the in-memory scrollback (and
therefore the WS clients) held 27,116+ B within ~3 s of the jolt, while
console.log had advanced only +1,075 B — and after full recovery console.log
**still lacked the kernel banner** (`Linux version` grep = 0) while carrying
the later `systemd-logind` lines (grep = 2). The mechanism is in the code:
`drain_console_endpoint` fans out to a tokio broadcast channel (capacity
4096 messages); `spawn_console_log_writer` does one async `write_all` **per
message** and on `RecvError::Lagged` simply continues — i.e., it **skips the
lagged messages**. Under v53.0's one-byte-at-a-time backlog flush (tens of
thousands of 1-byte messages), the fs-writing consumer falls past the 4096
window and the persisted console.log loses chunks (here: the burst region
containing the kernel banner). The WS/scrollback surface stayed byte-complete
(the scrollback is pushed synchronously in the drain thread; the WS relay
kept up — proven by the ws2/ws3 byte-prefix consistency). The final console.log
content could not be audited directly: the agent truncates console.log on
graceful stop by design, and the VM delete removed the runtime dir before the
scenario's final copy (a script flaw, §7) — the finding rests on the mid-run
greps plus the code path, and is recorded as a follow-up, not a proven defect
report.

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
adopting agent's start — the marker genuinely had not arrived (the console
was dead, §4.4) — logged the stall, dispatched the remediation reboot, and
delivery revived within ~1 s. No second fire ("standing down" absent), no
guest reboot (boot count in the recovered stream: exactly 1; CH pid
constant), and the marker arrived with the recovered backlog. In this
scenario the watchdog is not a false positive to explain away — it is the
containment layer doing precisely its documented job, and this leg is the
first live evidence of the full chain (reader stalls on a dead serial
manager → watchdog fires → serial manager re-created → drain reader delivers
the complete backlog).

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
  copy — a script flaw). Finding 2 (§4.5) therefore rests on mid-run greps
  (banner absent, logind present) plus the code path, not on a preserved
  file.
- Whether CH executed or rejected the watchdog's `reboot_vm` call is
  indeterminate from outside; only its effect (serial delivery revived, guest
  not rebooted) is observed.
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

1. **The reader's own claims hold live**: wherever CH's serial manager is
   alive, the #471 reader delivers the late-attach backlog completely and
   losslessly through the deployed agent's console path (74,972 B, every boot
   marker, byte-consistent across reconnects, interactive pokes honored), in
   the #8322 crawl regime (~5.5 KB/s) with **no 278 B hard stop** — the exact
   failure mode the reader was built to remove.
2. **But the deployed-stack late-attach after an agent death mid-output is
   gated upstream of the reader** by CH v53.0's silent serial-manager thread
   death (E4-class), now shown reachable through a plain agent SIGKILL with
   output in flight. No reader can fix a dead sender; in this shape the
   release's layered containment — not #471 alone — is what recovers: the
   boot watchdog fired at exactly its `stall_secs`, re-created the serial
   manager via the CH api, and the drain reader then delivered everything.
   The system-level outcome (complete console recovery within ~2.5 minutes
   of agent restart, guest never rebooted, no operator action) is the
   behavior v0.3.0's containment story promised.
3. **Recorded follow-ups** (beyond this leg's scope): (a) adoption-level
   detection of a never-served serial connection (0 B + unacked poke within
   a bounded window) with proactive serial-manager revival, instead of
   waiting out the watchdog's full `stall_secs`; (b) the console.log
   writer's `Lagged`-skip under one-byte flush floods (§4.5) — the persisted
   audit surface is not completeness-guaranteed in this regime, while the
   WS/scrollback surface is; (c) the upstream E4 report can now cite a
   production-shaped trigger (plain SIGKILL mid-output), not just
   `SO_LINGER 0`; (d) v53-era runbooks/procedures that SIGKILL agents
   (m4.3-S1-style) should note the quiet-guest caveat — kill during active
   console output kills the serial manager.
