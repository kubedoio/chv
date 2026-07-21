# CellHV Core Journal Executor

**Status:** Implemented but deliberately unwired  
**Scope:** Runtime-neutral execution boundary after durable operation acceptance

`cellhv-core-executor` consumes the executor-only `ExecutionHandle`. It has no
Cloud Hypervisor, provider, control-plane, or transport dependency and no
production runtime implementation. `chv-agent` does not construct it yet.

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
- a Cloud Hypervisor runtime implementation;
- provider preparation and cleanup;
- T3 real-KVM qualification.

None of those pending items may be inferred from the side-effect-free executor
tests.
