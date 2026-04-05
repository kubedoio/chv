# ADR-0007: Migration is a validated-matrix feature, not a blanket platform promise

## Status
Accepted

## Context
Migration behavior depends on host, storage, runtime version, and workload characteristics.

## Decision
Expose migration only on explicitly validated combinations.
Deny migration automatically when prerequisites are not met.

## Consequences
### Positive
- honest product behavior
- fewer unsafe operations
- easier support policy

### Negative
- narrower marketing story
- less feature parity language
