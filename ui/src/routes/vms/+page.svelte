<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import CreateVMModal from '$lib/components/modals/CreateVMModal.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus, Activity, AlertCircle, ShieldCheck } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';
	import { invalidateAll } from '$app/navigation';

	let { data }: { data: PageData } = $props();

	let modalOpen = $state(false);

	const model = $derived(data.vms);
	const items = $derived(model.items);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Name or node...' },
		{ 
			key: 'powerState', 
			label: 'Power', 
			type: 'select' as const, 
			options: [
				{ value: 'running', label: 'Running' },
				{ value: 'stopped', label: 'Stopped' },
				{ value: 'paused', label: 'Paused' },
				{ value: 'crashed', label: 'Crashed' }
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
		{ key: 'name', label: 'Workload' },
		{ key: 'node_id', label: 'Host Node' },
		{ key: 'power_state', label: 'State', align: 'center' as const },
		{ key: 'health', label: 'Posture' },
		{ key: 'cpu', label: 'CPU', align: 'right' as const },
		{ key: 'memory', label: 'Memory', align: 'right' as const },
		{ key: 'last_task', label: 'Recent Operation' }
	];

	function mapPowerTone(state: string): any {
		switch (state) {
			case 'running': return 'healthy';
			case 'stopped': return 'neutral';
			case 'paused': return 'warning';
			case 'crashed': return 'failed';
			default: return 'neutral';
		}
	}

	function mapHealthTone(health: string): any {
		switch (health) {
			case 'healthy': return 'healthy';
			case 'warning': return 'warning';
			case 'critical': return 'failed';
			default: return 'neutral';
		}
	}

	const tableRows = $derived(items.map(item => ({
		...item,
		power_state: { label: item.power_state, tone: mapPowerTone(item.power_state) },
		health: { label: item.health, tone: mapHealthTone(item.health) }
	})));

	const attentionVms = $derived(items.filter(v => v.health !== 'healthy' || (v.alerts ?? 0) > 0).slice(0, 3));
	const pageDef = getPageDefinition('/vms');
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef}>
		{#snippet actions()}
			<button class="btn-primary" onclick={() => (modalOpen = true)}>
				<Plus size={14} />
				Deploy Workload
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<CreateVMModal bind:open={modalOpen} onSuccess={() => invalidateAll()} />

	<div class="inventory-metrics">
		<CompactMetricCard 
			label="Total Catalog" 
			value={items.length} 
			color="neutral"
		/>
		<CompactMetricCard 
			label="Active Runs" 
			value={items.filter(v => v.power_state.toLowerCase() === 'running').length} 
			trend={+1}
			color="primary"
		/>
		<CompactMetricCard 
			label="Posture Alert" 
			value={items.filter(v => v.health !== 'healthy').length} 
			color={items.filter(v => v.health !== 'healthy').length > 0 ? 'warning' : 'neutral'}
		/>
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
					title="Discovery Filter Active"
					description="No workloads match the current projection."
					hint="Refine your search parameters or check archived objects."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={tableRows} 
					rowHref={(row) => `/vms/${row.vm_id}`} 
				/>
			{/if}
		</section>

		<aside class="support-area">
			<SectionCard title="Anomaly Detection" icon={AlertCircle} badgeLabel={String(attentionVms.length)}>
				{#if attentionVms.length === 0}
					<p class="empty-hint">Signals nominal. Posture is stable.</p>
				{:else}
					<ul class="attention-list">
						{#each attentionVms as vm}
							<li>
								<a href="/vms/{vm.vm_id}" class="attention-card">
									<div class="attention-card__main">
										<span class="res-name">{vm.name}</span>
										<span class="res-issue">{vm.alerts || 0} signals / {vm.health}</span>
									</div>
								</a>
							</li>
						{/each}
					</ul>
				{/if}
			</SectionCard>

			<SectionCard title="Provisioning Log" icon={ShieldCheck}>
				<ul class="task-list">
					<li class="task-item">
						<span class="task-label">Replication Engine</span>
						<span class="task-time">Online</span>
					</li>
					<li class="task-item">
						<span class="task-label">Migration Target Sync</span>
						<span class="task-time">Active</span>
					</li>
				</ul>
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
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		overflow: hidden;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.empty-hint {
		font-size: 11px;
		color: var(--color-neutral-400);
		padding: 1rem;
		text-align: center;
	}

	.attention-list {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.attention-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
		text-decoration: none;
		color: var(--color-neutral-800);
		transition: background 0.1s ease;
	}

	.attention-card:hover {
		background: var(--bg-surface-hover);
	}

	.attention-card__main {
		display: flex;
		flex-direction: column;
	}

	.res-name {
		font-size: 11px;
		font-weight: 700;
	}

	.res-issue {
		font-size: 9px;
		color: var(--color-danger);
		font-weight: 600;
		text-transform: uppercase;
	}

	.task-list {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.task-item {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		padding: 0.35rem 0.5rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
	}

	.task-label {
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.task-time {
		color: var(--color-success);
		font-weight: 700;
		text-transform: uppercase;
		font-size: 9px;
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
