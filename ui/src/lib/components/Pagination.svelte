<script lang="ts">
	import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-svelte';
	import { generatePageNumbers } from '$lib/utils/table.svelte';

	interface Props {
		page: number;
		pageSize: number;
		totalItems: number;
		pageSizeOptions?: number[];
		onPageChange: (page: number) => void;
		onPageSizeChange?: (size: number) => void;
	}

	let {
		page,
		pageSize,
		totalItems,
		pageSizeOptions = [10, 25, 50, 100],
		onPageChange,
		onPageSizeChange
	}: Props = $props();

	// Derived values
	const totalPages = $derived(Math.max(1, Math.ceil(totalItems / pageSize)));
	const startItem = $derived(totalItems === 0 ? 0 : (page - 1) * pageSize + 1);
	const endItem = $derived(Math.min(page * pageSize, totalItems));
	const pageNumbers = $derived(generatePageNumbers(page, totalPages, 7));

	// Jump to page input
	let jumpToPage = $state('');

	function handleJumpToPage(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			const newPage = parseInt(jumpToPage, 10);
			if (!isNaN(newPage) && newPage >= 1 && newPage <= totalPages) {
				onPageChange(newPage);
				jumpToPage = '';
			}
		}
	}

	function handlePageSizeChange(e: Event) {
		const newSize = parseInt((e.target as HTMLSelectElement).value, 10);
		onPageSizeChange?.(newSize);
	}

	function goToPage(newPage: number) {
		if (newPage >= 1 && newPage <= totalPages && newPage !== page) {
			onPageChange(newPage);
		}
	}
</script>

<nav class="pagination" aria-label="Pagination">
	<!-- Items info -->
	<div class="pagination-info">
		<span class="text-sm text-muted">
			Showing <strong>{startItem}</strong> to <strong>{endItem}</strong> of <strong>{totalItems}</strong> items
		</span>
	</div>

	<!-- Page controls -->
	<div class="pagination-controls">
		<!-- First page -->
		<button
			type="button"
			class="pagination-btn"
			disabled={page === 1}
			onclick={() => goToPage(1)}
			aria-label="Go to first page"
			title="First page"
		>
			<ChevronsLeft size={16} />
		</button>

		<!-- Previous page -->
		<button
			type="button"
			class="pagination-btn"
			disabled={page === 1}
			onclick={() => goToPage(page - 1)}
			aria-label="Go to previous page"
			title="Previous page"
		>
			<ChevronLeft size={16} />
		</button>

		<!-- Page numbers -->
		<div class="pagination-pages" role="group" aria-label="Page numbers">
			{#each pageNumbers as pageNum}
				{#if pageNum === null}
					<span class="pagination-ellipsis" aria-hidden="true">...</span>
				{:else}
					<button
						type="button"
						class="pagination-page"
						class:active={pageNum === page}
						aria-current={pageNum === page ? 'page' : undefined}
						onclick={() => goToPage(pageNum)}
					>
						{pageNum}
					</button>
				{/if}
			{/each}
		</div>

		<!-- Next page -->
		<button
			type="button"
			class="pagination-btn"
			disabled={page === totalPages}
			onclick={() => goToPage(page + 1)}
			aria-label="Go to next page"
			title="Next page"
		>
			<ChevronRight size={16} />
		</button>

		<!-- Last page -->
		<button
			type="button"
			class="pagination-btn"
			disabled={page === totalPages}
			onclick={() => goToPage(totalPages)}
			aria-label="Go to last page"
			title="Last page"
		>
			<ChevronsRight size={16} />
		</button>
	</div>

	<!-- Page size selector and jump to -->
	<div class="pagination-extras">
		{#if onPageSizeChange}
			<div class="page-size-selector">
				<label for="page-size" class="sr-only">Items per page</label>
				<select
					id="page-size"
					class="page-size-select"
					value={pageSize}
					onchange={handlePageSizeChange}
				>
					{#each pageSizeOptions as size}
						<option value={size}>{size} / page</option>
					{/each}
				</select>
			</div>
		{/if}

		<div class="jump-to-page">
			<label for="jump-page" class="jump-label">Go to</label>
			<input
				type="number"
				id="jump-page"
				class="jump-input"
				min="1"
				max={totalPages}
				placeholder="#"
				bind:value={jumpToPage}
				onkeydown={handleJumpToPage}
				aria-label={`Jump to page (1-${totalPages})`}
			/>
		</div>
	</div>
</nav>

<style>
	.pagination {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.75rem 1rem;
		background: white;
		border-top: 1px solid var(--color-neutral-200);
	}

	.pagination-info {
		flex-shrink: 0;
	}

	.pagination-info strong {
		color: var(--color-neutral-900);
		font-weight: 600;
	}

	.pagination-controls {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}

	.pagination-btn,
	.pagination-page {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 2rem;
		height: 2rem;
		padding: 0 0.5rem;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
		background: white;
		color: var(--color-neutral-700);
		font-size: var(--text-sm);
		font-weight: 500;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-default);
	}

	.pagination-btn:hover:not(:disabled),
	.pagination-page:hover:not(.active) {
		background: var(--color-neutral-50);
		border-color: var(--color-neutral-300);
	}

	.pagination-btn:focus-visible,
	.pagination-page:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
	}

	.pagination-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.pagination-pages {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}

	.pagination-page.active {
		background: var(--color-primary);
		border-color: var(--color-primary);
		color: white;
	}

	.pagination-ellipsis {
		padding: 0 0.5rem;
		color: var(--color-neutral-400);
		user-select: none;
	}

	.pagination-extras {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.page-size-select {
		padding: 0.375rem 1.75rem 0.375rem 0.75rem;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
		background: white;
		color: var(--color-neutral-700);
		font-size: var(--text-sm);
		cursor: pointer;
		appearance: none;
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%2364748b' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
		background-repeat: no-repeat;
		background-position: right 0.375rem center;
	}

	.page-size-select:focus {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
	}

	.jump-to-page {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.jump-label {
		font-size: var(--text-sm);
		color: var(--color-neutral-500);
		white-space: nowrap;
	}

	.jump-input {
		width: 3.5rem;
		height: 2rem;
		padding: 0 0.5rem;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
		text-align: center;
	}

	.jump-input:focus {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
		border-color: var(--color-primary);
	}

	/* Remove number input spinner */
	.jump-input::-webkit-outer-spin-button,
	.jump-input::-webkit-inner-spin-button {
		appearance: none;
		margin: 0;
	}

	.jump-input[type='number'] {
		appearance: textfield;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}

	/* Responsive */
	@media (max-width: 768px) {
		.pagination {
			flex-direction: column;
			align-items: stretch;
		}

		.pagination-info {
			text-align: center;
		}

		.pagination-controls {
			justify-content: center;
		}

		.pagination-extras {
			justify-content: center;
			flex-wrap: wrap;
		}
	}
</style>
