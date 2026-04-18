<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import CreateVMModal from '$lib/components/modals/CreateVMModal.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus, Activity, AlertCircle, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';
	import { invalidateAll } from '$app/navigation';

	let { data }: { data: PageData } = $props();

	let modalOpen = $state(false);

	const model = $derived(data.vms);
	const items = $derived(model.items);

	const stats = $derived([
		{ label: 'Total VMs', value: items.length },
		{ label: 'Running', value: items.filter(v => v.power_state === 'running').length, status: 'healthy' as const },
		{ label: 'Stopped', value: items.filter(v => v.power_state === 'stopped').length, status: 'neutral' as const },
		{ label: 'Degraded', value: items.filter(v => v.health !== 'healthy').length, status: items.filter(v => v.health !== 'healthy').length > 0 ? 'warning' as const : 'neutral' as const },
		{ label: 'Open Alerts', value: items.reduce((sum, v) => sum + (v.alerts || 0), 0), status: items.reduce((sum, v) => sum + (v.alerts || 0), 0) > 0 ? 'critical' as const : 'neutral' as const }
	]);

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
		{ key: 'name', label: 'VM' },
		{ key: 'node_id', label: 'Node' },
		{ key: 'power_state', label: 'Power', align: 'center' as const },
		{ key: 'health', label: 'Health' },
		{ key: 'cpu', label: 'CPU', align: 'right' as const },
		{ key: 'memory', label: 'Memory', align: 'right' as const },
		{ key: 'volume_count', label: 'Volumes', align: 'center' as const },
		{ key: 'nic_count', label: 'NICs', align: 'center' as const },
		{ key: 'last_task', label: 'Last Task' },
		{ key: 'alerts', label: 'Alerts', align: 'center' as const }
	];

	function mapPowerTone(state: string): any {
		switch (state) {
			case 'running': return 'healthy';
			case 'stopped': return 'unknown';
			case 'paused': return 'warning';
			case 'crashed': return 'failed';
			default: return 'unknown';
		}
	}

	function mapHealthTone(health: string): any {
		switch (health) {
			case 'healthy': return 'healthy';
			case 'warning': return 'warning';
			case 'critical': return 'failed';
			default: return 'unknown';
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
				Create VM
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<CreateVMModal bind:open={modalOpen} onSuccess={() => invalidateAll()} />

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
					title="No virtual machines match your query"
					description="Adjust your search criteria or create a new instance."
					hint="You can use 'Enroll Node' to add more compute capacity first."
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
			<div class="support-panel">
				<div class="support-panel__header">
					<AlertCircle size={14} style="color: var(--color-danger)" />
					<h3>System Alerts</h3>
				</div>
				<div class="support-panel__content">
					{#if attentionVms.length === 0}
						<p class="empty-hint">Workloads behaving as expected.</p>
					{:else}
						<ul class="attention-list">
							{#each attentionVms as vm}
								<li>
									<a href="/vms/{vm.vm_id}" class="attention-card">
										<div class="attention-card__main">
											<span class="res-name">{vm.name}</span>
											<span class="res-issue">{vm.alerts} alerts · {vm.health}</span>
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
					<Activity size={14} />
					<h3>Workload Tasks</h3>
				</div>
				<div class="support-panel__content">
					<ul class="task-list">
						<li>
							<div class="task-item">
								<span class="task-label">Replication</span>
								<span class="task-time">Just now</span>
							</div>
						</li>
						<li>
							<div class="task-item">
								<span class="task-label">Live Migration</span>
								<span class="task-time">5m ago</span>
							</div>
						</li>
					</ul>
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

	.attention-list, .task-list {
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
		color: var(--color-danger-dark);
	}

	.task-item {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-sm);
		padding: 0.25rem 0.5rem;
	}

	.task-time {
		color: var(--shell-text-muted);
		font-size: var(--text-xs);
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
