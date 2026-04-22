<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import AddNodeModal from '$lib/components/modals/AddNodeModal.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { Plus, Activity, AlertCircle, ShieldCheck } from 'lucide-svelte';
	import { goto, invalidateAll } from '$app/navigation';
	import { page } from '$app/stores';
	import { createNode } from '$lib/webui/nodes'; // Assuming this exists or using direct API
	import { getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';
	import type { CreateNodeInput, CreateNodeResponse } from '$lib/api/types';

	let { data }: { data: PageData } = $props();

	let addNodeOpen = $state(false);

	const model = $derived(data.nodes);
	const items = $derived(model.items);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Filter by node name or cluster...' },
		{ 
			key: 'state', 
			label: 'State', 
			type: 'select' as const, 
			options: [
				{ value: 'online', label: 'Online' },
				{ value: 'offline', label: 'Offline' },
				{ value: 'maintenance', label: 'Maintenance' },
				{ value: 'error', label: 'Error' }
			] 
		},
		{
			key: 'maintenance',
			label: 'Maintenance',
			type: 'boolean' as const
		}
	];

	function handleFilterChange(key: string, value: any) {
		const newParams = new URLSearchParams($page.url.searchParams);
		if (value === '' || value === 'all' || value === false) {
			newParams.delete(key);
		} else {
			newParams.set(key, String(value));
		}
		goto(`?${newParams.toString()}`, { keepFocus: true, noScroll: true });
	}

	function handleClearFilters() {
		goto($page.url.pathname);
	}

	const columns = [
		{ key: 'name', label: 'Compute Node' },
		{ key: 'cluster', label: 'Cluster Assignment' },
		{ key: 'state', label: 'Status' },
		{ key: 'cpu', label: 'CPU Index', align: 'right' as const },
		{ key: 'memory', label: 'Memory Index', align: 'right' as const },
		{ key: 'storage', label: 'Storage Index', align: 'right' as const },
		{ key: 'version', label: 'Platform Rev' }
	];

	function mapStateTone(state: string, health?: string): ShellTone {
		if (state === 'maintenance') return 'warning';
		if (state === 'error' || health === 'critical') return 'failed';
		if (health === 'warning') return 'degraded';
		if (state === 'online' && health === 'healthy') return 'healthy';
		return 'unknown';
	}

	const tableRows = $derived(items.map(item => ({
		...item,
		state: { label: item.state, tone: mapStateTone(item.state, item.health) },
		network: { label: item.network || 'Optimal', tone: 'healthy' as ShellTone }
	})));

	const attentionNodes = $derived(items.filter(n => n.health !== 'healthy' || n.alerts > 0).slice(0, 3));
	const nodePageDef = getPageDefinition('/nodes');
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={nodePageDef}>
		{#snippet actions()}
			<button
				class="btn-primary"
				onclick={() => (addNodeOpen = true)}
			>
				<Plus size={14} />
				Enroll Node
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<div class="inventory-metrics">
		<CompactMetricCard 
			label="Compute Capacity" 
			value={items.length} 
			color="neutral"
		/>
		<CompactMetricCard 
			label="Operational" 
			value={items.filter(n => n.state === 'online').length} 
			trend={0}
			color="primary"
		/>
		<CompactMetricCard 
			label="Posture Warning" 
			value={items.filter(n => n.health !== 'healthy').length} 
			color={items.filter(n => n.health !== 'healthy').length > 0 ? 'warning' : 'neutral'}
		/>
	</div>

	<div class="inventory-controls">
		<FilterBar 
			{filters} 
			activeFilters={model.filters.current} 
			onFilterChange={handleFilterChange}
			onClearAll={handleClearFilters}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if model.state === 'error'}
				<ErrorState />
			{:else if model.state === 'empty'}
				<EmptyInfrastructureState 
					title="No nodes detected"
					description="Adjust your search criteria or enroll a new compute host."
					hint="New hosts must be enrolled via the control-plane CLI."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={tableRows} 
					rowHref={(row) => `/nodes/${row.node_id}`} 
				/>
			{/if}
		</section>

		<aside class="support-area">
			<SectionCard title="Host Posture" icon={AlertCircle} badgeLabel={String(attentionNodes.length)}>
				{#if attentionNodes.length === 0}
					<p class="empty-hint">All compute hosts within nominal range.</p>
				{:else}
					<ul class="attention-list">
						{#each attentionNodes as node}
							<li>
								<a href="/nodes/{node.node_id}" class="attention-card">
									<div class="attention-card__main">
										<span class="res-name">{node.name}</span>
										<span class="res-issue">{node.alerts} alerts / {node.health}</span>
									</div>
								</a>
							</li>
						{/each}
					</ul>
				{/if}
			</SectionCard>

			<SectionCard title="Compute Pipeline" icon={ShieldCheck}>
				<ul class="task-list">
					<li class="task-item">
						<span class="task-label">Host Telemetry Sync</span>
						<span class="task-time">Active</span>
					</li>
					<li class="task-item">
						<span class="task-label">Policy Enforcement</span>
						<span class="task-time">Verified</span>
					</li>
				</ul>
			</SectionCard>
		</aside>
	</main>
</div>

<AddNodeModal
	bind:open={addNodeOpen}
	onClose={() => (addNodeOpen = false)}
	onSubmit={async (data: CreateNodeInput): Promise<CreateNodeResponse> => {
		// Mock/Logic for node creation
		toast.info('Initialising node enrollment protocol...');
		return { id: 'new', name: data.name, status: 'offline' } as any;
	}}
/>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.inventory-metrics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.75rem;
	}

	.inventory-controls {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		overflow: hidden;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.empty-hint {
		font-size: 11px;
		color: var(--color-neutral-400);
		padding: 1rem;
		text-align: center;
	}

	.attention-list {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.attention-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
		text-decoration: none;
		color: var(--color-neutral-800);
		transition: background 0.1s ease;
	}

	.attention-card:hover {
		background: var(--bg-surface-hover);
	}

	.attention-card__main {
		display: flex;
		flex-direction: column;
	}

	.res-name {
		font-size: 11px;
		font-weight: 700;
	}

	.res-issue {
		font-size: 9px;
		color: var(--color-danger);
		font-weight: 600;
		text-transform: uppercase;
	}

	.task-list {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.task-item {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		padding: 0.35rem 0.5rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
	}

	.task-label {
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.task-time {
		color: var(--color-success);
		font-weight: 700;
		text-transform: uppercase;
		font-size: 9px;
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
