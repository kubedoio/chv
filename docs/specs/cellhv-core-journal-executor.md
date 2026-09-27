# CellHV Core Journal Executor

**Status:** Composed in production (Core M1). Real KVM evidence is T3; exercised on Linux without KVM or with fake adapters in tests.  
**Scope:** Runtime-neutral execution boundary after durable operation acceptance

`cellhv-core-executor` consumes the executor-only `ExecutionHandle` and composes `CoreVmRuntime`.

## Invariants

- Only `ClaimResult::Acquired` authorizes one external effect.
- `ClaimResult::Replay` never calls the runtime or finishes the operation, and
  quarantines that VM for the executor lifetime.
- Restart schedules only `Ready` operations. `InspectRequired` is surfaced to
  recovery and cannot enter the execution queue.
- Operation IDs are deduplicated for the executor lifetime.
- The only ingress is a stable-order durable journal scan; callers cannot
  submit accepted operations or choose dispatch order.
- `queue_capacity` is the exact total admitted backlog, including running and
  pending work, enforced by admission permits.
- At most the configured number of effects execute concurrently.
- Effects for one VM never overlap; work for other VMs may proceed concurrently.
- Runtime failures are a closed public-safe code enum. Runtime result values
  must be JSON objects no larger than 64 KiB canonical form, depth 16, and
  4096 nodes; invalid results remain `Running` and quarantine the VM.
- Claim, finish, replay, and result-validation ambiguity quarantines the VM and
  prevents every later same-VM effect. Reports contain stable codes only and
  never authority errors, paths, tokens, or panic payloads.
- A runtime task panic closes ingress, discards pending work, aborts remaining
  tasks, and reports failure without launching another effect.
- Graceful shutdown closes ingress, drains acquired work through fenced terminal
  persistence, and joins the scheduler. The authority actor must be shut down
  only after executor shutdown returns.
- Cancellation after claim leaves the operation `Running` and therefore
  `InspectRequired`; it never authorizes an automatic retry.

## Pending

- production composition in `chv-agent`;
- an ownership inspector and explicit attempt-supersede transition;
- T3 real-KVM qualification.

None of those pending items may be inferred from the side-effect-free executor
tests.

## Effector contract (M2.2a)

`CloudHypervisorCoreRuntime` (`crates/chv-agent-runtime-ch`) is the Core
executor's single full-side-effect effector: for an acquired `CreateVm` it is
responsible for the entire VM bring-up that the legacy reconcile path
(`prepare_vm_resources`) used to do — stord volume open/attach, nwd
topology/NIC setup, the VM runtime directory, and the Cloud Hypervisor
create. Start/stop/reboot/delete go straight to the adapter; `UpdateVm` and
the attach/detach operations are `Unsupported` (fail-closed, out of the
RC-lifecycle until the M2.2b/M3 projection work).

- **Request envelope.** The journaled `operation.request` is the canonical
  envelope `{"command": {...}, "expected_vm_version": N}`. The effector
  de-envelopes it into the internally-tagged `MutationCommand`; it never
  deserializes the envelope directly into `VmDefinition` (which denies unknown
  fields).
- **Neutral controller.** The effector performs all stord/nwd side effects
  through `chv_hypervisor_api::HostResourceController` — runtime-ch depends on
  the low shared crate only. The production implementation
  (`chv_agent_core::resources::AgentResourceController`) connects a fresh
  stord/nwd client per call; a deterministic public mock lives in
  `chv-agent-runtime-ch` for cross-crate tests. A down stord/nwd surfaces as
  `RuntimeUnavailable` at execute time (create/delete), never at construction.
- **Unified socket convention.** The VM API socket is always
  `{runtime_dir}/vms/{vm_id}/vm.sock` (mode 0o775, tolerant `chv-stord` group
  chown) — the same convention legacy reconcile uses.
- **In-memory handle residual (documented).** Successful creates keep the
  stord/nwd handle map in the effector's memory only. M2.2a adds no durable
  handle persistence: after an agent restart a delete can no longer drain the
  map, logs the residual, and still returns success (the delete already
  happened; a leaked handle is not an infinite-retry failure). M2.2b adds the
  NodeCache projection after Core execution.
- **Request-modeling residuals.** The Core create request does not yet carry
  disk size/seed options, cloud-init userdata, hypervisor overrides, or
  per-NIC addressing — the effector passes empty open options, `None`
  userdata/overrides, and the shared `DEFAULT_NIC_CIDR` with an empty gateway,
  rather than inventing values. Legacy reconcile's cache-coupled
  prepare/cleanup remains active until its M2.3 deletion.
