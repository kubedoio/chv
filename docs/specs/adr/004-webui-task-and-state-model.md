# ADR-004 WebUI Task and State Model

## Status
Accepted

## Date
2026-04-15

## Context
Virtualization UIs become hard to trust when actions disappear into background processes or when state is ambiguous.

## Decision
The WebUI treats tasks and state as first-class UI objects.

### Rules
- every mutating operator action creates a task record
- task progress is visible at global and resource scope
- resource pages show filtered tasks relevant to that resource
- node and VM states must reflect the backend state machine, not ad hoc front-end labels
- degraded states are visible and actionable

### Reactive State Layer

All mutating actions MUST flow through `liveState.invalidateAndRefresh()` (or the `mutateWithRefresh()` wrapper). This ensures:

1. **Page cache invalidation** — SvelteKit load functions re-run
2. **Sidebar inventory refresh** — nodes and VMs re-fetched
3. **Real-time sync** — the SSE task stream calls the same path for background changes
4. **Deduplication** — each terminal task event (`Completed`, `Failed`, `Cancelled`) triggers cache invalidation exactly once, even when the same event is observed via both the SSE stream and the polling fallback, or replayed across a reconnect. The deduplication window is bounded so that long-running sessions do not accumulate unbounded state, while still tolerating the gap between disconnect and reconnect without dropping events.

#### Invariant

> **No page component may call `invalidateAll()` or `invalidatePattern()` directly.**
> Use `mutateWithRefresh()` instead.

#### Implementation

- `live-state.svelte.ts` — unified store holding inventory, cache, and invalidation logic
- `task-stream.svelte.ts` — SSE consumer (fetch-based, with Authorization header) that feeds into liveState
- `mutation.svelte.ts` — convenience wrapper for all mutating BFF calls

## Required task states
- queued
- running
- succeeded
- failed
- cancelled
- awaiting-operator-input (reserved for later)

## Required resource health states
- healthy
- warning
- degraded
- failed
- unknown

## Consequences
Pros:
- better operator trust
- better troubleshooting
- cleaner alignment with auditability
- consistent reactive state across mutations, sidebar, and real-time stream

Cons:
- requires consistent backend task persistence
- UI must handle eventual consistency carefully
- developers must use `mutateWithRefresh()`; direct invalidation is forbidden and enforced by CI
