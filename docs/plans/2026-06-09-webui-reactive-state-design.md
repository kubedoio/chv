# WebUI Reactive State & Real-Time Updates — Design

## Context

The CHV web UI (`ui/`) uses two independent data caches that do not communicate:

1. **Sidebar inventory store** (`inventory.svelte.ts`) — holds nodes and VMs for the left navigation tree. Loaded once on mount. Only refreshed when lifecycle actions are triggered from the sidebar itself.
2. **Page data cache** (`api-cache.svelte.ts`) — holds list/detail data for each route. Refreshed via SvelteKit `invalidateAll()` and `invalidatePattern()` after mutations.

When a user creates a VM from `CreateVMModal.svelte`, the modal calls `invalidatePattern('vms:')` and `invalidateAll()`. This refreshes the VM list page, but the sidebar inventory store still holds stale data. The left panel does not show the new VM until the user reloads the page or triggers a sidebar action.

The backend already exposes an SSE endpoint at `/v1/tasks/stream` that polls the `operations` table every 3 seconds and streams task completions (`CreateVm`, `StartVm`, `DeleteVm`, etc.). The frontend does not consume this endpoint.

## Goals

1. **Immediate self-action sync** — After any user-initiated CRUD operation, all affected UI surfaces (sidebar, list page, detail page, pinned VMs) update without manual refresh.
2. **Background change detection** — Changes from other users, CLI tools, or background automation are detected and reflected in the UI within seconds.
3. **Unified source of truth** — Eliminate the split between sidebar inventory and page data caches. All UI surfaces read from the same reactive state layer.
4. **Desktop-app feel** — The UI feels alive. State is always current. No stale data visible to the user.

## Non-Goals

- Mobile push notifications
- Offline support or optimistic conflict resolution
- Rewriting the API client layer (both legacy REST and BFF clients stay)
- Full WebSocket bidirectional event bus (Phase 3, out of scope for this design)
- Replacing SvelteKit `load` functions with client-only data fetching

## Architecture

### Three-Phase Rollout

| Phase | Focus | Backend Changes | Frontend Changes | Timeline |
|-------|-------|-----------------|------------------|----------|
| 1 | Mutation wiring | None | Standardize post-mutation refresh | ~1 day |
| 2 | Task-stream reactivity | None | Consume SSE `/v1/tasks/stream` | ~2-3 days |
| 3 | Unified liveState | Extend SSE or add lightweight polling | Merge inventory + cache into one store | ~3-4 days |

---

## Phase 1 — Mutation Wiring (Immediate Relief)

### Problem
Today, mutation success handlers are inconsistent. Some call `invalidateAll()`, some call `invalidatePattern()`, some call both, and almost none call `inventory.fetch()`. The sidebar is never told to refresh.

### Solution
Create a centralized `liveState.invalidateAndRefresh()` utility that every mutation calls on success.

```ts
// lib/stores/live-state.svelte.ts
export const liveState = {
  async invalidateAndRefresh(opts: {
    patterns?: string[];      // e.g. ['vms:', 'nodes:']
    sidebar?: boolean;        // also refresh inventory
    detailId?: string;        // also refresh this specific resource
    delayMs?: number;         // optional second invalidate after delay
  }) {
    if (opts.patterns) {
      for (const p of opts.patterns) invalidatePattern(p);
    }
    if (opts.sidebar) {
      await inventory.fetch();
    }
    if (opts.detailId) {
      await this.refreshDetail(opts.detailId);
    }
    await invalidateAll();
    if (opts.delayMs) {
      setTimeout(() => {
        if (opts.patterns) {
          for (const p of opts.patterns) invalidatePattern(p);
        }
        invalidateAll();
      }, opts.delayMs);
    }
  }
};
```

### Adoption
Replace every ad-hoc post-mutation refresh with this utility:

