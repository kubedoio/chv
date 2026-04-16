<script lang="ts">
	import { PageShell, FilterPanel, StateBanner, ResourceTable } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/volumes');
	const model = $derived(data.volumes);

	function statusTone(status: string): ShellTone {
		switch (status.toLowerCase()) {
			case 'attached':
			case 'ready':
				return 'healthy';
				case 'detaching':
				case 'attaching':
				return 'warning';
			case 'failed':
			case 'error':
				return 'failed';
			default:
				return 'unknown';
		}
	}

	function healthTone(health: string): ShellTone {
		switch (health.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'degraded':
				return 'degraded';
			case 'failed':
				return 'failed';
			case 'warning':
				return 'warning';
			default:
				return 'unknown';
		}
	}

	function lastTaskTone(task: string): ShellTone {
		const t = task.toLowerCase();
		if (t.includes('fail') || t.includes('error')) return 'failed';
		if (t.includes('success') || t.includes('complete')) return 'healthy';
		return 'unknown';
	}

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'status',
			label: 'Status',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All statuses' },
				{ value: 'attached', label: 'Attached' },
				{ value: 'detached', label: 'Detached' },
				{ value: 'attaching', label: 'Attaching' },
				{ value: 'detaching', label: 'Detaching' },
				{ value: 'failed', label: 'Failed' },
				{ value: 'unknown', label: 'Unknown' }
			]
		},
		{
			name: 'health',
			label: 'Health',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All health' },
				{ value: 'healthy', label: 'Healthy' },
				{ value: 'degraded', label: 'Degraded' },
				{ value: 'failed', label: 'Failed' },
				{ value: 'unknown', label: 'Unknown' }
			]
		},
		{ name: 'nodeId', label: 'Node', type: 'search' as const }
	];

	const columns = [
		{ key: 'name', label: 'Volume' },
		{ key: 'node_id', label: 'Node' },
		{ key: 'size', label: 'Size' },
		{ key: 'status', label: 'Status' },
		{ key: 'health', label: 'Health' },
		{ key: 'attached_vm_id', label: 'Attached VM' },
		{ key: 'last_task', label: 'Last task' }
	];

	const rows = $derived(
		model.items.map((item) => ({
			volume_id: item.volume_id,
			name: item.name,
			node_id: item.node_id,
			size: item.size,
			status: { label: item.status, tone: statusTone(item.status) },
			health: { label: item.health, tone: healthTone(item.health) },
			attached_vm_id: item.attached_vm_id || '-',
			last_task: { label: item.last_task, tone: lastTaskTone(item.last_task) }
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.volume_id;
		return typeof id === 'string' ? `/volumes/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<FilterPanel filters={filterConfig} values={model.filters.current} />

	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Volume inventory unavailable"
			description="The volume roster could not be loaded from the BFF."
			hint="The UI keeps volume health, status, and task transparency ready for the next healthy refresh."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No volumes match the current view"
			description="Widen the filters or create a volume to populate this page."
			hint="The list remains URL-backed so a filtered volume view can be shared between operators."
		/>
	{:else}
		<ResourceTable {columns} {rows} {rowHref} emptyTitle="No volumes" />
	{/if}
</PageShell>
