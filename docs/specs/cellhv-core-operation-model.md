# CellHV Core Operation Model

**Status:** Proposed  
**Date:** 2026-07-21  
**Authority:** ADR-016 and ADR-017  
**Phase:** B, slice 2 - transport-neutral mutation application service

## 1. Purpose and boundary

`cellhv-core-operations` is the single transport-neutral application service
for mutations accepted by the `chv-agent` CellHV Core authority. It depends on
the platform-neutral Core types and the one Core store. It has no dependency on
the control plane, a network protocol, Cloud Hypervisor, KVM, `chv-stord`,
`chv-nwd`, libvirt, or an OpenStack model.

This slice durably accepts desired state and manages operation journal state.
It does not execute a VM command, call a provider, inspect a process, prepare an
attachment, change observed state, or route any production API. A future
executor must consume its durable journal; it must not create another operation
authority.

## 2. Submission envelope

The internal `SubmitMutation` envelope contains:

- a caller-selected operation identifier;
- a non-empty idempotency scope and non-empty idempotency key;
- the expected VM resource version;
- exactly one closed `MutationCommand`: create, update, delete, start, stop, or
  reboot.

The durable request is canonical JSON containing both the command and expected
resource version. Its fingerprint is the lowercase SHA-256 digest of that
canonical representation. The store retains the canonical request, fingerprint,
operation, accepted resource version, and `(scope, key)` mapping together.
Fingerprint identity therefore includes mutation content and concurrency
precondition, not the transport encoding or caller-proposed operation ID.

## 3. Replay before state inspection

Submission computes the canonical request and resolves `(scope, key)` before
reading current VM state. An exact replay returns the original operation and
accepted resource version even when the first acceptance changed the VM version
or a later accepted delete tombstoned the VM. Reuse of the same scoped key with
a different fingerprint or canonical request fails as an idempotency conflict.

Only a previously unseen key proceeds to desired-state validation and the
atomic store transaction. This ordering is required: reading current state
first would incorrectly reject a valid retry after the original mutation had
already committed.

## 4. Desired-state acceptance

Acceptance performs no external side effect. It atomically records the desired
VM definition or delete tombstone, resource-version reservation, accepted
operation, idempotency mapping, and `operation.accepted` event.

- Create requires expected version 1, definition version 1, and unknown
  observed power state.
- Update requires definition version `expected + 1`, preserves observed power
  state, and cannot change requested power state. Attachment topology updates
  are explicitly unsupported in this slice.
- Start and stop change only requested power state and advance the resource
  version. Reboot sets requested state to running while reserving a new
  version.
- Delete records a tombstone and retains operation history.
- Stale-version and validation failures leave no operation, mapping, event, or
  version reservation.

## 5. State transitions and retry bound

The implemented operation transition graph is:

```text
accepted -> running(token T) -> succeeded (fenced by T)
                             -> failed (fenced by T)
                             -> unsupported (fenced by T)
accepted --------------------> unsupported
```

An accepted operation starts with attempt count zero and a fixed maximum of
three attempts. `claim_attempt` commits `running`, increments the attempt count,
persists a caller-supplied attempt token, and emits `operation.running` before
any future executor may perform an external side effect. Replaying the same
token is idempotent and does not increment the attempt count. A different token
cannot claim a running operation. Success, failure, and post-claim unsupported
outcomes compare-and-set against the active token, so a stale worker cannot
finish another worker's attempt. Terminal operations reject every claim.

Success and failure require a prior running claim. Unsupported may be recorded
from accepted or running because capability rejection need not perform a side
effect. A terminal write commits exactly one status and correlated event:
`operation.succeeded`, `operation.failed`, or `operation.unsupported`. Success
may have a canonical result and no error; failed and unsupported require a
canonical error and have no result. Terminal states are immutable.

## 6. Restart classification

On restart, the store returns only accepted and running operations in stable
`(accepted_at, operation_id)` order. The application service classifies them:

| Durable state | Classification | Meaning |
|---|---|---|
| accepted | `Ready` | no attempt was claimed |
| running, attempts below maximum | `Retryable` | ownership inspection may plan a bounded recovery; direct execution is forbidden |
| running, attempts at maximum | `RetryBudgetExhausted` | recovery must choose and persist a terminal policy |
| any terminal state | `Terminal` | classifier result for an explicitly supplied operation; terminal rows are omitted from the incomplete list |

Classification does not itself retry, inspect runtime reality, mark failure,
or assume whether a prior side effect occurred. Re-adoption and ambiguous
outcome policy belong to Phase C recovery and must use ownership evidence.
No API currently supersedes an active attempt token. Such an API may be added
only with the Phase C ownership-inspection proof in the same bounded change.

## 7. Dependencies and authority guards

The crate may depend only on `cellhv-core-types`, `cellhv-core-store`, and
serialization/error utilities. The architecture guard rejects dependencies on
control-plane crates, cloud-platform models, runtime/provider crates, and a
second durable authority. Protocol adapters must construct the submission
envelope and call this service; they must not issue SQL or implement their own
journal.

## 8. Explicitly pending

- production native or legacy API routing and authorization;
- durable execution-step population and executor scheduling;
- Cloud Hypervisor lifecycle calls and observed-state updates;
- process recovery, ownership transitions, and ambiguous-side-effect policy;
- NodeCache import and single-authority cutover;
- storage and network provider execution;
- backup/restore and T2 disposable-host evidence;
- libvirt, OpenStack, and O3K compatibility qualification.

The Phase B exit gate, T2 acceptance, runtime execution, and compatibility
claims remain open regardless of these T1 operation tests.
