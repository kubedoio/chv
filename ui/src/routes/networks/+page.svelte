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

	const page = getPageDefinition('/networks');
	const model = $derived(data.networks);

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'health',
			label: 'Health',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All health' },
				{ value: 'healthy', label: 'Healthy' },
				{ value: 'warning', label: 'Warning' },
				{ value: 'degraded', label: 'Degraded' }
			]
		},
		{
			name: 'exposure',
			label: 'Exposure',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All exposure' },
				{ value: 'private', label: 'Private' },
				{ value: 'nat', label: 'NAT' },
				{ value: 'public', label: 'Public' }
			]
		}
	];

	function mapHealthTone(health: string): ShellTone {
		switch (health.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'warning':
				return 'warning';
			case 'degraded':
				return 'degraded';
			default:
				return 'unknown';
		}
	}

	function exposureTone(exposure: string): ShellTone {
		switch (exposure) {
			case 'public':
				return 'warning';
			case 'nat':
				return 'unknown';
			default:
				return 'healthy';
		}
	}

	const posture = $derived(() => {
		const items = model.items;
		return {
			total: items.length,
			healthy: items.filter((n) => n.health === 'healthy').length,
			degraded: items.filter((n) => n.health !== 'healthy').length,
			public: items.filter((n) => n.exposure === 'public').length,
			alerts: items.reduce((sum, n) => sum + n.alerts, 0)
		};
	});

	const attentionNetworks = $derived(model.items.filter((n) => n.health !== 'healthy' || n.alerts > 0));

	const columns = [
		{ key: 'name', label: 'Network' },
		{ key: 'scope', label: 'Scope' },
		{ key: 'health', label: 'Health' },
		{ key: 'attached_vms', label: 'VMs' },
		{ key: 'exposure', label: 'Exposure' },
		{ key: 'policy', label: 'Policy' },
		{ key: 'last_task', label: 'Last task' },
		{ key: 'alerts', label: 'Alerts' }
	];

	const rows = $derived(
		model.items.map((item) => ({
			network_id: item.network_id,
			name: item.name,
			scope: item.scope,
			health: { label: item.health, tone: mapHealthTone(item.health) },
			attached_vms: item.attached_vms,
			exposure: { label: item.exposure, tone: exposureTone(item.exposure) },
			policy: item.policy,
			last_task: item.last_task,
			alerts: item.alerts
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.network_id;
		return typeof id === 'string' ? `/networks/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<div class="networks-header">
		<PostureStrip
			chips={[
				{ label: 'Networks', value: posture().total },
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
					label: 'Public',
					value: posture().public,
					variant: posture().public > 0 ? 'warning' : 'default'
				},
				{
					label: 'Alerts',
					value: posture().alerts,
					variant: posture().alerts > 0 ? 'failed' : 'default'
				}
			]}
		/>
	</div>

	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Network inventory unavailable"
			description="The network roster could not be loaded from the control plane."
			hint="Navigation remains available while the network view recovers."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No networks match the current view"
			description="Try widening the health or exposure filters, or create a network to populate this page."
		/>
	{:else}
		<section class="attention-panel" aria-labelledby="attention-title">
			<div class="attention-panel__header">
				<h2 id="attention-title" class="attention-panel__title">Networks needing attention</h2>
				{#if attentionNetworks.length === 0}
					<Badge label="All healthy" tone="healthy" />
				{:else}
					<Badge label="{attentionNetworks.length} open" tone="warning" />
				{/if}
			</div>

			{#if attentionNetworks.length === 0}
				<div class="attention-panel__empty">
					<p class="attention-panel__empty-text">All networks are operating normally.</p>
				</div>
			{:else}
				<ul class="attention-list" role="list">
					{#each attentionNetworks as net}
						<li class="attention-item">
							<div class="attention-item__main">
								<div class="attention-item__name">{net.name}</div>
								<div class="attention-item__meta">
									<Badge label={net.health} tone={mapHealthTone(net.health)} />
									{#if net.alerts > 0}
										<span class="attention-signal attention-signal--alert">
											<AlertTriangle size={14} aria-hidden="true" />
											{net.alerts} alert{net.alerts === 1 ? '' : 's'}
										</span>
									{/if}
								</div>
							</div>
								<a href={`/networks/${net.network_id}`} class="attention-item__cta">
									View network
									<ArrowRight size={14} aria-hidden="true" />
								</a>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		<div class="posture-grid">
			<PostureCard
				label="Public exposure"
				value="{posture().public} network{posture().public === 1 ? '' : 's'}"
				note="Networks with public routing or direct internet exposure."
			/>
			<PostureCard
				label="Degraded networks"
				value={posture().degraded}
				note={posture().degraded > 0 ? 'Networks with health or policy issues.' : 'No degraded networks.'}
			/>
			<PostureCard
				label="Total attached VMs"
				value={model.items.reduce((sum, n) => sum + n.attached_vms, 0)}
				note="Workloads connected across all network scopes."
			/>
			<PostureCard
				label="Active alerts"
				value={posture().alerts}
				note={posture().alerts > 0 ? 'Unresolved network-scoped alerts.' : 'No active network alerts.'}
			/>
		</div>

		<section class="inventory-section" aria-labelledby="inventory-title">
			<h2 id="inventory-title" class="inventory-section__title">Network inventory</h2>
			<FilterPanel filters={filterConfig} values={model.filters.current} />
			<ResourceTable {columns} {rows} {rowHref} emptyTitle="No networks match the current filters" />
			<UrlPagination
				page={model.page.page}
				pageSize={model.page.pageSize}
				totalItems={model.page.totalItems}
				basePath="/networks"
				params={model.filters.current}
			/>
		</section>
	{/if}
</PageShell>

<style>
	.networks-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
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
