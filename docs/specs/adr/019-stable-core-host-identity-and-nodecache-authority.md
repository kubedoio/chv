# ADR-019: Stable Core Host Identity and NodeCache Authority Modes

## Status

Proposed

## Date

2026-07-21

## Implementation status

This ADR is proposed because its production composition is incomplete. The
machine-readable policy records the intended invariants and their implementation
state; it does not assert that `chv-agent` enforces the complete decision.

Implemented now:

- the Core store persists one host identity and does not expose an identity
  replacement operation;
- `cellhv-core-startup` resolves existing Core, importable NodeCache,
  configured-seed, and pre-creation enrollment identities with strict
  precedence and conflict checks, and generates a UUID only when all supplied
  sources are absent;
- its explicit fresh initializer accepts only a resolved fresh decision and
  delegates the sole filesystem mutation to `OperationService::create_new`;
- NodeCache import preserves its source ID exactly and rejects the reserved
  placeholders in this ADR before producing an import plan;
- migration markers bind the source checksum, imported host ID, VM manifest,
  and irreversible cutover state;
- the startup coordinator validates migration checksum/archive evidence, but
  remains an unwired library;
- an unwired `NodeCacheAuthority` facade owns one cache, models legacy, Core,
  and blocked authority modes, freezes the VM-authoritative projection in Core
  mode, and guards every current VM mutator and whole-cache save.

Not implemented now:

- production startup selection or invocation of the resolver/initializer in
  `cmd/chv-agent`;
- identity-preserving enrollment or a separate Controller binding;
- production construction of the authority-mode facade, process-wide mode
  selection, and exclusion of direct mutable `NodeCache` access;
- production handler/reconciler exclusion in `core-vm-authority` and `blocked`.

The repository currently contradicts the proposed end state in known, visible
ways: `cmd/chv-agent/src/main.rs::initial_node_id` returns the literal
`unknown`; `load_or_initialize_cache` replaces non-`NotFound` load failures
with a fresh cache; enrollment assigns the Controller-issued ID directly to
`NodeCache.node_id`; and `AgentServer` plus `Reconciler` still construct and
mutate VM-authoritative cache state. These paths must be changed together with
production cutover. They are not evidence that this ADR is implemented.

## Context

ADR-016 makes `chv-agent` the single CellHV Core runtime, and ADR-017 makes
Core the single VM mutation and recovery authority. Phase B therefore needs an
unambiguous host identity when neither a Core database nor an importable
`NodeCache` exists. It also needs an explicit boundary for the retained JSON
cache after Core cutover.

The current agent behavior is not a valid Core initialization policy. It uses
the configured `node_id` when present, otherwise constructs the literal
`unknown`, and an enrollment response may later replace that value. It also
starts with an empty cache after any cache-load error. Those behaviors predate
durable Core authority and would permit placeholder identity, identity change,
or silent identity regeneration.

Linux `/etc/machine-id` is stable on a correctly provisioned host but is often
copied in images and reveals a host identifier outside CellHV's ownership. It
is not sufficient evidence that an absent or unreadable Core database is safe
to replace.

`NodeCache` also contains both VM-authoritative fields and compatibility-only
agent fields. Retaining the latter for enrollment and control-plane telemetry
must not leave JSON as an independent VM authority after cutover.

## Decision

### Core host identity

1. A Core host has one opaque, non-empty, non-placeholder `HostId`. The values
   `unknown`, `unset`, `none`, and `null`, compared case-insensitively after
   trimming, are reserved and invalid for Core authority.
2. An existing validated Core database is authoritative. Its host ID is never
   replaced from configuration, `NodeCache`, Controller enrollment, hostname,
   or machine identity.
3. An importable pre-cutover `NodeCache` supplies its exact existing `node_id`.
   The importer does not rewrite it. A reserved placeholder cannot be imported
   as Core identity.
4. `AgentConfig.node_id` is a fresh-initialization seed and a subsequent
   consistency assertion. It is not an override. If another authoritative
   source exists, a non-empty configured value must match it exactly or startup
   fails closed.
