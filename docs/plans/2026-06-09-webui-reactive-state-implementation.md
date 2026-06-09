# WebUI Reactive State & Real-Time Updates — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the WebUI sidebar, list pages, and detail pages stay synchronized in real time after any CRUD operation or background change, using the existing SSE task stream.

**Architecture:** Three-phase rollout: (1) wire all mutation success handlers to refresh both page data and sidebar inventory, (2) consume the backend's existing SSE `/v1/tasks/stream` endpoint to detect background changes, (3) merge `inventory.svelte.ts` and `api-cache.svelte.ts` into a single `liveState` reactive store.

**Tech Stack:** Svelte 5 (runes), SvelteKit, TypeScript, Vitest, Playwright, SSE (EventSource), BFF REST client.

---

## Prerequisites

- The backend SSE endpoint `/v1/tasks/stream` is already deployed and working.
- `ui/` uses Svelte 5 runes and has Vitest + Playwright installed.
- All changes are scoped to `ui/src/`.

---

## Phase 1 — Mutation Wiring

### Task 1: Create `live-state.svelte.ts`

**Files:**
- Create: `ui/src/lib/stores/live-state.svelte.ts`

**Step 1: Read existing stores for reference**

Run:
```bash
cat ui/src/lib/stores/inventory.svelte.ts
cat ui/src/lib/stores/api-cache.svelte.ts
```

Expected: You see `InventoryStore` class and `cachedFetch` / `invalidatePattern` functions.

**Step 2: Write the new store**

Create `ui/src/lib/stores/live-state.svelte.ts`:

```typescript
import { browser } from '$app/environment';
import { invalidateAll } from '$app/navigation';
import { getStoredToken } from '$lib/api/client';
import { inventory } from './inventory.svelte';
import { invalidatePattern } from './api-cache.svelte';

export interface InvalidateOpts {
	patterns?: string[];
	sidebar?: boolean;
	detailId?: string;
	delayMs?: number;
}

class LiveState {
	async invalidateAndRefresh(opts: InvalidateOpts = {}) {
		if (!browser) return;

		if (opts.patterns) {
			for (const p of opts.patterns) {
				invalidatePattern(p as import('./api-cache.svelte').CacheKey);
			}
		}

		if (opts.sidebar) {
			await inventory.fetch();
		}

		await invalidateAll();

		if (opts.delayMs && opts.delayMs > 0) {
			setTimeout(() => {
				if (opts.patterns) {
					for (const p of opts.patterns) {
						invalidatePattern(p as import('./api-cache.svelte').CacheKey);
					}
				}
				invalidateAll();
			}, opts.delayMs);
		}
	}
}

export const liveState = new LiveState();
```

**Step 3: Verify it compiles**

Run:
```bash
cd ui && npx svelte-check --tsconfig ./tsconfig.json --output human 2>&1 | grep -E "error|Error" | head -20
```

Expected: Zero errors related to `live-state.svelte.ts`.

**Step 4: Commit**

```bash
git add ui/src/lib/stores/live-state.svelte.ts
git commit -m "feat(state): add liveState.invalidateAndRefresh helper"
```

---

### Task 2: Wire VM mutations

**Files:**
- Modify: `ui/src/routes/vms/+page.svelte:175-181`
- Modify: `ui/src/routes/vms/[id]/+page.svelte:103-156`
- Modify: `ui/src/lib/components/vms/DeleteVMModal.svelte:30-40`

**Step 1: Update `vms/+page.svelte`**

Read lines 175-181. Replace:

```svelte
<CreateVMModal
		bind:open={modalOpen}
		onSuccess={async () => {
			invalidatePattern('vms:');
			await invalidateAll();
		}}
	/>
```

With:

```svelte
<script>
	import { liveState } from '$lib/stores/live-state.svelte';
	// ... existing imports ...
</script>

<!-- ... existing markup ... -->

<CreateVMModal
		bind:open={modalOpen}
		onSuccess={async () => {
			await liveState.invalidateAndRefresh({ patterns: ['vms:'], sidebar: true });
		}}
	/>
```

**Step 2: Update `vms/[id]/+page.svelte`**

Find the `handleAction` function around lines 103-156. Replace every ad-hoc refresh block with `liveState.invalidateAndRefresh()`.

