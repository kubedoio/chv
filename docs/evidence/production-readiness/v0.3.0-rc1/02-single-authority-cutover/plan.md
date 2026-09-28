# CHV Production-Readiness — Prompt 02 Single-Authority Cutover Plan

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [02-single-authority-cutover](/docs/prompts/production-readiness/02-single-authority-cutover.md)
> Capability maturity: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Issue anchors: #231 (single-authority cutover), #185 (EPIC standalone Core).

---

## 1. Current authority (grounded inventory, 2026-09-26)

> **Historical snapshot:** this section records the state of `main` at merge
> `731a89cf` (2026-09-26), before any campaign milestone landed. The gaps it
> lists are closed by later sections of this plan: the journal is wired to the
> production executor (M2.2a), the second-authority Reconciler and
> mode-selection hazards are removed (M2.3), and the recovery/fault matrix is
> verified (M2.4). Do not read §1 as the current state of `main`.

The campaign verified the following on current `main` (merge `731a89cf`):

- **Default mode `legacy`:** the sole *effectful* authority is the legacy daemon —
  `AgentServer` legacy branches + the 5 s `Reconciler` — mutating the provider
  (Cloud Hypervisor adapter), stord, and nwd directly, keyed on the
  state-machine `NodeCache` JSON. No Core operation is created.
- **Mode `core-managed`:** `AgentServer` routes all five lifecycle gRPC calls
  through `legacy_core_adapter` → `OperationService::submit` (durable SQLite
  `accepted`), **but** the `JournalExecutor`'s only ingress (`scan_ready`) is
  never called in production → accepted ops are never claimed/executed/finished;
  **and** the `Reconciler` still runs and mutates NodeCache + provider directly
  (a second authority); **and** `resize/attach/detach` are `unimplemented!`.
- **Mode `core-native`:** Core + native HTTP API only (no legacy surface);
  same effect-dead journal.
- **Control plane:** BFF `create_vm`/`delete_vm`/`resize_vm` write control-plane
  SQLite directly (vms/desired_state/volumes/networks/operations); `mutate_vm`
  goes through `LifecycleService` → Orchestrator → agent gRPC (the supported
  production request path to the agent). The agent is where the lifecycle
  authority must live.
- **Adapter status:** `legacy_core_adapter` is already de-facto dispatched from
  `AgentServer` core branches (the module doc "deliberately not called" is
  stale). It provides deterministic, length-prefixed, lossless identity +
  idempotency mapping and fails closed on unsupported fields.
- **NodeCache:** single-file atomic save (`tmp+fsync+rename`, `AuthorityLock`,
  save-order guard) — safe against torn writes, but two writers in core-managed
  (AgentServer/Reconciler vs Core journal) can diverge.

**Root cause of Prompt 02:** the Core journal is a durable *acceptance* authority
that is effect-dead (no production execution/replay scheduler), while legacy paths
remain the only effector — exactly the "accepted-but-never-executed" +
"two-authority divergent desired state" gap the adapter spec warns about.

## 2. Target architecture (single authority)

```text
supported request paths (agent gRPC lifecycle / native Core API)
        |
        v
bounded validation + conversion   (legacy_core_adapter / native submit)
        |
        v
single durable Core acceptance    (OperationService: idempotency+fingerprint CAS,
        |                           requester/external-id/timestamp/generation,
        |                           accepted resource version, fenced attempt)
        v
Core executor scheduler           (JournalExecutor::scan_ready driven in production,
        |                           claim+fence, bounded concurrency, quarantine)
        v
Core runtime (single effector)    (CloudHypervisorCoreRuntime + stord/nwd/VM-dir
        |                           prep+cleanup, NodeCache compatibility projection)
        v
terminal result persisted          (succeeded/failed/unsupported, replay+audit)
```

- **One durable acceptance authority**: `OperationService`.
- **One durable execution authority**: the wired `JournalExecutor` (restart-safe,
  attempt-fenced, InspectRequired quarantine on ambiguous windows).
