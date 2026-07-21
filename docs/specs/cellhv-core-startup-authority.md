# CellHV Core Startup Authority Coordinator

Status: Phase B library and fault-injection tests; not wired into production.

## Boundary

`cellhv-core-startup` is a library used to make the future `chv-agent` startup
authority choice. It creates no daemon, database format, operation engine, VM
process, storage attachment, or network attachment. All database access passes
through `cellhv-core-operations::OperationService`; NodeCache conversion passes
through `cellhv-nodecache-migration`.

Neither `cmd/chv-agent` nor its current launch/reconcile paths call this crate.
The public `AuthorityDecision` is the explicit input for that later wiring.

## Durable Protocol

For a legacy cache with no database, the coordinator:

1. acquires the exclusive advisory lock derived by `cellhv-core-fs` from the
   NodeCache path; `NodeCache::save` uses the same lock;
2. reads the exact NodeCache bytes once and validates a complete import plan;
3. writes those bytes to a temporary archive with `create_new` and mode `0600`;
4. syncs the file, atomically renames it, and syncs the archive directory;
5. creates a `0600` migration target through `OperationService` and imports;
6. commits the explicit checksum-bound cutover marker;
7. returns `ActivateCore` and releases the lock.

A restart accepts an existing archive only when its SHA-256 matches the exact
source. It can complete a matching, synced temporary archive after a returned
error and reopen. An imported marker is resumed only with the matching source
and archive. A cutover marker is irreversible through this API. A database left
after an interrupted create is resumed only if every authority and journal table
is empty and the source is still present. These in-process fault hooks do not
simulate power loss or prove persistence on a specific filesystem.

The bounded filesystem trust model requires each immediate cache, archive, and
database parent to pre-exist as a real, service-owner-owned `0700` directory.
The parents may differ. Authority files must be regular, service-owner-owned,
owner-only files. Symlinks, special files, multi-link files, and aliases between
configured paths or derived archive-temporary, lock, and SQLite sidecar paths
fail closed using lexical/normalized and existing-inode checks.
Directory-fd-relative `openat` hardening remains appropriate if this trust
boundary is later broadened.

## Fail-Closed Decisions

| NodeCache | Core database | Marker | Result |
|---|---|---|---|
| absent | absent | n/a | `InitializeFreshCore`; only the shared exclusion lock is created |
| valid | absent | n/a | archive, import, cut over, `ActivateCore` |
| absent | present | none | `ActivateCore` for a native Core database |
| absent | present | none plus migration archive | error: interrupted migration source missing |
| absent | present | imported | error: imported source missing |
| absent | present | cutover | verify owner-only archive checksum, then `ActivateCore` |
| valid | present | none and pristine | resume create-before-import crash |
| valid | present | none and non-pristine | error: unrelated competing authority |
| matching | present | imported | verify archive, complete cutover |
| matching | present | cutover | verify archive, `ActivateCore` |
| changed | present | imported/cutover | checksum mismatch; never activate JSON |
| malformed or unsupported | import/resume path | absent/pristine | parser error; no new database or import |
| archive mismatch | any migration path | any | archive error; no import/cutover |

Future production wiring must treat `ActivateCore` as an exclusion rule: it
must not construct a VM-writable NodeCache. The retained JSON and archive are
compatibility/recovery evidence only.

ADR-019 proposes the pending fresh-host identity and process-wide compatibility
mode policy. `InitializeFreshCore` does not authorize a placeholder identity or an
in-memory/fallback authority. Production wiring must persist either a valid
fresh seed or a one-time generated UUID before publishing the authority. It
must select `core-vm-authority` only after that durable creation succeeds.
