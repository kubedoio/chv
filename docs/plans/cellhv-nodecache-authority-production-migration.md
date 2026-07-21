# NodeCache Authority Production Migration Audit

Status: factual migration plan; production callers remain unchanged.

## Activation Composition

The dependency graph is acyclic: `chv-agent-core` depends on
`cellhv-core-startup`; startup depends on Core operations/types, migration, and
filesystem primitives, never on agent-core. `PendingActivatedStore` retains
the exact lock-held bytes until `AgentCoreActivation` constructs the private
facade and finishes the transaction.

Activation without a live cache returns no facade and creates no synthetic
JSON. Compatibility persistence for fresh hosts or retired imported sources
remains explicitly unresolved; recreating a file would make restart mistake it
for migration evidence. No second store or VM authority is introduced.

## Startup And Compatibility State

| Current evidence | Classification | Migration |
|---|---|---|
| `cmd/chv-agent/src/main.rs::load_or_initialize_cache` and `main` lines 247-328 | identity/startup authority | Resolve ADR-019 identity first, construct one mode/path-bound facade, and replace enrollment's direct `node_id` assignment with `apply_enrollment`. |
| `resolve_tls_paths`, `certificate_rotation_due`, and `main` lines 575-613 | compatibility certificate state | Read `compatibility_snapshot`; use `record_certificate_rotation`; persist with pathless `save`. |
| `enqueue_pending_message`, `flush_pending_messages`, `send_or_defer_control_plane_message` | compatibility outbox/connectivity | Use `enqueue_pending_message`, snapshot pending messages, `replace_pending_control_plane_messages`, and `set_connectivity_state`. |
| telemetry loops at `main.rs` lines 661-940 | mixed | Node identity/generation/error/connectivity use `host_id` and compatibility snapshot. VM, volume, and network reports must query Core projections after cutover, not frozen JSON. |

## AgentServer

| Current evidence | Classification | Migration |
|---|---|---|
| `apply_node_desired_state` lines 150-174 | compatibility node generation | `observe_node_generation`; target ID must equal the frozen host ID. |
| `apply_vm_desired_state`, `apply_volume_desired_state`, `apply_network_desired_state` lines 177-320 | VM/resource authority | Legacy mode may use guarded generation/fragment methods; Core mode translates into the shared Core operation engine and never writes JSON. |
| `create_vm`, `start_vm`, `stop_vm`, `delete_vm`, `attach_volume`, `detach_volume` lines 553-1000 | VM lifecycle/attachments | Core operation journal and provider steps own these effects. Legacy adapter uses guarded attachment, desired-state, volume-handle, and removal methods only before cutover. |
| node scheduling/maintenance methods lines 1234-1355 | lifecycle gate | `node_state` remains frozen in Core mode. Introduce journaled Core host lifecycle commands before wiring these callers; do not treat them as telemetry. |
| remaining VM/network/storage RPCs lines 1359-2135 | VM/resource lifecycle | Route through Core operations/provider steps; cache reads of handles/fragments must become Core query projections. |

## Reconciler

| Current evidence | Classification | Migration |
|---|---|---|
| `current_state`, `transition_state`, `run_once` lines 121-360 | lifecycle gate | Read Core host lifecycle state; journal transitions. Core mode must not call the facade transition method. |
| `reconcile_networks` lines 364-658 | network authority | Replace JSON fragment/generation scans with Core query projections; keep `chv-nwd` as provider only. |
| `reconcile_volumes` lines 659-826 | storage authority | Replace JSON scans with Core projections. Legacy completion patches use the dedicated snapshot/clone completion methods; Core uses journal step completion. |
| `prepare_vm_resources` lines 827-1004 | attachment authority | Core owns attachment intent and provider handles. Legacy mode uses guarded volume-handle and VM-attachment methods. |
| `reconcile_vms`, `create_one_vm`, `delete_one_vm`, `reconcile_one_vm` lines 1006-1687 | VM lifecycle authority | Read Core definitions/requested state and execute the shared journal. Remove all VM JSON mutation after cutover. |
| `cleanup_vm_resources` lines 1688-1800 | attachment cleanup authority | Read Core attachment records; persist cleanup through journal steps. Legacy mode uses `remove_volume_handle` and `remove_vm_attachment`. |

## Control Plane Helpers

`crates/chv-agent-core/src/control_plane.rs::validate_generation` currently
accepts `&NodeCache`; split it into a value-based node-generation validator and
Core resource-version validation. `flush_pending_messages` is compatibility
only and can operate through detached pending-message snapshots plus the
dedicated replacement method. Neither helper should receive a raw cache.

## Facade Coverage Added By This Audit

- immutable `host_id` access;
- identity-preserving `apply_enrollment`, with legacy ID assignment only and
  exact-match enforcement in Core mode;
- `record_certificate_rotation`;
- guarded `set_volume_handle`, `remove_volume_handle`, and
  `remove_vm_attachment`;
- guarded, structured completion of snapshot and clone fields in legacy volume
  fragments.

No additional generic cache read API is appropriate. VM/resource reads still
needed by production callers must move to typed Core query projections rather
than a detached clone of the legacy database. This is the central production
wiring dependency, not a missing NodeCache facade method.
