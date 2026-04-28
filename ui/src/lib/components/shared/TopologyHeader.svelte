<script lang="ts">
	import { Activity } from 'lucide-svelte';
	import { selection } from '$lib/stores/selection.svelte';

	interface Props {
		zoom: number;
		onZoom: (next: number) => void;
		onFitFleet: () => void;
		onFitSelection: () => void;
	}

	let { zoom, onZoom, onFitFleet, onFitSelection }: Props = $props();
</script>

<div class="canvas-header">
	<div class="header-left">
		<Activity size={14} />
		<span class="title">Live Topology Canvas</span>
		<span class="context">/ {selection.active.label || 'Fleet'}</span>
	</div>
	<div class="header-right">
		<div class="zoom-controls">
			<button class="btn-zoom" type="button" onclick={() => onZoom(1)}>{Math.round(zoom * 100)}%</button>
			<button class="btn-zoom" type="button" aria-label="Zoom in" onclick={() => onZoom(zoom + 0.1)}>+</button>
			<button class="btn-zoom" type="button" aria-label="Zoom out" onclick={() => onZoom(zoom - 0.1)}>-</button>
			<button class="btn-zoom btn-zoom--text" type="button" onclick={onFitFleet}>Fit</button>
			<button class="btn-zoom btn-zoom--text" type="button" onclick={onFitSelection}>Focus</button>
		</div>
	</div>
</div>

<style>
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

	.btn-zoom--text {
		min-width: 2.4rem;
	}

	.btn-zoom:hover {
		background: var(--color-neutral-50);
	}
</style>
