<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
	import { getStoredToken } from '$lib/api/client';
	import { deleteImage } from '$lib/bff/images';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import FilterBar from '$lib/components/shared/FilterBar.svelte';
	import ImportImageModal from '$lib/components/storage/ImportImageModal.svelte';
	import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus } from 'lucide-svelte';
	import { goto, invalidateAll } from '$app/navigation';
	import { page as appPage } from '$app/stores';
	import ImagesTable from '$lib/components/images/ImagesTable.svelte';
	import ImagesSidebar from '$lib/components/images/ImagesSidebar.svelte';

	let { data }: { data: PageData } = $props();

	let modalOpen = $state(false);
	let deletingId = $state<string | null>(null);
	let deleteError = $state<string | null>(null);

	const model = $derived(data.images);
	const items = $derived(model.items);

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
		{ key: 'name', label: 'Artifact Name' },
		{ key: 'os', label: 'Projection/OS' },
		{ key: 'version', label: 'Rev' },
		{ key: 'status', label: 'Registry State' },
		{ key: 'size', label: 'Footprint', align: 'right' as const },
		{ key: 'usage_count', label: 'Instances', align: 'center' as const },
		{ key: '_actions', label: '', align: 'center' as const }
	];

	function mapStatusTone(status: string): any {
		switch (status) {
			case 'ready': return 'healthy';
			case 'pending': return 'warning';
			case 'failed': return 'failed';
			case 'deprecated': return 'neutral';
			default: return 'neutral';
		}
	}

	const tableRows = $derived(items.map(item => ({
		...item,
		status: { label: item.status, tone: mapStatusTone(item.status) }
	})));

	const pendingImages = $derived(items.filter(i => i.status === 'pending').slice(0, 3));
	const pageDef = getPageDefinition('/images');

	async function handleDelete(imageId: string, imageName: string, usageCount: number) {
		let confirmMsg = `Delete artifact "${imageName}"?`;
		if (usageCount > 0) {
			confirmMsg = `CRITICAL: Artifact "${imageName}" is referenced by ${usageCount} active workloads.\n\nProceed with destructive deletion?`;
		}
		if (!confirm(confirmMsg)) return;

		deletingId = imageId;
		deleteError = null;

		try {
			const token = getStoredToken() ?? undefined;
			await deleteImage({ image_id: imageId }, token);
			await invalidateAll();
		} catch (err: any) {
			deleteError = err.message ?? 'Failed to delete image';
		} finally {
			deletingId = null;
		}
	}
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef}>
		{#snippet actions()}
			<Button variant="primary" onclick={() => (modalOpen = true)}>
				<Plus size={14} />
				Ingest Image
			</Button>
		{/snippet}
	</PageHeaderWithAction>

	<ImportImageModal bind:open={modalOpen} onSuccess={() => invalidateAll()} />

	{#if deleteError}
		<div class="operation-alert operation-alert--danger">
			<span>{deleteError}</span>
			<button type="button" onclick={() => (deleteError = null)}>Dismiss</button>
		</div>
	{/if}

	<div class="inventory-metrics">
		<CompactMetricCard
			label="Catalog Size"
			value={items.length}
			color="neutral"
		/>
		<CompactMetricCard
			label="Operational Ready"
			value={items.filter(i => i.status === 'ready').length}
			color="primary"
		/>
		<CompactMetricCard
			label="Pending Ingestion"
			value={items.filter(i => i.status === 'pending').length}
			color={items.filter(i => i.status === 'pending').length > 0 ? 'warning' : 'neutral'}
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
		<ImagesTable
			state={model.state}
			errorMessage={model.errorMessage}
			{columns}
			{tableRows}
			{deletingId}
			{handleDelete}
		/>
		<ImagesSidebar {pendingImages} />
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

	.operation-alert {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-radius: var(--radius-xs);
		font-size: 11px;
		font-weight: 600;
	}

	.operation-alert--danger {
		background: var(--color-danger-light);
		color: var(--color-danger);
		border: 1px solid var(--color-danger);
	}

	.operation-alert button {
		background: transparent;
		border: none;
		color: inherit;
		cursor: pointer;
		text-decoration: underline;
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}

	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
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

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
