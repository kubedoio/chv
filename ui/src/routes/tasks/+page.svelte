<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/shared/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import ResourceLink from '$lib/components/shell/ResourceLink.svelte';
	import DurationLine from '$lib/components/shell/DurationLine.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import type { PageData } from './$types';
	import { History, User, Activity, Clock, ShieldAlert } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/tasks');
	const tasks = $derived(data.tasks);
	const items = $derived(tasks.items);

	const filters = $derived([
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Task ID or resource...' },
		{
			key: 'status',
			label: 'Status',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				...tasks.filters.options.statuses.map((s: string) => ({
					value: s,
					label: getTaskStatusMeta(s).label
				}))
			]
		},
		{
			key: 'window',
			label: 'Window',
			type: 'select' as const,
			options: [
				{ value: 'active', label: 'Active only' },
				{ value: '24h', label: 'Last 24 hours' },
				{ value: '7d', label: 'Last 7 days' },
				{ value: 'all', label: 'All time' }
			]
		}
	]);

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
		{ key: 'operation', label: 'Operation ID' },
		{ key: 'status', label: 'State' },
		{ key: 'resource', label: 'Target' },
		{ key: 'actor', label: 'Principal' },
		{ key: 'started', label: 'Timestamp' },
		{ key: 'duration', label: 'Ops Duration', align: 'right' as const }
	];

	const rows = $derived(
		items.map((task) => {
			const statusMeta = getTaskStatusMeta(task.status);
			return {
				...task,
				status: { label: statusMeta.label, tone: statusMeta.tone },
				started: new Date(task.started_unix_ms).toLocaleString('en-US', {
					month: 'short',
					day: 'numeric',
					hour: 'numeric',
					minute: '2-digit'
				})
			};
		})
	);

	const failedTasks = $derived(items.filter(t => t.status === 'failed').slice(0, 3));
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={page} />

	<div class="inventory-metrics">
		<CompactMetricCard 
			label="Total Mutations" 
			value={tasks.page.totalItems} 
			color="neutral"
		/>
		<CompactMetricCard 
			label="Active Ops" 
			value={items.filter(t => t.status === 'running').length} 
			color="primary"
		/>
		<CompactMetricCard 
			label="Failed" 
			value={items.filter(t => t.status === 'failed').length} 
			color={items.filter(t => t.status === 'failed').length > 0 ? 'danger' : 'neutral'}
		/>
	</div>

	<div class="inventory-controls">
		<FilterBar 
			{filters} 
			activeFilters={tasks.filters.current} 
			onFilterChange={handleFilterChange}
			onClearAll={() => goto($appPage.url.pathname)}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if tasks.state === 'error'}
				<ErrorState />
			{:else if tasks.state === 'empty'}
				<EmptyInfrastructureState 
					title="Operation trace empty" 
					description="Adjust your search criteria or time window." 
					hint="All control-plane mutations are recorded in the audit registry."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={rows}
				>
					{#snippet cell({ column, row })}
						{#if column.key === 'operation'}
							<div class="op-cell">
								<span class="op-name">{row.operation}</span>
								<span class="op-id">{row.task_id}</span>
							</div>
						{:else if column.key === 'resource'}
							<ResourceLink kind={row.resource_kind} id={row.resource_id} name={row.resource_name} compact />
						{:else if column.key === 'actor'}
							<div class="actor-cell">
								<User size={10} />
								<span>{row.actor}</span>
							</div>
						{:else if column.key === 'duration'}
							<DurationLine startedMs={row.started_unix_ms} finishedMs={row.finished_unix_ms} />
						{:else if typeof row[column.key] === 'object' && row[column.key]?.tone}
							<StatusBadge label={row[column.key].label} tone={row[column.key].tone} />
						{:else}
							<span class="cell-text">{row[column.key]}</span>
						{/if}
					{/snippet}
				</InventoryTable>
			{/if}
		</section>

		<aside class="support-area">
			<SectionCard title="Mutation Audit" icon={History}>
				<div class="audit-summary">
					<div class="summary-row">
						<span>Retention Policy</span>
						<span>30 Days</span>
					</div>
					<div class="summary-row">
						<span>Consistency</span>
						<span>Verified</span>
					</div>
				</div>
			</SectionCard>

			<SectionCard title="Failed Operations" icon={ShieldAlert} badgeLabel={String(failedTasks.length)}>
				{#if failedTasks.length === 0}
					<p class="empty-hint">No operational failures in the current window.</p>
				{:else}
					<ul class="attention-list">
						{#each failedTasks as task}
							<li>
								<div class="attention-card">
									<div class="attention-card__main">
										<span class="res-name">{task.operation}</span>
										<span class="res-issue">Failure Registry: {task.task_id.split('-')[0]}</span>
									</div>
								</div>
							</li>
						{/each}
					</ul>
				{/if}
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

	.inventory-metrics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.75rem;
	}

	.inventory-controls {
		border-radius: 0.35rem;
		overflow: hidden;
	}

	.single-col {
		grid-template-columns: 1fr !important;
	}

	.mono-id {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--shell-text-muted);
	}

	.actor-cell {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.actor-icon {
		opacity: 0.5;
	}
</style>
