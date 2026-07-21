# Phase B Host Identity Resolver Evidence

Date: 2026-07-21

Original slice scope: pure host identity resolution and explicit fresh-store
initialization. At the time of this slice, production startup and VM runtime
behavior were unchanged.

## Subsequent Production Wiring

Commit `e4448a6c` wired this resolver into the explicit, default-off
`core-native` startup path in `cmd/chv-agent/src/main.rs`. That path trims an
empty configured node identifier to no seed, resolves or creates the durable
Core host identity, and starts without enrollment or Controller access. The
default and omitted authority mode remains `legacy`, whose enrollment identity
path is unchanged.

## Machine evidence

- Existing Core identity has highest precedence and retains its resource
  version; importable NodeCache identity is second and remains exact.
- Configured and enrollment identities are assertions against higher sources;
  every mismatch fails before generation or filesystem access.
- Reserved placeholders fail for every source, including generated output.
- An exhaustive 81-case source matrix matches the declared precedence model.
- The injected generator is called exactly once only when all sources are
  absent.
- Pure resolution leaves the target path absent. The explicit initializer
  refuses non-fresh decisions and revalidates fresh identity version and
  reserved-value invariants at the store boundary.
- Fresh schema and host identity commit in one SQLite transaction in a private
  sibling file. The database is validated, checkpointed, file-synced, then
  published with `renameat2(RENAME_NOREPLACE)` and a parent-directory fsync.
  Thus the authority path is absent or contains a complete identity-bearing
  store; it is never exposed as an identity-empty migrated database.
- Injected host-insertion failure leaves neither the authority path nor staging
  files. Two concurrent creators produce exactly one winner and one
  `AlreadyExists` result, and the winner remains valid after reopen.
- An injected post-rename error is observed only after another handle can open
  the complete host-bearing authority. The initializer returns the error but
  preserves that published evidence for deterministic restart resolution.
- Unsafe parent permissions and non-UTF-8 paths fail before filesystem
  mutation. The architecture guard rejects public fresh-authorization fields.
- Reopening the resulting Core database returns the same host identity.

Focused verification:

```text
cargo test -p cellhv-core-startup
cargo test -p cellhv-core-store -p cellhv-core-operations -p cellhv-core-startup
cargo clippy -p cellhv-core-store -p cellhv-core-operations -p cellhv-core-startup --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Non-claims

The original unit evidence does not by itself prove production lease ordering.
The later `core-native` process acceptance harness covers resolver composition,
restart identity, and process exclusion, but does not prove enrollment
integration or legacy-to-Core identity convergence. Native mode refuses a live
NodeCache rather than importing it. Neither this evidence nor the later wiring
proves VM lifecycle, process re-adoption, host-reboot recovery, real KVM,
libvirt, or cloud compatibility.
