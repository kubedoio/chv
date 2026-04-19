# Task Plan: CHV Gap Remediation Sprint 3

## Goal

Close remaining production gaps: hardcoded login, CPU/memory inventory, dead console code, stale docs.

## Tasks

### T1: Replace hardcoded admin/admin login with DB + bcrypt (P1 security)
- `crates/chv-webui-bff/Cargo.toml` — add `bcrypt = { workspace = true }`
- `crates/chv-webui-bff/src/handlers/auth.rs` — query users table, bcrypt::verify
- Migration 0008_users.sql already exists with seeded admin + bcrypt hash
- Claims use actual user_id/username/role from DB row
- Tests: valid login, wrong password, unknown user

### T2: Implement CPU/memory host probing (P2 correctness)
- `crates/chv-agent-core/src/inventory.rs:46-47` — currently returns 0/0
- Use `num_cpus` crate for CPU count, parse `/proc/meminfo` for memory
- Graceful fallback to 0 on unsupported platforms

### T3: Remove dead BootLogViewer code (P3 cleanup)
- `ui/src/lib/api/client.ts` — remove `getBootLogs` method
- `ui/src/lib/bff/types.ts` — remove `boot_logs` field

### T4: Update serial-console-todo.md (P3 docs)
- Mark JWT validation + exp enforcement as done (sprint 2)

## Status
**Currently in Phase 1** - Starting T1
