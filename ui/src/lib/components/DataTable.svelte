<script lang="ts" context="module">
	import type { Component, Snippet } from 'svelte';
	
	export interface Column<T = unknown> {
		key: string;
		title: string;
		sortable?: boolean;
		filterable?: boolean;
		width?: string;
		align?: 'left' | 'center' | 'right';
		render?: (row: T) => string | Snippet<[T]>;
	}
</script>

<script lang="ts" generics="T">
	import { 
		CheckSquare, 
		Square, 
		MinusSquare, 
		ChevronUp, 
		ChevronDown,
		GripVertical,
		Settings2,
		X
	} from 'lucide-svelte';
	import { slide } from 'svelte/transition';
	import SkeletonRow from './SkeletonRow.svelte';
	import EmptyState from './EmptyState.svelte';

	interface Props {
		data: T[];
		columns: Column<T>[];
		loading?: boolean;
		selectable?: boolean;
		selectedIds?: string[];
		sortColumn?: string | null;
		sortDirection?: 'asc' | 'desc' | null;
		page?: number;
		pageSize?: number;
		totalItems?: number;
		emptyIcon?: unknown;
		emptyTitle?: string;
		emptyDescription?: string;
		onSort?: (column: string, direction: 'asc' | 'desc' | null) => void;
		onSelect?: (ids: string[]) => void;
		onRowClick?: (row: T) => void;
		rowId: (row: T) => string;
		children?: Snippet<[T]>;
	}

	let {
		data,
		columns,
		loading = false,
		selectable = false,
		selectedIds = [],
		sortColumn = null,
		sortDirection = null,
		emptyIcon,
		emptyTitle = 'No data',
		emptyDescription = 'There are no items to display',
		onSort,
		onSelect,
		onRowClick,
		rowId,
		children
	}: Props = $props();

	// Local state
	let lastSelectedId = $state<string | null>(null);
	let columnVisibilityOpen = $state(false);
	let visibleColumns = $state<Set<string>>(new Set(columns.map(c => c.key)));
	let resizingColumn = $state<string | null>(null);
	let columnWidths = $state<Record<string, string>>({});
	let tableRef = $state<HTMLTableElement | null>(null);

	// Initialize column widths from column definitions
	$effect(() => {
		const widths: Record<string, string> = {};
		columns.forEach(col => {
			if (col.width) {
				widths[col.key] = col.width;
			}
		});
		columnWidths = widths;
	});

	// Filtered columns based on visibility
	const visibleColumnList = $derived(
		columns.filter(col => visibleColumns.has(col.key))
	);

	// Selection handling
	const selectedSet = $derived(new Set(selectedIds));
	const isAllSelected = $derived(
		selectable && data.length > 0 && data.every(row => selectedSet.has(rowId(row)))
	);
	const isPartiallySelected = $derived(
		selectable && data.some(row => selectedSet.has(rowId(row))) && !isAllSelected
	);

	function handleSort(column: Column<T>) {
		if (!column.sortable || !onSort) return;

		let newDirection: 'asc' | 'desc' | null;
		if (sortColumn === column.key) {
			// Cycle: asc -> desc -> none
			if (sortDirection === 'asc') {
				newDirection = 'desc';
			} else if (sortDirection === 'desc') {
				newDirection = null;
			} else {
				newDirection = 'asc';
			}
		} else {
			newDirection = 'asc';
		}

		onSort(column.key, newDirection);
	}

	function getSortAriaSort(column: Column<T>): 'ascending' | 'descending' | 'none' {
		if (!column.sortable || sortColumn !== column.key) return 'none';
		return sortDirection === 'asc' ? 'ascending' : 'descending';
	}

	function toggleSelectAll() {
		if (!onSelect) return;

		if (isAllSelected) {
			onSelect([]);
		} else {
			const allIds = data.map(row => rowId(row));
			onSelect(allIds);
		}
	}

	function handleSelect(row: T, event: MouseEvent) {
		if (!onSelect) return;

		const id = rowId(row);
		const newSelected = new Set(selectedIds);

		if (event.shiftKey && lastSelectedId) {
			// Range selection
			const ids = data.map(r => rowId(r));
			const startIdx = ids.indexOf(lastSelectedId);
			const endIdx = ids.indexOf(id);

			if (startIdx !== -1 && endIdx !== -1) {
				const [min, max] = startIdx < endIdx ? [startIdx, endIdx] : [endIdx, startIdx];
				for (let i = min; i <= max; i++) {
					newSelected.add(ids[i]);
				}
			}
		} else {
			// Toggle single
			if (newSelected.has(id)) {
				newSelected.delete(id);
			} else {
				newSelected.add(id);
			}
		}

		lastSelectedId = id;
		onSelect(Array.from(newSelected));
	}

	function toggleColumn(key: string) {
		const newSet = new Set(visibleColumns);
		if (newSet.has(key)) {
			// Don't allow hiding the last column
			if (newSet.size > 1) {
				newSet.delete(key);
			}
		} else {
			newSet.add(key);
		}
		visibleColumns = newSet;
	}

	// Column resizing
	function startResize(column: Column<T>, event: MouseEvent) {
		event.preventDefault();
		resizingColumn = column.key;

		const startX = event.clientX;
		const startWidth = tableRef?.querySelector(`th[data-column="${column.key}"]`)?.getBoundingClientRect().width ?? 150;

		function handleMouseMove(e: MouseEvent) {
			if (!resizingColumn) return;
			const diff = e.clientX - startX;
			const newWidth = Math.max(50, startWidth + diff);
			columnWidths = { ...columnWidths, [resizingColumn]: `${newWidth}px` };
		}

		function handleMouseUp() {
			resizingColumn = null;
			document.removeEventListener('mousemove', handleMouseMove);
			document.removeEventListener('mouseup', handleMouseUp);
		}

		document.addEventListener('mousemove', handleMouseMove);
		document.addEventListener('mouseup', handleMouseUp);
	}

	function getCellAlignment(align?: 'left' | 'center' | 'right'): string {
		switch (align) {
			case 'center': return 'text-center';
			case 'right': return 'text-right';
			default: return 'text-left';
		}
	}

	function renderCellContent(column: Column<T>, row: T): string {
		const value = getValue(row, column.key);
		return value === null || value === undefined ? '—' : String(value);
	}

	function getValue(obj: unknown, path: string): unknown {
		const keys = path.split('.');
		let value: unknown = obj;
		for (const key of keys) {
			if (value === null || value === undefined) return undefined;
			value = (value as Record<string, unknown>)[key];
		}
		return value;
	}
