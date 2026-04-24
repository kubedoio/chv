<script lang="ts">
	import { Server, Box, Database } from 'lucide-svelte';

	interface Props {
		type: string;
		label: string;
		id: string;
	}

	let { type, label, id }: Props = $props();

	const iconClass = $derived(
		type === 'node'
			? 'text-[var(--color-primary)] bg-[rgba(var(--color-primary-rgb),0.1)]'
			: type === 'vm'
				? 'text-[var(--color-accent)] bg-[rgba(var(--color-accent-rgb),0.1)]'
				: 'text-[var(--color-neutral-500)]'
	);
</script>

<section class="flex items-center gap-3">
	<div class="w-10 h-10 grid place-items-center bg-[var(--bg-surface-muted)] border border-[var(--border-subtle)] rounded-[var(--radius-xs)] {iconClass}">
		{#if type === 'node'}<Server size={18} />
		{:else if type === 'vm'}<Box size={18} />
		{:else}<Database size={18} />{/if}
	</div>
	<div class="flex flex-col gap-[0.125rem]">
		<h3 class="text-sm font-extrabold text-[var(--color-neutral-900)] m-0 leading-none">{label}</h3>
		<span class="text-[9px] font-bold text-[var(--color-neutral-400)] font-mono">ID // {id.slice(0, 12)}</span>
	</div>
</section>
