# Phase B Native API Evidence

Date: 2026-07-21

This evidence covers the bounded native API contract skeleton, not the Phase B
exit gate and not VM lifecycle execution.

| Claim | Evidence |
|---|---|
| One mutation authority | `router(AuthorityHandle)` accepts the shared authority handle; the private `DbActor`, its unbounded channel, and direct `OperationService` construction path have been removed |
| Durable create/update/delete acceptance | Router calls `AuthorityHandle::submit`; service/store tests cover idempotency and resource-version conflicts |
| Truthful lifecycle reporting | Capability defaults are false and start/stop/reboot return HTTP 422 before journaling |
| Platform-neutral reads | Contract tests cover host/capability, VM, operation, and event response paths |
| Nonblocking ordered persistence | Every handler calls the bounded `AuthorityHandle`; the shared actor executes service calls on its named authority thread, not Tokio workers |
| Fail-closed local socket | Bind tests require an exact owner-owned `0700` parent, owner-only `0600` socket, and existing path refusal |
| Bounded production composition | Commit `e4448a6c` starts this listener only when `authority_mode = "core-native"`; omitted/default `legacy` mode retains its existing gRPC/runtime path |
| Contract stability | Checked-in `cellhv-core-api-v1.json` snapshot is parsed and asserted by tests |
| Focused API tests | `cargo test -p cellhv-core-api`: 9 passed, including fail-closed HTTP 503 after authority shutdown |
| Cross-surface composition | `cargo test -p chv-agent-core native_and_legacy_adapter_share_one_operation_journal`: one native create and one translated legacy start enter the same handle and are read from one journal; the test lives above the API dependency boundary |
| Architecture enforcement | Guard scans every production Rust module in `cellhv-core-api`, requires a router `AuthorityHandle`, rejects private database actors and direct, aliased, or qualified `OperationService` use, and forbids upward `chv-agent-core` dependencies including dev dependencies |

The later `core-native` process harness adds process-level evidence for socket
startup/cleanup, durable definition identity, idempotent replay, restart, and
single-process exclusion. It deliberately has no VM executor or provider
dependencies: start, stop, and reboot remain HTTP 422 and cannot cause a host
side effect.

Residual risk: native and legacy requests do not yet enter one production
operation engine. Native mode refuses live NodeCache state; the default legacy
gRPC path still uses its existing runtime authority. Therefore
`AGENT-CORE-002` through `AGENT-CORE-005`, Phase C lifecycle/recovery, and all
real-KVM claims remain open. The router's original tests are T1 evidence; the
standalone process harness is not T2/T3 infrastructure qualification.
