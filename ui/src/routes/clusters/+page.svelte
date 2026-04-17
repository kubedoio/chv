<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Blocks, AlertTriangle, Activity, Server, Plus, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/clusters');
	const model = $derived(data.clusters);
	const items = $derived(model.items);

	const stats = $derived([
		{ label: 'Total Clusters', value: String(items.length) },
		{ label: 'Healthy', value: String(items.filter(c => c.state === 'healthy').length), status: 'healthy' as const },
		{ label: 'Degraded', value: String(items.filter(c => c.state === 'degraded' || c.state === 'failed').length), status: 'critical' as const },
		{ label: 'Maintenance', value: String(items.filter(c => c.maintenance).length), status: 'warning' as const },
		{ label: 'Total Alerts', value: String(items.reduce((sum, c) => sum + (c.alerts || 0), 0)), status: 'neutral' as const }
	]);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Cluster or DC name...' },
		{
			key: 'state',
			label: 'Readiness',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				{ value: 'healthy', label: 'Healthy' },
				{ value: 'degraded', label: 'Degraded' },
				{ value: 'maintenance', label: 'Maintenance' }
			]
		}
	];

	function handleFilterChange(key: string, value: any) {
		const newParams = new URLSearchParams($appPage.url.searchParams);
		if (value === '' || value === 'all') {
			newParams.delete(key);
		} else {
			newParams.set(key, String(value));
		}
		goto(`?${newParams.toString()}`, { keepFocus: true, noScroll: true });
	}

	const columns = [
		{ key: 'name', label: 'Cluster' },
		{ key: 'datacenter', label: 'Datacenter' },
		{ key: 'node_count', label: 'Nodes', align: 'center' as const },
		{ key: 'state', label: 'Readiness' },
		{ key: 'capacity', label: 'Peak Load' },
		{ key: 'active_tasks', label: 'Tasks', align: 'right' as const },
		{ key: 'alerts', label: 'Alerts', align: 'right' as const }
	];

	const rows = $derived(
		items.map((item) => {
			const peak = Math.max(item.cpu_percent, item.memory_percent, item.storage_percent);
			return {
				...item,
				capacity: { 
					label: `${peak}% peak`, 
					tone: peak >= 85 ? 'failed' : peak >= 70 ? 'degraded' : peak >= 50 ? 'warning' : 'healthy' 
				},
				state: {
					label: item.state,
					tone: item.state === 'healthy' ? 'healthy' : item.state === 'degraded' ? 'degraded' : 'failed'
				}
			};
		})
	);

	const attentionClusters = $derived(items.filter(c => c.state !== 'healthy' || c.alerts > 0).slice(0, 3));
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={page}>
		{#snippet actions()}
			<button class="btn-primary">
				<Plus size={14} />
				Enroll Cluster
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<div class="posture-strip-wrapper">
		<CompactStatStrip {stats} />
	</div>

	<div class="inventory-controls">
		<FilterBar 
			{filters} 
			activeFilters={model.filters.current} 
			onFilterChange={handleFilterChange}
			onClearAll={() => goto($appPage.url.pathname)}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if model.state === 'error'}
				<ErrorState />
			{:else if model.state === 'empty'}
				<EmptyInfrastructureState 
					title="No clusters enrolled" 
					description="Connect a compute provider to the control plane." 
					hint="Enrollment will populate this inventory with health and capacity metrics."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={rows}
					rowHref={(row) => `/clusters/${row.cluster_id}`}
				/>
			{/if}
		</section>

		<aside class="support-area">
			<SectionCard title="Priority Inspection" icon={AlertTriangle} badgeTone={attentionClusters.length > 0 ? 'warning' : 'neutral'}>
				{#if attentionClusters.length === 0}
					<p class="empty-hint">All clusters reporting nominal readiness.</p>
				{:else}
					<ul class="priority-list">
						{#each attentionClusters as cluster}
							<li>
								<div class="priority-item">
									<div class="priority-main">
										<span class="p-name">{cluster.name}</span>
										<div class="p-meta">
											<span class="p-dc">{cluster.datacenter}</span>
											<span class="dot">·</span>
											<span class="p-stat" class:degraded={cluster.state !== 'healthy'}>{cluster.state}</span>
										</div>
									</div>
									<a href="/clusters/{cluster.cluster_id}" class="p-link">
										<ChevronRight size={14} />
									</a>
								</div>
							</li>
						{/each}
					</ul>
				{/if}
			</SectionCard>

			<SectionCard title="Fleet Capacity" icon={Activity}>
				<div class="capacity-stat">
					<div class="cap-header">
						<span>Total Compute Nodes</span>
						<span>{items.reduce((sum, c) => sum + c.node_count, 0)}</span>
					</div>
					<div class="cap-header">
						<span>Hotspots (>80%)</span>
						<span class={items.filter(c => Math.max(c.cpu_percent, c.memory_percent) > 80).length > 0 ? 'failed' : ''}>
							{items.filter(c => Math.max(c.cpu_percent, c.memory_percent) > 80).length}
						</span>
					</div>
				</div>
			</SectionCard>
		</aside>
	</main>
</div>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.posture-strip-wrapper {
		margin-top: -0.25rem;
	}

	.inventory-controls {
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		overflow: hidden;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.priority-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.priority-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.25rem;
	}

	.priority-main {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.p-name {
		font-weight: 600;
		font-size: var(--text-sm);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.p-meta {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: 10px;
		color: var(--shell-text-muted);
	}

	.p-stat.degraded {
		color: var(--color-danger);
		font-weight: 700;
	}

	.p-link {
		color: var(--shell-text-muted);
	}

	.capacity-stat {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.cap-header {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-xs);
		font-weight: 500;
	}

	.cap-header .failed {
		color: var(--color-danger);
		font-weight: 700;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 0.5rem 0;
	}

	@media (max-width: 1200px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
