# Phase B Shared Authority Actor Evidence

Date: 2026-07-21

Scope: library-only serialized access to the existing Core operation service.
Production `chv-agent` request and Cloud Hypervisor lifecycle paths are
unchanged.

## Machine Evidence

- concurrent identical submissions produce one accepted operation and
  idempotent replays;
- serialized different requests sharing a key produce one operation and one
  fingerprint conflict;
- serialized mutations at one resource version produce one acceptance and one
  stale-version failure;
- an actor restart over the same database preserves operation identity and
  replay behavior;
- queue-ordered shutdown completes prior work, rejects later requests, and is
  explicitly joined;
- dropping a reply receiver after enqueue does not cancel durable acceptance;
- a gated worker proves bounded full-queue backpressure, cancellation before
  enqueue without mutation, ordered shutdown, and a nonblocked single-thread
  Tokio timer;
- dropping the owning join guard closes handles and joins the OS thread;
- the unwired legacy adapter can submit its transport-neutral intent through
  the shared actor without calling `AgentServer` or `VmRuntime`;
- the architecture guard restricts production/build dependencies of
  `cellhv-core-operations` to Core store/types and general-purpose
  serialization, error, and async crates; dev-dependencies are not part of this
  production boundary.

Focused verification commands:

```text
cargo test -p cellhv-core-operations -p chv-agent-core
cargo clippy -p cellhv-core-operations -p chv-agent-core --all-targets -- -D warnings
python3 -B scripts/check-cellhv-core-architecture.py
python3 -B -m unittest tests/test_cellhv_core_architecture.py
```

## Non-Claims

This does not wire legacy gRPC or the native API into Core, complete
`AGENT-CORE-002`, establish a process-wide singleton, execute VMM/provider side
effects, alter production VM behavior, or provide T2/T3 evidence. It is partial
T1 evidence for the future shared mutation path, idempotency, and resource
version requirements.
The existing native API database actor remains separate and must be replaced by
this handle during production wiring; this unit slice is not shared-path proof.
