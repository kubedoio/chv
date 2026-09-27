# CHV Production-Readiness — Prompt 02 Single-Authority Cutover Plan

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [02-single-authority-cutover](/docs/prompts/production-readiness/02-single-authority-cutover.md)
> Capability maturity: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Issue anchors: #231 (single-authority cutover), #185 (EPIC standalone Core).

---

## 1. Current authority (grounded inventory, 2026-09-26)

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
    only effector and NodeCache has exactly one writer. CoreNative mode has no
    NodeCache (documented, not wired); legacy mode is untouched.
    Legacy reconcile `prepare_vm_resources`/`cleanup_vm_resources` remain
    in place for legacy mode until M2.3's gating deletes them (capability already
    folded into the Core runtime in M2.2a; the deliberate 2a scope boundary kept the
    cache-coupled legacy path untouched).
- **M2.3 — Remove the second authority.** Delete the now-dead legacy provider-
  mutation code paths (prepare/cleanup resources, reconcile_vms/volumes/networks
  mutation) that M2.2b gated off in core modes; the Reconciler becomes
  observe/health only for core-managed. Legacy mode remains available for
  migration but is not the campaign target.
- **M2.4 — Fault-injection + concurrency/replay matrix.** Deterministic fault
  points: (1) before durable acceptance; (2) after acceptance, before provider
  effect; (3) during/after provider effect; (4) before compatibility projection
  update; (5) before terminal persistence. Tests: identical-concurrent →
  one accepted op; idempotency-key reuse with different content → hard conflict;
  restart after provider success → no double launch/stop/delete; stale legacy
  generation cannot override newer accepted state; ambiguous ownership fails
  closed for destructive recovery; control-plane restart is not VM identity
  authority.
- **M2.5 — Real-KVM qualification.** Install pinned cloud-hypervisor (v43.0) +
  a minimal guest on this box (`/dev/kvm` present). Run
  Create/Start/Stop/Reboot/Delete plus a crash/restart replay scenario through
  the exact candidate path; capture evidence. If CH/guest install is not
  feasible on-site, report KVM-VERIFIED as **unproven** with the exact gap.

## 4. Evidence matrix (Prompt 02 acceptance)

| Criterion | Evidence |
|---|---|
| Exactly one durable authority accepts lifecycle mutations | M2.1/M2.3 code + tests |
| Production legacy handlers route through Core before provider side effects | M2.1/M2.3 (adapter routed in M2.2a; Reconciler gated in M2.2b) |
| Compatibility state derived or crash-consistent | M2.2b (NodeCache projection, rebuild from Core) |
| Required audit/idempotency/version metadata durable | M2.1b schema migration 0004 + tests (merged `297cb904`) |
| Crash/fault-injection matrix passes | M2.4 |
| Concurrent duplicate + conflict tests pass | M2.4 |
| Real-KVM lifecycle/restart/replay evidence passes | M2.5 (or unproven + gap) |
| Architecture/spec/guards describe the production path | this plan + spec updates (adapter/journal/owner docs un-staled) |
| Dead mutation paths removed only after compat no longer needs them | M2.3 |

## 5. Scope & non-scope

- **In scope:** Core acceptance+execution authority; executor wiring; adapter-
  bound legacy; NodeCache projection; Reconciler gating; durable metadata;
  fault-injection and concurrency/replay tests; real-KVM attempt.
- **Deferred / non-scope (unchanged):** `resize/attach/detach` Core support
  (currently `unimplemented!`) stays out unless required by the supported
  lifecycle set — the RC supports Create/Start/Stop/Reboot/Delete; storage/NIC
  attach is done at create. `UpdateVm/Attach/Detach` remain unsupported (fail
  closed), consistent with the adapter's lossless subset. No second daemon, no
  new VMM, no control-plane-as-Core-client migration (control-plane request
  surface routes to the agent, which is the authority). MULTI-HOST/FIELD remain
  unproven.
