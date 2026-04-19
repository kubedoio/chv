<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import ImportImageModal from '$lib/components/modals/ImportImageModal.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Plus, Download, Tag, Trash2 } from 'lucide-svelte';
	import { goto, invalidateAll } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	let modalOpen = $state(false);
	let deletingId = $state<string | null>(null);
	let deleteError = $state<string | null>(null);

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
		{ key: 'notes', label: 'Notes' },
		{ key: '_actions', label: '', align: 'center' as const }
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

	async function handleDelete(imageId: string, imageName: string, usageCount: number) {
		let confirmMsg = `Delete image "${imageName}"?`;
		if (usageCount > 0) {
			confirmMsg = `Warning: image "${imageName}" is currently used by ${usageCount} VM(s).\n\nDelete anyway?`;
		}
		if (!confirm(confirmMsg)) return;

		deletingId = imageId;
		deleteError = null;

		try {
			const res = await fetch('/api/v1/images/delete', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
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
				Import Image
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<ImportImageModal bind:open={modalOpen} onSuccess={() => invalidateAll()} />

	{#if deleteError}
		<div class="delete-error-banner">
			{deleteError}
			<button onclick={() => (deleteError = null)} class="dismiss-btn">Dismiss</button>
		</div>
	{/if}

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
				>
					{#snippet cell({ column, row })}
						{#if column.key === '_actions'}
							<button
								class="delete-btn"
								disabled={deletingId === row.image_id}
								onclick={(e) => { e.preventDefault(); e.stopPropagation(); handleDelete(row.image_id, row.name, row.usage_count); }}
								title="Delete image"
							>
								<Trash2 size={13} />
							</button>
						{:else if column.key === 'name'}
							<a href="/images/{row.image_id}" class="row-link">{row.name}</a>
						{:else if row[column.key] && typeof row[column.key] === 'object' && 'label' in row[column.key]}
							<span class="cell-badge" data-tone={row[column.key].tone}>{row[column.key].label}</span>
						{:else}
							<span class="cell-text">{row[column.key] ?? ''}</span>
						{/if}
					{/snippet}
				</InventoryTable>
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

	.row-link {
		color: var(--shell-text);
		font-weight: 600;
		text-decoration: none;
	}

	.row-link:hover {
		color: var(--shell-accent);
		text-decoration: underline;
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