5. With no Core database and no importable identity, a valid configured
   `node_id` is persisted transactionally before the authority handle or API is
   published.
6. If no valid configured seed exists, `chv-agent` generates one random UUID
   once, persists it in the newly created Core database in the same startup
   transaction, syncs the database according to the local-store contract, and
   only then publishes the authority. Generation is initialization, not a
   recovery fallback. Any existing unreadable, corrupt, empty, or incompatible
   database blocks initialization and is never replaced.
7. `/etc/machine-id`, DMI identifiers, MAC addresses, hostname, and Controller
   reachability are not Core host-ID sources. They may be reported as inventory
   or used by a future explicit duplicate-host diagnostic, but never generate,
   regenerate, or silently reconcile Core identity.
8. Controller enrollment must preserve the already selected Core host ID.
   Before Core creation, a successful legacy enrollment may provide the fresh
   seed. After Core creation, enrollment must either return the identical ID or
   use a separately modelled external binding that does not replace `HostId`.
   The current enrollment request cannot propose an existing ID and the current
   cache has no distinct binding field. Therefore enrolling an already
   initialized standalone Core with a differently issued ID fails explicitly
   until an identity-preserving protocol or binding record is implemented.
9. The Controller's record is a projection/binding, not another local host
   identity authority. Certificate rotation and Controller loss never change
   Core `HostId`.

### NodeCache compatibility authority modes

Every running agent selects exactly one of these modes before constructing VM
mutation handlers:

| Mode | Durable condition | JSON VM fields | Core VM mutations | Service behavior |
|---|---|---|---|---|
| `legacy-vm-authority` | no cutover marker and production has deliberately retained the legacy path | readable and writable under the existing cache lock | unavailable | existing compatibility behavior only; no native VM mutation claim |
| `core-vm-authority` | fresh Core initialization or validated cutover marker | retained evidence is readable; VM fields are immutable | all legacy and native VM mutations enter the shared Core operation engine | normal Core authority |
| `blocked` | corrupt, ambiguous, conflicting, unsupported, or identity-mismatched state | no VM writes | no mutations or host effects | fail readiness and require explicit recovery |

The mode is process-wide and is not selected per VM or per transport.

In `core-vm-authority` mode:

- VM fragments, VM generations, desired power state, VM attachment projections,
  and VM removal state in JSON cannot be mutated;
- Core SQLite is the only source for accepted VM definitions and requested
  state;
- compatibility-only fields may remain writable, including enrollment
  material, certificate metadata, observed connectivity/telemetry data, and
  queued outbound reports;
- lifecycle-gating `node_state`, including `Draining` and `Maintenance`, is
  frozen with the VM-authoritative projection and cannot transition in Core
  mode until those effects enter the durable Core operation journal;
- a whole-cache save is allowed only if the VM-authoritative projection is
  byte-for-byte equivalent to the frozen post-import projection; otherwise it
  fails before replacing the cache file;
- the cutover marker is irreversible through normal startup. A restart cannot
  select `legacy-vm-authority` after a validated cutover;
- legacy and native transports share one authority handle. Neither transport
  may select or bypass the cache mode.

`blocked` is a safety state, not an empty-state fallback. It must not start VM
reconciliation, call Cloud Hypervisor, call attachment providers for a VM
mutation, bind a mutating native API, or overwrite either persistence source.
Read-only diagnostics may be exposed if they cannot be mistaken for readiness.

## Startup decision table

