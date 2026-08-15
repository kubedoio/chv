import { beforeEach, describe, expect, it, vi } from 'vitest';
import { load } from 'js-yaml';

// architecture-canvas-store imports architecture-store, which transitively
// pulls in mutation.svelte → live-state.svelte → SvelteKit's $app/navigation
// and $env/dynamic/public. Mirror the mocks already used by
// architecture-store.test.ts so this suite runs cleanly under jsdom.
vi.mock('$env/dynamic/public', () => ({ env: {} }));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	invalidateAll: vi.fn()
}));
vi.mock('$lib/api/client', () => ({
	getStoredToken: vi.fn(() => 'test-token'),
	clearToken: vi.fn()
}));

vi.mock('$lib/stores/architecture-store.svelte', () => {
	return {
		architectureStore: {
			update: vi.fn()
		}
	};
});

import { architectureStore } from '$lib/stores/architecture-store.svelte';
import {
	architectureCanvasStore,
	type GraphPayload
} from './architecture-canvas-store.svelte';

const updateMock = architectureStore.update as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
	architectureCanvasStore.load(null);
	updateMock.mockReset();
});

describe('architectureCanvasStore — node CRUD', () => {
	it('addNode creates a deterministic id and marks dirty', () => {
		architectureCanvasStore.addNode('host', { x: 10, y: 20 }, 'chv-node-01');
		expect(architectureCanvasStore.nodes.length).toBe(1);
		expect(architectureCanvasStore.nodes[0].id).toBe('node-host-chv-node-01');
		expect(architectureCanvasStore.nodes[0].data.kind).toBe('host');
		expect(architectureCanvasStore.dirty).toBe(true);
	});

	it('addNode is idempotent on duplicate id', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		architectureCanvasStore.addNode('host', { x: 9, y: 9 }, 'h');
		expect(architectureCanvasStore.nodes.length).toBe(1);
		expect(architectureCanvasStore.nodes[0].position).toEqual({ x: 0, y: 0 });
	});

	it('updateNodeData merges partial fields and refuses to change kind', () => {
		architectureCanvasStore.addNode('instance', { x: 0, y: 0 }, 'app-01');
		architectureCanvasStore.updateNodeData('node-instance-app-01', {
			cpu_cores: 4,
			kind: 'host' // must be ignored
		});
		const n = architectureCanvasStore.nodes[0];
		expect(n.data.kind).toBe('instance');
		expect(n.data.cpu_cores).toBe(4);
	});

	it('removeNode also drops every incident edge', () => {
		architectureCanvasStore.addNode('instance', { x: 0, y: 0 }, 'app-01');
		architectureCanvasStore.addNode('host', { x: 100, y: 0 }, 'h-01');
		const r = architectureCanvasStore.addEdge(
			'node-instance-app-01',
			'node-host-h-01',
			'placed_on'
		);
		expect(r.ok).toBe(true);
		expect(architectureCanvasStore.edges.length).toBe(1);

		architectureCanvasStore.removeNode('node-host-h-01');
		expect(architectureCanvasStore.nodes.find((n) => n.id === 'node-host-h-01')).toBeUndefined();
		expect(architectureCanvasStore.edges.length).toBe(0);
	});

	it('removeNode clears selection if the removed node was selected', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		architectureCanvasStore.selectedNodeId = 'node-host-h';
		architectureCanvasStore.removeNode('node-host-h');
		expect(architectureCanvasStore.selectedNodeId).toBeNull();
	});
});

describe('architectureCanvasStore — edges', () => {
	beforeEach(() => {
		architectureCanvasStore.addNode('instance', { x: 0, y: 0 }, 'app-01');
		architectureCanvasStore.addNode('host', { x: 100, y: 0 }, 'h-01');
		architectureCanvasStore.addNode('network', { x: 100, y: 100 }, 'net-prod');
	});

	it('accepts an allowed (instance,host,placed_on) edge', () => {
		const r = architectureCanvasStore.addEdge(
			'node-instance-app-01',
			'node-host-h-01',
			'placed_on'
		);
		expect(r.ok).toBe(true);
		expect(architectureCanvasStore.edges.length).toBe(1);
	});

	it('rejects placed_on from instance to network with a descriptive reason', () => {
		const r = architectureCanvasStore.addEdge(
			'node-instance-app-01',
			'node-network-net-prod',
			'placed_on'
		);
		expect(r.ok).toBe(false);
		if (!r.ok) {
			expect(r.reason).toContain('placed_on');
			expect(r.reason).toContain('instance');
			expect(r.reason).toContain('network');
		}
	});

	it('rejects edges referencing unknown nodes', () => {
		const r = architectureCanvasStore.addEdge('node-host-missing', 'node-host-h-01', 'placed_on');
		expect(r.ok).toBe(false);
		if (!r.ok) {
			expect(r.reason).toContain('unknown');
		}
	});

	it('addEdge is idempotent on the same triple', () => {
		architectureCanvasStore.addEdge('node-instance-app-01', 'node-host-h-01', 'placed_on');
		architectureCanvasStore.addEdge('node-instance-app-01', 'node-host-h-01', 'placed_on');
		expect(architectureCanvasStore.edges.length).toBe(1);
	});

	it('addEdgeInferred picks the only allowed edge type for the kind pair', () => {
		const r = architectureCanvasStore.addEdgeInferred(
			'node-instance-app-01',
			'node-network-net-prod'
		);
		expect(r.ok).toBe(true);
		expect(architectureCanvasStore.edges[0].type).toBe('attached_to_network');
	});

	it('addEdgeInferred rejects unrelatable kind pairs', () => {
		architectureCanvasStore.addNode('user', { x: 0, y: 200 }, 'alice');
		const r = architectureCanvasStore.addEdgeInferred('node-user-alice', 'node-host-h-01');
		expect(r.ok).toBe(false);
	});

	it('removeEdge clears the edge and marks dirty', () => {
		architectureCanvasStore.addEdge('node-instance-app-01', 'node-host-h-01', 'placed_on');
		architectureCanvasStore.dirty = false; // reset for this assertion
		const id = architectureCanvasStore.edges[0].id;
		architectureCanvasStore.removeEdge(id);
		expect(architectureCanvasStore.edges.length).toBe(0);
		expect(architectureCanvasStore.dirty).toBe(true);
	});
});

