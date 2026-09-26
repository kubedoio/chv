# Prompt 02 — Single Durable Lifecycle Authority Cutover

Complete #231 and the close gate of #185, or their current successors, without introducing a second runtime daemon or a second lifecycle authority.

## Preconditions

- Execute Prompt 00.
- Read the current CellHV Core ADRs/specs and `docs/specs/cellhv-core-legacy-grpc-adapter.md`.
- Inspect current production lifecycle handlers, `legacy_core_adapter`, Core store/executor/journal, NodeCache mutation paths, runtime owner, and recovery paths.
- Revalidate whether the adapter is still "implemented, deliberately not wired" before planning work.

## Goal

Make exactly one durable CellHV Core acceptance path authoritative for:

- Create;
- Start;
- Stop;
- Reboot;
- Delete.

Legacy/control-plane APIs may remain for compatibility, but they must become bounded adapters into the same authority.

## Required invariant

```text
legacy/native request
       |
       v
bounded validation + conversion
       |
       v
single durable Core acceptance/coordinator
       |
       +--> durable identity/idempotency/audit/version state
       |
       +--> provider/runtime side effect
       |
       '--> derived compatibility projection
```

Provider side effects must not be accepted through an independent legacy state machine.

## Required durable metadata

Where applicable preserve durably:

- requester identity;
- external operation ID;
- request timestamp;
- deterministic idempotency scope/key;
- request-content fingerprint required for conflict detection;
- legacy desired generation;
- expected/accepted Core resource version;
- operation terminal result needed for replay/audit.

Do not keep replay-critical metadata only in memory.

## Required work

### 1. Authority inventory

For each lifecycle mutation identify:

- current public handler;
- current durable writes;
- NodeCache writes;
- provider/runtime side effects;
- Core operation created or not created;
- idempotency handling;
- crash windows.

Document every path that can currently mutate VM lifecycle outside Core acceptance.

### 2. Introduce one acceptance/coordinator boundary

Evolve existing Core components first.

Requirements:

- compatibility conversion is bounded and lossless for supported fields;
- unsupported fields/actions fail closed;
- acceptance is durable before side effects that must not be duplicated;
- retries return/converge on the same accepted operation;
- same idempotency key + different request content is a hard conflict;
- NodeCache becomes explicitly derived/projection state or is updated through a proven crash-consistent mechanism.

### 3. Fault injection

Create deterministic fault points around at least:

1. before durable acceptance;
2. after acceptance, before provider side effect;
3. during/after provider side effect;
4. before compatibility projection update;
5. before terminal operation persistence.

For every point prove restart/replay behavior.

### 4. Concurrency/replay tests

Prove:

- two identical concurrent requests converge on one accepted operation;
- conflicting reuse of an idempotency key is rejected;
- restart after provider success does not launch/stop/delete twice;
- stale compatibility generation cannot override newer accepted state;
- ambiguous runtime/process ownership fails closed for destructive recovery;
- management/control-plane restart does not become VM identity authority.

### 5. Real KVM qualification

For the exact candidate path, run Create/Start/Stop/Reboot/Delete plus restart/recovery on real KVM/Cloud Hypervisor.

Include at least one crash/restart scenario where an operation is replayed after a durable acceptance boundary.

## Acceptance criteria

- Exactly one durable authority accepts lifecycle mutations.
- Production legacy handlers route through Core before provider side effects.
- Compatibility state is derived or crash-consistent.
- Required audit/idempotency/version metadata is durable.
- Crash/fault-injection matrix passes.
- Concurrent duplicate and conflict tests pass.
- Real-KVM lifecycle/restart/replay evidence passes.
- Architecture/spec/guards describe the production path, not a transitional unwired adapter.
- Dead mutation paths are removed only after compatibility no longer needs them.

## Forbidden outcomes

- adding a parallel `cellhvd`;
- leaving legacy handlers as a second authority behind a feature flag;
- silently dropping unsupported legacy fields;
- writing NodeCache first and "eventually" creating a Core operation;
- making control-plane/BFF packages dependencies of Core;
- expanding to another VMM;
- calling the cutover complete from unit tests alone.

## Exit gate

Prompt 02 passes when all supported lifecycle mutations have one durable acceptance authority and real-KVM restart/replay evidence proves that crashes cannot create duplicate or divergent VM side effects.