| Component | Current | New |
|-----------|---------|-----|
| `CreateVMModal.svelte` | `invalidatePattern('vms:')` + `invalidateAll()` | `liveState.invalidateAndRefresh({ patterns: ['vms:'], sidebar: true })` |
| `DeleteVMModal.svelte` | `invalidatePattern('vms:')` + `invalidateAll()` | `liveState.invalidateAndRefresh({ patterns: ['vms:'], sidebar: true, delayMs: 2000 })` |
| `VmDetailActions.svelte` | `invalidatePattern('vms:')` + `invalidateAll()` + delayed second invalidate | `liveState.invalidateAndRefresh({ patterns: ['vms:'], sidebar: true, detailId: vmId, delayMs: 2000 })` |
| `CreateNetworkModal.svelte` | `invalidatePattern('networks:')` + `invalidateAll()` | `liveState.invalidateAndRefresh({ patterns: ['networks:'], sidebar: true })` |
| `VmSnapshots.svelte` | `invalidatePattern('vms:')` + `invalidateAll()` | `liveState.invalidateAndRefresh({ patterns: ['vms:'], sidebar: true, detailId: vmId })` |
| `CreateVMTemplateModal.svelte` | ad-hoc `loadData()` | `liveState.invalidateAndRefresh({ patterns: ['templates:'], sidebar: true })` |
| `ImportImageModal.svelte` | `invalidateAll()` | `liveState.invalidateAndRefresh({ patterns: ['images:'], sidebar: true })` |
| `images/+page.svelte` (delete) | `invalidateAll()` | `liveState.invalidateAndRefresh({ patterns: ['images:'], sidebar: true })` |

### Sidebar Lifecycle Actions
The sidebar already calls `inventory.fetch()` after its own lifecycle actions. Keep this, but also route through `liveState.invalidateAndRefresh()` so detail pages refresh when a VM is started from the sidebar context menu.

---

## Phase 2 — Task-Stream Reactivity (Background Change Detection)

### Problem
Phase 1 only handles changes initiated by the current user. If another user creates a VM, or a CLI tool deletes a node, the UI stays stale indefinitely.

### Solution
Consume the existing SSE endpoint `/v1/tasks/stream` and build a lightweight event bus.

#### Backend Endpoint (already exists)

```
GET /v1/tasks/stream?resource_ids=vm-1,vm-2&resource_kinds=vm,node
```

Returns SSE events every 3 seconds:
```json
{
  "items": [
    {
      "task_id": "op-123",
      "status": "Completed",
      "summary": "CreateVm",
      "resource_kind": "vm",
      "resource_id": "vm-new-456",
      "event_unix_ms": 1752067200000
    }
  ]
}
```

#### Frontend: Task Stream Store

```ts
// lib/stores/task-stream.svelte.ts
class TaskStreamStore {
  private eventSource: EventSource | null = null;
  private seenTasks = new Set<string>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  status: 'idle' | 'connecting' | 'open' | 'error' = $state('idle');

  connect(resourceKinds?: string[]) {
    const url = new URL('/v1/tasks/stream', window.location.origin);
    if (resourceKinds?.length) {
      url.searchParams.set('resource_kinds', resourceKinds.join(','));
    }

    this.eventSource = new EventSource(url.toString(), { withCredentials: true });
    this.status = 'connecting';

    this.eventSource.onopen = () => { this.status = 'open'; };
    this.eventSource.onerror = () => {
      this.status = 'error';
      this.scheduleReconnect();
    };
    this.eventSource.onmessage = (e) => {
      const payload = JSON.parse(e.data);
      for (const item of payload.items) {
        if (!this.seenTasks.has(item.task_id)) {
          this.seenTasks.add(item.task_id);
          this.handleTaskUpdate(item);
        }
      }
    };
  }

  private handleTaskUpdate(task: TaskUpdate) {
    // Only act on terminal states
    if (!['Completed', 'Failed', 'Cancelled'].includes(task.status)) return;

    const pattern = `${task.resource_kind}s:`;
    liveState.invalidateAndRefresh({
      patterns: [pattern],
      sidebar: true,
      detailId: task.resource_id,
    });
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, 10000);
  }

  disconnect() {
    this.eventSource?.close();
    this.eventSource = null;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.status = 'idle';
  }
}

export const taskStream = new TaskStreamStore();
```

#### Connection Lifecycle

