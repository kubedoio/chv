# Comprehensive Review Report — CHV Codebase

## Review Profile
- **Date**: 2026-04-17
- **Branch reviewed**: main (sprint-1-completion)
- **Architecture**: Wave 0 (10 per-package agents) + Wave 1 (11 foundation agents)
- **Total agents**: 21
- **Total findings (deduplicated)**: ~103

## Findings Summary

| Severity | Found | Fixed | Deferred |
|----------|-------|-------|----------|
| CRITICAL | 13 | 7 | 6 |
| HIGH | 25 | 12 | 13 |
| MEDIUM | ~30 | 0 | ~30 |
| LOW | ~35 | 0 | ~35 |

## Fixes Applied (this branch)

### Security (CRITICAL)
1. **JWT secret validation**: `load_controlplane_config` now rejects the insecure default and secrets <32 chars
2. **TLS fallback removed**: gRPC server now returns fatal error instead of silently falling back to plaintext
3. **chv-nwd.service capabilities**: Added `CAP_NET_ADMIN`/`CAP_NET_RAW` and `/run/netns` write access
4. **controlplane.toml**: Added required `jwt_secret` placeholder
5. **agent.toml**: Changed default `control_plane_addr` from `http://` to `https://`

### Documentation/Config (CRITICAL + HIGH)
6. **node_modules/ removed from git**: Added to `.gitignore`, removed 151 MB of tracked vendor files
7. **dist/ added to .gitignore**: Build output directory now ignored
8. **nwd.toml phantom fields removed**: Removed `bridge_name`, `bridge_cidr`, `upstream_iface` (silently ignored at runtime)
9. **chv-agent.service**: Added missing `chv-stord.service` and `chv-nwd.service` dependencies
10. **chv-nwd.service**: Removed redundant `ExecStartPre` mkdir

### Type Safety (HIGH)
11. **NodeId/ResourceId trim-on-create**: `string_id_newtype!` now trims whitespace before storing
12. **Generation leading-zero rejection**: `FromStr` now rejects `"007"` (only canonical decimal accepted)

### Code Quality (HIGH)
13. **clippy manual_clamp**: Replaced `.min(200).max(1)` with `.clamp(1, 200)` across 6 handlers
14. **clippy manual_div_ceil**: Replaced manual formula with `.div_ceil()` across 6 handlers
15. **list_tasks pagination**: Fixed SQL injection pattern (format! → bound params) and total count (rows.len → COUNT(*))
16. **Events stable ORDER BY**: Added `event_id DESC` as secondary sort key to prevent page duplication/skipping
17. **Dead code suppressed**: `MaintenanceNodeRow.reason` annotated `#[allow(dead_code)]`
18. **Unused import removed**: `warn` import in container.rs (now using `error!`)
19. **Config test fixed**: Replaced legacy non-SQLite DB URL with `sqlite://`, added jwt_secret to test fixture

## Deferred Findings (FIX IN FOLLOW-UP)

### Security (must address before production)
- admin/admin seeded in migration — generate random password at install time
- Certificate rotation endpoint has no mTLS authentication
- All BFF read endpoints lack authentication
- Admin routes lack authentication
- create_vm/delete_vm ignore JWT claims for requested_by
- JWT role field never checked (RBAC phantom)
- JWT 7-day lifespan with no revocation
- No rate limiting on login endpoint

### Silent Failures
- expose_service fire-and-forget (`let _ = ...await`)
- detach_volume ignores close_volume failure
- flush_pending_messages loses messages after first failure
- Enrollment continues after cache save fails
- Overview handler swallows all DB errors with unwrap_or(0)
- Stub handlers return 200 OK in production
- HealthAggregator unconditional Bootstrapping→HostReady
- Cache mutex held across async I/O

### Architecture
- Inverted dependency: chv-controlplane-service → chv-webui-bff
- Duplicated NodeState enum in two crates
- chv-errors depends on proto-generated chv-stord-api
- BFF contains 46 raw sqlx queries bypassing store layer

### ADR Compliance
- Missing iSCSI/Ceph storage backends (mandatory MVP-1)
- Missing partition VM-creation gate (ADR-006)
- Missing /tests/ directory, chvctl command, chv-state crate

## Verification
- `cargo build --workspace`: SUCCESS (0 errors, 0 warnings)
- `cargo test --workspace`: SUCCESS (227 tests, 0 failures)
- `cargo clippy --workspace`: SUCCESS (0 warnings)
