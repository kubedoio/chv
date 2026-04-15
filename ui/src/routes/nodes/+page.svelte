<script lang="ts">
	import { PageShell, FilterPanel, ResourceTable, StateBanner, UrlPagination } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/nodes');
	const model = $derived(data.nodes);

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'state',
			label: 'State',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				{ value: 'discovered', label: 'Discovered' },
				{ value: 'bootstrapping', label: 'Bootstrapping' },
				{ value: 'host_ready', label: 'Host ready' },
				{ value: 'storage_ready', label: 'Storage ready' },
				{ value: 'network_ready', label: 'Network ready' },
				{ value: 'tenant_ready', label: 'Tenant ready' },
				{ value: 'degraded', label: 'Degraded' },
				{ value: 'draining', label: 'Draining' },
				{ value: 'maintenance', label: 'Maintenance' },
				{ value: 'failed', label: 'Failed' }
			]
		},
		{
			name: 'maintenance',
			label: 'Maintenance',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All nodes' },
				{ value: 'true', label: 'In maintenance' },
				{ value: 'false', label: 'Scheduling enabled' }
			]
		}
	];

	const columns = [
		{ key: 'name', label: 'Node' },
		{ key: 'cluster', label: 'Cluster' },
		{ key: 'state', label: 'State' },
		{ key: 'cpu', label: 'CPU' },
		{ key: 'memory', label: 'Memory' },
		{ key: 'storage', label: 'Storage' },
		{ key: 'network', label: 'Network' },
		{ key: 'version', label: 'Version' },
		{ key: 'maintenance', label: 'Maintenance' }
	];

	function mapStateTone(state: string): 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown' {
		switch (state.toLowerCase()) {
			case 'online':
			case 'host_ready':
			case 'storage_ready':
			case 'network_ready':
			case 'tenant_ready':
				return 'healthy';
			case 'maintenance':
			case 'bootstrapping':
			case 'draining':
				return 'warning';
			case 'offline':
			case 'degraded':
				return 'degraded';
			case 'failed':
				return 'failed';
			case 'discovered':
				return 'unknown';
			default:
				return 'unknown';
		}
	}

	function mapNetworkTone(network: string): 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown' {
		switch (network.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'warning':
				return 'warning';
			case 'degraded':
				return 'degraded';
			case 'failed':
			case 'error':
				return 'failed';
			default:
				return 'unknown';
		}
	}

	const rows = $derived(
		model.items.map((item) => ({
			node_id: item.node_id,
			name: item.name,
			cluster: item.cluster,
			state: { label: item.state, tone: mapStateTone(item.state) },
			cpu: item.cpu,
			memory: item.memory,
			storage: item.storage,
			network: { label: item.network, tone: mapNetworkTone(item.network) },
			version: item.version,
			maintenance: {
				label: item.maintenance ? 'In maintenance' : 'Enabled',
				tone: item.maintenance ? ('warning' as const) : ('healthy' as const)
			}
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.node_id;
		return typeof id === 'string' ? `/nodes/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<FilterPanel filters={filterConfig} values={model.filters.current} />

	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Node inventory unavailable"
			description="The control-plane view model for nodes could not be assembled from the current responses."
			hint="The shell remains usable while the page waits for a healthy refresh."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No nodes match the current view"
			description="Try widening the search or state filters, or enroll a compute host to populate this page."
			hint="Node list filters stay URL-backed so operators can share a filtered view."
		/>
	{:else}
		<ResourceTable {columns} {rows} {rowHref} emptyTitle="No nodes" />
		<UrlPagination
			page={model.page.page}
			pageSize={model.page.pageSize}
			totalItems={model.page.totalItems}
			basePath="/nodes"
			params={model.filters.current}
		/>
	{/if}
</PageShell>