- **Connect** when the user logs in (in `+layout.svelte` or `SidebarNav.svelte` `onMount`).
- **Disconnect** on logout or page unload.
- **Filter** by `resource_kinds=vm,node,network,image,volume` to avoid noise from unrelated operations.
- **Deduplicate** by `task_id` — the SSE stream returns the last 30 seconds of operations, so the same task may appear across multiple ticks.

#### UX Impact

With this in place:
- User A creates a VM → User B's sidebar updates within ~3 seconds.
- A background task completes (e.g., image import) → the Images list refreshes automatically.
- A CLI tool deletes a node → the node tree updates automatically.

---

## Phase 3 — Unified liveState Layer (Eliminate Cache Drift)

### Problem
Even with Phases 1 and 2, the system still maintains two separate caches (`inventory` and `api-cache`). They can drift if one refetch fails or if timing is off.

### Solution
Merge both into a single `liveState` reactive store.

```ts
// lib/stores/live-state.svelte.ts
class LiveState {
  // Sidebar data
  nodes = $state<NodeListItem[]>([]);
  vms = $state<VmListItem[]>([]);
  pinnedVms = $derived(this.vms.filter(v => v.status === 'running').slice(0, 3));
  vmsByNode = $derived(groupBy(this.vms, 'node_id'));

  // Page caches (replaces api-cache.svelte.ts)
  private cache = new Map<string, { data: unknown; ts: number }>();
  private readonly LIST_TTL = 30_000;
  private readonly DETAIL_TTL = 60_000;

  // Fetch methods
  async fetchInventory() { /* calls BFF listNodes + listVms */ }
  async fetchList(key: string, fetcher: () => Promise<unknown>) { /* with TTL */ }
  async fetchDetail(key: string, fetcher: () => Promise<unknown>) { /* with TTL */ }

  // Invalidation
  invalidate(pattern: string) {
    for (const key of this.cache.keys()) {
      if (key.startsWith(pattern)) this.cache.delete(key);
    }
  }

  invalidateAll() {
    this.cache.clear();
  }

  // Reactive refresh — called by mutations and task stream
  async invalidateAndRefresh(opts: { ... }) {
    // Phase 1 logic, but operating on this unified store
  }
}

export const liveState = new LiveState();
```

### Migration Path

1. Create `live-state.svelte.ts` alongside existing stores.
2. Update `SidebarNav.svelte` and `NavInfrastructureTree.svelte` to read from `liveState` instead of `inventory`.
3. Update `api-cache.svelte.ts` to delegate to `liveState` (or replace it page by page).
4. Once all pages use `liveState`, delete `inventory.svelte.ts` and `api-cache.svelte.ts`.

### Polling Fallback

The SSE task stream only covers operations that create task records. Some state changes (e.g., node health transitions, VM metrics updates) may not generate tasks. Add lightweight polling to `liveState`:

```ts
// In live-state.svelte.ts
$effect(() => {
  const interval = setInterval(() => {
    liveState.fetchInventory();
  }, 10_000);
  return () => clearInterval(interval);
});
```

This is identical to the pattern already used in `dashboard.svelte.ts`.

---

## Data Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   User Action   │     │  Background/CLI │     │  Other User     │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Mutation API   │     │  Orchestrator   │     │  Mutation API   │
│   (BFF/REST)    │     │   (task worker) │     │   (BFF/REST)    │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         │              ┌────────┴────────┐              │
         │              │  operations DB  │              │
         │              └────────┬────────┘              │
         │                       │                       │
         │                       ▼                       │
         │              ┌─────────────────┐              │
         │              │ /v1/tasks/stream│              │
         │              │   (SSE, 3s)     │              │
         │              └────────┬────────┘              │
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                     liveState.invalidateAndRefresh()             │
│  • invalidatePattern('vms:')  • invalidatePattern('nodes:')    │
│  • inventory.fetch()          • detail refresh                  │
│  • invalidateAll()                                              │
└─────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Unified Reactive Store                       │
│         ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│         │   Sidebar   │    │  List Page  │    │ Detail Page │  │
│         │   (tree)    │    │  (table)    │    │  (panel)    │  │
│         └─────────────┘    └─────────────┘    └─────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| SSE connection drops | Auto-reconnect with 10s backoff. Show subtle "Reconnecting..." indicator in sidebar footer. |
| SSE returns 401/403 | Disconnect, redirect to login. |
| Mutation succeeds but refresh fails | Toast warns user: "Action succeeded, but the view may be stale. Refresh manually if needed." |
| Task stream fires but resource fetch fails | Log error, skip update for this tick. Next tick or manual refresh will recover. |
| Duplicate task_id from SSE | Deduplicated by `seenTasks` Set. |
| Browser does not support EventSource | Fall back to 10s polling of `GET /v1/tasks` (list endpoint). |

