# CellHV Core Implementation Handoff

Date: 2026-07-21

Branch: `agent/cellhv-core-execution-fencing-discovery`

## Current repository state

The worktree was clean when this handoff was written. The relevant commit chain is:

- `75eda99f fix(core): make execution fencing replay safe`
- `9843d169 feat(core): fence execution and harden discovery evidence`
- `e4448a6c feat(core): wire standalone native authority mode`
- `77b92356 feat(core): compose native runtime ownership prerequisites`
- `20683081 feat(core): add activated store and native listener owners`
- `d1cf78f0 feat(core): enforce identity and cache authority boundaries`
- `a64cdcc2 feat(core): converge authority transports and startup policy`
- `59500071 feat(core): serialize shared authority access`

At handoff time, `75eda99f` was one commit ahead of
`origin/agent/cellhv-core-execution-fencing-discovery`.

## What is implemented

### Core authority composition

- `chv-agent` has an explicit, default-off `authority_mode = "core-native"`.
- Omitted/default mode remains `legacy`.
- Native dispatch occurs before legacy NodeCache, Controller, VMM, provider,
  reconciler, console, and metrics construction.
- Native mode owns one runtime lease, one SQLite authority, one serialized
  authority actor, and one private Unix HTTP listener.
- Native identity, VM definitions, operations, and events survive restart.
- SIGINT/SIGTERM perform ordered shutdown.
- Authority-owned listener startup can recover an unreachable stale socket,
  while live sockets, regular files, and symlinks are preserved and refused.

### Durable operation execution fencing

- Core schema migration v2 adds active and completed attempt fencing state.
- Exact v1 databases upgrade transactionally; populated legacy Running rows
  are preserved as ambiguous rather than silently retried.
- Attempt tokens have one bounded, visible-ASCII canonical form enforced in
  Rust, SQLite, and reopen validation.
- Claims return `ClaimResult::Acquired` or `ClaimResult::Replay`.
- Only `Acquired` is allowed to authorize a future external side effect.
- Same-token replay does not increment attempts; competing tokens conflict.
- Terminal writes compare-and-set against the active attempt token.
- Exact completion replay resolves a dropped finish reply; mismatched replay
  conflicts.
- Restarted Running work is `InspectRequired`, not automatically retryable.
- Protocol transports receive `AuthorityHandle`; execution transitions are on
  a separate `ExecutionHandle` and guarded from transport/provider use.

### OpenStack discovery evidence

- A fail-closed Path A discovery runner exists at
  `scripts/openstack-discovery/run-path-a.py`.
- It verifies immutable lab inputs, uses constrained command execution,
  redacts artifacts, restores service state, verifies cleanup, and records
  exact command/artifact provenance.
- Local runner output is permanently an unsigned structural candidate. It
  cannot self-certify T5 completion.
- Complete T5 evidence remains impossible until a trusted external lab
  attestation policy and trust root are accepted and used in a real disposable
  OpenStack lab.
- The checked-in discovery report remains honestly `partial/blocked`.
- O3K evidence may test native API compatibility and idempotency, but cannot
  substitute for Nova/libvirt/Neutron/Cinder T5 evidence.

### Documentation

- The current-state inventory and Phase B evidence pages were refreshed after
  native production composition landed.
- They distinguish opt-in native authority from the still-default legacy path
  and keep authority convergence, VM execution, recovery, providers, libvirt,
  and platform qualification explicitly incomplete.

## Verification completed

The latest focused combined verification passed:

- `cellhv-core-store`: 28 tests
- `cellhv-core-operations`: 28 tests
- `cellhv-core-api`: 21 tests
- `cellhv-core-runtime-owner`: 8 tests
- strict all-target Clippy for those four crates
- Core architecture guard
- architecture unit tests: 36
- OpenStack discovery tests: 57
- formatting and `git diff --check`

Earlier on the same implementation chain, the full Rust workspace passed with
zero failures. Workspace-wide all-target Clippy still has only the previously
established `origin/main` diagnostics in Criterion and control-plane test code;
focused changed-crate Clippy is green.

## Explicitly not implemented

Do not infer any of the following from the completed fencing work:

- no Core journal executor exists yet;
- native Core does not launch, start, stop, reboot, inspect, or delete a VM;
- native power endpoints remain truthful unsupported responses;
- no production Cloud Hypervisor process ownership marker exists;
- no PID/start-time/executable/socket identity inventory or re-adoption exists;
- no retry/supersede transition exists for `InspectRequired` work;
- legacy gRPC and native requests do not yet use one production operation
  authority;