Before (example at ~133-137):
```typescript
invalidatePattern('vms:');
await invalidateAll();
if (['shutdown', 'poweroff'].includes(action)) {
	setTimeout(() => {
		invalidatePattern('vms:');
		invalidateAll();
	}, 2000);
}
```

After:
```typescript
await liveState.invalidateAndRefresh({
	patterns: ['vms:'],
	sidebar: true,
	detailId: vmId,
	delayMs: ['shutdown', 'poweroff'].includes(action) ? 2000 : undefined
});
```

Apply the same pattern to all action handlers in the file (start, shutdown, poweroff, restart, delete, migrate).

**Step 3: Update `DeleteVMModal.svelte`**

Read lines 30-40. Replace:
```typescript
onSuccess?.();
```

With the same pattern if it doesn't already call liveState. Actually, `DeleteVMModal` receives `onSuccess` from its parent, so the parent (`vms/[id]/+page.svelte` or `vms/+page.svelte`) should pass the liveState call. Check each usage site.

In `vms/[id]/+page.svelte`, the delete flow likely shows `DeleteVMModal` with an `onSuccess` prop. Update it to:
```svelte
<DeleteVMModal
	bind:open={deleteModalOpen}
	vm={vm}
	onSuccess={async () => {
		await liveState.invalidateAndRefresh({ patterns: ['vms:'], sidebar: true });
	}}
/>
```

**Step 4: Run svelte-check**

```bash
cd ui && npx svelte-check --tsconfig ./tsconfig.json 2>&1 | tail -5
```

Expected: No new errors.

**Step 5: Commit**

```bash
git add ui/src/routes/vms/+page.svelte ui/src/routes/vms/[id]/+page.svelte ui/src/lib/components/vms/DeleteVMModal.svelte
git commit -m "feat(state): wire VM mutations through liveState"
```

---

### Task 3: Wire Network mutations

**Files:**
- Modify: `ui/src/routes/networks/+page.svelte:220-221`

**Step 1: Update the onSuccess handler**

Replace:
```typescript
invalidatePattern('networks:');
await invalidateAll();
```

With:
```typescript
await liveState.invalidateAndRefresh({ patterns: ['networks:'], sidebar: true });
```

**Step 2: Commit**

```bash
git add ui/src/routes/networks/+page.svelte
git commit -m "feat(state): wire network mutations through liveState"
```

---

### Task 4: Wire Volume and Node detail mutations

**Files:**
- Modify: `ui/src/routes/volumes/[id]/+page.svelte:103-147`
- Modify: `ui/src/routes/nodes/[id]/+page.svelte:42-43`

**Step 1: Update volumes detail page**

Replace all `await invalidateAll()` calls with `liveState.invalidateAndRefresh({ patterns: ['volumes:'], sidebar: true })`.

**Step 2: Update nodes detail page**

Replace:
```typescript
invalidatePattern('nodes:');
await invalidateAll();
```

With:
```typescript
await liveState.invalidateAndRefresh({ patterns: ['nodes:'], sidebar: true });
```

**Step 3: Commit**

```bash
git add ui/src/routes/volumes/[id]/+page.svelte ui/src/routes/nodes/[id]/+page.svelte
git commit -m "feat(state): wire volume and node mutations through liveState"
```

---

### Task 5: Wire Template mutations

**Files:**
- Modify: `ui/src/routes/templates/+page.svelte:94-116`
- Modify: `ui/src/routes/templates/+page.svelte:202-227` (onSuccess props)

**Step 1: Create a standardized refresh function**

At the top of the `<script>` block in `templates/+page.svelte`, add:

```typescript
import { liveState } from '$lib/stores/live-state.svelte';

async function refreshTemplates() {
	await liveState.invalidateAndRefresh({ sidebar: true });
	await loadData();
}
```

**Step 2: Replace all `onSuccess={loadData}` with `onSuccess={refreshTemplates}`**

Find lines 202, 215, 227 and update each `onSuccess` prop.

**Step 3: Commit**

```bash
git add ui/src/routes/templates/+page.svelte
git commit -m "feat(state): wire template mutations through liveState"
```

---

### Task 6: Wire Image mutations

**Files:**
- Modify: `ui/src/routes/images/+page.svelte:96`
- Modify: `ui/src/routes/images/+page.svelte:115`

