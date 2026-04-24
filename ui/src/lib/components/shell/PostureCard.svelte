<script lang="ts">
	import { ShieldCheck, AlertTriangle } from 'lucide-svelte';

	interface Props {
		status: string;
		actual_state?: string;
	}

	let { status, actual_state }: Props = $props();

	const isHealthy = $derived(status === 'online' || actual_state === 'running');
</script>

<div class="flex items-center gap-3 p-3 rounded-sm {isHealthy ? 'border-l-2 border-[var(--color-success)]' : 'border-l-2 border-[var(--color-warning)]'}" style={isHealthy ? 'background: rgba(var(--color-success-rgb), 0.08)' : 'background: rgba(var(--color-warning-rgb), 0.08)'}>
	{#if isHealthy}
		<ShieldCheck size={14} class="text-[var(--color-success)]" />
		<div class="flex flex-col gap-[0.125rem]">
			<span class="text-[10px] font-extrabold text-[var(--color-neutral-900)]">HEALTH_NOMINAL</span>
			<span class="text-[10px] text-[var(--color-neutral-500)]">Signals within expected thresholds.</span>
		</div>
	{:else}
		<AlertTriangle size={14} class="text-[var(--color-warning)]" />
		<div class="flex flex-col gap-[0.125rem]">
			<span class="text-[10px] font-extrabold text-[var(--color-neutral-900)]">DEGRADED_STATE</span>
			<span class="text-[10px] text-[var(--color-neutral-500)]">Incomplete signal chain or offline.</span>
		</div>
	{/if}
</div>
