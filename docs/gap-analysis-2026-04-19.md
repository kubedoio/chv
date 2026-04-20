# CHV Platform Gap Analysis

Generated: 2026-04-19
Overall completeness: **~56%** (58/104 features)

## Completeness by Area

| Area | Score | Complete | Partial | Stub | Missing |
|------|-------|----------|---------|------|---------|
| VM Lifecycle | 75% | 7 | 2 | 1 | 2 |
| Storage | 38% | 2 | 5 | 1 | 5 |
| Networking | 47% | 2 | 5 | 3 | 5 |
| Nodes | 91% | 8 | 1 | 1 | 0 |
| Images | 50% | 2 | 1 | 3 | 0 |
| Auth | 33% | 1 | 1 | 1 | 3 |
| Monitoring | 33% | 3 | 1 | 0 | 5 |
| Operations | 67% | 3 | 2 | 0 | 1 |
| Quotas | 0% | 0 | 1 | 1 | 2 |

## Priority Matrix — What to Build Next

### P0: Ship-blocking (must have for first real deployment)

1. **User Management** — can't add users, change passwords, or manage roles
   - DB: users table exists
   - Backend: only login handler, no CRUD
   - UI: no user management page
   - Effort: S (CRUD on existing table)

2. **Image Delete** — imported images can't be removed
   - DB: table exists
   - Backend: no handler
   - UI: no button
   - Effort: S

3. **Cloud-init Support** — VMs boot without configuration
   - DB: needs cloud_init_data column or table
   - Backend: pass cloud-init to CH via --cloud-init flag
   - UI: editor exists (CloudInitEditor.svelte) but not wired
   - Effort: M

### P1: Enterprise-expected (competitors all have these)

4. **VM Snapshots** — no way to save VM state before changes
   - DB: new snapshots table
   - Proto: new SnapshotVm/RestoreSnapshot RPCs
   - Backend: CH API supports snapshotting
   - UI: snapshot button on VM detail
   - Effort: M

5. **Volume Snapshots** — no storage point-in-time recovery
   - DB: new volume_snapshots table
   - Backend: storage backend snapshot support
   - UI: snapshot button on volume detail
   - Effort: M

6. **Firewall Rules** — no network security enforcement
   - DB: new firewall_rules table
   - Backend: nwd integration with nftables
   - UI: FirewallRuleEditor.svelte exists but not wired
   - Effort: M

7. **VM Migration** — can't move VMs between nodes
   - DB: target_node_id column exists
   - Proto: implied in desired_state
   - Backend: orchestrator needs migration workflow
   - UI: migrate action on VM detail
   - Effort: L

8. **Metrics + Prometheus** — no performance visibility
   - DB: none needed (time-series)
   - Backend: expose /metrics endpoint
   - UI: metrics page with graphs
   - Effort: M

### P2: Differentiation (what makes CHV better than Proxmox)

9. **API Tokens** — programmatic access without user passwords
10. **Backup Jobs** — scheduled VM/volume backups
11. **CLI Tool** — command-line management
12. **LDAP/SSO Integration** — enterprise auth
13. **VM Templates** — reusable VM configurations
14. **Resource Quotas** — per-user/tenant limits

### P3: Polish (nice-to-have)

15. **Audit Logging** — who did what when
16. **Dark mode for login page** — currently only main shell
17. **API Documentation** — OpenAPI/Swagger
18. **Network Policies** — advanced SDN
19. **Storage Backend Management** — Ceph/LVM/NFS UI

## Existing Code That Can Be Leveraged

| Component | Location | Status | Can Serve |
|-----------|----------|--------|-----------|
| CloudInitEditor.svelte | ui/src/lib/components/forms/ | Built, not wired | Cloud-init support |
| CloudInitPreview.svelte | ui/src/lib/components/forms/ | Built, not wired | Cloud-init support |
| FirewallRuleEditor.svelte | ui/src/lib/components/shared/ | Built, not wired | Firewall rules |
| VMExportImport.svelte | ui/src/lib/components/ | Built, not wired | Backup/export |
| NodeHealthDashboard.svelte | ui/src/lib/components/ | Built, not wired | Node monitoring |
| VMMetricsWidget.svelte | ui/src/lib/components/vms/ | Built, not wired | VM metrics |
| VMMetricsHistory.svelte | ui/src/lib/components/vms/ | Built, not wired | VM metrics history |
| MetricsChart.svelte | ui/src/lib/components/charts/ | Built | Metrics dashboards |
| Sparkline.svelte | ui/src/lib/components/charts/ | Built | Inline metrics |
| TopResourceConsumers.svelte | ui/src/lib/components/ | Built | Dashboard |
| CreateTemplateModal.svelte | ui/src/lib/components/ | Built, not wired | VM templates |
| CreateNetworkModal.svelte | ui/src/lib/components/modals/ | Built | Network creation |
| CreateStoragePoolModal.svelte | ui/src/lib/components/modals/ | Built | Storage pools |
| ImportImageModal.svelte | ui/src/lib/components/modals/ | Built | Image import |
| AddNodeModal.svelte | ui/src/lib/components/modals/ | Built | Node enrollment |

## Recommended Sprint Sequence

| Sprint | Theme | Items | Effort |
|--------|-------|-------|--------|
| 8 | User & Image management | P0: #1 user CRUD, #2 image delete, wire existing modals | S |
| 9 | Cloud-init & Templates | P0: #3 cloud-init, wire CloudInitEditor, templates | M |
| 10 | Snapshots | P1: #4 VM snapshots, #5 volume snapshots | M |
| 11 | Network security | P1: #6 firewall rules, wire FirewallRuleEditor | M |
| 12 | Monitoring | P1: #8 Prometheus metrics, wire chart components | M |
| 13 | Migration & Backup | P1: #7 VM migration, P2: #10 backup jobs | L |
| 14 | Enterprise auth | P2: #9 API tokens, #12 LDAP/SSO | M |