**Step 1: Update delete handler**

Replace `await invalidateAll()` with:
```typescript
await liveState.invalidateAndRefresh({ patterns: ['images:'], sidebar: true });
```

**Step 2: Update ImportImageModal onSuccess**

Replace:
```svelte
onSuccess={() => invalidateAll()}
```

With:
```svelte
onSuccess={async () => {
	await liveState.invalidateAndRefresh({ patterns: ['images:'], sidebar: true });
}}
```

**Step 3: Commit**

```bash
git add ui/src/routes/images/+page.svelte
git commit -m "feat(state): wire image mutations through liveState"
```

---

### Task 7: Wire Sidebar lifecycle actions

**Files:**
- Modify: `ui/src/lib/components/shell/SidebarNav.svelte:45`, `122`, `139`, `156`

**Step 1: Import liveState**

Add to imports:
```typescript
import { liveState } from '$lib/stores/live-state.svelte';
```

**Step 2: Replace direct `inventory.fetch()` calls**

Replace each `await inventory.fetch()` with:
```typescript
await liveState.invalidateAndRefresh({ sidebar: true });
```

Keep the direct `inventory.fetch()` only for the initial `onMount` load.

**Step 3: Commit**

```bash
git add ui/src/lib/components/shell/SidebarNav.svelte
git commit -m "feat(state): wire sidebar lifecycle actions through liveState"
```

---

## Phase 2 — Task Stream Reactivity

### Task 8: Create `task-stream.svelte.ts`

**Files:**
- Create: `ui/src/lib/stores/task-stream.svelte.ts`
- Create: `ui/src/lib/stores/task-stream.test.ts`

**Step 1: Write the failing test**

Create `ui/src/lib/stores/task-stream.test.ts`:

```typescript
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TaskStreamStore, type TaskUpdate } from './task-stream.svelte';

describe('TaskStreamStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
	});

	it('connects to SSE endpoint with resource_kinds filter', () => {
		const esMock = {
			close: vi.fn(),
			onopen: null as any,
			onerror: null as any,
			onmessage: null as any,
		};
		const ES = vi.fn().mockImplementation(() => esMock);
		vi.stubGlobal('EventSource', ES);

		const store = new TaskStreamStore();
		store.connect(['vm', 'node']);

		expect(ES).toHaveBeenCalledWith(
			expect.stringContaining('/v1/tasks/stream?resource_kinds=vm%2Cnode'),
			expect.anything()
		);
		expect(store.status).toBe('connecting');
		store.disconnect();
	});

	it('handles task completion and calls handler', () => {
		const esMock = { close: vi.fn(), onopen: null, onerror: null, onmessage: null } as any;
		vi.stubGlobal('EventSource', vi.fn().mockImplementation(() => esMock));

		const handler = vi.fn();
		const store = new TaskStreamStore();
		store.onTaskCompleted = handler;
		store.connect();

		esMock.onopen?.();
		expect(store.status).toBe('open');

		const task: TaskUpdate = {
			task_id: 'op-1',
			status: 'Completed',
			summary: 'CreateVm',
			resource_kind: 'vm',
			resource_id: 'vm-123',
			event_unix_ms: Date.now(),
		};

		esMock.onmessage?.({ data: JSON.stringify({ items: [task] }) });
		expect(handler).toHaveBeenCalledWith(task);

		store.disconnect();
	});

	it('deduplicates by task_id', () => {
		const esMock = { close: vi.fn(), onopen: null, onerror: null, onmessage: null } as any;
		vi.stubGlobal('EventSource', vi.fn().mockImplementation(() => esMock));

		const handler = vi.fn();
		const store = new TaskStreamStore();
		store.onTaskCompleted = handler;
		store.connect();

		const payload = JSON.stringify({
			items: [{
				task_id: 'op-1',
				status: 'Completed',
				summary: 'CreateVm',
				resource_kind: 'vm',
				resource_id: 'vm-123',
				event_unix_ms: Date.now(),
			}],
		});

		esMock.onmessage?.({ data: payload });
		esMock.onmessage?.({ data: payload });

		expect(handler).toHaveBeenCalledTimes(1);
		store.disconnect();
	});
});
```

**Step 2: Run the failing test**

