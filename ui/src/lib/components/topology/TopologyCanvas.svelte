<script lang="ts">
	import { goto } from '$app/navigation';
	import { inventory } from '$lib/stores/inventory.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { Activity, Box, Loader2, MoreHorizontal, Server } from 'lucide-svelte';
	import { fade, draw } from 'svelte/transition';

	type TopologyTarget = 'fleet' | 'node' | 'vm';

	interface MenuAction {
		label: string;
		hint?: string;
		dangerous?: boolean;
		disabled?: boolean;
		run: () => void;
	}

	let zoom = $state(1);
	let contextMenu = $state<{
		x: number;
		y: number;
		title: string;
		subtitle: string;
		type: TopologyTarget;
		actions: MenuAction[];
	} | null>(null);
	let menuElement = $state<HTMLDivElement | null>(null);

	function getVmNodeId(vm: (typeof inventory.vms)[number]): string {
		return vm.node_id ?? 'unassigned';
	}

	const displayNodes = $derived(
		inventory.nodes.map((n, i) => {
			const columns = Math.max(1, Math.ceil(Math.sqrt(Math.max(inventory.nodes.length, 1))));
			const hostVms = inventory.vms.filter((vm) => getVmNodeId(vm) === n.id);
			return {
				...n,
				x: 190 + (i % columns) * 260,
				y: 245 + Math.floor(i / columns) * 230,
				status: n.status === 'online' ? 'healthy' : n.status === 'error' ? 'danger' : 'warning',
				vmCount: hostVms.length
			};
		})
	);

	const displayVms = $derived(
		(() => {
			const siblingIndex = new Map<string, number>();
			const siblingCount = new Map<string, number>();
			for (const vm of inventory.vms) {
				const nodeId = getVmNodeId(vm);
				siblingCount.set(nodeId, (siblingCount.get(nodeId) ?? 0) + 1);
			}

			return inventory.vms.map((v) => {
				const nodeId = getVmNodeId(v);
				const parent = displayNodes.find((n) => n.id === nodeId);
				const index = siblingIndex.get(nodeId) ?? 0;
				siblingIndex.set(nodeId, index + 1);
				const total = siblingCount.get(nodeId) ?? 1;
				const spread = Math.min(320, Math.max(120, total * 82));
				const offset = total === 1 ? 0 : -spread / 2 + (spread / Math.max(total - 1, 1)) * index;
				const isRunning = v.actual_state === 'running';
				return {
					...v,
					x: parent ? parent.x + offset : 120 + index * 100,
					y: parent ? parent.y - 110 : 95,
					status: isRunning ? 'healthy' : v.actual_state === 'failed' ? 'danger' : 'warning',
					nodeId,
					stateLabel: isRunning ? 'Running' : v.actual_state || 'Unknown'
				};
			});
		})()
	);

	const topologyBounds = $derived(
		(() => {
			const points = [...displayNodes, ...displayVms];
			if (points.length === 0) return '0 0 800 600';
			const xs = points.map((p) => p.x);
			const ys = points.map((p) => p.y);
			const minX = Math.min(...xs) - 170;
			const minY = Math.min(...ys) - 120;
			const width = Math.max(800, Math.max(...xs) - minX + 170);
			const height = Math.max(600, Math.max(...ys) - minY + 140);
			return `${minX} ${minY} ${width} ${height}`;
		})()
	);

	function getStatusColor(status: string) {
		switch (status) {
			case 'healthy': return 'var(--color-success)';
			case 'warning': return 'var(--color-warning)';
			case 'danger': return 'var(--color-danger)';
			default: return 'var(--color-neutral-400)';
		}
	}

	function setZoom(next: number) {
		zoom = Math.min(1.5, Math.max(0.7, next));
	}

	function closeContextMenu() {
		contextMenu = null;
	}

	function clampContextMenu() {
		if (!contextMenu || !menuElement) return;
		const rect = menuElement.getBoundingClientRect();
		const x = Math.min(Math.max(contextMenu.x, 8), window.innerWidth - rect.width - 8);
		const y = Math.min(Math.max(contextMenu.y, 8), window.innerHeight - rect.height - 8);
		if (x !== contextMenu.x || y !== contextMenu.y) {
			contextMenu = { ...contextMenu, x, y };
		}
	}

	function openContextMenu(event: MouseEvent, target: TopologyTarget, resource?: any) {
		event.preventDefault();
		event.stopPropagation();

		if (target === 'node' && resource) {
			contextMenu = {
				x: event.clientX,
				y: event.clientY,
				title: resource.name,
				subtitle: `${resource.vmCount} workloads · ${resource.status}`,
				type: target,
				actions: [
					{ label: 'Open host', hint: 'Details', run: () => goto(`/nodes/${resource.id}`) },
					{ label: 'Show instances', hint: 'Filtered list', run: () => goto(`/vms?node_id=${resource.id}`) },
					{ label: 'Show networks', hint: 'Host scope', run: () => goto(`/networks?node_id=${resource.id}`) },
					{ label: 'Show storage', hint: 'Host scope', run: () => goto(`/storage?node_id=${resource.id}`) }
				]
			};
		} else if (target === 'vm' && resource) {
			contextMenu = {
				x: event.clientX,
				y: event.clientY,
				title: resource.name,
				subtitle: `${resource.stateLabel} · ${resource.nodeId}`,
				type: target,
				actions: [
					{ label: 'Open instance', hint: 'Details', run: () => goto(`/vms/${resource.id}`) },
					{ label: 'Open console', hint: 'Serial', disabled: resource.status !== 'healthy', run: () => goto(`/vms/${resource.id}?tab=console`) },
					{ label: 'View metrics', hint: 'Performance', run: () => goto(`/vms/${resource.id}?tab=metrics`) },
					{ label: 'Review events', hint: 'Audit trail', run: () => goto(`/events?resource_id=${resource.id}`) }
				]
			};
		} else {
			contextMenu = {
				x: event.clientX,
				y: event.clientY,
				title: 'Global Fleet',
				subtitle: `${inventory.nodes.length} hosts · ${inventory.vms.length} workloads`,
				type: 'fleet',
				actions: [
					{ label: 'Open instances', hint: 'All workloads', run: () => goto('/vms') },
					{ label: 'Open tasks', hint: 'Operations', run: () => goto('/tasks') },
					{ label: 'Open events', hint: 'Fleet log', run: () => goto('/events') },
					{ label: 'Refresh topology', hint: 'Fetch latest', run: () => inventory.fetch() }
				]
			};
		}

		requestAnimationFrame(clampContextMenu);
	}

	function runMenuAction(action: MenuAction) {
		if (action.disabled) return;
		closeContextMenu();
		action.run();
	}

	function handleTopologyKeydown(event: KeyboardEvent, target: TopologyTarget, resource?: any) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			if (target === 'node' && resource) {
				selection.select('node', resource.id, resource.name);
			} else if (target === 'vm' && resource) {
				selection.select('vm', resource.id, resource.name);
			}
		}
	}

	$effect(() => {
		if (!contextMenu) return;

		function handleDocumentClick(event: MouseEvent) {
			if (menuElement?.contains(event.target as Node)) return;
			closeContextMenu();
		}

		function handleDocumentKeydown(event: KeyboardEvent) {
			if (event.key === 'Escape') closeContextMenu();
		}

		document.addEventListener('click', handleDocumentClick, { capture: true });
		document.addEventListener('keydown', handleDocumentKeydown, { capture: true });
		return () => {
			document.removeEventListener('click', handleDocumentClick, { capture: true });
			document.removeEventListener('keydown', handleDocumentKeydown, { capture: true });
		};
	});
