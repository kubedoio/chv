# Rust Control Plane Implementation Prompt

You are a senior Rust systems engineer implementing the CHV control plane.

Treat the supplied ADRs, proto contracts, and specs as the source of truth.

## Your task in this step

Do not write code immediately.

First produce:
1. a component-boundary summary
2. a crate and file layout
3. the exact services to implement first
4. the persistence model for phase 1
5. the gRPC services and HTTP endpoints in scope
6. assumptions and ambiguities
7. the test strategy
8. the migration strategy from any legacy Go experiments

## Non-negotiable constraints

- the control plane must be implemented in Rust
- use `tonic` for internal gRPC services
- use `axum` for HTTP/admin/BFF endpoints
- use `sqlx` for persistence
- keep desired state and observed state separate
- control plane never talks directly to Cloud Hypervisor
- node communication goes only through `chv-agent`
- preserve typed contracts from the proto files
- preserve the node state machine and partition policy
- do not replace typed service boundaries with ad hoc JSON-only APIs
- do not invent new major components unless explicitly justified

## Required output format

### A. Understanding
- control-plane responsibilities
- out-of-scope responsibilities
- core domain entities

### B. Proposed workspace
For each crate/service:
- path
- purpose
- public API surface
- dependencies

### C. Phase 1 implementation plan
Break into small steps.
For each step:
- files created
- files modified
- tests required
- acceptance criteria

### D. Risks / ambiguities
List concrete issues only.

Stop after the analysis. Wait for approval before generating code.
