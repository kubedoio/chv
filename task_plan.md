# Task Plan: Sprint 9 — P1 Gap Closure + Templates

## Goal

Close the highest-impact P1 gaps (VM templates, firewall rules, metrics) and wire the 7 pre-built but unused UI components to real backends.

## Phases
- [x] Phase 1: Research (gap analysis + component inventory)
- [x] Phase 2: Plan approach
- [ ] Phase 3: Implement
- [ ] Phase 4: Verify and deliver

## Tasks

### T1: VM Templates — DB + Backend + Wire UI (P1, high leverage)
The templates UI page is FULLY BUILT (imports, CRUD, create-from-template flow) but calls
APIs that don't exist. This is the highest-leverage item in the entire gap analysis.
- Migration 0013: CREATE TABLE vm_templates (template_id, name, description, cpu, memory_bytes, disk_size_bytes, image_id, cloud_init_userdata, network_id, created_by, created_at, updated_at)
- Migration 0013: CREATE TABLE cloud_init_templates (template_id, name, description, content, created_by, created_at, updated_at)
- BFF handlers: list_vm_templates, create_vm_template, delete_vm_template, list_cloud_init_templates, delete_cloud_init_template
- Routes: /v1/vm-templates, /v1/vm-templates/create, /v1/vm-templates/delete, /v1/cloud-init-templates, /v1/cloud-init-templates/delete
- UI: Already built. Just needs the backend to respond.

### T2: Firewall Rules — DB + Backend + Wire UI (P1)
FirewallRuleEditor.svelte is built. Needs:
- Migration 0014: CREATE TABLE firewall_rules (rule_id, network_id, direction, action, protocol, port_range, source_cidr, description, priority, created_at)
- BFF handlers: list_firewall_rules (by network_id), create_rule, delete_rule
- Routes: /v1/networks/firewall-rules, /v1/networks/firewall-rules/create, /v1/networks/firewall-rules/delete
- Wire FirewallRuleEditor into network detail page

### T3: Storage Pools — DB + Backend + Wire UI (P1)
CreateStoragePoolModal.svelte is built. The UI already calls listStoragePools().
- Migration 0015: CREATE TABLE storage_pools (pool_id, node_id, name, backend_class, total_bytes, used_bytes, status, created_at, updated_at)
- BFF handlers: list_storage_pools, create_storage_pool
- Routes: /v1/storage-pools, /v1/storage-pools/create
- Wire into storage page

### T4: Wire Metrics Components (P1)
VMMetricsWidget, VMMetricsHistory, MetricsChart, Sparkline, TopResourceConsumers,
NodeHealthDashboard — all built, none wired.
- Add basic metrics BFF handler returning node/VM resource usage from observed_state tables
- Wire VMMetricsWidget into VM detail page
- Wire NodeHealthDashboard into node detail page
- Wire TopResourceConsumers into overview page

### T5: User Management UI Page
Sprint 8 added the backend (list/create/update/delete users). Add the UI page.
- New route: /settings/users
- User list table with role badges
- Create/edit/delete buttons
- Follow the mockup from gap-analysis-mockups.html

## Decisions Made
- Templates use two separate tables (vm_templates + cloud_init_templates) matching the UI's existing two-tab design
- Firewall rules are per-network (not per-VM) matching the network detail page context
- Storage pools are the admin view of storage backends per node
- Metrics use existing observed_state data (no Prometheus yet — that's sprint 12)

## Status
**Currently in Phase 3** - Starting T1
