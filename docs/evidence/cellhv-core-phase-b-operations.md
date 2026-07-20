# CellHV Core Phase B Operation Slice Evidence

**Date:** 2026-07-21  
**Phase:** B, slice 2 - transport-neutral mutation application service  
**Status:** slice implementation verified at T1; production routing and execution absent

## Evidence

| Gate | Command or artifact | Result |
|---|---|---|
| Focused operation tests | `cargo test -p cellhv-core-operations` | pass: 12 tests |
| Combined focused regression | `cargo test -p cellhv-core-operations -p cellhv-core-store -p cellhv-core-types -p chv-agent-core -p chv-config` | pass: 193 tests (12 operations, 15 store, 10 types, 147 agent, 9 config) |
| Deterministic request identity | `fingerprint_is_deterministic_and_command_sensitive` | pass |
| Replay before current-state read | replay after version reservation and tombstone tests | pass; original operation is returned |
| Idempotency conflict | changed command or expected version under the same scoped key | pass; conflict and no second acceptance |
| Atomic desired-state acceptance | create, power, stale-version, and delete tests | pass; no partial journal or version reservation on rejection |
| State transition and retry bound | `execution_transitions_are_bounded_and_terminal_is_immutable` | pass; three claims maximum and terminal single assignment |
| Restart reconstruction | stable incomplete-journal ordering and retry-boundary tests | pass |
| Architecture authority guard | `python3 -B scripts/check-cellhv-core-architecture.py` | pass |
| Architecture negative tests | `python3 -B -m unittest tests/test_cellhv_core_architecture.py` | pass: 15 tests |
| Full workspace compile/lint/regression | `cargo check --workspace`; `cargo clippy --workspace -- -D warnings`; `cargo test --workspace` | pass: 906 tests, 0 failed, 3 documented release/environment-dependent ignores |

## What the tests prove

The service creates a canonical request envelope, resolves exact replay before
reading mutable VM state, validates desired-state rules, and delegates one
atomic acceptance transaction to the sole Core store. Attempt claims are
durable before a future side effect. Terminal status and event writes are
atomic, retry counts are bounded, and terminal outcomes cannot be overwritten.
Restart reconstruction reports durable incomplete work without fabricating an
execution result.

## Non-claims and residual risk

- No production native or legacy handler calls this service.
- No provider, Cloud Hypervisor process, VM API socket, runtime directory,
  storage attachment, or network attachment is touched.
- No executor consumes claimed work; tests call transition methods directly.
- A running operation after restart is classified, not reconciled with runtime
  reality. Phase C must define re-adoption and ambiguous-outcome policy.
- No durable operation steps or ownership transitions are populated.
- NodeCache remains unchanged and no authority cutover has occurred.
- Backup/restore, T2 host evidence, and the Phase B exit gate remain pending.
- No libvirt, OpenStack, or O3K compatibility claim follows from this evidence.

This evidence is T1 coverage of the bounded application-service slice only. It
does not establish VM lifecycle execution or production readiness.
