<script lang="ts">
	import { Box, Database, Network, Cpu } from 'lucide-svelte';

	interface Props {
		kind: string;
		id: string;
		name?: string;
		compact?: boolean;
	}

	let { kind, id, name, compact = false }: Props = $props();

	const label = $derived(name || id);
	
	const config = $derived.by(() => {
		const k = kind.toLowerCase();
		if (k.includes('vm')) return { icon: Box, href: `/vms/${id}`, label: 'VM' };
		if (k.includes('volume')) return { icon: Database, href: `/volumes/${id}`, label: 'Volume' };
		if (k.includes('network')) return { icon: Network, href: `/networks/${id}`, label: 'Network' };
		if (k.includes('node') || k.includes('host')) return { icon: Cpu, href: `/nodes/${id}`, label: 'Node' };
		return { icon: Box, href: '#', label: kind };
	});
</script>

<a href={config.href} class="resource-link" class:is-compact={compact}>
	<config.icon size={12} class="res-icon" />
	{#if !compact}
		<span class="res-kind">{config.label}</span>
	{/if}
	<span class="res-name">{label}</span>
</a>

<style>
	.resource-link {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		text-decoration: none;
		color: var(--shell-text);
		font-weight: 500;
		transition: color 0.15s ease;
	}

	.resource-link:hover {
		color: var(--shell-accent);
	}

	.res-icon {
		color: var(--shell-text-muted);
	}

	.res-kind {
		font-size: 10px;
		text-transform: uppercase;
		font-weight: 700;
		color: var(--shell-text-muted);
		letter-spacing: 0.05em;
	}

	.res-name {
		font-size: var(--text-sm);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 200px;
	}

	.is-compact .res-name {
		font-size: var(--text-xs);
	}
</style>
