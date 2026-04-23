<script lang="ts">
	import { Server, Box, Database } from 'lucide-svelte';

	interface Props {
		type: string;
		label: string;
		id: string;
	}

	let { type, label, id }: Props = $props();
</script>

<section class="entity-identity">
	<div class="entity-icon" class:is-node={type === 'node'} class:is-vm={type === 'vm'}>
		{#if type === 'node'}<Server size={18} />
		{:else if type === 'vm'}<Box size={18} />
		{:else}<Database size={18} />{/if}
	</div>
	<div class="entity-meta">
		<h3 class="entity-name">{label}</h3>
		<span class="entity-id">ID // {id.slice(0, 12)}</span>
	</div>
</section>

<style>
	.entity-identity {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.entity-icon {
		width: 40px;
		height: 40px;
		display: grid;
		place-items: center;
		background: var(--bg-surface-muted);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		color: var(--color-neutral-500);
	}

	.entity-icon.is-node { color: var(--color-primary); background: rgba(var(--color-primary-rgb), 0.1); }
	.entity-icon.is-vm { color: var(--color-accent); background: rgba(var(--color-accent-rgb), 0.1); }

	.entity-meta {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.entity-name {
		font-size: 14px;
		font-weight: 800;
		color: var(--color-neutral-900);
		margin: 0;
		line-height: 1;
	}

	.entity-id {
		font-size: 9px;
		font-weight: 700;
		color: var(--color-neutral-400);
		font-family: var(--font-mono);
	}
</style>