```bash
cd ui && npx vitest run src/lib/stores/task-stream.test.ts 2>&1
```

Expected: FAIL with "TaskStreamStore is not defined" or "Cannot find module".

**Step 3: Implement the store**

Create `ui/src/lib/stores/task-stream.svelte.ts`:

```typescript
export interface TaskUpdate {
	task_id: string;
	status: string;
	summary: string;
	resource_kind: string;
	resource_id: string;
	event_unix_ms: number;
}

export class TaskStreamStore {
	private eventSource: EventSource | null = null;
	private seenTasks = new Set<string>();
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

	status: 'idle' | 'connecting' | 'open' | 'error' = $state('idle');
	onTaskCompleted: ((task: TaskUpdate) => void) | null = null;

	connect(resourceKinds?: string[]) {
		if (typeof EventSource === 'undefined') {
			this.status = 'error';
			return;
		}

		const url = new URL('/v1/tasks/stream', window.location.origin);
		if (resourceKinds?.length) {
			url.searchParams.set('resource_kinds', resourceKinds.join(','));
		}

		this.eventSource = new EventSource(url.toString(), { withCredentials: true });
		this.status = 'connecting';

		this.eventSource.onopen = () => {
			this.status = 'open';
		};

		this.eventSource.onerror = () => {
			this.status = 'error';
			this.scheduleReconnect();
		};

		this.eventSource.onmessage = (e) => {
			try {
				const payload = JSON.parse(e.data);
				if (!Array.isArray(payload.items)) return;

				for (const item of payload.items) {
					if (!this.seenTasks.has(item.task_id)) {
						this.seenTasks.add(item.task_id);
						if (['Completed', 'Failed', 'Cancelled'].includes(item.status)) {
							this.onTaskCompleted?.(item as TaskUpdate);
						}
					}
				}
			} catch {
				// Ignore malformed SSE messages
			}
		};
	}

	private scheduleReconnect() {
		if (this.reconnectTimer) return;
		this.reconnectTimer = setTimeout(() => {
			this.reconnectTimer = null;
			this.connect();
		}, 10_000);
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

**Step 4: Run the passing test**

```bash
cd ui && npx vitest run src/lib/stores/task-stream.test.ts 2>&1
```

Expected: All 3 tests pass.

**Step 5: Commit**

```bash
git add ui/src/lib/stores/task-stream.svelte.ts ui/src/lib/stores/task-stream.test.ts
git commit -m "feat(state): add TaskStreamStore with SSE EventSource"
```

---

### Task 9: Connect task stream to liveState

**Files:**
- Modify: `ui/src/lib/stores/live-state.svelte.ts`
- Modify: `ui/src/lib/components/shell/SidebarNav.svelte`

**Step 1: Update live-state to handle task events**

Add to `live-state.svelte.ts`:

```typescript
import { taskStream, type TaskUpdate } from './task-stream.svelte';

// Map task summaries to cache patterns
const TASK_PATTERN_MAP: Record<string, string> = {
	CreateVm: 'vms:',
	StartVm: 'vms:',
	ShutdownVm: 'vms:',
	PoweroffVm: 'vms:',
	RestartVm: 'vms:',
	DeleteVm: 'vms:',
	MigrateVm: 'vms:',
	SnapshotVm: 'vms:',
	RestoreSnapshot: 'vms:',
	CreateNode: 'nodes:',
	DeleteNode: 'nodes:',
	CreateNetwork: 'networks:',
	DeleteNetwork: 'networks:',
	CreateVolume: 'volumes:',
	DeleteVolume: 'volumes:',
	ResizeVolume: 'volumes:',
	ImportImage: 'images:',
	DeleteImage: 'images:',
	CreateVmTemplate: 'templates:',
	DeleteVmTemplate: 'templates:',
};

function handleTaskCompleted(task: TaskUpdate) {
	const pattern = TASK_PATTERN_MAP[task.summary];
	if (!pattern) return;

	liveState.invalidateAndRefresh({
		patterns: [pattern],
		sidebar: true,
		detailId: task.resource_id,
	});
}

// Auto-wire on import
if (browser) {
	taskStream.onTaskCompleted = handleTaskCompleted;
}
```

**Step 2: Start the stream in SidebarNav**

In `SidebarNav.svelte`, in `onMount`:

```typescript
import { taskStream } from '$lib/stores/task-stream.svelte';

