# ADR-0005: Runtime VM disks are raw in MVP-1

## Status
Accepted

## Context
qcow2 is mature, but raw runtime disks provide a simpler and lower-risk path for online resize and operational consistency in MVP-1.

## Decision
- source images may be imported as qcow2 or raw
- imported images are normalized to raw
- runtime-attached VM disks are raw

## Consequences
### Positive
- simpler online resize path
- less runtime disk complexity
- cleaner support matrix

### Negative
- less flexible image-layer features at runtime
- potentially larger storage footprint