---

## Testing & Verification

### Unit Tests
- `task-stream.svelte.ts` — mock EventSource, verify deduplication, verify correct `liveState` calls per task type.
- `live-state.svelte.ts` — verify cache TTL, verify invalidate pattern matching, verify `invalidateAndRefresh` sequencing.

### Integration Tests
- Create VM → assert sidebar contains new VM within 5s.
- Delete VM from detail page → assert sidebar no longer shows VM within 5s.
- Start VM from sidebar → assert detail page status changes within 5s.

### Manual Checklist
- [ ] Create VM from modal → sidebar updates immediately
- [ ] Delete VM from list page → sidebar updates immediately
- [ ] Start VM from detail page → sidebar status dot changes
- [ ] Create network → sidebar count updates
- [ ] Import image → images list refreshes automatically when task completes
- [ ] Open two browser tabs → action in Tab A reflects in Tab B within ~5s
- [ ] Disconnect network → SSE reconnects, UI resumes updating
- [ ] Logout → SSE disconnects cleanly

---

## Rollout Sequence

### Phase 1: Mutation Wiring
1. Create `live-state.svelte.ts` with `invalidateAndRefresh()` helper.
2. Audit every mutation handler in modals and pages.
3. Replace ad-hoc refresh logic with `liveState.invalidateAndRefresh()`.
4. Verify all CRUD flows manually.

### Phase 2: Task Stream
1. Create `task-stream.svelte.ts` with EventSource wrapper.
2. Connect in `SidebarNav.svelte` on mount, disconnect on destroy.
3. Wire `handleTaskUpdate` to call `liveState.invalidateAndRefresh()`.
4. Add sidebar footer connection status indicator.
5. Test with two browser sessions.

### Phase 3: Unified Store
1. Port `inventory.svelte.ts` data into `live-state.svelte.ts`.
2. Port `api-cache.svelte.ts` logic into `live-state.svelte.ts`.
3. Update all components to read from `liveState`.
4. Add 10s inventory polling fallback.
5. Remove `inventory.svelte.ts` and `api-cache.svelte.ts`.
6. Run full manual checklist.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| SSE connection thrashing under load | Add 10s reconnect delay. Use `seenTasks` deduplication to avoid redundant fetches. |
| Flashing UI from rapid refetch | Batch invalidations within a 500ms window. Use Svelte 5 runes for smooth transitions. |
| Scope creep into full WebSocket rewrite | Strictly limit to existing SSE endpoint. Document Phase 3 boundary. |
| Template page `loadData()` pattern is incompatible | Refactor template page to use standard `invalidatePattern` + `invalidateAll` before Phase 1. |
| Safari / corporate proxy blocks SSE | Detect EventSource failure and fall back to 10s polling of `POST /v1/tasks`. |

---

## Success Criteria

- [ ] Creating a VM updates the sidebar tree within 2 seconds (no manual refresh).
- [ ] Deleting a VM removes it from the sidebar tree within 2 seconds.
- [ ] Power actions (start/stop/restart) update both detail page and sidebar within 2 seconds.
- [ ] Changes from a second browser session appear in the first session within 5 seconds.
- [ ] No manual page refresh is required to see current state after any operation.
- [ ] SSE connection status is visible to the user (connected / reconnecting).
- [ ] `inventory.svelte.ts` and `api-cache.svelte.ts` are replaced by `live-state.svelte.ts`.