</script>

<div class="topology-canvas">
	<div class="canvas-header">
		<div class="header-left">
			<Activity size={14} />
			<span class="title">Live Topology Canvas</span>
			<span class="context">/ {selection.active.label || 'Fleet'}</span>
		</div>
		<div class="header-right">
			<div class="zoom-controls">
				<button class="btn-zoom" type="button" onclick={() => setZoom(1)}>{Math.round(zoom * 100)}%</button>
				<button class="btn-zoom" type="button" aria-label="Zoom in" onclick={() => setZoom(zoom + 0.1)}>+</button>
				<button class="btn-zoom" type="button" aria-label="Zoom out" onclick={() => setZoom(zoom - 0.1)}>-</button>
			</div>
		</div>
	</div>

	<div
		class="svg-container"
		role="application"
		aria-label="Interactive live fleet topology"
		oncontextmenu={(e) => openContextMenu(e, 'fleet')}
	>
		{#if inventory.isLoading}
			<div class="canvas-loading">
				<Loader2 size={24} class="animate-spin" />
				<span>Fetching technical topology...</span>
			</div>
		{:else}
			<svg viewBox={topologyBounds} class="canvas-svg" style:transform={`scale(${zoom})`}>
				<!-- Connections -->
				{#each displayVms as vm}
					{@const parent = displayNodes.find(n => n.id === vm.nodeId)}
					{#if parent}
						<path 
							d="M {vm.x} {vm.y + 22} C {vm.x} {vm.y + 70}, {parent.x} {parent.y - 70}, {parent.x} {parent.y - 32}"
							class="connection-line"
							style:--status-color={getStatusColor(vm.status)}
							in:draw={{duration: 1000}}
						/>
					{/if}
				{/each}

				<!-- Nodes (Hypervisors) -->
				{#each displayNodes as node}
					<g 
						class="node-group" 
						class:node-group--active={selection.active.id === node.id}
						role="button"
						tabindex="0"
						aria-label="Host {node.name}, {node.vmCount} workloads"
						onclick={() => selection.select('node', node.id, node.name)}
						onkeydown={(e) => handleTopologyKeydown(e, 'node', node)}
						oncontextmenu={(e) => openContextMenu(e, 'node', node)}
					>
						<rect 
							x={node.x - 74} y={node.y - 34}
							width="148" height="68"
							rx="6"
							class="node-box"
							style:--status-color={getStatusColor(node.status)}
						/>
						<Server x={node.x - 58} y={node.y - 11} size={18} color={getStatusColor(node.status)} />
						<text x={node.x - 28} y={node.y - 5} class="node-label">
							{node.name}
						</text>
						<text x={node.x - 28} y={node.y + 14} class="node-meta">
							{node.vmCount} workloads
						</text>
						<circle cx={node.x + 55} cy={node.y - 20} r="4" class="status-indicator" style:fill={getStatusColor(node.status)} />
						<MoreHorizontal x={node.x + 46} y={node.y + 10} size={16} color="var(--color-neutral-500)" />
					</g>
				{/each}

				<!-- VMs -->
				{#each displayVms as vm}
					<g 
						class="vm-group"
						class:vm-group--active={selection.active.id === vm.id}
						role="button"
						tabindex="0"
						aria-label="Instance {vm.name}, {vm.stateLabel}"
						onclick={() => selection.select('vm', vm.id, vm.name)}
						onkeydown={(e) => handleTopologyKeydown(e, 'vm', vm)}
						oncontextmenu={(e) => openContextMenu(e, 'vm', vm)}
					>
						<rect 
							x={vm.x - 58} y={vm.y - 24}
							width="116" height="48"
							rx="5"
							class="vm-box"
							style:--status-color={getStatusColor(vm.status)}
						/>
						<Box x={vm.x - 45} y={vm.y - 9} size={15} color={getStatusColor(vm.status)} />
						<text x={vm.x - 22} y={vm.y - 2} class="vm-label">
							{vm.name}
						</text>
						<text x={vm.x - 22} y={vm.y + 14} class="vm-meta">
							{vm.stateLabel}
						</text>
						<circle cx={vm.x + 43} cy={vm.y - 12} r="3" class="status-indicator" style:fill={getStatusColor(vm.status)} />
					</g>
				{/each}
			</svg>
		{/if}

		{#if contextMenu}
			<div
				bind:this={menuElement}
				class="topology-menu"
				class:topology-menu--danger={contextMenu.type === 'vm'}
				style:left={`${contextMenu.x}px`}
				style:top={`${contextMenu.y}px`}
				role="menu"
				aria-label="Topology actions for {contextMenu.title}"
				transition:fade={{ duration: 80 }}
			>
				<div class="topology-menu__header">
					<strong>{contextMenu.title}</strong>
					<span>{contextMenu.subtitle}</span>
				</div>
				<div class="topology-menu__items">
					{#each contextMenu.actions as action}
						<button
							type="button"
							role="menuitem"
							disabled={action.disabled}
							class:topology-menu__item--danger={action.dangerous}
							onclick={() => runMenuAction(action)}
						>
							<span>{action.label}</span>
							{#if action.hint}<small>{action.hint}</small>{/if}
						</button>
					{/each}
				</div>
			</div>
		{/if}
	</div>

	<div class="canvas-footer">
		<div class="legend">
			<div class="legend-item"><div class="dot dc"></div> Datacenter</div>
			<div class="legend-item"><div class="dot node"></div> Node</div>
			<div class="legend-item"><div class="dot vm"></div> Workload</div>
		</div>
	</div>
</div>

<style>
	.topology-canvas {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 500px;
		box-shadow: var(--shadow-sm);
		overflow: hidden;
	}

	.canvas-header {
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--border-subtle);
		display: flex;
		justify-content: space-between;
		align-items: center;
		background: var(--bg-surface-muted);
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: var(--color-neutral-600);
	}

	.header-left .title {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.header-left .context {
		font-size: var(--text-xs);
		color: var(--color-neutral-400);
	}

	.zoom-controls {
		display: flex;
		gap: 2px;
	}

	.btn-zoom {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		padding: 2px 8px;
		font-size: 10px;
		cursor: pointer;
		border-radius: 2px;
	}

	.btn-zoom:hover {
		background: var(--color-neutral-50);
	}

	.svg-container {
		flex: 1;
		background-color: var(--bg-base);
		background-image: 
			radial-gradient(var(--dot-grid) 1px, transparent 0),
			radial-gradient(var(--dot-grid) 1px, transparent 0);
		background-position: 0 0, 10px 10px;
		background-size: 20px 20px;
		overflow: hidden;
		position: relative;
	}

	.canvas-loading {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 1rem;
		color: var(--color-neutral-400);
		font-size: 11px;
	}

	:global(.animate-spin) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.canvas-svg {
		width: 100%;
		height: 100%;
		transform-origin: center;
		transition: transform 180ms var(--ease-default);
	}

	.connection-line {
		stroke: var(--status-color, var(--color-neutral-200));
		stroke-width: 1.25;
		fill: none;
		opacity: 0.48;
		stroke-dasharray: 6 7;
		animation: dash 20s linear infinite;
	}

	@keyframes dash {
		to { stroke-dashoffset: 100; }
	}

	.node-box {
		fill: var(--bg-surface);
		stroke: color-mix(in srgb, var(--status-color) 36%, var(--border-subtle));
		stroke-width: 1;
		filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.04));
		transition:
			fill 180ms var(--ease-default),
			stroke 180ms var(--ease-default),
			stroke-width 180ms var(--ease-default);
	}

	.node-group {
		cursor: pointer;
	}

	.node-group:hover .node-box {
		stroke: var(--color-primary);
		stroke-width: 2;
	}

	.node-group--active .node-box {
		stroke: var(--color-primary);
		stroke-width: 2;
		fill: var(--color-primary-light);
	}

	.node-label {
		font-size: 12px;
		font-weight: 600;
		fill: var(--color-neutral-800);
	}

	.node-meta,
	.vm-meta {
		font-size: 9px;
		font-weight: 500;
		fill: var(--color-neutral-500);
	}

	.vm-box {
		fill: var(--bg-surface);
		stroke: color-mix(in srgb, var(--status-color) 32%, var(--border-subtle));
		stroke-width: 1;
		transition:
			fill 180ms var(--ease-default),
			stroke 180ms var(--ease-default),
			stroke-width 180ms var(--ease-default);
	}

	.vm-group {
		cursor: pointer;
	}

	.vm-group:hover .vm-box {
		stroke: var(--color-primary);
		stroke-width: 2;
	}

	.vm-group--active .vm-box {
		fill: var(--color-accent-soft);
		stroke: var(--color-accent);
		stroke-width: 2;
	}

	.vm-label {
		font-size: 10px;
		font-weight: 600;
		fill: var(--color-neutral-700);
	}

	.status-indicator {
		filter: drop-shadow(0 0 5px color-mix(in srgb, currentColor 35%, transparent));
	}

	.topology-menu {
		position: fixed;
		z-index: 60;
		width: 14rem;
		border: 1px solid var(--shell-line-strong);
		border-radius: var(--radius-sm);
		background: var(--shell-surface);
		box-shadow: var(--shadow-lg);
		overflow: hidden;
	}

	.topology-menu__header {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
		padding: 0.65rem 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		background: var(--shell-surface-muted);
	}

	.topology-menu__header strong {
		font-size: var(--text-sm);
		color: var(--shell-text);
	}

	.topology-menu__header span {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.topology-menu__items {
		padding: 0.25rem;
	}

	.topology-menu__items button {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		width: 100%;
		gap: 0.5rem;
		padding: 0.5rem;
		border: 0;
		border-radius: var(--radius-xs);
		background: transparent;
		color: var(--shell-text);
		cursor: pointer;
		text-align: left;
	}

	.topology-menu__items button:hover:not(:disabled),
	.topology-menu__items button:focus-visible {
		background: var(--shell-surface-muted);
		outline: none;
	}

	.topology-menu__items button:disabled {
		cursor: not-allowed;
		color: var(--color-neutral-400);
	}

	.topology-menu__items small {
		font-size: 9px;
		color: var(--shell-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	@media (prefers-reduced-motion: reduce) {
		.connection-line {
			animation: none;
		}

		.canvas-svg,
		.node-box,
		.vm-box {
			transition: none;
		}
	}

	.canvas-footer {
		padding: 0.5rem 0.75rem;
		border-top: 1px solid var(--border-subtle);
		background: var(--bg-surface-muted);
	}

	.legend {
		display: flex;
		gap: 1rem;
	}

	.legend-item {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 10px;
		color: var(--color-neutral-500);
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}

	.dot.dc { background: var(--color-info); }
	.dot.node { background: var(--color-primary); }
	.dot.vm { background: var(--color-accent); }
</style>
