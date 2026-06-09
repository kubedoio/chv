# WebUI Component Spec

## Scope

The SvelteKit frontend (`/ui`) that runs in the browser and communicates with the CHV control plane exclusively via the WebUI BFF HTTP API.

## Responsibilities

- Render resource inventory (VMs, nodes, networks, volumes, images, templates)
- Execute lifecycle actions (create, start, stop, delete, migrate, etc.)
- Display real-time task status and operation history
- Provide VM console access (VNC/serial via WebSocket)

## State Architecture

### Single Source of Truth

`liveState` (`$lib/stores/live-state.svelte.ts`) is the single source of truth for:
- Inventory state (`nodes`, `vms`, `inventoryLoading`)
- API cache (`cachedFetch`, `invalidateCachePattern`)
- Mutation-driven refresh (`invalidateAndRefresh`)

### Mutation Pattern (REQUIRED)

```svelte
<script>
  import { mutateWithRefresh } from '$lib/stores/mutation.svelte';

  async function handleDelete(vmId: string) {
    await mutateWithRefresh(
      () => deleteVm({ vm_id: vmId }, token),
      { patterns: ['vms:'], successMessage: 'VM deleted' }
    );
  }
</script>
```

**Forbidden:**
- Importing `invalidateAll` from `$app/navigation` in page components
- Importing `invalidatePattern` from `$lib/stores/api-cache.svelte`
- Calling `invalidateAll()` or `invalidatePattern()` directly

### Real-Time Sync

`taskStream` (`$lib/stores/task-stream.svelte.ts`) connects to `/v1/tasks/stream` via fetch-based SSE and calls `liveState.handleTaskCompleted()` for every terminal task. This updates the sidebar and page state without user interaction.

### Fallback

If the SSE stream fails (401, network error), `taskStream` falls back to POST polling every 10s.

## Failure Behavior

- **BFF unreachable** — pages show loading skeletons; sidebar shows "Reconnecting" status
- **Mutation fails** — `mutateWithRefresh()` shows toast error and does NOT refresh state
- **Stream disconnects** — exponential backoff reconnect (1s → 30s max)
- **Token expires** — stream stops retrying; user must re-login

## Testing

- Unit tests for `liveState`, `taskStream`, and `mutateWithRefresh`
- Compliance tests that scan all `+page.svelte` files for forbidden patterns
- E2E tests for critical flows (create VM → see it in sidebar → delete VM → sidebar updates)

## References

- ADR-004: WebUI Task and State Model
- ADR-002: WebUI Architecture Boundary
- `ui/src/lib/stores/live-state.svelte.ts`
- `ui/src/lib/stores/task-stream.svelte.ts`
- `ui/src/lib/stores/mutation.svelte.ts`