| Core database | NodeCache identity/state | Configured `node_id` | Enrollment result | Decision |
|---|---|---|---|---|
| valid | absent, or compatibility-only cache whose valid `node_id` exactly matches the durable Core ID, with no migration marker/archive | empty or exact match | absent or exact match | use durable Core ID; `core-vm-authority` |
| valid | compatibility-only cache whose `node_id` is invalid or differs from the durable Core ID | any | any | `blocked`; retained-cache identity conflict |
| valid | source absent, with cutover marker and matching durable archive | empty or exact match | absent or exact match | use durable Core ID; `core-vm-authority` |
| valid | source absent, with uncut imported marker or missing/mismatched archive | any | any | `blocked`; incomplete or unverifiable migration |
| valid | retained VM cache with matching imported host ID, source checksum, manifest, and cutover marker | empty or exact match | absent or exact match | use durable Core ID; retain cache as frozen evidence; `core-vm-authority` |
| valid | retained VM cache without a proven matching cutover marker, or with different identity/checksum/manifest | any | any | `blocked`; competing or ambiguous authority |
| valid | any otherwise consistent cache | different valid configured value | any | `blocked`; configuration conflict |
| valid | any otherwise consistent cache | empty or exact match | different issued ID | `blocked`; enrollment identity conflict |
| absent | importable, valid non-placeholder ID | empty or exact match | absent or exact match | import exact ID, cut over, then `core-vm-authority` |
| absent | importable, valid non-placeholder ID | different valid value | any | `blocked`; migration identity conflict |
| absent | importable, valid non-placeholder ID | empty or exact match | different issued ID | `blocked`; enrollment identity conflict |
| absent | placeholder ID with any VM-authoritative or unsupported data | any | any | `blocked`; operator recovery required |
| absent | placeholder, compatibility-only empty cache | valid configured seed | absent or exact match | durably archive/retire compatibility evidence, then create Core with seed; `core-vm-authority` |
| absent | no importable identity | valid configured seed | absent or exact match | create and sync Core with seed; `core-vm-authority` |
| absent | no importable identity | valid configured seed | different valid pre-creation enrollment ID | `blocked`; do not choose between two proposed identities |
| absent | no importable identity | empty/placeholder | successful pre-creation enrollment with a valid ID | create and sync Core with issued ID; `core-vm-authority` |
| absent | no importable identity | empty/placeholder | absent or unavailable | generate and persist one UUID; `core-vm-authority`; later enrollment is subject to identity preservation |
| absent | any | any | invalid or placeholder issued ID | `blocked`; invalid enrollment identity |
| unreadable, corrupt, empty, or incompatible | any | any | any | `blocked`; never create a replacement database |
| absent | unexpected archive exists, or cache/archive evidence disagrees | any | any | `blocked`; preserve all evidence |

An empty legacy cache containing only a placeholder is not authoritative proof
of identity. Production wiring must archive or explicitly retire it without
silently discarding enrollment material. If it contains any VM-authoritative
or otherwise unsupported data, it is not eligible for fresh initialization.

## Consequences

### Positive

- standalone startup no longer depends on Controller availability;
- host identity survives restart and cannot silently follow image or hostname
  changes;
- legacy enrollment remains possible before first Core initialization;
- post-cutover JSON remains useful without becoming a second VM database;
- ambiguous recovery fails before VM host effects.

### Negative

- enrolling an already initialized standalone host requires a protocol update
  or explicit external binding if the Controller would issue a different ID;
- current `NodeCache` callers require a process-wide authority facade before
  production cutover;
- placeholder legacy caches require an explicit recovery path;
- generating a UUID requires a carefully fsynced create-new startup sequence.

## Rejected alternatives

### Use `/etc/machine-id` as Core identity

Rejected because image cloning and reprovisioning can duplicate or change it,
and because OS identity is not proof that Core persistence may be recreated.

### Let Controller enrollment replace Core identity

Rejected because Controller state is a projection and Controller availability
must not control local identity.

### Keep NodeCache writable beside Core after cutover

Rejected because two independently mutable representations can disagree and
violate ADR-016 and ADR-017.

### Regenerate identity when the database cannot be opened

Rejected because corruption or permission failure is not evidence of a new
host and replacement could orphan running workloads.

## Acceptance conditions

- a static policy declaration records the source precedence, reserved values,
  machine-ID prohibition, enrollment rule, and three cache modes;
- startup tests cover every decision-table row and prove durable ID stability
  across restart;
- corruption and identity conflicts fail without creating or replacing files;
- post-cutover tests cover every NodeCache VM mutator and whole-cache save;
- transport tests prove legacy and native requests share one authority handle;
- a subprocess test proves restart never reactivates JSON VM authority after
  cutover.
