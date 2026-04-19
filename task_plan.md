# Task Plan: CHV Gap Remediation Sprint 2

## Goal

Close remaining security and quality gaps identified by the CEO plan review on 2026-04-19.

---

## Completed (Sprint 1 — all prior T1-T4)

All original Sprint 1 tasks were implemented before this plan was created:

- [x] T1: Images schema (os/version/usage_count columns, handler queries)
- [x] T2: Network attached_vms populated (get_network joins vm_nic_desired_state)
- [x] T3: VM create + delete endpoints (POST /v1/vms/create and /v1/vms/delete)
- [x] T4: Pagination on all list handlers (page/page_size + total_count/total_pages)

---

## Sprint 2 — Security & Quality Gaps

### Completed

- [x] **P0: Fix failing test** (`cmd/chv-agent/src/main.rs:797`)
  `certificate_rotation_due_respects_interval` — created temp files via `tempfile::NamedTempFile`
  so `Path::exists()` check passes. Pure test change.

- [x] **P1: JWT validation on serial console** (`crates/chv-agent-core/src/console_server.rs`)
  Added real HS256 JWT decode with `jsonwebtoken`. Added `jwt_secret` field to `AgentConfig`.
  Extracted `validate_console_token()` as production function. Config guards reject insecure
  defaults. 5 unit tests (valid, expired, empty, malformed, wrong secret) + 2 config tests.

- [x] **P2: Quotas admin auth** (`ui/src/routes/quotas/+page.svelte`)
  Wired `isAdmin` to JWT `role` claim via shared `getStoredRole()` utility in `client.ts`.
  Admin-only "Adjust Quota" button gated by `{#if isAdmin}`. Handles base64url decoding.
  6 unit tests for `getStoredRole()`.

- [x] **P3: gRPC stub audit** (`crates/chv-agent-core/`)
  Audit found all 56 `Status::unimplemented("")` stubs are inside `#[cfg(test)]` blocks
  (test mocks), not production code. Production VM lifecycle handlers are fully implemented.
  Only production stub is `resize_volume` (agent_server.rs:762), explicitly deferred to Phase 4.
  No implementation work needed.

### Deferred

- **CPU/memory inventory probe** (`crates/chv-agent-core/src/inventory.rs:46-47`)
  Returns 0/0. Deferred until the platform needs real scheduling decisions.

---

## Verification Gate

- `cargo test --workspace` — all tests pass
- `cargo check --workspace` — clean
- `cd ui && npm run build` — no TypeScript errors

## Status

**COMPLETE** — All Sprint 2 items delivered. Branch `worktree-gap-remediation-sprint2`.
