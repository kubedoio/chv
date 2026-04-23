<script lang="ts">
	import { Info, X } from 'lucide-svelte';
	import { selection } from '$lib/stores/selection.svelte';
</script>

<header class="drawer-header">
	<div class="header-title">
		<Info size={12} />
		<span>TECH_INSPECTOR</span>
	</div>
	<button class="btn-close" onclick={() => selection.clear()} aria-label="Close inspector">
		<X size={12} />
	</button>
</header>

<style>
	.drawer-header {
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--border-subtle);
		display: flex;
		justify-content: space-between;
		align-items: center;
		background: var(--bg-surface-muted);
	}

	.header-title {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 10px;
		font-weight: 800;
		color: var(--color-neutral-500);
		letter-spacing: 0.1em;
	}

	.btn-close {
		background: transparent;
		border: none;
		color: var(--color-neutral-400);
		cursor: pointer;
		padding: 0.25rem;
		display: flex;
	}

	.btn-close:hover { color: var(--color-neutral-900); }
</style>
