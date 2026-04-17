<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import ResourceLink from '$lib/components/shell/ResourceLink.svelte';
	import DurationLine from '$lib/components/shell/DurationLine.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import type { PageData } from './$types';
	import { History, User, Activity, Clock } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/tasks');
	const tasks = $derived(data.tasks);
	const items = $derived(tasks.items);

	const stats = $derived([
		{ label: 'Total Tasks', value: tasks.page.totalItems },
		{ label: 'Running', value: items.filter(t => t.status === 'running').length, status: 'warning' as const },
		{ label: 'Failed', value: items.filter(t => t.status === 'failed').length, status: 'critical' as const },
		{ label: 'Completed', value: items.filter(t => t.status === 'succeeded').length, status: 'healthy' as const }
	]);

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
		{ key: 'task_id', label: 'Task ID' },
		{ key: 'status', label: 'Status' },
		{ key: 'operation', label: 'Operation' },
		{ key: 'resource', label: 'Target Resource' },
		{ key: 'actor', label: 'Triggered By' },
		{ key: 'started', label: 'Started' },
		{ key: 'duration', label: 'Duration', align: 'right' as const }
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
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={page} />

	<div class="posture-strip-wrapper">
		<CompactStatStrip {stats} />
	</div>

	<div class="inventory-controls">
		<FilterBar 
			{filters} 
			activeFilters={tasks.filters.current} 
			onFilterChange={handleFilterChange}
			onClearAll={() => goto($appPage.url.pathname)}
		/>
	</div>

	<main class="inventory-main single-col">
		<section class="inventory-table-area">
			{#if tasks.state === 'error'}
				<ErrorState />
			{:else if tasks.state === 'empty'}
				<EmptyInfrastructureState 
					title="No tasks match these filters" 
					description="Adjust your search criteria or time window." 
					hint="Recent mutations in the control plane will appear here shortly."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={rows}
				>
					{#snippet cell({ column, row })}
						{#if column.key === 'task_id'}
							<span class="mono-id">{row.task_id}</span>
						{:else if column.key === 'resource'}
							<ResourceLink kind={row.resource_kind} id={row.resource_id} name={row.resource_name} compact />
						{:else if column.key === 'actor'}
							<div class="actor-cell">
								<User size={12} class="actor-icon" />
								<span>{row.actor}</span>
							</div>
						{:else if column.key === 'duration'}
							<DurationLine startedMs={row.started_unix_ms} finishedMs={row.finished_unix_ms} />
						{:else}
							{row[column.key]}
						{/if}
					{/snippet}
				</InventoryTable>
			{/if}
		</section>
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
