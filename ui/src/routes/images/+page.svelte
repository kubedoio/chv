<script lang="ts">
	import { getStoredToken } from '$lib/api/client';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import ImportImageModal from '$lib/components/modals/ImportImageModal.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus, Download, Tag, Trash2, Box } from 'lucide-svelte';
	import { goto, invalidateAll } from '$app/navigation';
	import { page as appPage } from '$app/stores';

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
			const res = await fetch('/v1/images/delete', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
					...(token ? { 'Authorization': `Bearer ${token}` } : {})
				},
				body: JSON.stringify({ image_id: imageId })
			});

			if (!res.ok) {
				const body = await res.json().catch(() => ({}));
				throw new Error(body.error ?? `HTTP ${res.status}`);
			}

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
			<button class="btn-primary" onclick={() => (modalOpen = true)}>
				<Plus size={14} />
				Ingest Image
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<ImportImageModal bind:open={modalOpen} onSuccess={() => invalidateAll()} />

	{#if deleteError}
		<div class="operation-alert operation-alert--danger">
			<span>{deleteError}</span>
			<button onclick={() => (deleteError = null)}>Dismiss</button>
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
		<section class="inventory-table-area">
			{#if model.state === 'error'}
				<ErrorState />
			{:else if model.state === 'empty'}
				<EmptyInfrastructureState
					title="No artifacts detected"
					description="Adjust your search criteria or ingest a new distribution image."
					hint="Images are foundational blocks for all compute workloads."
				/>
			{:else}
				<InventoryTable
					{columns}
					rows={tableRows}
				>
					{#snippet cell({ column, row })}
						{#if column.key === '_actions'}
							<button
								class="btn-icon-destructive"
								disabled={deletingId === row.image_id}
								onclick={(e) => { e.preventDefault(); e.stopPropagation(); handleDelete(row.image_id, row.name, row.usage_count); }}
								title="Purge Image"
							>
								<Trash2 size={13} />
							</button>
						{:else if column.key === 'name'}
							<div class="artifact-identity">
								<span class="artifact-name">{row.name}</span>
								{#if row.is_template}
									<span class="artifact-tag">SYS</span>
								{/if}
							</div>
						{:else if row[column.key] && typeof row[column.key] === 'object' && 'label' in row[column.key]}
							<StatusBadge label={row[column.key].label} tone={row[column.key].tone} />
						{:else}
							<span class="cell-text">{row[column.key] ?? ''}</span>
						{/if}
					{/snippet}
				</InventoryTable>
			{/if}
		</section>

		<aside class="support-area">
			<SectionCard title="Ingestion Pipeline" icon={Download} badgeLabel={String(pendingImages.length)}>
				{#if pendingImages.length === 0}
					<p class="empty-hint">No active artifact transmissions detected.</p>
				{:else}
					<ul class="attention-list">
						{#each pendingImages as img}
							<li>
								<div class="attention-card">
									<div class="attention-card__main">
										<span class="res-name">{img.name}</span>
										<span class="res-issue">Ingesting · {img.size}</span>
									</div>
								</div>
							</li>
						{/each}
					</ul>
				{/if}
			</SectionCard>

			<SectionCard title="Base Manifest" icon={Tag}>
				<div class="artifact-manifest">
					<div class="manifest-row">
						<span>Standard Templates</span>
						<span>Online</span>
					</div>
					<div class="manifest-row">
						<span>Global Projections</span>
						<span>3 Verified</span>
					</div>
				</div>
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

	.artifact-identity {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.artifact-name {
		font-weight: 700;
		color: var(--color-neutral-900);
	}

	.artifact-tag {
		font-size: 8px;
		font-weight: 800;
		color: #ffffff;
		background: var(--color-neutral-400);
		padding: 1px 3px;
		border-radius: 2px;
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
		color: var(--color-neutral-800);
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
		color: var(--color-warning);
		font-weight: 600;
		text-transform: uppercase;
	}

	.artifact-manifest {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.manifest-row {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		color: var(--color-neutral-600);
		padding: 0.35rem 0.5rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
	}

	.manifest-row span:last-child {
		font-weight: 700;
		color: var(--color-neutral-900);
	}

	.btn-icon-destructive {
		background: transparent;
		border: 1px solid transparent;
		color: var(--color-neutral-400);
		padding: 4px;
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.1s ease;
	}

	.btn-icon-destructive:hover:not(:disabled) {
		color: var(--color-danger);
		border-color: var(--color-danger-light);
		background: var(--color-danger-light);
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

	.delete-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		background: none;
		border: 1px solid transparent;
		border-radius: 0.25rem;
		padding: 0.2rem 0.35rem;
		cursor: pointer;
		color: var(--shell-text-muted);
		transition: color 0.15s ease, border-color 0.15s ease, background 0.15s ease;
	}

	.delete-btn:hover:not(:disabled) {
		color: var(--color-danger, #c0392b);
		border-color: var(--color-danger, #c0392b);
		background: rgba(192, 57, 43, 0.06);
	}

	.delete-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.delete-error-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		background: rgba(192, 57, 43, 0.08);
		border: 1px solid var(--color-danger, #c0392b);
		border-radius: 0.35rem;
		padding: 0.5rem 0.75rem;
		font-size: var(--text-sm);
		color: var(--color-danger, #c0392b);
	}

	.dismiss-btn {
		background: none;
		border: none;
		cursor: pointer;
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		padding: 0.1rem 0.25rem;
	}

	.dismiss-btn:hover {
		color: var(--shell-text);
	}

	.cell-text {
		font-variant-numeric: tabular-nums;
	}

	.cell-badge {
		display: inline-block;
		padding: 0.1rem 0.4rem;
		border-radius: 0.2rem;
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: capitalize;
	}

	.cell-badge[data-tone="healthy"] { background: rgba(39, 174, 96, 0.12); color: var(--color-ok-dark, #1e8449); }
	.cell-badge[data-tone="warning"] { background: rgba(243, 156, 18, 0.12); color: var(--color-warning-dark, #d68910); }
	.cell-badge[data-tone="failed"]  { background: rgba(192, 57, 43, 0.12); color: var(--color-danger, #c0392b); }
	.cell-badge[data-tone="unknown"] { background: var(--shell-surface-muted); color: var(--shell-text-muted); }

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