onMount(() => {
	inventory.fetch();
	taskStream.connect(['vm', 'node', 'network', 'volume', 'image']);

	return () => {
		taskStream.disconnect();
	};
});
```

**Step 3: Commit**

```bash
git add ui/src/lib/stores/live-state.svelte.ts ui/src/lib/components/shell/SidebarNav.svelte
git commit -m "feat(state): wire task stream into liveState and SidebarNav"
```

---

### Task 10: Add connection status indicator to sidebar

**Files:**
- Modify: `ui/src/lib/components/shell/SidebarNav.svelte`

**Step 1: Import taskStream status**

```typescript
import { taskStream } from '$lib/stores/task-stream.svelte';
```

**Step 2: Add a subtle status dot in the sidebar footer**

Find the footer/nav area of `SidebarNav.svelte` (usually near the bottom). Add:

```svelte
<div class="stream-status" title={taskStream.status === 'open' ? 'Live updates connected' : taskStream.status === 'error' ? 'Reconnecting...' : 'Live updates off'}>
	<span class="status-dot" class:connected={taskStream.status === 'open'} class:error={taskStream.status === 'error'}></span>
	<span class="status-label">{taskStream.status === 'open' ? 'Live' : taskStream.status === 'error' ? 'Reconnecting' : 'Off'}</span>
</div>
```

Add scoped styles:
```svelte
<style>
	.stream-status {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.25rem 0.75rem;
		font-size: 0.7rem;
		color: var(--color-neutral-500);
	}
	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-neutral-400);
	}
	.status-dot.connected {
		background: var(--color-success);
	}
	.status-dot.error {
		background: var(--color-danger);
	}
</style>
```

**Step 3: Commit**

```bash
git add ui/src/lib/components/shell/SidebarNav.svelte
git commit -m "feat(ui): add SSE stream status indicator to sidebar"
```

---

### Task 11: Add EventSource fallback for restricted environments

**Files:**
- Modify: `ui/src/lib/stores/task-stream.svelte.ts`

**Step 1: Add polling fallback**

When `EventSource` is unavailable, fall back to polling `POST /v1/tasks` every 10 seconds.

Add a `startPollingFallback()` method to `TaskStreamStore`:

```typescript
private pollTimer: ReturnType<typeof setInterval> | null = null;

startPollingFallback(intervalMs = 10_000) {
	this.stopPollingFallback();
	this.pollTimer = setInterval(async () => {
		try {
			const token = getStoredToken();
			if (!token) return;
			const res = await fetch('/v1/tasks', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
				body: JSON.stringify({ page: 1, page_size: 50 }),
			});
			if (!res.ok) return;
			const data = await res.json();
			for (const item of data.items || []) {
				if (!this.seenTasks.has(item.task_id)) {
					this.seenTasks.add(item.task_id);
					if (['Completed', 'Failed', 'Cancelled'].includes(item.status)) {
						this.onTaskCompleted?.(item);
					}
				}
			}
		} catch {
			// Silently ignore polling errors
		}
	}, intervalMs);
}

stopPollingFallback() {
	if (this.pollTimer) {
		clearInterval(this.pollTimer);
		this.pollTimer = null;
	}
}
```

Update `connect()` to use fallback:
```typescript
connect(resourceKinds?: string[]) {
	if (typeof EventSource === 'undefined') {
		this.startPollingFallback();
		return;
	}
	// ... existing SSE logic ...
}

disconnect() {
	this.stopPollingFallback();
	// ... existing disconnect logic ...
}
```

Add import:
```typescript
import { getStoredToken } from '$lib/api/client';
```

**Step 2: Commit**

```bash
git add ui/src/lib/stores/task-stream.svelte.ts
git commit -m "feat(state): add EventSource fallback polling for task stream"
```

---

## Phase 3 — Unified liveState Layer

### Task 12: Port inventory data into liveState

**Files:**
- Modify: `ui/src/lib/stores/live-state.svelte.ts`
- Modify: `ui/src/lib/stores/inventory.svelte.ts` (deprecate)

**Step 1: Move inventory fields and fetch logic into liveState**

Update `live-state.svelte.ts`:

```typescript
import { listNodes } from '$lib/bff/nodes';
import { listVms } from '$lib/bff/vms';
import type { NodeWithResources, VM } from '$lib/api/types';
import type { NodeListItem, VmListItem } from '$lib/bff/types';

