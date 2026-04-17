<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus, Download, Tag, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	const model = $derived(data.images);
	const items = $derived(model.items);

	const stats = $derived([
		{ label: 'Total Images', value: items.length },
		{ label: 'Ready', value: items.filter(i => i.status === 'ready').length, status: 'healthy' as const },
		{ label: 'Pending', value: items.filter(i => i.status === 'pending').length, status: 'warning' as const },
		{ label: 'Deprecated', value: items.filter(i => i.status === 'deprecated').length, status: 'neutral' as const },
		{ label: 'Total Usage', value: items.reduce((sum, i) => sum + (i.usage_count || 0), 0), status: 'neutral' as const }
	]);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Name/OS...' },
		{ 
			key: 'status', 
			label: 'Status', 
			type: 'select' as const, 
			options: [
				{ value: 'ready', label: 'Ready' },
				{ value: 'pending', label: 'Pending' },
				{ value: 'failed', label: 'Failed' },
				{ value: 'deprecated', label: 'Deprecated' }
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
		{ key: 'name', label: 'Name' },
		{ key: 'os', label: 'Type/OS' },
		{ key: 'version', label: 'Version' },
		{ key: 'status', label: 'Status' },
		{ key: 'last_updated', label: 'Updated' },
		{ key: 'usage_count', label: 'Usage', align: 'center' as const },
		{ key: 'size', label: 'Size', align: 'right' as const },
		{ key: 'notes', label: 'Notes' }
	];

	function mapStatusTone(status: string): any {
		switch (status) {
			case 'ready': return 'healthy';
			case 'pending': return 'warning';
			case 'failed': return 'failed';
			case 'deprecated': return 'unknown';
			default: return 'unknown';
		}
	}

	const tableRows = $derived(items.map(item => ({
		...item,
		status: { label: item.status, tone: mapStatusTone(item.status) },
		notes: item.usage_count > 50 ? 'High usage base' : 'Standard image'
	})));

	const pendingImages = $derived(items.filter(i => i.status === 'pending').slice(0, 3));
	const pageDef = getPageDefinition('/images');
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef}>
		{#snippet actions()}
			<button class="btn-primary">
				<Plus size={14} />
				Import Image
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
					title="No images or templates found"
					description="Adjust your search criteria or import a new cloud image."
					hint="Images are typically large artifacts. Ensure you have sufficient staging storage."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={tableRows} 
					rowHref={(row) => `/images/${row.image_id}`} 
				/>
			{/if}
		</section>

		<aside class="support-area">
			<div class="support-panel">
				<div class="support-panel__header">
					<Download size={14} />
					<h3>Image Imports</h3>
				</div>
				<div class="support-panel__content">
					{#if pendingImages.length === 0}
						<p class="empty-hint">No active image ingestion tasks.</p>
					{:else}
						<ul class="attention-list">
							{#each pendingImages as img}
								<li>
									<div class="attention-card">
										<div class="attention-card__main">
											<span class="res-name">{img.name}</span>
											<span class="res-issue">Pending ingestion · {img.size}</span>
										</div>
									</div>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			</div>

			<div class="support-panel">
				<div class="support-panel__header">
					<Tag size={14} />
					<h3>Global Templates</h3>
				</div>
				<div class="support-panel__content">
					<p class="empty-hint">3 standard templates published by System.</p>
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
