<script lang="ts">
	import { selection } from '$lib/stores/selection.svelte';
	import { getStoredToken } from '$lib/api/client';
	import { getVm } from '$lib/bff/vms';
	import { getNode } from '$lib/bff/nodes';
	import type { VmSummary, NodeSummary } from '$lib/bff/types';
	import { fade } from 'svelte/transition';
	import { ChevronRight, Loader2 } from 'lucide-svelte';
	import InspectorHeader from './InspectorHeader.svelte';
	import EntityIdentity from './EntityIdentity.svelte';
	import PostureCard from './PostureCard.svelte';
	import MetricBars from './MetricBars.svelte';
	import PropertyMesh from './PropertyMesh.svelte';
	import MutationControls from './MutationControls.svelte';
	import InspectorEmptyState from './InspectorEmptyState.svelte';

	const active = $derived(selection.active);
	let details = $state<any>(null);
	let isLoading = $state(false);

	$effect(() => {
		if (active.id) {
			fetchDetails(active.type, active.id);
		} else {
			details = null;
		}
	});

	function normalizeNodeStatus(state: string): string {
		const s = state.toLowerCase();
		if (s.includes('ready') || s.includes('online') || s.includes('active')) return 'online';
		if (s.includes('error') || s.includes('fail')) return 'error';
		if (s.includes('maint')) return 'maintenance';
		return 'offline';
	}

	function normalizeVmState(state: string): string {
		return state.toLowerCase();
	}

	async function fetchDetails(type: string, id: string) {
		const token = getStoredToken();
		if (!token) return;

		isLoading = true;

		try {
			if (type === 'node') {
				const res = await getNode({ node_id: id }, token);
				const summary = res.summary as NodeSummary & { architecture?: string; provider_type?: string };
				details = {
					...summary,
					id: summary.node_id,
					status: normalizeNodeStatus(summary.state),
					architecture: summary.architecture || 'x86_64',
					provider_type: summary.provider_type || 'LOCAL_HOST'
				};
			} else if (type === 'vm') {
				const res = await getVm({ vm_id: id }, token);
				const summary = res.summary as VmSummary & { architecture?: string; provider_type?: string };
				details = {
					...summary,
					id: summary.vm_id,
					actual_state: normalizeVmState(summary.power_state),
					status: normalizeVmState(summary.power_state),
					architecture: summary.architecture || 'x86_64',
					provider_type: summary.provider_type || 'LOCAL_HOST'
				};
			}
		} catch (err) {
			console.error('Failed to fetch inspector details:', err);
			details = null;
		} finally {
			isLoading = false;
		}
	}
</script>

<div class="inspect-drawer">
	<InspectorHeader />

	{#if active.id}
		<div class="drawer-content" in:fade={{ duration: 100 }}>
			<EntityIdentity type={active.type} label={active.label} id={active.id} />

			{#if isLoading}
				<div class="drawer-loading">
					<Loader2 size={16} class="animate-spin" />
					<span>COLLECTING_TELEMETRY...</span>
				</div>
			{:else if details}
				<div class="inspector-sections">
					<div class="section">
						<div class="label">Operational Posture</div>
						<PostureCard status={details.status} actual_state={details.actual_state} />
					</div>

					<div class="section">
						<div class="label">System Pulse</div>
						<MetricBars cpuUsage={0} memoryUsagePercent={0} />
					</div>

					<div class="section">
						<div class="label">Property Mesh</div>
						<PropertyMesh
							architecture={details.architecture}
							providerType={details.provider_type}
						/>
					</div>

					<div class="section">
						<div class="label">Mutation Controls</div>
						<MutationControls />
					</div>

					<a href="/{active.type}s/{active.id}" class="inspect-full-link">
						<span>FULL_INSPECTION_DETAIL</span>
						<ChevronRight size={14} />
					</a>
				</div>
			{/if}
		</div>
	{:else}
		<InspectorEmptyState />
	{/if}
</div>

<style>
	.inspect-drawer {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-surface);
	}

	.drawer-content {
		flex: 1;
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		overflow-y: auto;
	}

	.drawer-loading {
		padding: 3rem 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		font-size: 10px;
		font-weight: 800;
		color: var(--color-neutral-500);
	}

	.animate-spin {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.inspector-sections {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
	}

	.section .label {
		font-size: 9px;
		font-weight: 800;
		color: var(--color-neutral-500);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.inspect-full-link {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem;
		background: rgba(var(--color-primary-rgb), 0.1);
		color: var(--color-primary);
		text-decoration: none;
		font-size: 10px;
		font-weight: 800;
		border-radius: var(--radius-xs);
		margin-top: 0.5rem;
	}

	.inspect-full-link:hover {
		background: rgba(var(--color-primary-rgb), 0.15);
	}
</style>