function normalizeNodeStatus(state: string): NodeWithResources['status'] {
	const s = state.toLowerCase();
	if (s.includes('ready') || s.includes('online') || s.includes('active')) return 'online';
	if (s.includes('error') || s.includes('fail')) return 'error';
	if (s.includes('maint')) return 'maintenance';
	return 'offline';
}

function mapNode(item: NodeListItem): NodeWithResources {
	return {
		id: item.node_id,
		name: item.name,
		hostname: item.name,
		ip_address: '',
		status: normalizeNodeStatus(item.state),
		is_local: false,
		resources: { vms: 0, images: 0, storage_pools: 0, networks: 0 },
		capabilities: '',
		last_seen_at: '',
		created_at: '',
		updated_at: '',
	};
}

function mapVm(item: VmListItem): VM {
	const state = item.power_state.toLowerCase();
	return {
		id: item.vm_id,
		name: item.name,
		node_id: item.node_id,
		image_id: '',
		storage_pool_id: '',
		network_id: '',
		desired_state: state,
		actual_state: state,
		vcpu: 0,
		memory_mb: 0,
		disk_path: '',
		seed_iso_path: '',
		workspace_path: '',
		ip_address: '',
		mac_address: '',
		console_type: 'serial',
	};
}

class LiveState {
	// Inventory
	nodes = $state<NodeWithResources[]>([]);
	vms = $state<VM[]>([]);
	inventoryLoading = $state(true);

	// Derived
	pinnedVms = $derived(this.vms.filter((v) => v.actual_state === 'running').slice(0, 3));

	async fetchInventory() {
		const token = getStoredToken();
		if (!token) {
			this.inventoryLoading = false;
			return;
		}
		try {
			const [nodesRes, vmsRes] = await Promise.all([
				listNodes({ page: 1, page_size: 100, filters: {} }, token),
				listVms({ page: 1, page_size: 100, filters: {} }, token),
			]);
			this.nodes = (nodesRes.items || []).map(mapNode);
			this.vms = (vmsRes.items || []).map(mapVm);
		} catch (err) {
			console.error('Failed to load inventory:', err);
			this.nodes = [];
			this.vms = [];
		} finally {
			this.inventoryLoading = false;
		}
	}

	// ... rest of existing liveState methods ...
}
```

**Step 2: Deprecate inventory store**

Update `inventory.svelte.ts` to delegate to `liveState`:

```typescript
import { liveState } from './live-state.svelte';

export const inventory = {
	get nodes() { return liveState.nodes; },
	get vms() { return liveState.vms; },
	get isLoading() { return liveState.inventoryLoading; },
	async fetch() { return liveState.fetchInventory(); },
};
```

**Step 3: Commit**

```bash
git add ui/src/lib/stores/live-state.svelte.ts ui/src/lib/stores/inventory.svelte.ts
git commit -m "feat(state): port inventory into liveState"
```

---

### Task 13: Port api-cache into liveState

**Files:**
- Modify: `ui/src/lib/stores/live-state.svelte.ts`
- Modify: `ui/src/lib/stores/api-cache.svelte.ts` (deprecate)

**Step 1: Move cache logic into liveState**

Add to `LiveState` class:

```typescript
private cache = new Map<string, { data: unknown; timestamp: number; ttl: number }>();
private readonly LIST_TTL = 30_000;
private readonly DETAIL_TTL = 60_000;

private isFresh(entry: { timestamp: number; ttl: number }) {
	return Date.now() - entry.timestamp < entry.ttl;
}

async cachedFetch<T>(key: string, fetcher: () => Promise<T>, ttlMs?: number): Promise<T> {
	if (!browser) return fetcher();
	const entry = this.cache.get(key) as { data: T; timestamp: number; ttl: number } | undefined;
	if (entry && this.isFresh(entry)) return entry.data;

	try {
		const data = await fetcher();
		this.cache.set(key, { data, timestamp: Date.now(), ttl: ttlMs ?? this.LIST_TTL });
		return data;
	} catch (err) {
		if (entry) {
			console.warn(`[liveState] fetch error for key "${key}", returning stale data`, err);
			return entry.data;
		}
		throw err;
	}
}