describe('architectureCanvasStore — serialize / load round-trip', () => {
	it('load(serialize()) is idempotent (graph equal, dirty reset)', () => {
		architectureCanvasStore.addNode('instance', { x: 1, y: 2 }, 'app-01');
		architectureCanvasStore.addNode('host', { x: 3, y: 4 }, 'h-01');
		architectureCanvasStore.addEdge('node-instance-app-01', 'node-host-h-01', 'placed_on');
		const snapshot = architectureCanvasStore.serialize();

		architectureCanvasStore.load(snapshot);
		expect(architectureCanvasStore.dirty).toBe(false);
		expect(architectureCanvasStore.serialize()).toEqual(snapshot);
	});

	it('serialize emits the v1.0 wire shape', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		const s: GraphPayload = architectureCanvasStore.serialize();
		expect(s.version).toBe('1.0');
		expect(Array.isArray(s.nodes)).toBe(true);
		expect(Array.isArray(s.edges)).toBe(true);
	});

	it('load(null) resets to empty', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		architectureCanvasStore.load(null);
		expect(architectureCanvasStore.nodes).toEqual([]);
		expect(architectureCanvasStore.edges).toEqual([]);
		expect(architectureCanvasStore.dirty).toBe(false);
	});

	it('serialize returns deep copies (caller mutation does not poison the store)', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		const s = architectureCanvasStore.serialize();
		s.nodes[0].position.x = 999;
		expect(architectureCanvasStore.nodes[0].position.x).toBe(0);
	});
});

describe('architectureCanvasStore — generateYaml', () => {
	it('emits valid YAML parseable by js-yaml.load', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'chv-node-01');
		architectureCanvasStore.addNode('instance', { x: 100, y: 0 }, 'app-01');
		const out = architectureCanvasStore.generateYaml();
		const parsed = load(out) as Record<string, unknown>;
		expect(parsed).toBeDefined();
		expect(parsed.apiVersion).toBe('chv.kubedo.io/v1alpha1');
		expect(parsed.kind).toBe('CHVArchitecture');
	});

	it('contains the expected node names under the right YAML sections', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'chv-node-01');
		architectureCanvasStore.addNode('network', { x: 0, y: 0 }, 'tenant-prod');
		architectureCanvasStore.addNode('instance', { x: 0, y: 0 }, 'app-01');

		const out = architectureCanvasStore.generateYaml();
		const parsed = load(out) as Record<string, Array<{ name: string }>>;
		expect(parsed.servers?.[0]?.name).toBe('chv-node-01');
		expect(parsed.networks?.[0]?.name).toBe('tenant-prod');
		expect(parsed.instances?.[0]?.name).toBe('app-01');
	});

	it('falls back to "untitled" when a node has no name field', () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, '');
		const out = architectureCanvasStore.generateYaml();
		const parsed = load(out) as { servers?: Array<{ name: string }> };
		expect(parsed.servers?.[0]?.name).toBe('untitled');
	});

	it('emits empty graph as just metadata + apiVersion (no kind sections)', () => {
		const out = architectureCanvasStore.generateYaml();
		const parsed = load(out) as Record<string, unknown>;
		expect(parsed.apiVersion).toBe('chv.kubedo.io/v1alpha1');
		expect(parsed.servers).toBeUndefined();
	});
});

describe('architectureCanvasStore — persist', () => {
	it('calls architectureStore.update with both blobs and clears dirty', async () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		expect(architectureCanvasStore.dirty).toBe(true);

		updateMock.mockResolvedValueOnce({
			id: 'arch-1',
			name: 'a',
			display_name: null,
			description: null,
			environment: null,
			status: 'draft',
			owner_user_id: null,
			last_validation_status: null,
			last_fleet_check_status: null,
			version_number: 8,
			created_at: '',
			updated_at: '',
			archived_at: null
		});

		const arch = await architectureCanvasStore.persist('arch-1', 7);
		expect(arch.id).toBe('arch-1');
		expect(architectureCanvasStore.dirty).toBe(false);

		expect(updateMock).toHaveBeenCalledTimes(1);
		const [id, expectedVersion, fields] = updateMock.mock.calls[0];
		expect(id).toBe('arch-1');
		expect(expectedVersion).toBe(7);
		expect(typeof fields.design_graph_json).toBe('string');
		expect(typeof fields.latest_yaml).toBe('string');

		// Sanity: the JSON blob is a parseable graph payload.
		const parsedJson = JSON.parse(fields.design_graph_json as string) as GraphPayload;
		expect(parsedJson.version).toBe('1.0');
		expect(parsedJson.nodes.length).toBe(1);
		// The YAML blob also parses.
		expect(load(fields.latest_yaml as string)).toBeDefined();
	});

	it('keeps dirty=true when update rejects', async () => {
		architectureCanvasStore.addNode('host', { x: 0, y: 0 }, 'h');
		updateMock.mockRejectedValueOnce(new Error('boom'));
		await expect(architectureCanvasStore.persist('arch-1', 7)).rejects.toThrow('boom');
		expect(architectureCanvasStore.dirty).toBe(true);
	});
});
