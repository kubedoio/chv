<script lang="ts">
	/**
	 * Architecture Designer canvas.
	 *
	 * Mounts the palette rail and `<CanvasPane>` (which itself hosts
	 * `<SvelteFlow>`) inside an `<SvelteFlowProvider>`. The provider is
	 * required for the inner pane's `useSvelteFlow()` call to resolve —
	 * `screenToFlowPosition` is needed to convert palette drop coordinates
	 * into graph-space positions.
	 *
	 * Reads the editable graph from `architectureCanvasStore` (Svelte 5 runes
	 * are reactive automatically) and notifies the parent via `onChange()`
	 * whenever the store's `dirty` flag flips on, so the page can update its
	 * Save button state.
	 *
	 * Per-node validation badges are computed here (not inside each node
	 * component) so node components stay pure: we inject a `findingSeverity`
	 * field on `data` that the shared `NodeShell` reads.
	 */

	import '@xyflow/svelte/dist/style.css';
	import { SvelteFlowProvider, type Node as FlowNode, type NodeTypes } from '@xyflow/svelte';

	import { architectureCanvasStore } from '$lib/stores/architecture-canvas-store.svelte';
	import type { Finding } from '$lib/bff/architectures';

	import HostNode from '../nodes/HostNode.svelte';
	import NetworkNode from '../nodes/NetworkNode.svelte';
	import DatastoreNode from '../nodes/DatastoreNode.svelte';
	import ImageNode from '../nodes/ImageNode.svelte';
	import TemplateNode from '../nodes/TemplateNode.svelte';
	import InstanceNode from '../nodes/InstanceNode.svelte';
	import UserNode from '../nodes/UserNode.svelte';
	import RoleNode from '../nodes/RoleNode.svelte';
	import CanvasPalette from './CanvasPalette.svelte';
	import CanvasPane from './CanvasPane.svelte';
	import { severityForNode } from './canvas-handlers';

	interface Props {
		/** Validation findings used to render per-node badges. */
		findings?: ReadonlyArray<Finding>;
		/** Called when the store's dirty flag flips on so the parent can mark dirty. */
		onChange?: () => void;
	}

	let { findings = [], onChange }: Props = $props();

	// SvelteFlow's `NodeTypes` is widened (`data: any`) at the framework
	// boundary to accommodate arbitrary node-data shapes. Our per-kind
	// components are typed against `NodeProps`, which already widens `data`,
	// so the structural cast through `unknown` is the documented escape
	// hatch — no runtime impact.
	const nodeTypes = {
		host: HostNode,
		network: NetworkNode,
		datastore: DatastoreNode,
		image: ImageNode,
		template: TemplateNode,
		instance: InstanceNode,
		user: UserNode,
		role: RoleNode
	} as unknown as NodeTypes;

	const decoratedNodes = $derived.by<FlowNode[]>(() =>
		architectureCanvasStore.nodes.map((n) => ({
			...n,
			data: {
				...n.data,
				findingSeverity: severityForNode(n, findings)
			}
		}))
	);

	// Forward dirty→true transitions to the parent. Reading `dirty` inside a
	// `$effect` makes this run whenever the flag flips; we only call onChange
	// on the false→true edge (the store resets to false on `load()` and
	// successful `persist()`, which we explicitly do NOT want to forward).
	let lastDirty = $state(false);
	$effect(() => {
		const isDirty = architectureCanvasStore.dirty;
		if (isDirty && !lastDirty) {
			onChange?.();
		}
		lastDirty = isDirty;
	});
</script>

<div class="canvas-grid" data-testid="architecture-canvas-root">
	<CanvasPalette />
	<SvelteFlowProvider>
		<CanvasPane nodes={decoratedNodes} {nodeTypes} />
	</SvelteFlowProvider>
</div>

<style>
	.canvas-grid {
		display: grid;
		grid-template-columns: 180px 1fr;
		min-height: 600px;
		height: 100%;
		width: 100%;
		background: var(--bg-surface, #fff);
		border: 1px solid var(--color-neutral-200, #ddd5c8);
		border-radius: var(--radius-md, 0.5rem);
		overflow: hidden;
	}
</style>
