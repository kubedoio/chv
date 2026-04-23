<script lang="ts">
	import { ShieldCheck, AlertTriangle } from 'lucide-svelte';

	interface Props {
		status: string;
		actual_state?: string;
	}

	let { status, actual_state }: Props = $props();

	const isHealthy = $derived(status === 'online' || actual_state === 'running');
</script>

<div class="posture-card" class:is-warning={!isHealthy}>
	{#if isHealthy}
		<ShieldCheck size={14} class="text-success" />
		<div class="posture-info">
			<span class="status">HEALTH_NOMINAL</span>
			<span class="detail">Signals within expected thresholds.</span>
		</div>
	{:else}
		<AlertTriangle size={14} class="text-warning" />
		<div class="posture-info">
			<span class="status">DEGRADED_STATE</span>
			<span class="detail">Incomplete signal chain or offline.</span>
		</div>
	{/if}
</div>

<style>
	.posture-card {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		background: rgba(var(--color-success-rgb), 0.08);
		border-left: 2px solid var(--color-success);
		border-radius: 2px;
	}

	.posture-card.is-warning {
		background: rgba(var(--color-warning-rgb), 0.08);
		border-left-color: var(--color-warning);
	}

	.posture-info {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.posture-info .status {
		font-size: 10px;
		font-weight: 800;
		color: var(--color-neutral-900);
	}

	.posture-info .detail {
		font-size: 10px;
		color: var(--color-neutral-500);
	}

	.text-success { color: var(--color-success); }
	.text-warning { color: var(--color-warning); }
</style>
