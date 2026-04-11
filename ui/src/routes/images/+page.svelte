<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { Image, Plus, RefreshCw } from 'lucide-svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';
	import DataTable from '$lib/components/DataTable.svelte';
	import Pagination from '$lib/components/Pagination.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import StateBadge from '$lib/components/StateBadge.svelte';
	import ImportImageModal from '$lib/components/ImportImageModal.svelte';
	import ProgressBar from '$lib/components/ProgressBar.svelte';
	import { useTable } from '$lib/utils/table.svelte';
	import type { Image as ImageType, ImportProgress } from '$lib/api/types';

	const token = getStoredToken();
	const client = createAPIClient({ token: token ?? undefined });
	let items: ImageType[] = $state([]);
	let loading = $state(true);
	let importModalOpen = $state(false);
	let pollInterval: ReturnType<typeof setInterval> | null = $state(null);

	// Track progress for importing images
	let progressMap = $state<Map<string, ImportProgress>>(new Map());

	// Check if any images are in importing state
	const hasImportingImages = $derived(items.some(img => img.status === 'importing'));

	// Table state management - reactive to items
	let table = $derived(useTable<ImageType>({
		data: items,
		pageSize: 10
	}));

	// Filter options
	const filterOptions = [
		{
			key: 'status',
			label: 'Status',
			type: 'select' as const,
			options: [
				{ value: 'ready', label: 'Ready' },
				{ value: 'importing', label: 'Importing' },
				{ value: 'pending', label: 'Pending' },
				{ value: 'failed', label: 'Failed' }
			]
		},
		{
			key: 'os_family',
			label: 'OS Family',
			type: 'select' as const,
			options: [
				{ value: 'linux', label: 'Linux' },
				{ value: 'windows', label: 'Windows' },
				{ value: 'bsd', label: 'BSD' }
			]
		}
	];

	// Table columns definition
	const columns = [
		{
			key: 'name',
			title: 'Name',
			sortable: true
		},
		{
			key: 'os_family',
			title: 'OS Family',
			render: (img: ImageType) => img.os_family || 'Unknown'
		},
		{
			key: 'architecture',
			title: 'Architecture',
			align: 'center' as const,
			width: '110px',
			render: (img: ImageType) => img.architecture || '—'
		},
		{
			key: 'status',
			title: 'Status',
			sortable: true,
			width: '200px',
			render: (img: ImageType) => {
				const progress = progressMap.get(img.id);
				if (img.status === 'importing' && progress) {
					return 'importing';
				}
				return img.status;
			}
		},
		{
			key: 'cloud_init_supported',
			title: 'Cloud-Init',
			align: 'center' as const,
			width: '100px',
			render: (img: ImageType) => img.cloud_init_supported ? 'Yes' : 'No'
		},
		{
			key: 'format',
			title: 'Format',
			align: 'center' as const,
			width: '80px'
		},
		{
			key: 'local_path',
			title: 'Path',
			render: (img: ImageType) => {
				const parts = img.local_path.split('/');
				return parts[parts.length - 1];
			}
		}
	];

	async function loadImages() {
		loading = true;
		try {
			items = await client.listImages();
		} catch {
			toast.error('Failed to load images');
			items = [];
		} finally {
			loading = false;
		}
	}

	async function loadProgressForImportingImages() {
		const importingItems = items.filter(img => img.status === 'importing');

		for (const item of importingItems) {
			try {
				const progress = await client.getImageProgress(item.id);
				if (progress) {
					progressMap.set(item.id, progress);
					// Trigger reactivity by creating a new map
					progressMap = new Map(progressMap);
				}
			} catch {
				// Ignore errors for progress - image might not have started importing yet
			}
		}
	}

	function startPolling() {
		// Poll every 3 seconds if importing, otherwise every 30
		const interval = hasImportingImages ? 3000 : 30000;
		pollInterval = setInterval(async () => {
			await loadImages();
			if (hasImportingImages) {
				await loadProgressForImportingImages();
			}
		}, interval);
	}

	function stopPolling() {
		if (pollInterval) {
			clearInterval(pollInterval);
			pollInterval = null;
		}
	}

	onMount(() => {
		if (!token) {
			goto('/login');
			return;
		}
		loadImages();
		loadProgressForImportingImages();
		startPolling();
	});

	onDestroy(() => {
		stopPolling();
	});

	function handleSort(column: string, direction: 'asc' | 'desc' | null) {
		if (direction) {
			table.setSort(column, direction);
		} else {
			table.clearSort();
		}
	}

	function getProgressForImage(imageId: string): ImportProgress | undefined {
		return progressMap.get(imageId);
	}

	function getProgressLabel(progress: ImportProgress | undefined): string {
		if (!progress) return 'Importing...';
		return `Importing: ${progress.status}`;
	}

	function getProgressColor(status: string): 'blue' | 'green' | 'yellow' | 'red' {
		switch (status) {
			case 'ready':
				return 'green';
			case 'failed':
				return 'red';
			case 'validating':
				return 'yellow';
			default:
				return 'blue';
		}
	}
</script>

<section class="table-card">
	<div class="card-header px-4 py-3 flex justify-between items-center">
		<div>
			<div class="text-[11px] uppercase tracking-[0.16em] text-muted">Images</div>
			<div class="mt-1 text-lg font-semibold">Cloud Images</div>
		</div>
		<div class="flex items-center gap-2">
			<button onclick={loadImages} class="p-2 hover:bg-chrome rounded" title="Refresh">
				<RefreshCw size={16} class={hasImportingImages ? 'animate-spin' : ''} />
			</button>
			<button onclick={() => (importModalOpen = true)} class="button-primary flex items-center gap-2 px-4 py-2 rounded text-sm">
				<Plus size={16} />
				Import
			</button>
		</div>
	</div>

	<!-- Filter bar -->
	<FilterBar
		filters={filterOptions}
		activeFilters={table.filters}
		onFilterChange={table.setFilter}
		onClearAll={table.clearAllFilters}
	/>

	<!-- Data table -->
	<DataTable
		data={table.paginatedData}
		{columns}
		{loading}
		sortColumn={table.sortColumn ?? undefined}
		sortDirection={table.sortDirection}
		emptyIcon={Image as unknown as typeof import('svelte').SvelteComponent}
		emptyTitle="No images yet"
		emptyDescription="Import cloud images to create VMs from"
		onSort={handleSort}
		rowId={(img: ImageType) => img.id}
	>
		{#snippet children(img: ImageType)}
			{@const progress = getProgressForImage(img.id)}
			{#if img.status === 'importing' && progress}
				<div class="w-32">
					<ProgressBar
						value={progress.progress_percent}
						size="sm"
						color={getProgressColor(progress.status)}
					/>
					{#if progress.speed && progress.speed !== '0 B/s'}
						<div class="text-[10px] text-muted mt-1">
							{progress.speed}
						</div>
					{/if}
				</div>
			{:else}
				<StateBadge label={img.status} />
			{/if}
		{/snippet}
	</DataTable>

	<!-- Pagination -->
	{#if !loading && table.totalItems > 0}
		<Pagination
			page={table.page}
			pageSize={table.pageSize}
			totalItems={table.totalItems}
			onPageChange={table.setPage}
			onPageSizeChange={table.setPageSize}
		/>
	{/if}
</section>

<ImportImageModal bind:open={importModalOpen} onSuccess={loadImages} />

<style>
	.animate-spin {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}
</style>