- **Exactly one effector**: the Core runtime. Legacy/control-plane handle
  requests only through the adapter. The `Reconciler`'s provider-mutating paths
  are disabled in core modes (it no longer converges NodeCache/provider).
- **NodeCache is a compatibility projection**: rebuilt from Core store on
  startup and updated only as a consequence of Core execution; never mutated as
  an independent authority.
- **Durable metadata** (schema migration): requester identity, external
  operation ID, request timestamp, idempotency scope/key + request-content
  fingerprint, legacy desired generation, accepted Core resource version,
  terminal result — persisted, never memory-only.

## 3. Milestones (each a reviewable PR; evidence recorded)

- **M2.1 — Executor wiring + durable metadata.** Drive `scan_ready` on a
  periodic scheduler in production composition (core-managed & core-native);
  add the durable audit/metadata fields via store migration 0004 and populate
  them from the adapter intent / native submit. Test: accepted→claimed→executed
  →finished in-process; restart replays non-terminal ops; fenced attempt tokens
  prevent double side effects.

  > **Status: COMPLETE.** M2.1a executor wiring — merged `796ca38c`
  > (PR #257, evidence
  > `m2.1a-executor-wiring.md`). M2.1b durable operation metadata (requester,
  > external op ID, request timestamp, legacy generation) — merged `297cb904`
  > (PR #258, migration 0004, evidence `m2.1b-durable-metadata.md`).
- **M2.2 — Single effector runtime** (split: side effects first, then projection).
  - **M2.2a — Full side-effect Core runtime.** Extend the Core runtime to perform
    the full VM side effect (stord open/attach, nwd ensure/attach, VM dir, CH
    create/start/stop/reboot/delete) and fix the CreateVm envelope-parse latent bug.
    `CloudHypervisorCoreRuntime` becomes the one effector for Core lifecycle ops;
    shared conventions (`vm_runtime_dir`, bridge/naming, `nic_id`, 0o775+chown) are
    folded into `chv-hypervisor-api` and reused; a neutral `HostResourceController`
    trait keeps the runtime decoupled from the stord/nwd clients.
    > **Status: COMPLETE.** Merged `d1028097` (PR #259, evidence
    > `m2.2a-single-effector-runtime.md`).
  - **M2.2b — NodeCache projection after Core execution.** NodeCache becomes a
    *projection* derived from Core execution: in core-managed mode it is mutated
    ONLY after a terminal Succeeded Core outcome (`Ok(None)` from the executor's
    runtime path) by the `ProjectingCoreRuntime` composition wrapper
    (`chv-agent-core/src/projection.rs`), and it is REBUILT from the Core store
    at startup (`NodeCache::rebuild_from_core`, crash-consistency). Projection is
    best-effort (warn + skip; rebuild repairs on restart) and never changes the
    executor Result. The canonical request envelope
    (`{"command": …, "expected_vm_version": N}`) is now a shared public
    `CanonicalRequest` in `cellhv-core-operations` (single source of truth,
    replacing runtime-ch's private `CanonicalEnvelope`). A necessary single-writer
    precondition lands here too: the legacy Reconciler's provider mutation
    (`reconcile_networks/volumes/vms`) is gated off in core-managed mode via
    `Reconciler::set_provider_mutation_enabled(false)` so the Core runtime is the
    only effector and NodeCache has exactly one writer; completing that
    enforcement, EVERY legacy `agent_server` fragment-writing / provider-
    effecting RPC fails closed (`unimplemented`) in core-managed — the four
    `apply_*_desired_state` handlers, the stord plane (`resize/snapshot/
    restore_volume`, `delete_volume_snapshot`, `clone_volume`), the live-VM CH
    plane (`pause/resume_vm`, `power_button_vm`, `add_disk`, `remove_device`,
    `add_net`, `resize_disk`, `snapshot_vm`, `restore_snapshot`, `coredump_vm`,
    `migrate_vm`), and the nwd plane (`start/stop/restart_network`,
    `update_overlay`, `send_gratuitous_arp`) — so no second writer or provider
    side effect can run behind the Core authority. Deliberate boundaries: the
    lifecycle handlers the control plane needs stay core-routed
    (`create_vm`/`start_vm`/`stop_vm`/`reboot_vm`/`delete_vm`); `resize_vm` /
    `attach_volume` / `detach_volume` were already gated; read-only/handshake
    handlers remain; and operator node-state transitions
    (`pause/resume_node_scheduling`, `drain_node`, `enter/exit_maintenance`)
    remain available (node-level operational state, no provider side effect).
    Net effect: features Core M1 does not model (snapshot/restore, migration,
    live device hot-plug, storage snapshot/clone) are unavailable in
    core-managed and fail loudly rather than acting behind the authority. The
    startup rebuild
    runs inside `start_core_managed` before the executor poller starts, so no
    crash-recovery op can race the rebuild. CoreNative mode has no
    NodeCache (documented, not wired); legacy mode is untouched.
    Legacy reconcile `prepare_vm_resources`/`cleanup_vm_resources` remain
    in place for legacy mode until M2.3's gating deletes them (capability already
    folded into the Core runtime in M2.2a; the deliberate 2a scope boundary kept the
    cache-coupled legacy path untouched).
    > **Status: COMPLETE.** Merged `befa5129` (PR #260, evidence
    > `m2.2b-nodecache-projection.md`). M2.2 is now **COMPLETE**.
- **M2.3 — Remove the second authority (structural; legacy preserved).** Replaces
  the runtime `provider_mutation` flag/setter with an **immutable construction-time
  mutation surface**: `Reconciler::mutation: Option<LegacyMutation>` (mutation-only
  VM `runtime_dir`), `Reconciler::new_legacy` as the *only* mutation-capable
  constructor (explicit opt-in, renamed from the neutral `new`), and
  `Reconciler::new_observe_only` for core-managed (no mutation state, no setter).
  `reconcile_networks/volumes/vms` fail closed at their first statement
  (`require_mutation`), so the Reconciler is observe/health-only for core-managed.
  At the 28 M2.2b `agent_server` fail-closed gates: unchanged. `cmd/chv-agent`
  composes via an **exhaustive `AgentAuthorityMode` match**
  (`CoreManaged → observe-only`, `Legacy → legacy`, `CoreNative → unreachable!`),
  making a future mode variant a compile error rather than a silent mutation
  default. The plan's original "delete the now-dead legacy mutation paths"
  framing was superseded by the campaign decision **not to break legacy mode**:
  the legacy mutation bodies (`prepare_vm_resources`/`cleanup_vm_resources`,
  `reconcile_vms/volumes/networks`) are **retained** as legacy mode's mutation
  surface (a supported migration path, out of campaign scope) and are
  structurally unreachable in core-managed.
  > **Status: COMPLETE.** Merged `ef1f9330` (PR #261, evidence
  > `m2.3-observe-only-reconciler.md`). M2.3 done; M2.4 is next.
- **Inter-milestone: hardening sweep R1.** A comprehensive review of post-M2.3
  main (8-round adversarial loop, three lenses + process/consumer/spec lenses)
  triaged 15 findings (3 MAJOR: permanent post-restart quarantine of in-flight
  operations, silent executor death, blind core-managed telemetry) and landed
  restart-interruption markers (`running` + marker = `InspectRequired`,
  discoverable via `/v1/operations` `recovery_assessment` and resolvable via
  the node-scoped agent RPC), executor fatality with non-zero process exit,
  honest core-managed telemetry (`health_status: "Unknown"`, fail-closed
  observe-only drain with `drain_blocked` alert), BFF observed-known counting
  and fail-closed snapshot restore, and spec/doc alignment (operation-model,
  recovery-assessment-journal, journal-executor, authority-actor,
  ARCHITECTURE, agent-spec, failure-matrix, OPERATIONS runbook). Legacy mode
  untouched (M2.3 decision preserved).
  > **Status: COMPLETE.** Merged `4e3ae1f5` (PR #269, evidence
  > `hardening-r1-sweep.md`). M2.4 remains next.
- **M2.4 — Fault-injection + concurrency/replay matrix.** Deterministic fault
  points: (1) before durable acceptance; (2) after acceptance, before provider
  effect; (3) during/after provider effect; (4) before compatibility projection
  update; (5) before terminal persistence. Tests: identical-concurrent →
  one accepted op; idempotency-key reuse with different content → hard conflict;
  restart after provider success → no double launch/stop/delete; stale legacy
  generation cannot override newer accepted state; ambiguous ownership fails
  closed for destructive recovery; control-plane restart is not VM identity
  authority.
  > **Status: COMPLETE.** Merged `94c9d9fe` (PR #271, evidence
  > `m2.4-fault-injection-matrix.md`). All five crash windows have
  > deterministic injection idioms with restart/replay proofs through the real
  > composition; the six-scenario matrix passes (canary flagship + mid-effect
  > crash + restart-spanning identity proofs are new). Reviewed through a
  > 2-round adversarial loop (R1: 4 MINOR + 8 INFO, all fixed; R2: wording +
  > coverage notes, closed). CI-VERIFIED only — mock adapter/controller, no
  > real cloud-hypervisor. M2.5 is next.
- **Inter-milestone: hardening sweep R2.** A comprehensive five-lens review of
  post-M2.4 main triaged 3 MAJOR + ~18 MINOR + ~10 INFO findings and landed:
  the executor failure-quarantine lifecycle with permit-preserving release
  and reconcile (replacing permanent post-restart quarantine), the
  LifecycleService mTLS interceptor (last node-facing service without peer
  certificates), the CP resolve relay with fail-closed egress and full
  validation, systemd `TimeoutStopSec=75` above the 60 s drain budget, the
  staged atomic bootstrap (migration target published via
  `rename_noreplace` — an interrupted bootstrap can no longer brick the
  node), the bounded failure-event ring (architecture-clean executor
  failure surfacing), durable TLS material writes, journal metrics, the
  path-traversal cluster closed at every join (`image_ref`,
  `volume_id`, agent-socket `{node_id}` substitution gated at the single
  join point plus request/enrollment boundaries), bounded listing with
  serving indexes, and OPERATIONS.md/spec alignment. Reviewed through a
  3-round adversarial loop (R1: 1 code MAJOR — `write_file_durable` never
  wrote — + 1 doc MAJOR + a dozen MINOR/INFO, all fixed; R2: 1 MAJOR —
  scrap self-healing unreachable in the production startup path — + MINORs,
  all fixed; R3: clean, converged). +17 tests vs baseline; legacy mode
  untouched (M2.3 decision preserved).
  > **Status: COMPLETE.** Merged `62db8bf6` (PR #272, evidence
  > `hardening-r2-sweep.md`). CI green on the merge; the separate Nightly
  > Packages workflow failed on this and every prior main SHA (glibc 2.38
  > toolchain drift vs the oldest Debian smoke target — pre-existing,
  > reported). Fixed post-sweep the same day in PR #273 (`d8e8e2e0`:
  > nightly+release build runners pinned to ubuntu-22.04, plus two
  > further latent release-pipeline defects the fix's dry-run
  > verification exposed and fixed). CI, Security, and Nightly Packages
  > all green on `d8e8e2e0` — the nightly's first green push-triggered
  > run in its history. M2.5 remains next.