invalidateCache(key: string) {
	if (!browser) return;
	this.cache.delete(key);
}

invalidateCachePattern(prefix: string) {
	if (!browser) return;
	for (const k of this.cache.keys()) {
		if (k.startsWith(prefix)) this.cache.delete(k);
	}
}

clearCache() {
	if (!browser) return;
	this.cache.clear();
}
```

Update `invalidateAndRefresh` to use the new cache methods internally (keep the same public API).

**Step 2: Deprecate api-cache**

Update `api-cache.svelte.ts`:

```typescript
import { liveState } from './live-state.svelte';

export type CacheKey = string;

export async function cachedFetch<T>(key: CacheKey, fetcher: () => Promise<T>, ttlMs?: number): Promise<T> {
	return liveState.cachedFetch(key, fetcher, ttlMs);
}

export function invalidate(key: CacheKey): void {
	liveState.invalidateCache(key);
}

export function invalidatePattern(prefix: CacheKey): void {
	liveState.invalidateCachePattern(prefix);
}

export function getCacheEntry<T>(key: CacheKey) {
	return undefined; // Stale — consumers should migrate to liveState
}

export function clearCache(): void {
	liveState.clearCache();
}
```

**Step 3: Commit**

```bash
git add ui/src/lib/stores/live-state.svelte.ts ui/src/lib/stores/api-cache.svelte.ts
git commit -m "feat(state): port api-cache into liveState"
```

---

### Task 14: Update SidebarNav and NavInfrastructureTree to use liveState

**Files:**
- Modify: `ui/src/lib/components/shell/SidebarNav.svelte`
- Modify: `ui/src/lib/components/shell/NavInfrastructureTree.svelte`
- Modify: `ui/src/lib/components/shell/SidebarPinnedVms.svelte`

**Step 1: Replace inventory imports with liveState**

In each file, replace:
```typescript
import { inventory } from '$lib/stores/inventory.svelte';
```

With:
```typescript
import { liveState } from '$lib/stores/live-state.svelte';
```

And replace all `inventory.` references with `liveState.`.

**Step 2: Commit**

```bash
git add ui/src/lib/components/shell/SidebarNav.svelte ui/src/lib/components/shell/NavInfrastructureTree.svelte ui/src/lib/components/shell/SidebarPinnedVms.svelte
git commit -m "feat(ui): migrate sidebar components to liveState"
```

---

### Task 15: Add inventory polling fallback to liveState

**Files:**
- Modify: `ui/src/lib/stores/live-state.svelte.ts`

**Step 1: Add polling effect**

Add to `LiveState` class:

```typescript
private inventoryPollId: ReturnType<typeof setInterval> | null = null;

startInventoryPolling(intervalMs = 10_000) {
	this.stopInventoryPolling();
	this.inventoryPollId = setInterval(() => {
		this.fetchInventory();
	}, intervalMs);
}

stopInventoryPolling() {
	if (this.inventoryPollId) {
		clearInterval(this.inventoryPollId);
		this.inventoryPollId = null;
	}
}
```

Start polling in `SidebarNav.svelte` `onMount`:

```typescript
onMount(() => {
	liveState.fetchInventory();
	liveState.startInventoryPolling();
	taskStream.connect(['vm', 'node', 'network', 'volume', 'image']);

	return () => {
		liveState.stopInventoryPolling();
		taskStream.disconnect();
	};
});
```

**Step 2: Commit**

```bash
git add ui/src/lib/stores/live-state.svelte.ts ui/src/lib/components/shell/SidebarNav.svelte
git commit -m "feat(state): add 10s inventory polling fallback to liveState"
```

---

### Task 16: Remove old stores

**Files:**
- Delete: `ui/src/lib/stores/inventory.svelte.ts`
- Delete: `ui/src/lib/stores/api-cache.svelte.ts`

**Step 1: Update all imports across the codebase**

Find all files that import from `inventory.svelte` or `api-cache.svelte` and redirect them to `live-state.svelte`.

```bash
cd ui && grep -rl "inventory.svelte" src/ && grep -rl "api-cache.svelte" src/
```

Update each import. For `api-cache.svelte`, replace `invalidatePattern` imports with `liveState.invalidateAndRefresh` or keep a thin re-export in a new `cache-helpers.ts` if the import surface is too large.

Actually, to minimize blast radius, keep `api-cache.svelte.ts` as a thin re-export file for now:

```typescript
// ui/src/lib/stores/api-cache.svelte.ts
export { liveState, type InvalidateOpts } from './live-state.svelte';
export const invalidatePattern = (prefix: string) => liveState.invalidateCachePattern(prefix);
export const invalidate = (key: string) => liveState.invalidateCache(key);
export const cachedFetch = <T>(key: string, fetcher: () => Promise<T>, ttlMs?: number) =>
	liveState.cachedFetch(key, fetcher, ttlMs);
