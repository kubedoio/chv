import yaml from 'js-yaml';

import { architectureStore, type Architecture } from './architecture-store.svelte';
import {
	inferEdgeType,
	isEdgeAllowed,
	type EdgeType,
	type NodeKind
} from '$lib/components/architectures/canvas/edge-rules';

/**
 * Reactive Svelte 5 store backing the Architecture Designer canvas.
 *
 * The store holds three pieces of state:
 *   1. `nodes` / `edges` — the visual graph the user is editing.
 *   2. `selectedNodeId` — currently inspected node (drives the inspector).
 *   3. `dirty` — true whenever local state diverges from the last persisted
 *      snapshot (`load(...)` or successful `persist(...)`).
 *
 * Persistence flows through the existing `architectureStore.update(...)` so
 * that:
 *   - `mutateWithRefresh` invalidates live-state caches,
 *   - `StaleVersionError` propagates to the page (banner + Reload),
 *   - both `design_graph_json` and `latest_yaml` save in a single request.
 *
 * SSR/test safety: this module must not import `$app/*` at top level; only
 * `$lib/...` and external packages. (`architecture-store` already mocks
 * `$app/navigation` in its own tests; we don't reach for it directly.)
 */

// ---------------------------------------------------------------------------
// Wire / graph types
// ---------------------------------------------------------------------------

export interface CanvasNodeData {
	readonly kind: NodeKind;
	name: string;
	[field: string]: unknown;
}

export interface CanvasNode {
	id: string;
	type: NodeKind;
	position: { x: number; y: number };
	data: CanvasNodeData;
}

export interface CanvasEdge {
	id: string;
	type: EdgeType;
	source: string;
	target: string;
	data: { relationship: EdgeType };
}

/**
 * v1.0 wire shape from `docs/specs/architecture-designer/contracts/graph-contract.md`.
 * This is what the BFF stores opaquely in `design_graph_json`.
 */
