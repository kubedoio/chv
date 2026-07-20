# CellHV Core Phase B Store Slice Evidence

**Date:** 2026-07-21  
**Phase:** B, slice 1 - platform-neutral domain and SQLite store  
**Status:** slice implementation verified at T1; production wiring intentionally absent

## Intended authority change

This slice adds the first durable store directly beneath `chv-agent`. It does
not activate a second runtime, database, operation engine, or VM process owner.
The database is not production VM authority until the explicit NodeCache
cutover slice passes its migration and rollback gates.

## Required evidence

| Gate | Command or artifact | Result |
|---|---|---|
| Store/domain/agent/config tests | `cargo test -p cellhv-core-types -p cellhv-core-store -p chv-agent-core -p chv-config` | pass: 181 tests (15 store, 10 types, 147 agent, 9 config) |
| Fresh v1 schema, checksum, and deterministic reopen validation | targeted `cellhv-core-store` tests | pass |
| Foreign-key and constraint enforcement | `migration_checksum_and_foreign_keys_are_enforced` | pass |
| Reopen and newer-version rejection | CRUD reopen and future-schema tests | pass |
| Corruption fails closed | corrupt and zero-byte preservation test | pass |
| Transaction rollback | rejected-event transaction test | pass; no operation, mapping, event, or version reservation remains |
| Canonical durable request intent | create acceptance and canonical terminal-outcome tests | pass; request JSON survives reopen in canonical form |
| Atomic desired-state acceptance | create, power-state, delete, idempotency, and competing-version tests | pass; desired state/tombstone, resource version, operation, mapping, and first event commit together |
| Formatting | `cargo fmt --all` followed by `cargo fmt --all -- --check` | pass |
| Focused lint | `cargo clippy -p cellhv-core-types -p cellhv-core-store -p chv-agent-core -p chv-config --all-targets -- -D warnings` | pass |
| Architecture authority guard | `python3 -B scripts/check-cellhv-core-architecture.py` | pass |
| Architecture negative tests | `python3 -B -m unittest tests/test_cellhv_core_architecture.py` | pass: 15 tests |
| Full workspace compile/lint/regression | `cargo check --workspace`; `cargo clippy --workspace -- -D warnings`; `cargo test --workspace` | pass: 906 tests, 0 failed, 3 documented release/environment-dependent ignores |

The schema and domain types also define aligned operation-step, event,
ownership, and recovery representations. Slice 1 writes the initial
`operation.accepted` event. The separately evidenced slice 2 writes running and
terminal operation transitions, but neither slice populates durable execution
steps or applies ownership/recovery transitions. Those behaviors belong to the
pending execution and recovery engines.

## Required artifact review

- migration list and exact schema version;
- configured database path and package ownership;
- open/integrity/migration failure behavior;
- typed transaction and compare-and-swap behavior;
- backup/restore compatibility statement, explicitly pending implementation;
- authority declaration updated to exactly one durable VM store;
- dependency guard still proves no control-plane or cloud-platform dependency.

## Non-scope

- no production VM launch, stop, delete, or recovery behavior change;
- no NodeCache cutover or dual-write path;
- no native API activation;
- no legacy-request operation routing;
- no operation executor or fabricated provider result;
- no provider redesign or privileged host mutation;
- no libvirt adapter, OpenStack driver, or compatibility claim;
- no second daemon, database, operation engine, or process owner.

## Rollback

Before authority cutover, the intended rollback procedure stops `chv-agent`,
restores the pre-upgrade package and configuration, and retains the new
database for diagnosis. It must not delete or reinterpret existing NodeCache
data. A future ordered upgrade must restore a verified pre-migration backup
when the prior binary cannot read the upgraded schema; automatic destructive
down-migration is forbidden.

Backup/restore tooling, ordered upgrade behavior, migration failure injection,
and rollback testing are pending. Current evidence covers only fresh v1
creation and validation-only reopen of that same schema.

## Residual risks

- NodeCache import and authority cutover are not implemented in this slice.
- Operation acceptance and idempotency are exercised by the application
  service, but no legacy or native request is routed through it yet.
- Raw store methods are internal persistence primitives; their tests are not
  evidence that direct CRUD or operation completion is a supported Core API.
- Durable operation-step execution and ownership and recovery transitions are
  pending execution/recovery-engine work.
- Real service permissions and SQLite behavior require T2 disposable-host
  evidence; unit tests alone are insufficient.
- Backup/restore and binary downgrade compatibility require implementation and
  release-lab validation.
- Phase C lifecycle, process re-adoption, and host-reboot recovery remain
  unproven.
- Libvirt and OpenStack discovery/qualification remain independent and
  incomplete.

## Acceptance disposition

No Phase B, Core lifecycle, libvirt, provider, or OpenStack acceptance scenario
is claimed by this template. The implementing change must record scenario-level
evidence and residual failures before changing this status.
