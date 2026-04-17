<script lang="ts">
	import {
		PageShell,
		FilterPanel,
		ResourceTable,
		StateBanner,
		UrlPagination,
		PostureStrip,
		PostureCard,
		Badge
	} from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { AlertTriangle, ArrowRight } from 'lucide-svelte';

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
				{ value: 'host_ready', label: 'Host ready' },
				{ value: 'draining', label: 'Draining' },
				{ value: 'degraded', label: 'Degraded' },
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

	function mapStateTone(state: string): ShellTone {
		switch (state.toLowerCase()) {
			case 'host_ready':
			case 'online':
				return 'healthy';
			case 'draining':
			case 'maintenance':
			case 'bootstrapping':
				return 'warning';
			case 'degraded':
				return 'degraded';
			case 'failed':
			case 'offline':
				return 'failed';
			default:
				return 'unknown';
		}
	}

	function mapNetworkTone(network: string): ShellTone {
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

	const posture = $derived(() => {
		const items = model.items;
		return {
			total: items.length,
			healthy: items.filter((n) => n.state === 'host_ready' && n.health === 'healthy').length,
			degraded: items.filter((n) => n.health !== 'healthy' || n.state === 'degraded').length,
			maintenance: items.filter((n) => n.maintenance).length,
			tasks: items.reduce((sum, n) => sum + n.active_tasks, 0),
			alerts: items.reduce((sum, n) => sum + n.alerts, 0)
		};
	});

	const attentionNodes = $derived(
		model.items.filter((n) => n.health !== 'healthy' || n.alerts > 0 || n.maintenance || n.state === 'draining')
	);

	const columns = [
		{ key: 'name', label: 'Node' },
		{ key: 'cluster', label: 'Cluster' },
		{ key: 'state', label: 'State' },
		{ key: 'cpu', label: 'CPU' },
		{ key: 'memory', label: 'Memory' },
		{ key: 'storage', label: 'Storage' },
		{ key: 'network', label: 'Network' },
		{ key: 'version', label: 'Version' },
		{ key: 'maintenance', label: 'Maintenance' },
		{ key: 'tasks', label: 'Tasks' }
	];

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
			},
			tasks: item.active_tasks
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.node_id;
		return typeof id === 'string' ? `/nodes/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<div class="nodes-header">
		<PostureStrip
			chips={[
				{ label: 'Nodes', value: posture().total },
				{
					label: 'Healthy',
					value: posture().healthy,
					variant: posture().healthy === posture().total ? 'healthy' : 'default'
				},
				{
					label: 'Degraded',
					value: posture().degraded,
					variant: posture().degraded > 0 ? 'degraded' : 'default'
				},
				{
					label: 'Maintenance',
					value: posture().maintenance,
					variant: posture().maintenance > 0 ? 'warning' : 'default'
				},
				{
					label: 'Tasks',
					value: posture().tasks,
					variant: posture().tasks > 0 ? 'warning' : 'default'
				},
				{
					label: 'Alerts',
					value: posture().alerts,
					variant: posture().alerts > 0 ? 'failed' : 'default'
				}
			]}
		/>
		<a href="/install" class="nodes-header__action">Enroll node</a>
	</div>

	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Node inventory unavailable"
			description="The node roster could not be loaded from the control plane."
			hint="Navigation remains available while the node view recovers."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No nodes match the current view"
			description="Try widening the search or state filters, or enroll a compute host to populate this page."
		/>
	{:else}
		<section class="attention-panel" aria-labelledby="attention-title">
			<div class="attention-panel__header">
				<h2 id="attention-title" class="attention-panel__title">Nodes needing attention</h2>
				{#if attentionNodes.length === 0}
					<Badge label="All healthy" tone="healthy" />
				{:else}
					<Badge label="{attentionNodes.length} open" tone="warning" />
				{/if}
			</div>

			{#if attentionNodes.length === 0}
				<div class="attention-panel__empty">
					<p class="attention-panel__empty-text">All nodes are operating normally.</p>
				</div>
			{:else}
				<ul class="attention-list" role="list">
					{#each attentionNodes as node}
						<li class="attention-item">
							<div class="attention-item__main">
								<div class="attention-item__name">{node.name}</div>
								<div class="attention-item__meta">
									<Badge label={node.state} tone={mapStateTone(node.state)} />
									<Badge label={node.health} tone={mapStateTone(node.health)} />
									{#if node.maintenance}
										<Badge label="Maintenance" tone="warning" />
									{/if}
									{#if node.alerts > 0}
										<span class="attention-signal attention-signal--alert">
											<AlertTriangle size={14} aria-hidden="true" />
											{node.alerts} alert{node.alerts === 1 ? '' : 's'}
										</span>
									{/if}
								</div>
							</div>
								<a href={`/nodes/${node.node_id}`} class="attention-item__cta">
									View node
									<ArrowRight size={14} aria-hidden="true" />
								</a>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		<div class="posture-grid">
			<PostureCard
				label="Capacity pressure"
				value="{model.items.filter((n) => parseInt(n.cpu, 10) >= 70 || parseInt(n.memory, 10) >= 80).length} nodes"
				note="Nodes with high CPU or memory utilization."
			/>
			<PostureCard
				label="Version skew"
				value="{new Set(model.items.map((n) => n.version)).size} versions"
				note="Distinct node versions across the fleet."
			/>
			<PostureCard
				label="Maintenance impact"
				value="{posture().maintenance} node{posture().maintenance === 1 ? '' : 's'}"
				note={posture().maintenance > 0 ? 'Scheduling is paused on affected nodes.' : 'No maintenance windows active.'}
			/>
			<PostureCard
				label="Active node tasks"
				value={posture().tasks}
				note="Tasks scoped to nodes in the last hour."
			/>
		</div>

		<section class="inventory-section" aria-labelledby="inventory-title">
			<h2 id="inventory-title" class="inventory-section__title">Node inventory</h2>
			<FilterPanel filters={filterConfig} values={model.filters.current} />
			<ResourceTable {columns} {rows} {rowHref} emptyTitle="No nodes match the current filters" />
			<UrlPagination
				page={model.page.page}
				pageSize={model.page.pageSize}
				totalItems={model.page.totalItems}
				basePath="/nodes"
				params={model.filters.current}
			/>
		</section>
	{/if}
</PageShell>

<style>
	.nodes-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.nodes-header__action {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: 1px solid color-mix(in srgb, var(--shell-line-strong) 75%, var(--shell-accent) 25%);
		border-radius: 0.85rem;
		padding: 0.55rem 0.8rem;
		background: color-mix(in srgb, var(--shell-surface) 70%, var(--shell-accent) 30%);
		color: var(--shell-text);
		text-decoration: none;
		font-size: 0.84rem;
		font-weight: 700;
		letter-spacing: 0.03em;
		text-transform: uppercase;
	}

	.nodes-header__action:hover {
		border-color: color-mix(in srgb, var(--shell-line-strong) 58%, var(--shell-accent) 42%);
		background: color-mix(in srgb, var(--shell-surface) 58%, var(--shell-accent) 42%);
	}

	.attention-panel {
		border: 1px solid var(--shell-line-strong);
		border-radius: 1.15rem;
		background: var(--shell-surface);
		padding: 1.25rem;
	}

	.attention-panel__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		margin-bottom: 1rem;
	}

	.attention-panel__title {
		font-size: 1.1rem;
		font-weight: 700;
		color: var(--shell-text);
		margin: 0;
	}

	.attention-panel__empty {
		padding: 1.5rem 0.5rem;
	}

	.attention-panel__empty-text {
		font-size: 0.95rem;
		color: var(--shell-text-secondary);
		margin: 0;
	}

	.attention-list {
		display: grid;
		gap: 0.6rem;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.attention-item {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 1rem;
		align-items: center;
		border: 1px solid var(--shell-line);
		border-radius: 0.9rem;
		background: var(--shell-surface-muted);
		padding: 0.9rem 1rem;
	}

	.attention-item__main {
		display: grid;
		gap: 0.35rem;
		min-width: 0;
	}

	.attention-item__name {
		font-weight: 700;
		color: var(--shell-text);
	}

	.attention-item__meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.attention-signal--alert {
		font-size: 0.8rem;
		color: var(--status-failed-text);
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
	}

	.attention-item__cta {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--shell-accent);
		text-decoration: none;
		white-space: nowrap;
	}

	.attention-item__cta:hover {
		text-decoration: underline;
	}

	.posture-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1rem;
	}

	.inventory-section {
		display: grid;
		gap: 1.2rem;
	}

	.inventory-section__title {
		font-size: 1rem;
		font-weight: 700;
		color: var(--shell-text);
		margin: 0;
	}

	@media (max-width: 1080px) {
		.posture-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 640px) {
		.posture-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
