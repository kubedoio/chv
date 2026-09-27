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
- **Delete drains REGARDLESS of the hypervisor delete result.** A failed
  hypervisor delete still triggers a best-effort detach+close of every tracked
  volume and detach of every tracked NIC, so a failed delete cannot strand open
  handles; the tracked entry is preserved so a later successful delete retry
  finishes the drain. This is in-process best-effort cleanup only — it is not
  crash-safe and does not persist the handle map.
- **Crash/restart remediation drill (manual, documented).** After a daemon
  crash leaves volumes open on chv-stord, a running→InspectRequired operation
  is never auto-cleaned. An operator enumerates leaked sessions and closes each
  leaky session via the chv-stord gRPC RPCs `ListVolumeSessions`/`CloseVolume`
  (invoked through the storage/service client surface — e.g. a helper script or
  control-plane admin tooling on the chv-stord socket, not a shell CLI; stord
  open is idempotent on `(volume, locator)`); NICs are detached via the nwd
  equivalent. This is the documented manual path until M2.2b/M3 persists
  handles.
- **Response-lost-after-commit window.** If chv-stord commits an open
  server-side but the RPC response is dropped (e.g. `RuntimeUnavailable`), the
  runtime never learns the handle and unwinds nothing for it — only the manual
  drill above covers that residual.
- **Path-safety boundary.** `vm_id`, `storage_ref`, and `network_ref` reject
  path separators (`/`, `\`), NUL, and the `.`/`..` dot components, both at the
  Core definition-validate authority gate
  (`VmDefinition`/`StorageAttachmentRef`/`NetworkAttachmentRef::validate` in
  cellhv-core-types — the submit path journals nothing unsafe) and again at the
  runtime boundary (`is_safe_resource_id`), so even a pre-journaled row with an
  unsafe id cannot become an fs-mutation primitive. The VM runtime dir is also
  canonicalized and required to be a strict descendant of
  `{runtime_dir}/vms`.
- **Network topology is not torn down on failure.** `ensure_network_topology`
  (bridge/rules) is NOT undone on create failure or in delete cleanup — it has
  shared per-network "ensure" semantics (matching legacy reconcile); only the
  per-VM NIC attach is detached.
- **Request-modeling residuals.** The Core create request does not yet carry
  disk size/seed options, cloud-init userdata, hypervisor overrides, or
  per-NIC addressing — the effector passes empty open options, `None`
  userdata/overrides, and the shared `DEFAULT_NIC_CIDR` with an empty gateway,
  rather than inventing values. Legacy reconcile's cache-coupled
  prepare/cleanup remains active until its M2.3 deletion.

## NodeCache projection (M2.2b)

In core-managed mode, NodeCache (the legacy compatibility store consumed by
`agent_server` list/get handlers, `VmSpec`, and cache helpers) is a **projection
derived from Core execution** — never an independent authority.

- **Only after a terminal Succeeded outcome.** `ProjectingCoreRuntime`
  (`chv-agent-core/src/projection.rs`) wraps the single effector and implements
  `CoreVmRuntime`. It forwards `execute` unchanged, and on exactly the
  executor's Succeeded path (`Ok(None)`) projects the outcome into NodeCache:
  `CreateVm` writes the legacy VmSpec fragment + generation + VM attachments;
  `DeleteVm` removes the VM axis state; `StartVm`/`RebootVm` set desired state
  `Running`; `StopVm` sets `Stopped`; `UpdateVm`/attach/detach are no-ops
  (out-of-lifecycle, fail closed as `Unsupported`). Projection is **best-effort**:
  any request-parse, projection, or cache-save failure warns and skips; it never
  changes the `Result` the executor sees. The cache is persisted only when a
  projection actually mutated it (a no-op arm or a skipped projection leaves no
  trace and no extra snapshot write).
- **Rebuild-on-startup crash model.** At startup (core-managed only) the VM axis
  of NodeCache is rebuilt from the Core store's authoritative VM list
  (`NodeCache::rebuild_from_core`), then persisted — **inside `start_core_managed`
  and strictly BEFORE `CoreRuntimeOwner::start` spawns the executor poller**, so a
  crash-recovery operation can never race (and be clobbered by) the rebuild. A
  crash that loses the projection (or its save) is repaired by the next startup
  rebuild; a stale compatibility cache cannot act as a second authority because
  the legacy Reconciler's provider mutation is gated off in this mode.
- **Single-writer precondition.** `Reconciler::set_provider_mutation_enabled`
  (default true; disabled in core-managed) makes the legacy reconcile path skip
  all three `reconcile_networks/volumes/vms` provider mutations in the
  `TenantReady` arm, so the Core runtime + projection are the only NodeCache
  writers/effectors in core modes. Completing the enforcement, the legacy
  `agent_server` gRPC mutators that would otherwise write a fragment or drive a
  provider side effect behind the Core authority FAIL CLOSED in core-managed
  mode with `unimplemented`: `apply_vm_desired_state` (direct VM-axis second
  writer), `apply_volume_desired_state`, `apply_network_desired_state`,
  `start_network`, `stop_network`, `restart_network`. The lifecycle handlers the
  control plane needs in core-managed are already core-routed
  (`create_vm`/`start_vm`/`stop_vm`/`reboot_vm`/`delete_vm`), and `resize_vm` /
  attach / detach were already gated.
- **Power-op generation staleness residual.** `StartVm`/`StopVm`/`RebootVm`
  project only the desired-state patch (`update_vm_desired_state`); the VM's
  fragment `generation` and attachments are NOT re-projected by these power
  ops — generation stays at the last create/rebuild value until the next
  `CreateVm`/`UpdateVm` or startup rebuild. This is a deliberate residual: the
  compatibility cache's generation faithfulness is bounded by what the
  projection writes.
- **Requested MAC projection.** Core M1 does not model a requested MAC (the
  effector lets the hypervisor assign one at runtime), but the legacy `VmSpec`
  requires a non-empty `mac_address` (`VmSpec::validate` rejects empty). A
  `NetworkAttachmentRef` with `mac_address: None` therefore projects a
  deterministic locally-administered unicast placeholder
  (`02:00:00:HH:HH:HH`, FNV-1a over `{vm_id}\0{network_ref}`) so the
  compatibility surface stays valid and stable across restarts; the actual
  runtime NIC MAC is observable independently, not via this projected `VmSpec`.
- **CoreNative not wired; legacy unchanged.** CoreNative mode has no NodeCache
  today (documented, not wired). Legacy mode keeps the legacy reconciler and its
  direct NodeCache mutations exactly as before.
