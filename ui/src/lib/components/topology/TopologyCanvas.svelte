<script lang="ts">
	import { inventory } from '$lib/stores/inventory.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { Server, Box, Database, Activity, Loader2 } from 'lucide-svelte';
	import { fade, draw } from 'svelte/transition';

	// Dynamic positioning logic
	const displayNodes = $derived(inventory.nodes.map((n, i) => ({
		...n,
		x: 200 + (i % 2) * 300,
		y: 150 + Math.floor(i / 2) * 200,
		status: n.status === 'online' ? 'healthy' : 'warning'
	})));

	const displayVms = $derived(inventory.vms.map((v, i) => {
		const parent = displayNodes.find(n => n.id === v.node_id);
		return {
			...v,
			x: parent ? parent.x + (i % 3 - 1) * 80 : 100,
			y: parent ? parent.y - 100 : 50,
			status: v.actual_state === 'running' ? 'healthy' : 'warning'
		};
	}));

	function getStatusColor(status: string) {
		switch (status) {
			case 'healthy': return 'var(--color-success)';
			case 'warning': return 'var(--color-warning)';
			case 'danger': return 'var(--color-danger)';
			default: return 'var(--color-neutral-400)';
		}
	}
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
				<button class="btn-zoom">100%</button>
				<button class="btn-zoom">+</button>
				<button class="btn-zoom">-</button>
			</div>
		</div>
	</div>

	<div class="svg-container">
		{#if inventory.isLoading}
			<div class="canvas-loading">
				<Loader2 size={24} class="animate-spin" />
				<span>Fetching technical topology...</span>
			</div>
		{:else}
			<svg viewBox="0 0 800 600" class="canvas-svg">
				<!-- Connections -->
				{#each displayVms as vm}
					{@const parent = displayNodes.find(n => n.id === vm.node_id)}
					{#if parent}
						<path 
							d="M {vm.x} {vm.y} L {parent.x} {parent.y}" 
							class="connection-line"
							in:draw={{duration: 1000}}
						/>
					{/if}
				{/each}

				<!-- Nodes (Hypervisors) -->
				{#each displayNodes as node}
					<g 
						class="node-group" 
						class:node-group--active={selection.active.id === node.id}
						onclick={() => selection.select('node', node.id, node.name)}
					>
						<rect 
							x={node.x - 60} y={node.y - 30} 
							width="120" height="60" 
							rx="4" 
							class="node-box"
							style:--status-color={getStatusColor(node.status)}
						/>
						<text x={node.x} y={node.y + 5} text-anchor="middle" class="node-label">
							{node.name}
						</text>
						<circle cx={node.x - 45} cy={node.y - 15} r="3" class="status-indicator" style:fill={getStatusColor(node.status)} />
					</g>
				{/each}

				<!-- VMs -->
				{#each displayVms as vm}
					<g 
						class="vm-group"
						class:vm-group--active={selection.active.id === vm.id}
						onclick={() => selection.select('vm', vm.id, vm.name)}
					>
						<rect 
							x={vm.x - 45} y={vm.y - 20} 
							width="90" height="40" 
							rx="2" 
							class="vm-box"
						/>
						<text x={vm.x} y={vm.y + 5} text-anchor="middle" class="vm-label">
							{vm.name}
						</text>
					</g>
				{/each}
			</svg>
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

	.animate-spin {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.canvas-svg {
		width: 100%;
		height: 100%;
	}

	.connection-line {
		stroke: var(--color-neutral-200);
		stroke-width: 1;
		fill: none;
		stroke-dasharray: 4;
		animation: dash 20s linear infinite;
	}

	@keyframes dash {
		to { stroke-dashoffset: 100; }
	}

	.node-box {
		fill: var(--bg-surface);
		stroke: var(--border-subtle);
		stroke-width: 1;
		transition: all 0.2s ease;
	}

	.node-group {
		cursor: pointer;
	}

	.node-group:hover .node-box {
		stroke: var(--color-primary);
		box-shadow: var(--shadow-md);
	}

	.node-group--active .node-box {
		stroke: var(--color-primary);
		stroke-width: 2;
		fill: var(--color-primary-light);
	}

	.node-label {
		font-size: 11px;
		font-weight: 600;
		fill: var(--color-neutral-800);
	}

	.vm-box {
		fill: var(--bg-surface);
		stroke: var(--border-subtle);
		stroke-width: 1;
	}

	.vm-group {
		cursor: pointer;
	}

	.vm-group:hover .vm-box {
		stroke: var(--color-accent);
	}

	.vm-group--active .vm-box {
		fill: var(--color-accent-soft);
		stroke: var(--color-accent);
		stroke-width: 2;
	}

	.vm-label {
		font-size: 10px;
		fill: var(--color-neutral-600);
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