- **M2.5 — Real-KVM qualification.** Install pinned cloud-hypervisor (v43.0) +
  a minimal guest on this box (`/dev/kvm` present). Run
  Create/Start/Stop/Reboot/Delete plus a crash/restart replay scenario through
  the exact candidate path; capture evidence. If CH/guest install is not
  feasible on-site, report KVM-VERIFIED as **unproven** with the exact gap.
  > **Status: COMPLETE.** Qualification passed 2026-09-28 (run 8e, post-#291
  > main `3ef619dc`): **47 PASS / 0 FAIL** through the exact candidate path —
  > core-managed authority, real TLS, token enrollment, chvctl-driven
  > create/start/reboot/stop/delete, two agent SIGKILL crash replays with VMM
  > adoption (same CH pid across both restarts, exactly one VMM at every
  > check), an interrupted-stop resolution through the node-local RPC, the
  > documented idempotent operator retry, and force-fallback convergence;
  > #291's delete-after-force-stop idempotence verified in-stack. Full
  > evidence: `m2.5-kvm-qualification.md` (runs 8a–8e, isolation experiments
  > e2–e7, artifacts preserved). **One guest-platform finding — root-caused,
  > recorded, and gating higher tiers but not this one:** fresh-VMM boots of
  > the full VM config (root + seed + NIC) freeze the guest mid-boot on
  > CH v43 × this stack (logind never starts, ACPI presses have no consumer;
  > the designed force fallback is what converged every S2 stop in 8c–8e).
  > chv's machinery is unaffected — the crash/replay/convergence design
  > absorbed the worst case exactly as designed — but "start a VM and the
  > guest boots" is false on this stack until the platform defect is fixed.
  > **KVM-VERIFIED applies to the lifecycle machinery; RELEASED-tier claims
  > stay blocked** on the freeze's CH-side root cause (e7 bisection: neither
  > the NIC nor the seed alone triggers it — the freeze requires the full
  > root+seed+NIC combination on a fresh VMM; mechanism open, follow-up
  > recorded in the evidence doc).

