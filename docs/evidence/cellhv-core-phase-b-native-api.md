# Phase B Native API Evidence

Date: 2026-07-21

This evidence covers the bounded native API contract skeleton, not the Phase B
exit gate and not VM lifecycle execution.

| Claim | Evidence |
|---|---|
| One mutation authority | `cellhv-core-api` depends on `cellhv-core-operations`, not the store or runtime |
| Durable create/update/delete acceptance | Router calls `OperationService::submit`; service/store tests cover idempotency and resource-version conflicts |
| Truthful lifecycle reporting | Capability defaults are false and start/stop/reboot return HTTP 422 before journaling |
| Platform-neutral reads | Contract tests cover host/capability, VM, operation, and event response paths |
| Nonblocking ordered persistence | All service calls execute on one named database actor thread, not Tokio workers |
| Fail-closed local socket | Bind tests require an exact owner-owned `0700` parent, owner-only `0600` socket, and existing path refusal |
| No production behavior change | `chv-agent` does not start this listener and no inert enable flag exists |
| Contract stability | Checked-in `cellhv-core-api-v1.json` snapshot is parsed and asserted by tests |
| Focused API tests | `cargo test -p cellhv-core-api`: 8 passed |
| Full workspace regression | workspace check and strict clippy passed; tests: 928 passed, 0 failed, 3 documented release/environment-dependent ignores |

Residual risk: the router is not exposed in production until NodeCache cutover,
legacy routing, stale socket ownership, and clean shutdown are implemented and
tested together. The current evidence is T1 only; it is not real-KVM evidence.
