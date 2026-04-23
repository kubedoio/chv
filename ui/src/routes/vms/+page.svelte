<script lang="ts">
	import type { PageData } from './$types';
	import InventoryListPage from '$lib/components/shell/InventoryListPage.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CreateVMModal from '$lib/components/modals/CreateVMModal.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { Plus, Activity, AlertCircle, ShieldCheck } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';
	import { invalidateAll } from '$app/navigation';

	let { data }: { data: PageData } = $props();

	let modalOpen = $state(false);
	const model = $derived(data.vms);
	const items = $derived(model.items);

	const pageDef = getPageDefinition('/vms');

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

	function handleFilterChange(key: string, value: unknown) {
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

	function mapPowerTone(state: string): string {
		switch (state) {
			case 'running': return 'healthy';
			case 'stopped': return 'neutral';
			case 'paused': return 'warning';
			case 'crashed': return 'failed';
			default: return 'neutral';
		}
	}

	function mapHealthTone(health: string): string {
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

	const metrics = $derived([
		{ label: 'Total Catalog', value: items.length, color: 'neutral' as const },
		{ label: 'Active Runs', value: items.filter(v => v.power_state.toLowerCase() === 'running').length, trend: +1, color: 'primary' as const },
		{ label: 'Posture Alert', value: items.filter(v => v.health !== 'healthy').length, color: items.filter(v => v.health !== 'healthy').length > 0 ? 'warning' as const : 'neutral' as const }
	]);
</script>

<InventoryListPage
	page={pageDef}
	{filters}
	activeFilters={model.filters.current}
	{metrics}
	onFilterChange={handleFilterChange}
	onClearFilters={handleClearFilters}
	state={model.state}
	emptyTitle="Discovery Filter Active"
	emptyDescription="No workloads match the current projection."
	emptyHint="Refine your search parameters or check archived objects."
	{columns}
	rows={tableRows}
	rowHref={(row) => `/vms/${row.vm_id}`}
>
	{#snippet headerActions()}
		<button class="btn-primary" onclick={() => (modalOpen = true)}>
			<Plus size={14} />
			Deploy Workload
		</button>
	{/snippet}

	{#snippet sidebar()}
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
	{/snippet}
</InventoryListPage>

<CreateVMModal bind:open={modalOpen} onSuccess={() => invalidateAll()} />

<style>
</style>
