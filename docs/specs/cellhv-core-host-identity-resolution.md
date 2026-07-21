# CellHV Core Host Identity Resolution

Status: Phase B library component; not wired into production startup

## Boundary

`cellhv-core-startup` owns the pure host identity resolver proposed by ADR-019.
It introduces no daemon, store, journal, API, provider, or VM lifecycle path.
The resolver performs no filesystem access. Its optional initializer delegates
fresh creation to the existing `cellhv-core-operations::OperationService`,
which remains the only application-service path to the Core store.

Production `cmd/chv-agent` does not call this API yet. The library therefore
does not satisfy the Phase B startup or NodeCache authority-mode gates.

## Inputs and precedence

`HostIdentityInputs` accepts four already-classified optional sources, in
descending precedence:

1. validated existing Core identity;
2. validated importable NodeCache identity;
3. configured fresh seed;
4. successful pre-creation enrollment identity.

Configuration parsing must represent an unset configured value as `None`.
Supplying an empty string is invalid; the resolver never silently turns a
present malformed value into absence.

The first present source is authoritative only when every other present source
matches it exactly. A mismatch is a typed conflict. All present identities are
validated before precedence is applied, so an invalid lower-precedence value
cannot be ignored. `unknown`, `unset`, `none`, and `null` are rejected after
trimmed, case-insensitive comparison. Valid opaque identity bytes otherwise
remain unchanged.

If no source is present, the resolver calls its generator exactly once. The
production convenience function supplies UUID v4; tests inject a deterministic
generator. The generated result receives the same reserved/empty validation.

## Decisions and persistence

The result distinguishes:

- `UseExistingCore`, preserving its complete identity and resource version;
- `UseImportableNodeCache`, preserving the exact imported identity;
- `InitializeFresh`, with source `ConfiguredSeed`,
  `PrecreationEnrollment`, or `Generated` and resource version 1.

The `InitializeFresh` payload is an opaque `FreshHostIdentity`: callers may
inspect its identity and source but cannot construct or modify its private
fields. Only the resolver issues this authorization capability. The store also
revalidates version 1 and reserved-ID invariants at its trust boundary.

Resolution alone never creates a path. `create_fresh_authority` refuses the
two non-fresh decisions and calls `OperationService::create_new` only for
`InitializeFresh`. It does not remove, replace, or fall back from an existing
database. The caller must acquire the runtime lease and complete the broader
ADR-019/startup decision before invoking it.

Configuration loading must represent an omitted identity as `None`. A supplied
empty or whitespace-only value is not an omission and fails closed as an
invalid identity; the resolver never normalizes it into permission to generate
a replacement identity.

`OperationService::create_new` delegates to the failure-atomic store
initializer. Schema and the version-1 host row commit together in a private
sibling SQLite file. Only after validation, WAL checkpoint, and file fsync is
that file published at the authority path with a no-replace atomic rename,
followed by parent-directory fsync. Fresh creation requires a real,
effective-user-owned 0700 parent and a UTF-8 path before any filesystem
mutation. Concurrent creation cannot replace the winner, and a pre-publication
failure cannot expose an identity-empty store. Once rename publishes the
complete database it is never unlinked on a later fsync or reopen error: the
returned error is an ambiguous-success result, and restart inspection resolves
the durable outcome from the preserved, complete authority file.

## Verification model

Tests enumerate all 81 combinations of absent, `host-a`, and `host-b` across
the four sources and compare the result with the precedence model. Focused
tests additionally cover every pairwise conflict, reserved values at every
source, empty raw inputs, invalid generated values, exact generator call count,
no filesystem mutation during resolution or rejected initialization, explicit
fresh creation, and stable identity after store reopen.
