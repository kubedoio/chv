<script lang="ts">
	/**
	 * Inner pane for the Architecture Designer canvas.
	 *
	 * Hosts `<SvelteFlow>` itself plus the drop / connect / delete / click
	 * handlers. Lives as a separate file so that `useSvelteFlow()` is invoked
	 * inside a `<SvelteFlowProvider>` tree (the parent `Canvas.svelte` wraps
	 * the provider). Splitting also keeps each file under the 300-line
	 * component cap enforced by `component-size.test.ts`.
	 */
	import {
		SvelteFlow,
		Background,
		Controls,
		ConnectionMode,
		useSvelteFlow,
		type NodeTypes,
		type Connection,
		type Node as FlowNode,
		type Edge as FlowEdge
	} from '@xyflow/svelte';

	import { architectureCanvasStore } from '$lib/stores/architecture-canvas-store.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { nextDefaultName, readDraggedKind } from './canvas-handlers';

	interface Props {
		/** Decorated nodes (with `findingSeverity` injected by Canvas.svelte). */
		nodes: ReadonlyArray<FlowNode>;
		/** Mapping of NodeKind → per-kind Svelte Flow node component. */
		nodeTypes: NodeTypes;
	}

	let { nodes, nodeTypes }: Props = $props();

	const { screenToFlowPosition } = useSvelteFlow();

	// Svelte Flow needs concrete `nodes` / `edges` array bindings for its
	// internal store sync. We mirror the upstream reactive arrays into local
	// `$state` arrays via `$effect` (so drag-position changes from SvelteFlow
	// flow back through `bind:`) and seed both arrays empty so that the
	// initial `[...nodes]` capture warning doesn't fire.
	let flowNodes = $state<FlowNode[]>([]);
	let flowEdges = $state<FlowEdge[]>([]);

	$effect(() => {
		flowNodes = [...nodes];
	});
	$effect(() => {
		flowEdges = [...architectureCanvasStore.edges];
	});

	function onConnect(connection: Connection): void {
		const result = architectureCanvasStore.addEdgeInferred(connection.source, connection.target);
		if (!result.ok) {
			toast.error(result.reason);
		}
	}

	function onDelete(params: { nodes: FlowNode[]; edges: FlowEdge[] }): void {
		for (const e of params.edges) {
			architectureCanvasStore.removeEdge(e.id);
		}
		for (const n of params.nodes) {
			architectureCanvasStore.removeNode(n.id);
		}
	}

	function onNodeClick({ node }: { node: FlowNode }): void {
		architectureCanvasStore.selectedNodeId = node.id;
	}

	function onPaneClick(): void {
		architectureCanvasStore.selectedNodeId = null;
	}

	function onDragOver(event: DragEvent): void {
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'copy';
		}
	}

	function onDrop(event: DragEvent): void {
		event.preventDefault();
		const kind = readDraggedKind(event);
		if (kind === null) return;
		const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
		const name = nextDefaultName(kind, architectureCanvasStore.nodes);
		architectureCanvasStore.addNode(kind, position, name);
	}
</script>

<div
	class="pane"
	role="application"
	aria-label="Architecture canvas"
	data-testid="canvas-dropzone"
	ondragover={onDragOver}
	ondrop={onDrop}
>
	<SvelteFlow
		bind:nodes={flowNodes}
		bind:edges={flowEdges}
		{nodeTypes}
		onconnect={onConnect}
		ondelete={onDelete}
		onnodeclick={onNodeClick}
		onpaneclick={onPaneClick}
		connectionMode={ConnectionMode.Loose}
		fitView
		deleteKey={['Backspace', 'Delete']}
	>
		<Background />
		<Controls />
	</SvelteFlow>
</div>

<style>
	.pane {
		width: 100%;
		height: 100%;
		min-height: 600px;
		position: relative;
	}
</style>
