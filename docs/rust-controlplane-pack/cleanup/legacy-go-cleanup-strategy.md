# Legacy Go Cleanup Strategy

## Goal

Clean the repository so old Go control-plane experiments do not confuse:
- architecture decisions
- current Rust implementation work
- code generation
- CI
- repository structure
- LLM coding agents

This is not only code deletion. It is a controlled cleanup.

## Cleanup principles

1. preserve anything still useful for reference
2. remove misleading half-implementations from active paths
3. prevent legacy Go experiments from looking authoritative
4. keep the repository understandable for future contributors and LLMs
5. leave a written migration trail

## Recommended cleanup model

### Keep
- prototypes that contain reusable domain insight
- docs that explain prior decisions
- test data or examples still useful for Rust implementation
- proto files and architecture docs that remain valid

### Remove or archive
- incomplete Go control-plane services
- stale Go module roots that imply the active backend is Go
- abandoned handlers, mocks, or experiments that conflict with Rust direction
- dead scripts and CI jobs for legacy Go backends
- duplicate contract definitions

### Archive path recommendation
Move legacy material into a clearly non-authoritative location such as:

```text
/legacy/go-experiments/
```

or

```text
/archive/go-controlplane-prototype/
```

Do not leave it mixed into active workspace paths.

## Required cleanup outputs

- active implementation path is obviously Rust
- legacy Go code is either archived or removed
- README reflects current architecture
- CI does not build stale Go backends accidentally
- no duplicate source-of-truth contracts remain
- repo tree is understandable in one scan

## README / docs expectation

Add a short note explaining:
- Go was used for exploratory development
- Rust is now the authoritative backend/control-plane direction
- legacy Go code is archived for reference only, if retained
