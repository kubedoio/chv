# Task Plan: Sprint 10 — Snapshots, Export/Import, Metrics Wiring

## Goal

Add VM snapshots (the #1 missing enterprise feature), wire the 6 pre-built metrics components, and connect VMExportImport for backup/restore capability.

## Phases
- [x] Phase 1: Research
- [x] Phase 2: Plan
- [ ] Phase 3: Implement
- [ ] Phase 4: Verify and deliver

## Tasks

### T1: VM Snapshots — Full vertical slice (P1, highest value)
Cloud Hypervisor supports snapshotting via its HTTP API:
- `PUT /api/v1/vm.snapshot` with `{"destination_url": "file:///path/to/snapshot"}`
- `PUT /api/v1/vm.restore` with `{"source_url": "file:///path/to/snapshot"}`

Implementation:
- Migration 0016: CREATE TABLE vm_snapshots (snapshot_id, vm_id, name, description, size_bytes, includes_memory, snapshot_path, status, created_at)
- Extend CloudHypervisorAdapter trait: snapshot_vm(), restore_snapshot(), list_snapshots()
- Implement in process.rs via CH HTTP API
- BFF handlers: list_vm_snapshots, create_snapshot, restore_snapshot, delete_snapshot
- Routes: /v1/vms/snapshots, /v1/vms/snapshots/create, /v1/vms/snapshots/restore, /v1/vms/snapshots/delete
- UI: Snapshots tab on VM detail page (table + create/restore/delete buttons)
- Snapshot storage: /run/chv/agent/vms/{vm_id}/snapshots/{snapshot_id}/

### T2: VM Export/Import (P1, backup capability)
VMExportImport.svelte is fully built. Needs backend:
- BFF handler: export_vm — creates a task, copies disk image to export path
- BFF handler: import_vm — accepts uploaded disk image, creates new VM
- Routes: /v1/vms/export, /v1/vms/import
- Wire VMExportImport.svelte into VM detail page

### T3: Wire Metrics Components (P1, 6 components)
All built, none connected. Need a metrics BFF endpoint:
- BFF handler: get_vm_metrics — returns CPU/memory/disk stats from vm_observed_state
- BFF handler: get_node_metrics — returns node resource stats from node_observed_state
- Wire VMMetricsWidget into VM detail Summary tab
- Wire VMMetricsHistory into VM detail (new Metrics tab)
- Wire NodeHealthDashboard into node detail page
- Wire TopResourceConsumers into overview page
- Wire MetricsChart and Sparkline as data visualization

### T4: RBAC Enforcement (P2, security)
Currently only user management checks roles. Add role checks to:
- create_vm, delete_vm, mutate_vm — require admin or operator
- create_template, delete_template — require admin or operator
- create_firewall_rule, delete_firewall_rule — require admin
- list endpoints remain accessible to all authenticated users (viewer can read)

### T5: API Tokens (P2, programmatic access)
- Migration 0017: CREATE TABLE api_tokens (token_id, user_id, name, token_hash, scope, expires_at, last_used_at, created_at)
- BFF handlers: list_tokens, create_token, revoke_token
- Token auth: accept Bearer tokens that are API tokens (not just JWTs)
- UI: API Tokens section in settings page

## Decisions Made
- Snapshots stored on local disk per VM (not centralized storage — that's Phase N)
- Export produces qcow2 files (CH native format)
- Metrics use existing observed_state tables (no Prometheus yet)
- RBAC: viewer=read, operator=read+write, admin=read+write+admin
- API tokens are SHA-256 hashed in DB, plain text shown once on creation

## Priority Order
T1 (snapshots) > T3 (metrics) > T4 (RBAC) > T2 (export) > T5 (API tokens)
T1 is the single most-requested enterprise feature. T3 has highest leverage (6 components).

## Status
**Currently in Phase 3** - Starting implementation