```

Then delete `inventory.svelte.ts` entirely since all consumers have been migrated.

**Step 2: Commit**

```bash
git add ui/src/lib/stores/inventory.svelte.ts ui/src/lib/stores/api-cache.svelte.ts
git commit -m "refactor(state): remove old inventory and api-cache stores"
```

---

### Task 17: Write unit tests for live-state

**Files:**
- Create: `ui/src/lib/stores/live-state.test.ts`

**Step 1: Write tests**

```typescript
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { liveState } from './live-state.svelte';

describe('liveState', () => {
	beforeEach(() => {
		liveState.clearCache();
		liveState.nodes = [];
		liveState.vms = [];
		vi.restoreAllMocks();
	});

	it('caches fetch results', async () => {
		const fetcher = vi.fn().mockResolvedValue({ data: 42 });
		const r1 = await liveState.cachedFetch('test-key', fetcher);
		const r2 = await liveState.cachedFetch('test-key', fetcher);
		expect(r1).toEqual({ data: 42 });
		expect(r2).toEqual({ data: 42 });
		expect(fetcher).toHaveBeenCalledTimes(1);
	});

	it('invalidates cache by pattern', async () => {
		const fetcher = vi.fn().mockResolvedValue({ data: 1 });
		await liveState.cachedFetch('vms:list', fetcher);
		liveState.invalidateCachePattern('vms:');
		await liveState.cachedFetch('vms:list', fetcher);
		expect(fetcher).toHaveBeenCalledTimes(2);
	});

	it('computes pinnedVMs from running VMs', () => {
		liveState.vms = [
			{ id: '1', actual_state: 'running', name: 'A' } as any,
			{ id: '2', actual_state: 'stopped', name: 'B' } as any,
			{ id: '3', actual_state: 'running', name: 'C' } as any,
			{ id: '4', actual_state: 'running', name: 'D' } as any,
		];
		expect(liveState.pinnedVms.length).toBe(3);
		expect(liveState.pinnedVms[0].name).toBe('A');
	});
});
```

**Step 2: Run tests**

```bash
cd ui && npx vitest run src/lib/stores/live-state.test.ts 2>&1
```

Expected: All tests pass.

**Step 3: Commit**

```bash
git add ui/src/lib/stores/live-state.test.ts
git commit -m "test(state): add liveState unit tests"
```

---

## Verification

### Task 18: Run full test suite

```bash
cd ui && npm run test 2>&1
```

Expected: All existing tests pass. Any new failures are from the changes above and must be fixed before proceeding.

### Task 19: Run build check

```bash
cd ui && npm run build 2>&1 | tail -20
```

Expected: Build completes with zero errors.

### Task 20: Manual verification checklist

Open the UI in a browser and verify:

- [ ] Create a VM → sidebar shows new VM within 2 seconds
- [ ] Delete a VM from list page → sidebar removes VM within 2 seconds
- [ ] Start a VM from detail page → sidebar status dot changes to green within 2 seconds
- [ ] Stop a VM from sidebar context menu → detail page status changes within 2 seconds
- [ ] Open two browser tabs → create VM in Tab A, Tab B sidebar updates within 5 seconds
- [ ] Disconnect network (dev tools) → sidebar shows "Reconnecting" status
- [ ] Reconnect network → sidebar shows "Live" status and resumes updates
- [ ] No manual page refresh needed for any of the above

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-06-09-webui-reactive-state-implementation.md`.**

**Two execution options:**

**1. Subagent-Driven (this session)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Parallel Session (separate)** — Open a new session with `superpowers:executing-plans`, batch execution with checkpoints.

Which approach?
