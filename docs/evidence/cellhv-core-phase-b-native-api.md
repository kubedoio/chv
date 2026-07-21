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
| No production behavior change | `chv-agent` does not start this listener and no inert enable flag exists |
| Contract stability | Checked-in `cellhv-core-api-v1.json` snapshot is parsed and asserted by tests |
| Focused API tests | `cargo test -p cellhv-core-api`: 9 passed, including fail-closed HTTP 503 after authority shutdown |
| Cross-surface composition | `cargo test -p chv-agent-core native_and_legacy_adapter_share_one_operation_journal`: one native create and one translated legacy start enter the same handle and are read from one journal; the test lives above the API dependency boundary |
| Architecture enforcement | Guard scans every production Rust module in `cellhv-core-api`, requires a router `AuthorityHandle`, rejects private database actors and direct, aliased, or qualified `OperationService` use, and forbids upward `chv-agent-core` dependencies including dev dependencies |

Residual risk: the router is not exposed in production until NodeCache cutover,
legacy routing, stale socket ownership, and clean shutdown are implemented and
tested together. The current evidence is T1 only; it is not real-KVM evidence.
