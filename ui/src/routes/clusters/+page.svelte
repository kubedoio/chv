<script lang="ts">
	import {
		PageShell,
		FilterPanel,
		ResourceTable,
		StateBanner,
		UrlPagination,
		Badge
	} from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { Search, ArrowRight, AlertTriangle, Activity, Server } from 'lucide-svelte';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/clusters');
	const model = $derived(data.clusters);

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'state',
			label: 'State',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				{ value: 'healthy', label: 'Healthy' },
				{ value: 'warning', label: 'Warning' },
				{ value: 'degraded', label: 'Degraded' },
				{ value: 'failed', label: 'Failed' }
			]
		},
		{
			name: 'maintenance',
			label: 'Maintenance',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All clusters' },
				{ value: 'true', label: 'In maintenance' },
				{ value: 'false', label: 'Scheduling enabled' }
			]
		}
	];

	function mapStateTone(state: string): ShellTone {
		switch (state.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'warning':
			case 'maintenance':
				return 'warning';
			case 'degraded':
				return 'degraded';
			case 'failed':
				return 'failed';
			default:
				return 'unknown';
		}
	}

	function capacityTone(cpu: number, memory: number, storage: number): ShellTone {
		const max = Math.max(cpu, memory, storage);
		if (max >= 85) return 'failed';
		if (max >= 70) return 'degraded';
		if (max >= 55) return 'warning';
		return 'healthy';
	}

	const posture = $derived(() => {
		const items = model.items;
		return {
			total: items.length,
			healthy: items.filter((c) => c.state === 'healthy').length,
			degraded: items.filter((c) => c.state === 'degraded' || c.state === 'failed').length,
			maintenance: items.filter((c) => c.maintenance).length,
			tasks: items.reduce((sum, c) => sum + c.active_tasks, 0),
			alerts: items.reduce((sum, c) => sum + c.alerts, 0)
		};
	});

	const attentionClusters = $derived(
		model.items.filter((c) => c.state !== 'healthy' || c.alerts > 0 || c.maintenance)
	);

	const columns = [
		{ key: 'name', label: 'Cluster' },
		{ key: 'datacenter', label: 'Datacenter' },
		{ key: 'node_count', label: 'Nodes' },
		{ key: 'state', label: 'Readiness' },
		{ key: 'maintenance', label: 'Maintenance' },
		{ key: 'version', label: 'Version' },
		{ key: 'capacity', label: 'Capacity' },
		{ key: 'active_tasks', label: 'Tasks' },
		{ key: 'alerts', label: 'Alerts' }
	];

	const rows = $derived(
		model.items.map((item) => ({
			cluster_id: item.cluster_id,
			name: item.name,
			datacenter: item.datacenter,
			node_count: item.node_count,
			state: { label: item.state, tone: mapStateTone(item.state) },
			maintenance: {
				label: item.maintenance ? 'In maintenance' : 'Enabled',
				tone: item.maintenance ? ('warning' as const) : ('healthy' as const)
			},
			version: item.version_skew ? `${item.version} (skew)` : item.version,
			capacity: {
				label: `${Math.max(item.cpu_percent, item.memory_percent, item.storage_percent)}% peak`,
				tone: capacityTone(item.cpu_percent, item.memory_percent, item.storage_percent)
			},
			active_tasks: item.active_tasks,
			alerts: item.alerts
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.cluster_id;
		return typeof id === 'string' ? `/clusters/${id}` : null;
	}

	let jumpQuery = $state('');
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<!-- Header row: search + posture strip -->
	<div class="clusters-header">
		<div class="clusters-header__search">
			<div class="clusters-search">
				<span class="clusters-search__icon" aria-hidden="true">
					<Search size={16} />
				</span>
				<input
					type="search"
					bind:value={jumpQuery}
					placeholder="Jump to cluster or datacenter…"
					class="clusters-search__input"
				/>
			</div>
		</div>
		<div class="clusters-header__posture" role="list" aria-label="Fleet posture">
			<div class="posture-chip" role="listitem">
				<span class="posture-chip__label">Clusters</span>
				<span class="posture-chip__value">{posture().total}</span>
			</div>
			<div class="posture-chip" role="listitem">
				<span class="posture-chip__label">Healthy</span>
				<span class="posture-chip__value posture-chip__value--healthy">{posture().healthy}</span>
			</div>
			<div class="posture-chip" role="listitem">
				<span class="posture-chip__label">Degraded</span>
				<span class="posture-chip__value posture-chip__value--degraded">{posture().degraded}</span>
			</div>
			<div class="posture-chip" role="listitem">
				<span class="posture-chip__label">Maintenance</span>
				<span class="posture-chip__value posture-chip__value--warning">{posture().maintenance}</span>
			</div>
			<div class="posture-chip" role="listitem">
				<span class="posture-chip__label">Tasks</span>
				<span class="posture-chip__value">{posture().tasks}</span>
			</div>
			<div class="posture-chip" role="listitem">
				<span class="posture-chip__label">Alerts</span>
				<span class="posture-chip__value posture-chip__value--degraded">{posture().alerts}</span>
			</div>
		</div>
	</div>

	{#if model.state === 'loading'}
		<StateBanner
			variant="loading"
			title="Loading cluster inventory"
			description="Cluster rollups are being assembled from control-plane state, readiness, and capacity signals."
			hint="Summary cards remain visible while the inventory refreshes."
		/>
	{:else if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Cluster inventory unavailable"
			description="The control plane could not assemble the current cluster rollups from datacenter state."
			hint="Navigation and other pages remain available while the cluster view recovers."
		/>
	{:else if model.state === 'degraded'}
		<StateBanner
			variant="degraded"
			title="Cluster posture may be incomplete"
			description="Some cluster rollups were delayed or could not be refreshed."
			hint="Review the attention list and inventory below for the latest known state."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No clusters registered yet"
			description="The fleet has no enrolled datacenters or clusters to display."
			hint="Once clusters are connected, this page will show readiness, capacity, and active work."
		/>
	{/if}

	{#if model.state === 'ready' || model.state === 'degraded'}
		<!-- Primary dominant panel: Clusters needing attention -->
		<section class="attention-panel" aria-labelledby="attention-title">
			<div class="attention-panel__header">
				<h2 id="attention-title" class="attention-panel__title">Clusters needing attention</h2>
				{#if attentionClusters.length === 0}
					<Badge label="All healthy" tone="healthy" />
				{:else}
					<Badge label="{attentionClusters.length} open" tone="warning" />
				{/if}
			</div>

			{#if attentionClusters.length === 0}
				<div class="attention-panel__empty">
					<div class="attention-panel__empty-icon" aria-hidden="true">
						<Server size={24} />
					</div>
					<p class="attention-panel__empty-text">All clusters are operating normally.</p>
				</div>
			{:else}
				<ul class="attention-list" role="list">
					{#each attentionClusters as cluster}
						<li class="attention-item">
							<div class="attention-item__main">
								<div class="attention-item__identity">
									<span class="attention-item__name">{cluster.name}</span>
									<span class="attention-item__dc">{cluster.datacenter}</span>
								</div>
								<div class="attention-item__meta">
									<Badge label={cluster.state} tone={mapStateTone(cluster.state)} />
									{#if cluster.maintenance}
										<Badge label="Maintenance" tone="warning" />
									{/if}
									{#if cluster.top_issue}
										<span class="attention-item__issue">{cluster.top_issue}</span>
									{/if}
								</div>
							</div>
							<div class="attention-item__signals">
								{#if cluster.active_tasks > 0}
									<span class="attention-signal">
										<Activity size={14} aria-hidden="true" />
										{cluster.active_tasks} task{cluster.active_tasks === 1 ? '' : 's'}
									</span>
								{/if}
								{#if cluster.alerts > 0}
									<span class="attention-signal attention-signal--alert">
										<AlertTriangle size={14} aria-hidden="true" />
										{cluster.alerts} alert{cluster.alerts === 1 ? '' : 's'}
									</span>
								{/if}
							</div>
								<a href={`/clusters/${cluster.cluster_id}`} class="attention-item__cta">
									View cluster
									<ArrowRight size={14} aria-hidden="true" />
								</a>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		<!-- Secondary posture summaries -->
		<div class="posture-grid" role="region" aria-label="Posture summaries">
			<article class="posture-card">
				<div class="posture-card__label">Fleet posture</div>
				<div class="posture-card__value">
					{posture().healthy} / {posture().total} healthy
				</div>
				<p class="posture-card__note">
					{#if posture().degraded > 0}
						{posture().degraded} cluster{posture().degraded === 1 ? '' : 's'} require operator review.
					{:else}
						No degraded clusters across the fleet.
					{/if}
				</p>
			</article>

			<article class="posture-card">
				<div class="posture-card__label">Capacity pressure</div>
				<div class="posture-card__value">
					{model.items.filter((c) => Math.max(c.cpu_percent, c.memory_percent, c.storage_percent) >= 70)
						.length} hotspots
				</div>
				<p class="posture-card__note">Clusters above 70% peak utilization.</p>
			</article>

			<article class="posture-card">
				<div class="posture-card__label">Maintenance impact</div>
				<div class="posture-card__value">
					{posture().maintenance} cluster{posture().maintenance === 1 ? '' : 's'}
				</div>
				<p class="posture-card__note">
					{#if posture().maintenance > 0}
						Scheduling is paused on affected clusters.
					{:else}
						No maintenance windows are active.
					{/if}
				</p>
			</article>

			<article class="posture-card">
				<div class="posture-card__label">Recent cluster tasks</div>
				<div class="posture-card__value">{posture().tasks} active</div>
				<p class="posture-card__note">Tasks scoped to clusters in the last hour.</p>
			</article>
		</div>

		<!-- Inventory section -->
		<section class="inventory-section" aria-labelledby="inventory-title">
			<h2 id="inventory-title" class="inventory-section__title">Cluster inventory</h2>
			<FilterPanel filters={filterConfig} values={model.filters.current} />
			<ResourceTable {columns} {rows} {rowHref} emptyTitle="No clusters match the current filters" />
			<UrlPagination
				page={model.page.page}
				pageSize={model.page.pageSize}
				totalItems={model.page.totalItems}
				basePath="/clusters"
				params={model.filters.current}
			/>
		</section>
	{/if}
</PageShell>

<style>
	.clusters-header {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 1rem;
		align-items: center;
	}

	.clusters-header__search {
		min-width: 0;
	}

	.clusters-search {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		border: 1px solid var(--shell-line-strong);
		border-radius: 0.85rem;
		background: var(--shell-surface);
		padding: 0.6rem 0.9rem;
		max-width: 24rem;
	}

	.clusters-search__icon {
		color: var(--shell-text-muted);
		flex-shrink: 0;
	}

	.clusters-search__input {
		flex: 1 1 auto;
		min-width: 0;
		border: 0;
		background: transparent;
		padding: 0;
		font-size: 0.92rem;
		color: var(--shell-text);
	}

	.clusters-search__input::placeholder {
		color: var(--shell-text-muted);
	}

	.clusters-header__posture {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.posture-chip {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		border: 1px solid var(--shell-line);
		border-radius: 999px;
		background: var(--shell-surface);
		padding: 0.35rem 0.8rem;
		font-size: 0.8rem;
	}

	.posture-chip__label {
		color: var(--shell-text-muted);
	}

	.posture-chip__value {
		font-weight: 700;
		color: var(--shell-text);
	}

	.posture-chip__value--healthy {
		color: var(--status-healthy-text);
	}

	.posture-chip__value--warning {
		color: var(--status-warning-text);
	}

	.posture-chip__value--degraded {
		color: var(--status-failed-text);
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
	}

	.attention-panel__empty {
		display: flex;
		align-items: center;
		gap: 0.85rem;
		padding: 1.5rem 0.5rem;
	}

	.attention-panel__empty-icon {
		display: grid;
		place-items: center;
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 999px;
		background: var(--status-healthy-bg);
		color: var(--status-healthy-text);
	}

	.attention-panel__empty-text {
		font-size: 0.95rem;
		color: var(--shell-text-secondary);
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
		grid-template-columns: 1fr auto auto;
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

	.attention-item__identity {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.attention-item__name {
		font-weight: 700;
		color: var(--shell-text);
	}

	.attention-item__dc {
		font-size: 0.85rem;
		color: var(--shell-text-muted);
	}

	.attention-item__meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.attention-item__issue {
		font-size: 0.85rem;
		color: var(--shell-text-secondary);
	}

	.attention-item__signals {
		display: flex;
		align-items: center;
		gap: 0.7rem;
	}

	.attention-signal {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		font-size: 0.8rem;
		color: var(--shell-text-muted);
	}

	.attention-signal--alert {
		color: var(--status-failed-text);
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

	.posture-card {
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.posture-card__label {
		font-size: 0.7rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.posture-card__value {
		margin-top: 0.35rem;
		font-size: 1.15rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.posture-card__note {
		margin-top: 0.25rem;
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--shell-text-secondary);
	}

	.inventory-section {
		display: grid;
		gap: 1.2rem;
	}

	.inventory-section__title {
		font-size: 1rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	@media (max-width: 1080px) {
		.clusters-header {
			grid-template-columns: 1fr;
		}

		.clusters-header__posture {
			justify-content: flex-start;
		}

		.attention-item {
			grid-template-columns: 1fr;
			gap: 0.6rem;
		}

		.posture-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 640px) {
		.posture-grid {
			grid-template-columns: 1fr;
		}

		.clusters-header__posture {
			display: grid;
			grid-template-columns: repeat(3, 1fr);
		}
	}
</style>