## 4. Evidence matrix (Prompt 02 acceptance)

| Criterion | Evidence |
|---|---|
| Exactly one durable authority accepts lifecycle mutations | M2.1/M2.3 code + tests |
| Production legacy handlers route through Core before provider side effects | M2.1/M2.3 (adapter routed in M2.2a; 28 legacy RPC gates M2.2b; Reconciler observe-only in M2.3) |
| Compatibility state derived or crash-consistent | M2.2b (NodeCache projection, rebuild from Core) |
| Required audit/idempotency/version metadata durable | M2.1b schema migration 0004 + tests (merged `297cb904`) |
| Crash/fault-injection matrix passes | M2.4 |
| Concurrent duplicate + conflict tests pass | M2.4 |
| Real-KVM lifecycle/restart/replay evidence passes | M2.5 run 8e: 47/47 on `3ef619dc` (`m2.5-kvm-qualification.md`); guest-platform freeze (fresh-VMM boots, CH v43 × this stack) recorded as a RELEASED-tier blocker |
| Architecture/spec/guards describe the production path | this plan + spec updates (adapter/journal/owner docs un-staled) |
| Second authority structurally impossible in production config | M2.3 (observe-only construction; exhaustive mode match; first-statement fail-closed mutation methods) |

## 5. Scope & non-scope

- **In scope:** Core acceptance+execution authority; executor wiring; adapter-
  bound legacy; NodeCache projection; Reconciler observe-only single-authority
  (M2.3); durable metadata;
  fault-injection and concurrency/replay tests; real-KVM attempt.
- **Deferred / non-scope (unchanged):** `resize/attach/detach` Core support
  (currently `unimplemented!`) stays out unless required by the supported
  lifecycle set — the RC supports Create/Start/Stop/Reboot/Delete; storage/NIC
  attach is done at create. `UpdateVm/Attach/Detach` remain unsupported (fail
  closed), consistent with the adapter's lossless subset. No second daemon, no
  new VMM, no control-plane-as-Core-client migration (control-plane request
  surface routes to the agent, which is the authority). MULTI-HOST/FIELD remain
  unproven.
