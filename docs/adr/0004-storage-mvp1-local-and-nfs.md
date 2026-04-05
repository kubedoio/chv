# ADR-0004: MVP-1 storage is local + NFS only

## Status
Accepted

## Context
Distributed or hyperconverged storage in MVP-1 would delay delivery and blur storage semantics.

## Decision
Support only:
- local storage pools
- NFS storage pools

DRBD and distributed storage are out of scope for MVP-1.

## Consequences
### Positive
- simpler and more explicit semantics
- lower implementation risk
- easier operational support

### Negative
- reduced mobility and HA options
- narrower feature surface than larger platforms