export interface GraphPayload {
	version: '1.0';
	nodes: CanvasNode[];
	edges: CanvasEdge[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Per graph-contract.md: node ids are deterministic — `node-<kind>-<name>`. */
function nodeId(kind: NodeKind, name: string): string {
	return `node-${kind}-${name}`;
}

function edgeId(sourceId: string, targetId: string, edgeType: EdgeType): string {
	return `edge-${sourceId}-to-${targetId}-${edgeType}`;
}

/**
 * Map node kinds to their corresponding YAML top-level section. Aligned with
 * `docs/specs/architecture-designer/contracts/yaml-contract.md`.
 *
 * `null` means "no direct YAML representation in the v1alpha1 contract" and
 * the section is omitted. (Currently every MVP kind has a section, but
 * keeping the typed null branch makes the function resilient if NodeKind
 * grows ahead of the YAML contract.)
 */
const KIND_TO_YAML_SECTION: Record<NodeKind, string | null> = {
	host: 'servers',
	network: 'networks',
	datastore: 'datastores',
	image: 'images',
	template: 'templates',
	instance: 'instances',
	user: 'users',
	role: 'roles'
};

interface YamlDoc {
	apiVersion: string;
	kind: string;
	metadata: { name: string };
	[section: string]: unknown;
}

/**
 * Best-effort YAML serialisation of the in-memory graph. This is the Phase-2
 * fallback so that `latest_yaml` is never an empty string when we save —
 * Phase 4+ will replace this with a server-side canonical generator.
 */
function graphToYaml(nodes: ReadonlyArray<CanvasNode>): string {
	const doc: YamlDoc = {
		apiVersion: 'chv.kubedo.io/v1alpha1',
		kind: 'CHVArchitecture',
		metadata: { name: 'untitled' }
	};

	for (const node of nodes) {
		const section = KIND_TO_YAML_SECTION[node.data.kind];
		if (section === null) {
			continue;
		}
		const list = (doc[section] as Array<Record<string, unknown>> | undefined) ?? [];
		// Spread `data` so any inspector-edited fields survive the round trip.
		// Strip the `kind` discriminator (encoded by the YAML section) and
		// pull `name` out so we can apply the "untitled" fallback below
		// without the spread overwriting it.
		const { kind: _kind, name: rawName, ...rest } = node.data;
		void _kind;
		const item: Record<string, unknown> = {
			name: typeof rawName === 'string' && rawName.length > 0 ? rawName : 'untitled',
			...rest
		};
		list.push(item);
		doc[section] = list;
	}

	return yaml.dump(doc, { noRefs: true, sortKeys: false });
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

class ArchitectureCanvasStore {
	nodes = $state<CanvasNode[]>([]);
	edges = $state<CanvasEdge[]>([]);
	selectedNodeId = $state<string | null>(null);
	dirty = $state(false);

	/**
	 * Hydrate from a persisted graph blob, or reset to empty when given
	 * `null` (newly created architecture). Resets `dirty` because we just
	 * synced with the server.
	 */
	load(graph: GraphPayload | null): void {
		if (graph === null) {
			this.nodes = [];
			this.edges = [];
		} else {
			// Defensive copies — never mutate caller-owned arrays.
			this.nodes = graph.nodes.map((n) => ({
				...n,
				position: { ...n.position },
				data: { ...n.data }
			}));
			this.edges = graph.edges.map((e) => ({ ...e, data: { ...e.data } }));
		}
		this.selectedNodeId = null;
		this.dirty = false;
	}

	addNode(kind: NodeKind, position: { x: number; y: number }, name: string): void {
		const id = nodeId(kind, name);
		// Reject duplicate ids quietly — id collisions on (kind, name) are the
		// caller's job to avoid (the inspector enforces unique names).
		if (this.nodes.some((n) => n.id === id)) {
			return;
		}
		this.nodes = [
			...this.nodes,
			{
				id,
				type: kind,
				position: { x: position.x, y: position.y },
				data: { kind, name }
			}
		];
		this.dirty = true;
	}

	updateNodeData(id: string, partial: Record<string, unknown>): void {
		const idx = this.nodes.findIndex((n) => n.id === id);
		if (idx < 0) {
			return;
		}
		const current = this.nodes[idx];
		// Forbid changing `kind` via this path — kind is structural.
		const { kind: _ignoredKind, ...safe } = partial;
		void _ignoredKind;
		const next: CanvasNode = {
			...current,
			data: { ...current.data, ...safe }
		};
		this.nodes = [...this.nodes.slice(0, idx), next, ...this.nodes.slice(idx + 1)];
		this.dirty = true;
	}

	removeNode(id: string): void {
		const before = this.nodes.length;
		this.nodes = this.nodes.filter((n) => n.id !== id);
		if (this.nodes.length === before) {
			return; // unknown id, no-op
		}
		this.edges = this.edges.filter((e) => e.source !== id && e.target !== id);
		if (this.selectedNodeId === id) {
			this.selectedNodeId = null;
		}
		this.dirty = true;
	}

	addEdge(
		source: string,
		target: string,
		edgeType: EdgeType
	): { ok: true } | { ok: false; reason: string } {
		const sourceNode = this.nodes.find((n) => n.id === source);
		const targetNode = this.nodes.find((n) => n.id === target);
		if (!sourceNode || !targetNode) {
			return {
				ok: false,
				reason: `unknown ${!sourceNode ? 'source' : 'target'} node`
			};
		}
		if (!isEdgeAllowed(sourceNode.data.kind, targetNode.data.kind, edgeType)) {
			return {
				ok: false,
				reason: `edge type ${edgeType} not allowed from ${sourceNode.data.kind} to ${targetNode.data.kind}`
			};
		}
		const id = edgeId(source, target, edgeType);
		if (this.edges.some((e) => e.id === id)) {
			return { ok: true }; // idempotent re-add
		}
		this.edges = [
			...this.edges,
			{ id, type: edgeType, source, target, data: { relationship: edgeType } }
		];
		this.dirty = true;
		return { ok: true };
	}

	removeEdge(id: string): void {
		const before = this.edges.length;
		this.edges = this.edges.filter((e) => e.id !== id);
		if (this.edges.length !== before) {
			this.dirty = true;
		}
	}

	/**
	 * Convenience used by canvas drag-connect handlers when no explicit edge
	 * type was chosen. Resolves to the first allowed edge type for the
	 * (source-kind, target-kind) pair, or returns an error.
	 */
	addEdgeInferred(source: string, target: string): { ok: true } | { ok: false; reason: string } {
		const sourceNode = this.nodes.find((n) => n.id === source);
		const targetNode = this.nodes.find((n) => n.id === target);
		if (!sourceNode || !targetNode) {
			return {
				ok: false,
				reason: `unknown ${!sourceNode ? 'source' : 'target'} node`
			};
		}
		const inferred = inferEdgeType(sourceNode.data.kind, targetNode.data.kind);
		if (inferred === null) {
			return {
				ok: false,
				reason: `no allowed edge type from ${sourceNode.data.kind} to ${targetNode.data.kind}`
			};
		}
		return this.addEdge(source, target, inferred);
	}

	/** Produce a v1.0 wire-shape snapshot. */
	serialize(): GraphPayload {
		return {
			version: '1.0',
			// Deep copies so callers can't aliasingly mutate store state.
			nodes: this.nodes.map((n) => ({
				...n,
				position: { ...n.position },
				data: { ...n.data }
			})),
			edges: this.edges.map((e) => ({ ...e, data: { ...e.data } }))
		};
	}

	/** Best-effort YAML rendering of the current graph. */
	generateYaml(): string {
		return graphToYaml(this.nodes);
	}

	/**
	 * Persist the current graph + YAML to the BFF in a single update call.
	 * `StaleVersionError` propagates so the page can render a Reload banner.
	 */
	async persist(architectureId: string, expectedVersion: number): Promise<Architecture> {
		const graphJson = JSON.stringify(this.serialize());
		const yamlBlob = this.generateYaml();
		const arch = await architectureStore.update(architectureId, expectedVersion, {
			design_graph_json: graphJson,
			latest_yaml: yamlBlob
		});
		this.dirty = false;
		return arch;
	}
}

export const architectureCanvasStore = new ArchitectureCanvasStore();
