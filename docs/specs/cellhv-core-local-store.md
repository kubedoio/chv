# CellHV Core Local Store Specification

**Status:** Proposed  
**Date:** 2026-07-21  
**Authority:** ADR-016 and ADR-017  
**Phase:** B, slices 1-2 - platform-neutral domain, SQLite store, and operation persistence

## 1. Purpose

This specification defines the first durable authority beneath `chv-agent`,
which is the CellHV Core runtime. It replaces neither `chv-agent` nor its Cloud
Hypervisor adapter. It introduces no VM lifecycle behavior.

The store is the only authoritative local database for CellHV-managed VM
identity and accepted configuration. The existing JSON `NodeCache` remains a
migration input and temporary compatibility cache until a separately tested
cutover. The control-plane database remains a fleet projection, not a second
node-runtime authority.

## 2. Required invariants

- Exactly one Core database is opened by `chv-agent` for a host.
- Host and VM identifiers are stable across daemon restart and host reboot.
- Every accepted mutation is atomic with its resource-version change.
- Foreign keys are enabled on every connection.
- SQLite WAL mode and an explicit busy timeout are configured before service.
- The fresh v1 schema creation is named, transactional, checksummed, and
  recorded; ordered upgrades from an older Core schema remain pending.
- An unknown newer schema version fails closed.
- Corruption or an unreadable non-empty database fails closed without mutation.
  Failure preservation for a future in-place upgrade remains to be designed
  and tested with the ordered migration path.
- Startup never replaces a failed database with an empty database.
- External clients, compatibility adapters, `chv-stord`, and `chv-nwd` never
  open this database.
- Cloud-platform, libvirt XML, Neutron, Cinder, tenant, quota, and scheduler
  models are not stored in the Core domain.

## 3. Location and ownership

The production path is selected by `chv-agent` configuration and must reside
under an agent-owned state directory. The package must create that directory
with least-privilege ownership. The database must not live in the ephemeral VM
runtime directory.

Tests use an explicit temporary path. No code may silently fall back from a
configured production path to an in-memory database or a second path.

One store handle owns connection creation. Callers use typed repository and
transaction methods; API handlers and compatibility adapters do not issue SQL.

## 4. Schema v1

The first migration creates these platform-neutral records:

| Table | Required identity and purpose |
|---|---|
| `schema_migrations` | monotonically increasing version, unique name, applied timestamp |
| `host_identity` | singleton host identifier and creation timestamp |
| `vms` | VM identifier, stable name, accepted specification, requested and observed power state, resource version, timestamps |
| `attachments` | stable attachment identifier, VM identifier, kind, provider/reference identity, requested and observed state |
| `operations` | operation identifier, kind, VM identifier, fingerprint, canonical durable request intent, state, result/error, timestamps |
| `operation_steps` | schema reserved for ordered durable steps aligned with the domain step state machine |
| `idempotency_keys` | caller scope and key mapped to one request fingerprint and operation identifier |
| `events` | ordered operation/VM-correlated event record |
| `ownership_markers` | schema aligned with the domain ownership and recovery classifications for a VM |

Identifiers are stored as validated, non-empty opaque strings so the later
NodeCache importer can preserve existing identity exactly. Compatibility
profiles that require a UUID accept only the UUID-shaped subset and must not
rewrite a Core identifier. Migration v1 stores timestamps as UTC RFC 3339 text.
Enums are closed domain values validated before storage. Accepted VM
definitions use deterministic JSON serialization; an encoding change requires
a new migration.

Required constraints include:

- unique VM identifier;
- resource version greater than zero;
- unique `(caller_scope, idempotency_key)`;
- at most one ownership marker per VM;
- explicit foreign-key deletion policies: VM deletion is tombstoned, while a
  physical parent-row deletion cascades attachments, steps, and VM ownership;
- foreign keys from attachments, steps, idempotency mappings, events, and
  ownership markers to their durable parents where applicable;
- operation and step state values restricted to implemented state machines.

The operation tables are created in slice 1 so the schema has one migration
lineage. Slice 2 routes transport-neutral mutation submissions through the
operation application service and persists accepted, running, and terminal
operation transitions. It still performs no VM lifecycle side effect and does
not populate durable execution steps or apply ownership transitions. Those
records must be written by the future executor and recovery policies that own
their state transitions, not inferred by the raw store.

## 5. Open and migration protocol

