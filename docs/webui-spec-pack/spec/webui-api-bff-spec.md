# WebUI API / BFF Spec

## Purpose
Provide browser-safe, session-aware, view-model-oriented APIs for the WebUI.

## Rules
- browser never calls internal node services directly
- browser never calls Cloud Hypervisor
- BFF can aggregate control-plane data into UI-optimized responses
- mutations map to task-creating backend operations

## Endpoint groups
- session/auth
- overview dashboard
- nodes
- virtual machines
- volumes
- networks
- tasks
- events
- maintenance
- settings/access

## Response style
- stable resource IDs
- explicit pagination metadata
- explicit filter metadata
- task references after every mutation
- human-readable state labels plus machine state fields

## Mutation response rule
Every mutation response must include:
- accepted/rejected state
- task id
- target resource id
- operator-visible summary
- next recommended refresh/stream path

## Streaming / live updates
MVP-1 options:
- polling first
- server-sent events later
- WebSocket optional later

Recommendation:
- polling first for simplicity
- SSE later for tasks/events if needed
