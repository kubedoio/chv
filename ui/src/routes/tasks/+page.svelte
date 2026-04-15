<script lang="ts">
	import { PageShell, FilterPanel, ResourceTable, StateBanner, UrlPagination } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/tasks');
	const tasks = $derived(data.tasks);
	const hasAppliedFilters = $derived(Object.keys(tasks.filters.applied).length > 0);

	const filterConfig = $derived([
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'status',
			label: 'Status',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				...tasks.filters.options.statuses.map((s) => ({
					value: s,
					label: getTaskStatusMeta(s).label
				}))
			]
		},
		{
			name: 'resourceKind',
			label: 'Resource',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All resources' },
				...tasks.filters.options.resourceKinds.map((k) => ({ value: k, label: k }))
			]
		},
		{
			name: 'window',
			label: 'Window',
			type: 'select' as const,
			options: [
				{ value: 'active', label: 'Active only' },
				{ value: '24h', label: 'Last 24 hours' },
				{ value: '7d', label: 'Last 7 days' },
				{ value: '30d', label: 'Last 30 days' },
				{ value: 'all', label: 'All time' }
			]
		}
	]);

	const columns = [
		{ key: 'task', label: 'Task' },
		{ key: 'status', label: 'Status' },
		{ key: 'operation', label: 'Operation' },
		{ key: 'resource', label: 'Resource' },
		{ key: 'actor', label: 'Actor' },
		{ key: 'started', label: 'Started' },
		{ key: 'finished', label: 'Finished' },
		{ key: 'duration', label: 'Duration' }
	];

	const rows = $derived(
		tasks.items.map((task) => {
			const statusMeta = getTaskStatusMeta(task.status);
			return {
				task: task.task_id,
				status: { label: statusMeta.label, tone: statusMeta.tone },
				operation: task.operation,
				resource: `${task.resource_kind} ${task.resource_id}`,
				actor: task.actor,
				started: formatTimestamp(task.started_unix_ms),
				finished: task.finished_unix_ms ? formatTimestamp(task.finished_unix_ms) : '—',
				duration: formatDuration(task.started_unix_ms, task.finished_unix_ms),
				resource_kind: task.resource_kind,
				resource_id: task.resource_id
			};
		})
	);

	function formatTimestamp(ms: number): string {
		return new Intl.DateTimeFormat('en-US', {
			month: 'short',
			day: 'numeric',
			hour: 'numeric',
			minute: '2-digit'
		}).format(new Date(ms));
	}

	function formatDuration(startedMs: number, finishedMs: number): string {
		if (!finishedMs) return '—';
		const elapsedSeconds = Math.max(Math.round((finishedMs - startedMs) / 1000), 0);
		if (elapsedSeconds < 60) return `${elapsedSeconds}s`;
		if (elapsedSeconds < 3600) return `${Math.round(elapsedSeconds / 60)}m`;
		if (elapsedSeconds < 86400) return `${Math.round(elapsedSeconds / 3600)}h`;
		return `${Math.round(elapsedSeconds / 86400)}d`;
	}

	function rowHref(row: Record<string, unknown>): string | null {
		const kind = String(row.resource_kind ?? '');
		const id = String(row.resource_id ?? '');
		if (!kind || !id) return null;
		if (kind === 'vm') return `/vms/${id}`;
		if (kind === 'node') return `/nodes/${id}`;
		return `/${kind}s/${id}`;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<FilterPanel filters={filterConfig} values={tasks.filters.current} />

	{#if tasks.state === 'error'}
		<StateBanner
			variant="error"
			title="Task center is unavailable"
			description="The task list could not be loaded from the BFF."
			hint="Accepted, running, and completed work stays distinct once the task feed is reachable again."
		/>
	{:else if tasks.state === 'empty'}
		<StateBanner
			variant="empty"
			title={hasAppliedFilters ? 'No tasks match the current filters' : 'No tasks yet'}
			description={hasAppliedFilters
				? 'Try widening the status, resource, or time window filters to bring more task history back into view.'
				: 'Accepted operations, active runs, and completed work will appear here once the control plane starts producing task records.'}
			hint="The task center keeps filter state even when the result set is empty."
		/>
	{:else}
		<ResourceTable
			{columns}
			{rows}
			{rowHref}
			emptyTitle="No tasks match"
			emptyDescription="Try adjusting filters to see more results."
		/>
		<UrlPagination
			page={tasks.page.page}
			pageSize={tasks.page.pageSize}
			totalItems={tasks.page.totalItems}
			basePath="/tasks"
			params={tasks.filters.current}
		/>
	{/if}
</PageShell>