Once production wiring is introduced, `chv-agent` will open the configured file
without deleting or renaming it. The current store primitive enables foreign
keys, sets the busy timeout and WAL policy, and then runs:

1. SQLite integrity validation appropriate to startup.
2. Inspection of the migration ledger and SQLite user version.
3. Rejection of unknown or inconsistent versions.
4. For fresh v1 creation, application of the v1 migration inside an explicit
   transaction.
5. Post-migration foreign-key and schema validation.
6. Publication of a ready store handle only after all checks pass.

A new database is permitted only when the configured path does not exist and
startup policy explicitly allows initialization. The v1 `open_existing` path
is validation-only because there is no older CellHV schema; future versions
must add pending-only ordered migration before changing the latest version. A
zero-length or malformed existing file is not evidence that initialization is
safe.

Migration SQL and application code ship together. A migration is immutable
after release; corrections use a new migration. Current tests prove fresh v1
creation, repeated open, checksum/schema validation, newer-version rejection,
foreign-key enforcement, and corrupt-file failure. Ordered upgrade, failure
injection during an upgrade, and migration rollback evidence are pending until
a v2 migration path exists.

## 6. Transaction and concurrency model

All write methods execute typed commands within one SQLite transaction. The
acceptance primitive canonicalizes and durably retains the platform-neutral
request intent. It atomically commits the complete desired VM state (or delete
tombstone), resource-version reservation, scoped idempotency mapping, accepted
operation, and first correlated event. This is desired-state acceptance only;
it does not execute a VM lifecycle side effect.

The public Rust methods in this store crate are internal persistence
primitives, not a public Core API and not independent lifecycle commands.
Production handlers must eventually route mutations through the implemented
transport-neutral operation application service. That service owns request
validation, idempotent acceptance, operation transition policy, and terminal
events. Authorization, durable execution steps, ownership transitions, and
provider execution remain pending. Direct VM CRUD and terminal-operation
methods must not be exposed to external callers.

Writes use bounded waiting and return a structured busy/unavailable error after
the configured timeout. Callers do not retry indefinitely. Read methods return
owned domain values and never expose SQLite rows or connections.

Compare-and-swap updates require the caller's expected resource version. A
stale version fails before mutation and leaves no operation or partial record.

## 7. Backup, restore, and rollback

The following is the required future operational contract. Slice 1 does not
implement or test backup, restore, or release rollback tooling.

Online backup uses SQLite's consistent backup mechanism or an equivalent
checkpointed procedure, never a raw copy of an active WAL database. A backup
manifest records schema version, host ID, creation time, and digest.

Restore is an offline operator action. It validates the manifest, digest,
schema compatibility, integrity, and host identity before atomically replacing
the inactive database. The prior database is retained until the restored store
opens successfully.

Code rollback is allowed only to a binary that understands the on-disk schema.
Destructive down-migrations are not automatic. If a release cannot read the
current schema, rollback restores the pre-upgrade backup while `chv-agent`
remains stopped. Rollback must never reactivate the JSON cache as an independent
authority.

## 8. NodeCache migration boundary

This slice does not cut over existing `NodeCache` data. A later Phase B slice
must define and test:

- deterministic mapping from current node and VM identifiers;
- validation and explicit rejection of malformed cache data;
- an idempotent import marker and recorded cutover state;
- one-way authority activation with no dual writes that can diverge;
- rollback behavior that preserves a single authority;
- reconciliation of control-plane compatibility requests through the same
  operation engine.

Until that cutover, schema existence does not by itself make the database the
production VM lifecycle authority.

## 9. Security and observability

Database paths must reject traversal and unsafe ownership. Secrets are not
stored unless a later threat model and schema change explicitly require them.
Logs include migration version, operation ID, and resource ID where applicable,
but never full VM specifications, credentials, or attachment secrets.

Metrics expose bounded store health, migration version, transaction latency,
and error counts. VM IDs, operation IDs, and idempotency keys are not metric
labels.

## 10. Acceptance evidence

Phase B slices 1-2 require T1 tests for domain validation and deterministic
serialization; fresh-v1 open, constraint, atomic acceptance, reopen, checksum,
and corruption behavior; and operation replay, transition, retry, terminal,
and restart classification. T2 disposable-host evidence, backup/restore,
ordered upgrade, migration rollback, production request routing, execution,
and NodeCache cutover are pending. Passing the current tests does not claim VM
lifecycle, recovery, provider, libvirt, or OpenStack support.
