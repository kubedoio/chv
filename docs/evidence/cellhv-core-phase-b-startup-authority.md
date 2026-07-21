# Phase B Startup Authority Evidence

Date: 2026-07-21

Scope: library-only startup migration/cutover coordinator. Production
`chv-agent` startup and Cloud Hypervisor lifecycle behavior are unchanged.

## Machine Evidence

- `cellhv-core-startup` has returned-error/reopen fault injection after archive
  file sync, archive rename, database creation, transactional import, and
  cutover commit. It is not power-loss or filesystem durability evidence.
- Tests prove exact-byte archival, fresh-host classification, imported versus
  cutover restart classification, malformed input rejection, archive mismatch
  rejection, and checksum-bound exclusion of a changed JSON cache.
- Deterministic tests serialize concurrent coordinators and a source replacement
  under the same advisory lock used by `NodeCache::save`.
- Filesystem tests cover unsafe parents, symlink locks, special files, pairwise
  and normalized path aliases, hardlink aliases, SQLite sidecar collisions, and
  explicit `0600` archive/database modes.
- The marker query is exposed read-only through the existing
  `OperationService`; the coordinator has no direct SQLite dependency.
- The architecture guard classifies the crate as part of the existing Core
  runtime and continues to forbid `cellhvd`, extra services, forbidden
  control-plane dependencies, and additional durable stores/operation engines.

Focused verification command:

```text
cargo test -p cellhv-core-fs -p cellhv-core-startup -p cellhv-core-operations -p cellhv-core-store -p chv-agent-core
cargo clippy -p cellhv-core-fs -p cellhv-core-startup -p cellhv-core-operations -p cellhv-core-store -p chv-agent-core --all-targets -- -D warnings
python3 -B scripts/check-cellhv-core-architecture.py
python3 -B -m unittest tests/test_cellhv_core_architecture.py
```

Result: 209 Rust unit tests passed (1 filesystem, 15 startup, 12 operations, 19 store, 162 agent), focused
Clippy passed with warnings denied, the architecture script passed, and all 15
architecture unit tests passed.

## Non-Claims

This is not production cutover evidence, real-KVM evidence, recovery/re-adoption
evidence, or acceptance-test qualification. `cmd/chv-agent` does not use the
decision yet, so existing VM launch and management behavior is unchanged.
