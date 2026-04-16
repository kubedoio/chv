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

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/images');
	const model = $derived(data.images);

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'status',
			label: 'Status',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All statuses' },
				{ value: 'ready', label: 'Ready' },
				{ value: 'pending', label: 'Pending' },
				{ value: 'failed', label: 'Failed' },
				{ value: 'deprecated', label: 'Deprecated' }
			]
		}
	];

	function mapStatusTone(status: string): ShellTone {
		switch (status) {
			case 'ready':
				return 'healthy';
			case 'pending':
				return 'warning';
			case 'failed':
				return 'failed';
			case 'deprecated':
				return 'degraded';
			default:
				return 'unknown';
		}
	}

	const columns = [
		{ key: 'name', label: 'Image / Template' },
		{ key: 'os', label: 'OS' },
		{ key: 'version', label: 'Version' },
		{ key: 'status', label: 'Status' },
		{ key: 'size', label: 'Size' },
		{ key: 'usage_count', label: 'Used by' },
		{ key: 'last_updated', label: 'Updated' }
	];

	const rows = $derived(
		model.items.map((item) => ({
			image_id: item.image_id,
			name: item.name,
			os: item.os,
			version: item.version,
			status: { label: item.status, tone: mapStatusTone(item.status) },
			size: item.size,
			usage_count: item.usage_count === 0 ? 'Unused' : `${item.usage_count} VM${item.usage_count === 1 ? '' : 's'}`,
			last_updated: item.last_updated
		}))
	);

	const summary = $derived(() => {
		const items = model.items;
		return {
			total: items.length,
			ready: items.filter((i) => i.status === 'ready').length,
			pending: items.filter((i) => i.status === 'pending').length,
			deprecated: items.filter((i) => i.status === 'deprecated').length,
			failed: items.filter((i) => i.status === 'failed').length
		};
	});
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Image inventory unavailable"
			description="The image and template roster could not be loaded."
			hint="Navigation remains available while the inventory recovers."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No images match the current view"
			description="Try widening the filters or import a base image to populate this page."
		/>
	{:else}
		<div class="images-summary">
			<div class="images-summary__card">
				<div class="images-summary__label">Total images</div>
				<div class="images-summary__value">{summary().total}</div>
			</div>
			<div class="images-summary__card">
				<div class="images-summary__label">Ready</div>
				<div class="images-summary__value images-summary__value--healthy">{summary().ready}</div>
			</div>
			<div class="images-summary__card">
				<div class="images-summary__label">Pending</div>
				<div class="images-summary__value images-summary__value--warning">{summary().pending}</div>
			</div>
			<div class="images-summary__card">
				<div class="images-summary__label">Deprecated</div>
				<div class="images-summary__value images-summary__value--degraded">{summary().deprecated}</div>
			</div>
			<div class="images-summary__card">
				<div class="images-summary__label">Failed</div>
				<div class="images-summary__value images-summary__value--failed">{summary().failed}</div>
			</div>
		</div>

		<section class="inventory-section" aria-labelledby="inventory-title">
			<h2 id="inventory-title" class="inventory-section__title">Image inventory</h2>
			<FilterPanel filters={filterConfig} values={model.filters.current} />
			<ResourceTable {columns} {rows} emptyTitle="No images match the current filters" />
			<UrlPagination
				page={model.page.page}
				pageSize={model.page.pageSize}
				totalItems={model.page.totalItems}
				basePath="/images"
				params={model.filters.current}
			/>
		</section>
	{/if}
</PageShell>

<style>
	.images-summary {
		display: grid;
		grid-template-columns: repeat(5, minmax(0, 1fr));
		gap: 1rem;
	}

	.images-summary__card {
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.images-summary__label {
		font-size: 0.7rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.images-summary__value {
		margin-top: 0.35rem;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.images-summary__value--healthy {
		color: var(--status-healthy-text);
	}

	.images-summary__value--warning {
		color: var(--status-warning-text);
	}

	.images-summary__value--degraded {
		color: var(--status-degraded-text);
	}

	.images-summary__value--failed {
		color: var(--status-failed-text);
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
		.images-summary {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
	}

	@media (max-width: 640px) {
		.images-summary {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
</style>