</script>

<div class="datatable-wrapper">
	<!-- Column visibility toggle -->
	{#if columns.length > 0}
		<div class="datatable-toolbar">
			<button
				type="button"
				class="toolbar-btn"
				onclick={() => columnVisibilityOpen = !columnVisibilityOpen}
				aria-expanded={columnVisibilityOpen}
			>
				<Settings2 size={14} />
				<span>Columns</span>
			</button>

			{#if columnVisibilityOpen}
				<div class="column-menu" transition:slide={{ duration: 150 }}>
					<div class="column-menu-header">
						<span>Show columns</span>
						<button
							type="button"
							class="menu-close"
							onclick={() => columnVisibilityOpen = false}
						>
							<X size={14} />
						</button>
					</div>
					<div class="column-list">
						{#each columns as column}
							<label class="column-item">
								<input
									type="checkbox"
									checked={visibleColumns.has(column.key)}
									onchange={() => toggleColumn(column.key)}
									disabled={visibleColumns.size === 1 && visibleColumns.has(column.key)}
								/>
								<span>{column.title}</span>
							</label>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	{/if}

	<div class="datatable-container">
		<table class="datatable" bind:this={tableRef} class:resizing={!!resizingColumn}>
			<thead class="datatable-head">
				<tr>
					{#if selectable}
						<th class="col-select" scope="col">
							<button
								type="button"
								class="select-btn"
								onclick={toggleSelectAll}
								aria-label={isAllSelected ? 'Deselect all' : 'Select all'}
							>
								{#if isAllSelected}
									<CheckSquare size={16} class="text-primary" />
								{:else if isPartiallySelected}
									<MinusSquare size={16} class="text-primary" />
								{:else}
									<Square size={16} />
								{/if}
							</button>
						</th>
					{/if}
					{#each visibleColumnList as column}
						<th
							class="datatable-th {getCellAlignment(column.align)}"
							class:sortable={column.sortable}
							style:width={columnWidths[column.key] ?? column.width}
							data-column={column.key}
							scope="col"
							aria-sort={getSortAriaSort(column)}
						>
							{#if column.sortable && onSort}
								<button
									type="button"
									class="sort-btn"
									onclick={() => handleSort(column)}
								>
									<span>{column.title}</span>
									<span class="sort-icons">
										<span class="sort-icon" class:active={sortColumn === column.key && sortDirection === 'asc'}>
											<ChevronUp size={14} />
										</span>
										<span class="sort-icon" class:active={sortColumn === column.key && sortDirection === 'desc'}>
											<ChevronDown size={14} />
										</span>
									</span>
								</button>
							{:else}
								<span>{column.title}</span>
							{/if}
							<!-- Resize handle -->
							<button
								type="button"
								class="resize-handle"
								onmousedown={(e) => startResize(column, e)}
								aria-label={`Resize ${column.title} column`}
								tabindex="-1"
							>
								<GripVertical size={12} />
							</button>
						</th>
					{/each}
					{#if children}
						<th class="datatable-th actions" scope="col">Actions</th>
					{/if}
				</tr>
			</thead>
			<tbody class="datatable-body">
				{#if loading}
					{#each Array(5) as _}
						<SkeletonRow columns={visibleColumnList.length + (selectable ? 1 : 0) + (children ? 1 : 0)} />
					{/each}
				{:else if data.length === 0}
					<tr>
						<td 
							colspan={visibleColumnList.length + (selectable ? 1 : 0) + (children ? 1 : 0)}
							class="empty-cell"
						>
							<EmptyState
								icon={emptyIcon}
								title={emptyTitle}
								description={emptyDescription}
							/>
						</td>
					</tr>
				{:else}
					{#each data as row}
						{@const id = rowId(row)}
						<tr
							class="datatable-row"
							class:selected={selectedSet.has(id)}
							class:clickable={!!onRowClick}
							onclick={() => onRowClick?.(row)}
						>
							{#if selectable}
								<td class="col-select">
									<button
										type="button"
										class="select-btn"
										onclick={(e) => { e.stopPropagation(); handleSelect(row, e); }}
										aria-label={selectedSet.has(id) ? 'Deselect row' : 'Select row'}
									>
										{#if selectedSet.has(id)}
											<CheckSquare size={16} class="text-primary" />
										{:else}
											<Square size={16} />
										{/if}
									</button>
								</td>
							{/if}
							{#each visibleColumnList as column}
								<td class="datatable-cell {getCellAlignment(column.align)}">
									{#if column.render}
										{@const rendered = column.render(row)}
										{#if typeof rendered === 'string'}
											{rendered}
										{:else}
											{@render rendered(row)}
										{/if}
									{:else}
										{renderCellContent(column, row)}
									{/if}
								</td>
							{/each}
							{#if children}
								<td class="datatable-cell actions">
									{@render children(row)}
								</td>
							{/if}
						</tr>
					{/each}
				{/if}
			</tbody>
		</table>
	</div>
</div>

<style>
	.datatable-wrapper {
		position: relative;
	}

	.datatable-toolbar {
		position: relative;
		display: flex;
		justify-content: flex-end;
		padding: 0.5rem 1rem;
		border-bottom: 1px solid var(--color-neutral-200);
	}

	.toolbar-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.375rem 0.75rem;
		background: white;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
		color: var(--color-neutral-600);
		font-size: var(--text-sm);
		cursor: pointer;
		transition: all var(--duration-fast);
	}

	.toolbar-btn:hover {
		border-color: var(--color-neutral-300);
		background: var(--color-neutral-50);
	}

	.column-menu {
		position: absolute;
		top: calc(100% + 0.25rem);
		right: 1rem;
		z-index: 50;
		min-width: 200px;
		background: white;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-lg);
	}

	.column-menu-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem;
		border-bottom: 1px solid var(--color-neutral-100);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.menu-close {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.25rem;
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		color: var(--color-neutral-400);
		cursor: pointer;
		transition: all var(--duration-fast);
	}

	.menu-close:hover {
		color: var(--color-neutral-600);
		background: var(--color-neutral-100);
	}

	.column-list {
		max-height: 300px;
		overflow-y: auto;
		padding: 0.5rem;
	}

	.column-item {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-size: var(--text-sm);
		color: var(--color-neutral-700);
		transition: background var(--duration-fast);
	}

	.column-item:hover {
		background: var(--color-neutral-50);
	}

	.column-item input[type='checkbox'] {
		accent-color: var(--color-primary);
	}

	.column-item input[type='checkbox']:disabled {
		cursor: not-allowed;
	}

	.datatable-container {
		overflow-x: auto;
		overflow-y: visible;
	}

	.datatable {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}

	.datatable-head {
		position: sticky;
		top: 0;
		z-index: 10;
	}

	.datatable-th {
		position: relative;
		padding: 0.75rem 1rem;
		background: var(--color-neutral-50);
		border-bottom: 1px solid var(--color-neutral-200);
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-neutral-500);
		white-space: nowrap;
		user-select: none;
	}

	.datatable-th.sortable {
		padding: 0;
	}

	.sort-btn {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		width: 100%;
		height: 100%;
		padding: 0.75rem 1rem;
		background: transparent;
		border: none;
		color: inherit;
		font: inherit;
		cursor: pointer;
		text-align: left;
	}

	.sort-btn:hover {
		background: var(--color-neutral-100);
	}

	.sort-icons {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: -2px;
	}

	.sort-icon {
		color: var(--color-neutral-300);
	}

	.sort-icon.active {
		color: var(--color-primary);
	}

	.resize-handle {
		position: absolute;
		right: 0;
		top: 0;
		bottom: 0;
		width: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		color: var(--color-neutral-300);
		cursor: col-resize;
		opacity: 0;
		transition: opacity var(--duration-fast);
	}

	.datatable-th:hover .resize-handle {
		opacity: 1;
	}

	.datatable.resizing .resize-handle {
		opacity: 1;
	}

	.datatable-body {
		background: white;
	}

	.datatable-row {
		transition: background-color var(--duration-fast);
	}

	.datatable-row:hover {
		background: var(--color-neutral-50);
	}

	.datatable-row.selected {
		background: var(--color-primary-light);
	}

	.datatable-row.clickable {
		cursor: pointer;
	}

	.datatable-cell {
		padding: 0.875rem 1rem;
		border-bottom: 1px solid var(--color-neutral-100);
		color: var(--color-neutral-700);
	}

	.col-select {
		width: 3rem;
		padding: 0.75rem;
		text-align: center;
	}

	.select-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.25rem;
		background: transparent;
		border: none;
		color: var(--color-neutral-400);
		cursor: pointer;
		transition: color var(--duration-fast);
		border-radius: var(--radius-sm);
	}

	.select-btn:hover {
		color: var(--color-primary);
		background: var(--color-neutral-100);
	}

	.text-primary {
		color: var(--color-primary);
	}

	.empty-cell {
		padding: 3rem 1rem;
	}

	/* Alignment utilities */
	.text-left {
		text-align: left;
	}

	.text-center {
		text-align: center;
	}

	.text-right {
		text-align: right;
	}
</style>
