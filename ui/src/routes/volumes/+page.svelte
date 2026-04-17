<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus, Database, AlertTriangle, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	const model = $derived(data.volumes);
	const items = $derived(model.items);

	const stats = $derived([
		{ label: 'Total Volumes', value: items.length },
		{ label: 'Attached', value: items.filter(v => v.attached_vm_id).length, status: 'healthy' as const },
		{ label: 'Available', value: items.filter(v => !v.attached_vm_id).length, status: 'neutral' as const },
		{ label: 'Degraded', value: items.filter(v => v.health !== 'healthy').length, status: items.filter(v => v.health !== 'healthy').length > 0 ? 'warning' as const : 'neutral' as const },
		{ label: 'Open Alerts', value: items.reduce((sum, v) => sum + (v.alerts || 0), 0), status: items.reduce((sum, v) => sum + (v.alerts || 0), 0) > 0 ? 'critical' as const : 'neutral' as const }
	]);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Name/VM/Node...' },
		{ 
			key: 'status', 
			label: 'Status', 
			type: 'select' as const, 
			options: [
				{ value: 'available', label: 'Available' },
				{ value: 'provisioning', label: 'Provisioning' },
				{ value: 'error', label: 'Error' }
			] 
		},
		{
			key: 'health',
			label: 'Health',
			type: 'select' as const,
			options: [
				{ value: 'healthy', label: 'Healthy' },
				{ value: 'warning', label: 'Warning' },
				{ value: 'critical', label: 'Critical' }
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

	function handleClearFilters() {
		goto($appPage.url.pathname);
	}

	const columns = [
		{ key: 'name', label: 'Volume' },
		{ key: 'backend', label: 'Backend' },
		{ key: 'attached_vm_name', label: 'Attached VM' },
		{ key: 'node_id', label: 'Node' },
		{ key: 'health', label: 'Health' },
		{ key: 'size', label: 'Size', align: 'right' as const },
		{ key: 'policy', label: 'Policy' },
		{ key: 'last_task', label: 'Last Task' },
		{ key: 'alerts', label: 'Alerts', align: 'center' as const }
	];

	function mapHealthTone(health: string): any {
		switch (health) {
			case 'healthy': return 'healthy';
			case 'warning': return 'warning';
			case 'degraded': return 'degraded';
			case 'critical': return 'failed';
			default: return 'unknown';
		}
	}

	const tableRows = $derived(items.map(item => ({
		...item,
		backend: item.backend || 'LocalDisk (LVM)',
		policy: item.policy || 'Standard',
		health: { label: item.health, tone: mapHealthTone(item.health) },
		alerts: 0 // Mock alerts if missing
	})));

	const attentionVolumes = $derived(items.filter(v => v.health !== 'healthy').slice(0, 3));
	const pageDef = getPageDefinition('/volumes');
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef}>
		{#snippet actions()}
			<button class="btn-primary">
				<Plus size={14} />
				Create Volume
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
			onClearAll={handleClearFilters}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if model.state === 'error'}
				<ErrorState />
			{:else if model.state === 'empty'}
				<EmptyInfrastructureState 
					title="No storage volumes found"
					description="Adjust your filters or provision a new block device."
					hint="Volumes are specific to nodes but can be migrated if the pool is shared."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={tableRows} 
					rowHref={(row) => `/volumes/${row.volume_id}`} 
				/>
			{/if}
		</section>

		<aside class="support-area">
			<div class="support-panel">
				<div class="support-panel__header">
					<Database size={14} />
					<h3>Storage Pressure</h3>
				</div>
				<div class="support-panel__content">
					{#if attentionVolumes.length === 0}
						<p class="empty-hint">Block devices healthy and responsive.</p>
					{:else}
						<ul class="attention-list">
							{#each attentionVolumes as vol}
								<li>
									<a href="/volumes/{vol.volume_id}" class="attention-card">
										<div class="attention-card__main">
											<span class="res-name">{vol.name}</span>
											<span class="res-issue">{vol.health} state · {vol.size}</span>
										</div>
										<ChevronRight size={14} />
									</a>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			</div>

			<div class="support-panel">
				<div class="support-panel__header">
					<AlertTriangle size={14} style="color: var(--color-warning)" />
					<h3>Degraded Pools</h3>
				</div>
				<div class="support-panel__content">
					<p class="empty-hint">No filesystem anomalies detected in observed pools.</p>
				</div>
			</div>
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
		grid-template-columns: 1fr 280px;
		gap: 1rem;
		align-items: start;
	}

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.support-panel {
		background: var(--shell-surface);
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		padding: 0.75rem;
	}

	.support-panel__header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		padding-bottom: 0.5rem;
	}

	.support-panel__header h3 {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		padding: 0.5rem 0;
	}

	.attention-list {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.attention-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border-radius: 0.25rem;
		text-decoration: none;
		color: var(--shell-text);
		transition: background 0.15s ease;
	}

	.attention-card:hover {
		background: var(--shell-line);
	}

	.attention-card__main {
		display: flex;
		flex-direction: column;
	}

	.res-name {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.res-issue {
		font-size: var(--text-xs);
		color: var(--color-warning-dark);
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