- storage and network provider profiles are not qualified;
- no real T3 KVM recovery result exists;
- no real T5 OpenStack/libvirt result exists;
- no libvirt adapter, XML translator, process, package, or URI mode exists.

## Normative sequencing constraint

ADR-018 is still `Proposed`. It forbids product libvirt implementation until:

1. real T5 `ch:///system` discovery is recorded;
2. Phase B has one durable legacy/native operation authority;
3. Phase C standalone lifecycle and recovery pass;
4. required Phase D storage/network profiles pass independently;
5. adapter process/package, authentication, namespace, XML, event, projection,
   and version-skew decisions are prototyped;
6. named maintainers accept the downstream/upstream policy.

The adapter, when eventually lawful, must remain external and delegate only to
the public Core API. It must never become a second VM lifecycle authority or
introduce libvirt/XML types into Core.

## Next implementation sequence

### 1. Bounded side-effect-free executor

Create a Core executor with an injected `CoreVmRuntime` trait and no production
Cloud Hypervisor wiring initially.

Required properties:

- bounded queue and bounded cross-VM concurrency;
- strict same-VM serialization;
- scheduling deduplication;
- only `ClaimResult::Acquired` crosses the side-effect boundary;
- `Replay` never performs an effect;
- only Accepted/Ready work is scheduled;
- `InspectRequired` is surfaced but never executed;
- effect completion is durably finished before executor shutdown completes;
- runtime owner shutdown order becomes listener, executor drain/join, actor,
  runtime lease;
- architecture guards allow execution capability only in the executor/runtime
  composition and continue rejecting API, compatibility, Controller/O3K, and
  provider access.

Required tests include claim-before-effect, concurrent schedule deduplication,
same-VM serialization, cross-VM concurrency bound, dropped claim reply, dropped
finish reply, Replay-without-effect, ambiguous restart preservation, queue
backpressure, and shutdown after effect but before finish.

### 2. Recovery-capable Cloud Hypervisor boundary

Before enabling production effects, replace the current in-memory-only process
ownership model with durable, inspectable evidence:

- canonical runtime directory and API socket naming;
- PID plus `/proc/<pid>/stat` start time or equivalent anti-reuse identity;
- executable identity;
- socket identity and liveness probe;
- Core ownership marker and owner host identity;
- `inspect` and `adopt` APIs that do not require an owned `Child` handle;
- fail-closed classification of owned, foreign, ambiguous, stale, and missing
  processes;
- no blind socket unlinking;
- explicit recovery decision required before superseding an active attempt.

Test foreign processes, PID reuse, replaced sockets, stale markers, crash after
effect/before finish, daemon restart with a live VM, and conflict preservation.

### 3. Native lifecycle activation and T3

Only after inspection/re-adoption is proven:

- translate the bounded Core VM definition to the existing Cloud Hypervisor
  adapter without duplicating runtime code;
- journal create/start/stop/reboot/delete before effects;
- persist observed state and ownership evidence;
- enable only capabilities that are executable;
- run a qualified Linux guest through real KVM;
- prove agent restart does not stop the VM and re-adopts it;
- add leak/fault cycles and host-reboot policy evidence.

### 4. Authority convergence and providers

- move legacy lifecycle intent through the same Core journal without unsafe
  dual authority or silent NodeCache/Core divergence;
- narrow `chv-stord` and `chv-nwd` validate/prepare/inspect/detach contracts;
- persist attachment ownership and fail cleanup/recovery errors closed;
- qualify minimum storage and network profiles independently.

### 5. External compatibility work

Run the real disposable T5 discovery and accept a path-selection/prototype ADR.
Only then implement the external libvirt delegation adapter if the evidence
selects it. A native Nova driver remains a separate candidate. O3K may continue
to exercise the public native Core API but is not OpenStack qualification.

## Review hazards for the next model

- Do not treat a replayed journal claim as permission to repeat a side effect.
- Do not auto-retry Running/`InspectRequired` work before inspecting external
  reality.
- Do not expose `ExecutionHandle` to HTTP/gRPC/compatibility transports.
- Do not wire the existing `ProcessCloudHypervisorAdapter` directly into Core;
  its in-memory `Child` registry and socket deletion behavior are not restart
  authority.
- Do not mark ADR-018 Accepted or add libvirt product code from local/O3K tests.
- Do not advertise capabilities before the corresponding production executor
  and recovery path are active.
- Preserve the default legacy startup path until an explicit, tested cutover
  policy is implemented.
