# CellHV NodeCache Authority-Mode Facade

Status: Phase B library slice; not wired into production.

## Boundary

`chv_agent_core::NodeCacheAuthority` owns one `NodeCache`, one validated
canonical persistence path, and one authority
mode. Its public API neither mentions nor returns `NodeCache`, accepts no
caller-supplied mutation closure, and exposes no mutable accessor or
`into_inner`. While unwired, construction is crate-private.

Construction rejects unsafe cache parents/destinations using the same
owner/mode/link contract as cache persistence and binds the canonical path.
`save()` accepts no path, so callers cannot redirect persistence after mode
selection.

- `LegacyVmAuthority` permits the existing JSON-backed VM mutations.
- `CoreVmAuthority` freezes the VM-authoritative projection and permits only
  compatibility updates that leave that projection byte-for-byte equivalent.
- `Blocked` rejects mutation and persistence.

The frozen projection is private, canonicalized with `BTreeMap`, and includes
the cache schema version, host identity, node state, VM/volume/network
generations and fragments, VM attachment observations, and volume handles.
These fields determine identity, desired state, runtime lifecycle, or
attachment preparation and therefore cannot remain writable after cutover.

Every current VM-authoritative `NodeCache` mutator is represented by a guarded
facade method. Compatibility writes use dedicated methods for node generation,
enrollment/certificate metadata, pending control-plane messages, last error,
and connectivity. Reads return a detached DTO containing only compatibility
fields. Node-state transitions are denied in Core mode because draining and
maintenance currently trigger VM lifecycle behavior. `save` independently
rechecks the projection so whole-cache persistence cannot conceal drift.

The architecture guard rejects public facade signatures containing
`NodeCache` or `FnOnce`, preventing reintroduction of a raw clone/reference or
caller-controlled mutation escape. It also rejects public raw-cache fields and
aliases, custom trait signatures, associated constants/statics,
conversion/dereference traits, serialization, and authority cloning within the
facade source boundary.

## Production Integration

`AgentServer`, `Reconciler`, and startup composition still use direct
`Arc<Mutex<NodeCache>>` access. Production enforcement requires construction
of exactly one facade after startup identity/lease selection, replacement of
every direct mutable cache reference, and static guards preventing new bypass
paths. Until then, `nodecache_authority_facade_enforced` is true while
`nodecache_authority_mode_enforced` remains false.

This slice does not satisfy production authority acceptance, restart recovery,
or `AGENT-CORE-004`. It changes no VM launch, stop, delete, or recovery path.
