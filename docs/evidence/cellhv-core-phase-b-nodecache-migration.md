# Phase B NodeCache Migration Evidence

Date: 2026-07-21

Scope: deterministic compatibility importer only. Production startup and VM
runtime behavior are unchanged.

## Implemented

- strict version-1 cache and legacy VM-spec parsing;
- stable host, VM, attachment, and resource-version mapping;
- SHA-256 source identity and idempotent migration marker;
- atomic import into the existing Core store;
- separate, checksum-bound cutover;
- pre-cutover rollback with journal and exact import-manifest drift guards;
- explicit rejection of malformed and non-lossless data;
- no daemon, database, operation engine, provider, or VMM dependency.

## Verification

```text
cargo test -p cellhv-nodecache-migration -p cellhv-core-store
  cellhv-nodecache-migration: 5 passed
  cellhv-core-store: 18 passed

cargo clippy -p cellhv-nodecache-migration -p cellhv-core-store --all-targets -- -D warnings
  passed

full workspace check, strict clippy, and tests
  928 passed, 0 failed, 3 documented release/environment-dependent ignores
```

The fixtures cover stable identity/checksum mapping, representative defaults,
multi-VM preflight atomicity, malformed input, unrepresentable input, exact
replay, changed-source conflict, marker tampering on reopen, imported and
cutover reopen, rollback drift, cutover, repeated cutover, and refusal to roll
back after cutover.

## Residual Risk

This evidence does not satisfy the Phase B production cutover gate. No startup
coordinator archives the source or switches write authority yet, and populated
legacy fields without a lossless Core representation fail explicitly. Keeping
the component unwired and rejecting unsupported data prevents dual authority
and silent data loss while the remaining routing and compatibility work is
implemented.
