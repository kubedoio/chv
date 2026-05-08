# Task Plan: WebUI Completion + CLI Tool

## Goal
1. Fix all broken/stub WebUI functionality (backup page broken endpoints, missing live migration UI, dual API client mess)
2. Implement `chvctl` CLI tool for managing the CHV environment from the command line

---

## Phase 1: WebUI — Fix Broken Backup Page

The backup-jobs page (`ui/src/routes/backup-jobs/+page.svelte`) uses the **old** `$lib/api/client.ts` which calls non-existent endpoints:
- `client.runBackupJob(id)` → calls `/api/v1/backup-jobs/${id}/run` (DOES NOT EXIST)
- `client.toggleBackupJob(id)` → calls `/api/v1/backup-jobs/${id}/toggle` (DOES NOT EXIST)
- `client.createBackupJob(...)` → calls `/api/v1/backup-jobs` POST (WRONG: actual is `/v1/backups/jobs`)
- `client.deleteBackupJob(id)` → calls `/api/v1/backup-jobs/${id}` DELETE (WRONG path)

**Actual BFF endpoints (in router.rs):**
- `POST /v1/backup-jobs` — legacy list (page payload)
- `GET /v1/backups/jobs` — RESTful list
- `POST /v1/backups/jobs` — create
- `PATCH /v1/backups/jobs/:job_id` — update
- `DELETE /v1/backups/jobs/:job_id` — delete
- `POST /v1/backups/jobs/:job_id/execute` — run now

**Fix:** Migrate backup page from old `createAPIClient()` to new `$lib/bff/` module. Create `$lib/bff/backups.ts` with proper endpoint mappings.

- [ ] 1.1: Create `ui/src/lib/bff/backups.ts` with functions: listBackupJobs, createBackupJob, deleteBackupJob, executeBackupJob, updateBackupJob, listBackupSchedules, listBackupHistory
- [ ] 1.2: Add backup endpoints to `ui/src/lib/bff/endpoints.ts`
- [ ] 1.3: Rewrite `backup-jobs/+page.svelte` to use new BFF client
- [ ] 1.4: Add missing "toggle" functionality (enable/disable via `updateBackupJob` with `enabled: true/false`)

---

## Phase 2: WebUI — Add Live Migration UI

The VM detail page has actions (start, shutdown, poweroff, reboot, delete) but **no migrate button**. The backend has `MigrateVm` RPC fully implemented (control plane orchestrator, migration state machine, agent-side live migration). Missing: UI trigger.

**Fix:** Add a "Migrate" action to the VM detail page that:
1. Shows a modal to select the target node
2. Calls `mutateVm({ vm_id, action: 'migrate', target_node_id })` via BFF
3. Shows migration progress via the tasks/operations stream

- [ ] 2.1: Add `migrate` to allowed actions in BFF `mutate_vm` handler (verify it flows to orchestrator's MigrateVm)
- [ ] 2.2: Add Migrate button to `VmDetailActions.svelte`
- [ ] 2.3: Create `VmMigrateModal.svelte` — node selector + confirmation
- [ ] 2.4: Wire modal into VM detail page, call mutateVm with target_node_id
- [ ] 2.5: Add BFF endpoint for listing eligible migration targets (nodes with TenantReady state)

---

## Phase 3: WebUI — Additional Missing Functionality

Other stubs and gaps identified:

- [ ] 3.1: Add `deleteImage` to BFF endpoints.ts and wire delete button on images page
- [ ] 3.2: Add snapshot management to BFF client (listVmSnapshots, createSnapshot, deleteSnapshot, restoreSnapshot already exist in router but no `$lib/bff/snapshots.ts`)
- [ ] 3.3: Wire VmSnapshots component to use new BFF functions (currently uses old client)
- [ ] 3.4: Add resize VM flow (backend exists: `POST /v1/vms/resize`)

---

## Phase 4: CLI Tool — `chvctl`

No CLI tool exists. All management is through the BFF/WebUI. Need a `chvctl` command that talks to the BFF HTTP API (authenticated with JWT).

### Architecture
- New crate: `cmd/chvctl/` 
- Dependencies: `clap` (CLI framework), `reqwest` (HTTP client), `serde_json`, `tokio`
- Auth: stores JWT in `~/.config/chvctl/credentials` after `chvctl login`
- Talks to: BFF HTTP API (same endpoints the WebUI uses)

### Subcommands

```
chvctl login                        # Authenticate, store token
chvctl vm list                      # List VMs
chvctl vm get <vm_id>               # Get VM details
chvctl vm create <name> [flags]     # Create VM
chvctl vm start <vm_id>             # Start VM
chvctl vm stop <vm_id>              # Graceful shutdown
chvctl vm reboot <vm_id>            # Reboot
chvctl vm delete <vm_id>            # Delete
chvctl vm migrate <vm_id> --to <node_id>  # Live migrate
chvctl vm resize <vm_id> --cpu N --memory N  # Resize
chvctl node list                    # List nodes
chvctl node get <node_id>           # Get node details
chvctl node drain <node_id>         # Drain node
chvctl node maintenance enter <node_id>  # Enter maintenance
chvctl node maintenance exit <node_id>   # Exit maintenance
chvctl image list                   # List images
chvctl image import <name> --url <url>   # Import image
chvctl image delete <image_id>      # Delete image
chvctl volume list                  # List volumes
chvctl volume snapshot <vol_id>     # Snapshot volume
chvctl volume clone <vol_id>        # Clone volume
chvctl network list                 # List networks
chvctl network create <name> [flags]  # Create network
chvctl task list                    # List operations/tasks
chvctl task watch                   # Stream task updates (SSE)
chvctl backup list                  # List backup jobs
chvctl backup run <job_id>          # Execute backup
chvctl user list                    # List users (admin)
chvctl user create <username> [flags]  # Create user (admin)
```

- [ ] 4.1: Scaffold `cmd/chvctl/` crate with Cargo.toml, main.rs, clap derive structure
- [ ] 4.2: Implement auth module (login, token storage, auto-refresh)
- [ ] 4.3: Implement HTTP client wrapper (base URL config, auth header injection, error handling)
- [ ] 4.4: Implement `vm` subcommands (list, get, create, start, stop, reboot, delete, migrate, resize)
- [ ] 4.5: Implement `node` subcommands (list, get, drain, maintenance)
- [ ] 4.6: Implement `image` subcommands (list, import, delete)
- [ ] 4.7: Implement `volume` subcommands (list, snapshot, clone)
- [ ] 4.8: Implement `network` subcommands (list, create, delete)
- [ ] 4.9: Implement `task` subcommands (list, watch via SSE)
- [ ] 4.10: Implement `backup` subcommands (list, run)
- [ ] 4.11: Implement `user` subcommands (list, create, delete)
- [ ] 4.12: Add output formatters (table, json, yaml)
- [ ] 4.13: Add to workspace Cargo.toml, verify `cargo check --workspace`

---

## Key Decisions
- CLI talks to BFF HTTP API (not directly to gRPC control plane) — same auth, same permissions model
- Use `clap` derive macros for ergonomic CLI definition
- Use `reqwest` for HTTP (not tonic gRPC) — simpler, no proto dependency in CLI
- Output defaults to table format, `--output json` for scripting
- Branch: `feat/webui-completion-and-cli`

## Errors Encountered
- (none yet)

## Status
**Currently in Phase 1** — Fixing broken backup page
