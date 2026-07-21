# `chv-agent` to CellHV Core Migration Map

**Decision:** evolve `chv-agent` in place. This map introduces no daemon, runtime behavior, store, or operation engine.

## Responsibility map

| Existing responsibility (implementation) | Classification | Migration seam |
|---|---|---|
| Binary bootstrap/config/service (`cmd/chv-agent/src/main.rs:214-510`) | `retain-in-agent-core` | Initialize the Phase B store and application service here; keep binary/service names. |
| Legacy Unix gRPC (`AgentServer::serve`, `agent_server.rs:111-145`) | `keep-as-compatibility-adapter` | Handlers translate proto inputs into the shared application service; no direct VMM/provider mutation after each method migrates. |
| Desired fragment acceptance (`apply_*_desired_state`, `agent_server.rs:149-430`) | `extract-to-core-module` | One application service validates, journals, and applies both legacy and future native requests. |
| JSON `NodeCache` (`cache.rs:151-258`) | `retire-after-migration` | Read-only/idempotent importer plus temporary projection; record cutover and never allow post-cutover independent mutation. |
| In-memory VM records (`VmRuntime`, `vm_runtime.rs:8-41`) | `extract-to-core-module` | Make it an execution facade backed by durable VM/operation state, not identity authority itself. |
| Reconcile state machine (`Reconciler`, `reconcile.rs:49-1200`) | `extract-to-core-module` | Invoke journaled operation steps; retain bounded retry logic only where its semantics match durable recovery. |
| Cloud Hypervisor process/API implementation (`ProcessCloudHypervisorAdapter`, `process.rs:84-263,459+`) | `retain-in-agent-core` | Narrow behind a stable VMM interface; add enumeration/ownership/re-adoption in place. Do not duplicate it. |
| Broad current VMM trait (`CloudHypervisorAdapter`, `adapter.rs:70-193`) | `unresolved-requires-ADR` | Phase C must decide the minimal supported surface versus migration/snapshot compatibility; no second backend. |
| VM console (`ConsoleServer`, `console_server.rs:21-397`) | `retain-in-agent-core` | Authorize through durable VM identity; continue consuming the same runtime facade. |
| Storage preparation (`prepare_vm_resources`, `reconcile.rs:843-895`) | `delegate-to-chv-stord` | Core journals attachment intent; stord retains provider execution and session persistence. |
| Storage provider APIs/backends (`StorageBackend`, stord crates) | `delegate-to-chv-stord` | Narrow and qualify existing contracts; no agent-local duplicate provider. |
| Network preparation (`prepare_vm_resources`, `reconcile.rs:897-983`) | `delegate-to-chv-nwd` | Core journals attachment intent; nwd returns concrete owned endpoint handles. |
| Linux network mutation (`NetworkServiceImpl`/`LinuxExecutor`) | `delegate-to-chv-nwd` | Audit privilege and ownership in Phase D; do not move broad host mutation into agent. |
| Provider child supervision (`DaemonSupervisor`, `supervisor.rs:10-201`) | `retain-in-agent-core` | Retain until a provider lifecycle ADR changes systemd/child ownership. |
| Enrollment, telemetry, fleet inventory (`main.rs`, `control_plane.rs`, `enrollment.rs`) | `keep-as-compatibility-adapter` | Optional manager client; failure must not gate local Core recovery. |
| Fleet scheduling, tenant/project/quota, global desired/observed projections, UI/Designer | `move-above-core` | Remain control-plane concerns and use public Core contracts. |
| Control-plane operation journal (`OperationRepository`, `controlplane-store/src/operations.rs`) | `keep-as-compatibility-adapter` | During Phase B correlate legacy operation IDs to the single local operation engine; later it becomes a projection, not VM mutation authority. |
| Direct handler/reconciler calls into VMM and providers | `retire-after-migration` | Remove per method only after both APIs use the shared durable application service. |

## End-state topology

```text
CellHV Controller / O3K / cloud integrations
                    |
       public Core API or compatibility API
                    |
                chv-agent
        CellHV Core runtime authority
          durable local state
          operation journal
          recovery and re-adoption
          Cloud Hypervisor lifecycle
                    |
         chv-stord        chv-nwd
                    |
          Cloud Hypervisor + Linux KVM
```

No second daemon is required: both API surfaces already terminate inside `AgentServer`; Phase B inserts a shared application service beneath that existing process, and `ProcessCloudHypervisorAdapter` remains the sole VMM implementation.

## Incremental seams and conflict prevention

1. **Durable state:** place the single SQLite store in/directly beneath `chv-agent-core`, opened once by `cmd/chv-agent`. The machine-readable authority declaration remains `durable_vm_store_count: 0` until that Phase B implementation lands, then changes atomically to one with an allowlisted path.
2. **Legacy entry:** translate current proto requests in `AgentServer` into typed commands retaining `RequestMeta.operation_id`, fingerprint, requester, and generation.
3. **Native entry:** the future Unix-socket native API translates into the same command types and calls the same application service. It never calls SQL, providers, or `VmRuntime` directly.
4. **NodeCache migration:** validate cache version and every fragment; import deterministic existing IDs in one transaction; persist source digest/cutover marker; make repeats no-ops; retain the original JSON for rollback. Malformed input fails before activation.
5. **Runtime reuse:** adapt `VmRuntime` and `ProcessCloudHypervisorAdapter`; do not create another process map or VMM crate. Add stable ownership markers and process enumeration there in Phase C.
6. **One-owner gate:** before host effects, acquire/check the durable VM ownership record and operation lease. Legacy/native requests with the same identity converge on one operation; conflicting fingerprints fail. Runtime-only or ambiguous ownership blocks destructive work.

## Rollback

This Phase A slice rolls back by reverting the documentation, schemas, registry, authority declaration, guard/tests, and CI step; it has no runtime or durable-data effect.

For Phase B, preserve the pre-cutover NodeCache and record the database migration/cutover version. Rollback may re-enable the compatibility reader only if no native-only mutation occurred after cutover; otherwise stop and require an explicit down-migration/export to prevent divergent VM identities. Never delete the Core database or regenerate IDs. Running Cloud Hypervisor processes remain untouched during management rollback.

## Phase B estimate and unresolved decisions

Based on the direct handler/reconciler coupling and absence of local persistence, Phase B remains **4-6 engineering weeks** for one senior Rust/Linux engineer, plus review and test support: store/migrations 1.5-2 weeks; operation/application service 1.5-2 weeks; two API adapters and NodeCache cutover 1-2 weeks. The exact native transport, database path/permissions, operation lease model, and breadth of the initial VMM command set require focused design review; none requires a second daemon.

## Implementation declaration

- Phase and slice: Phase A1 baseline and migration lock.
- Existing `chv-agent` code being evolved: `cmd/chv-agent`, `chv-agent-core`, `chv-agent-runtime-ch`.
- Runtime authority impact: none; guards declare and enforce one runtime identity.
- VMM backend: Cloud Hypervisor only.
- Network/storage path: existing nwd/stord boundaries, unchanged.
- Platform path: none.
- Acceptance IDs: `AGENT-CORE-001`, `AGENT-CORE-006`, `VMM-ID-001`, `CLAIM-001` at T0.
- Explicit non-scope: local SQLite authority, native API, runtime/recovery/provider behavior, OpenStack.
- Evidence: schemas/registries, architecture guard self-tests, CI results, and current-state inventory.
- Residual risks: static checks cannot prove runtime exclusivity; T2/T3 evidence awaits later phases.
- Estimated effort and owner: Phase A1 3-5 days, senior Rust/Linux virtualization engineer plus architecture review.
