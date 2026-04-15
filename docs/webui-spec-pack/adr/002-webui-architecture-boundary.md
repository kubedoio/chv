# ADR-002 WebUI Architecture Boundary

## Status
Accepted

## Context
The UI needs stable contracts and must not inherit internal backend complexity directly.

## Decision
The WebUI uses SvelteKit as the frontend application and talks only to a control-plane-facing HTTP/BFF service.

### Allowed path
Browser -> WebUI/BFF HTTP API -> control plane services -> node services

### Forbidden paths
- Browser -> `chv-agent`
- Browser -> `chv-stord`
- Browser -> `chv-nwd`
- Browser -> Cloud Hypervisor
- Browser -> internal gRPC APIs directly in MVP-1

## BFF responsibilities
- session/auth integration
- view-model shaping
- pagination/filtering/search
- task polling or streaming for UI updates
- safe mutation endpoints for operator actions

## Consequences
Pros:
- browser remains decoupled from internal service topology
- easier API evolution
- better security and auth boundary

Cons:
- requires a maintained adapter layer
- some backend data is denormalized into view models
