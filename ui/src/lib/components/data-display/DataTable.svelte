<script lang="ts" module>
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
	import { CheckSquare, Square, MinusSquare, ChevronUp, ChevronDown, GripVertical } from 'lucide-svelte';
	import SkeletonRow from './SkeletonRow.svelte';
	import EmptyState from '../feedback/EmptyState.svelte';
	import { useTableSelection } from './useTableSelection';
	import { useTableSorting } from './useTableSorting';
	import ColumnVisibilityDropdown from './ColumnVisibilityDropdown.svelte';

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
		data, columns, loading = false, selectable = false, selectedIds = [],
		sortColumn = null, sortDirection = null, emptyIcon, emptyTitle = 'No data',
		emptyDescription = 'There are no items to display', onSort, onSelect,
		onRowClick, rowId, children,
	}: Props = $props();

	let visibleColumns = $state<Set<string>>(new Set(columns.map((c) => c.key)));
	let resizingColumn = $state<string | null>(null);
	let tableRef = $state<HTMLTableElement | null>(null);

	function initColumnWidths(): Record<string, string> {
		const w: Record<string, string> = {};
		columns.forEach((c) => { if (c.width) w[c.key] = c.width; });
		return w;
	}
	let columnWidths = $state<Record<string, string>>(initColumnWidths());

	function getVisibleColumns() {
		return columns.filter((c) => visibleColumns.has(c.key));
	}

	const selection = useTableSelection({
		get data() { return data; },
		get rowId() { return rowId; },
		get selectedIds() { return selectedIds; },
		get onSelect() { return onSelect; },
	});

	const sorting = useTableSorting({
		get sortColumn() { return sortColumn; },
		get sortDirection() { return sortDirection; },
		get onSort() { return onSort; },
	});

	function toggleColumn(key: string) {
		const s = new Set(visibleColumns);
		if (s.has(key)) { if (s.size > 1) s.delete(key); } else { s.add(key); }
		visibleColumns = s;
	}

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
		return align === 'center' ? 'text-center' : align === 'right' ? 'text-right' : 'text-left';
	}

	function renderCellContent(column: Column<T>, row: T): string {
		const value = getValue(row, column.key);
		return value == null ? '—' : String(value);
	}

	function getValue(obj: unknown, path: string): unknown {
		const keys = path.split('.');
		let value: unknown = obj;
		for (const key of keys) {
			if (value == null) return undefined;
			value = (value as Record<string, unknown>)[key];
		}
		return value;
	}

	function getSortAriaSort(column: Column<T>): 'ascending' | 'descending' | 'none' {
		if (!column.sortable) return 'none';
		const dir = sorting.getSortDirection(column.key);
		return dir === null ? 'none' : dir === 'asc' ? 'ascending' : 'descending';
	}
</script>

<div class="datatable-wrapper">
	{#if columns.length > 0}
		<ColumnVisibilityDropdown {columns} {visibleColumns} onToggle={toggleColumn} />
	{/if}
	<div class="datatable-container">
		<table class="datatable" bind:this={tableRef} class:resizing={!!resizingColumn}>
			<thead class="datatable-head">
				<tr>
					{#if selectable}
						<th class="col-select" scope="col">
							<button type="button" class="select-btn" onclick={selection.toggleAll}
								aria-label={selection.isAllSelected ? 'Deselect all' : 'Select all'}>
								{#if selection.isAllSelected}
									<CheckSquare size={16} class="text-primary" />
								{:else if selection.isIndeterminate}
									<MinusSquare size={16} class="text-primary" />
								{:else}
									<Square size={16} />
								{/if}
							</button>
						</th>
					{/if}
					{#each getVisibleColumns() as column}
						<th class="datatable-th {getCellAlignment(column.align)}" class:sortable={column.sortable}
							style:width={columnWidths[column.key] ?? column.width} data-column={column.key}
							scope="col" aria-sort={getSortAriaSort(column)}>
							{#if column.sortable && onSort}
								<button type="button" class="sort-btn" onclick={() => sorting.handleSort(column.key)}>
									<span>{column.title}</span>
									<span class="sort-icons">
										<span class="sort-icon" class:active={sorting.getSortDirection(column.key) === 'asc'}>
											<ChevronUp size={14} />
										</span>
										<span class="sort-icon" class:active={sorting.getSortDirection(column.key) === 'desc'}>
											<ChevronDown size={14} />
										</span>
									</span>
								</button>
							{:else}
								<span>{column.title}</span>
							{/if}
							<button type="button" class="resize-handle"
								onmousedown={(e) => startResize(column, e)}
								aria-label={`Resize ${column.title} column`} tabindex="-1">
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
						<SkeletonRow columns={getVisibleColumns().length + (selectable ? 1 : 0) + (children ? 1 : 0)} />
					{/each}
				{:else if data.length === 0}
					<tr>
						<td colspan={getVisibleColumns().length + (selectable ? 1 : 0) + (children ? 1 : 0)}
							class="empty-cell">
							<EmptyState icon={emptyIcon} title={emptyTitle} description={emptyDescription} />
						</td>
					</tr>
				{:else}
					{#each data as row}
						{@const id = rowId(row)}
						<tr class="datatable-row" class:selected={selection.isRowSelected(row)}
							class:clickable={!!onRowClick} onclick={() => onRowClick?.(row)}>
							{#if selectable}
								<td class="col-select">
									<button type="button" class="select-btn"
										onclick={(e) => { e.stopPropagation(); selection.toggleRow(id, e); }}
										aria-label={selection.isRowSelected(row) ? 'Deselect row' : 'Select row'}>
										{#if selection.isRowSelected(row)}
											<CheckSquare size={16} class="text-primary" />
										{:else}
											<Square size={16} />
										{/if}
									</button>
								</td>
							{/if}
							{#each getVisibleColumns() as column}
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
	.datatable-wrapper { position: relative; }
	.datatable-container { overflow-x: auto; overflow-y: visible; }
	.datatable { width: 100%; border-collapse: collapse; font-size: var(--text-sm); }
	.datatable-head { position: sticky; top: 0; z-index: 10; }
	.datatable-th {
		position: relative; padding: 0.75rem 1rem;
		background: var(--color-neutral-50); border-bottom: 1px solid var(--color-neutral-200);
		font-size: var(--text-xs); font-weight: 600; text-transform: uppercase;
		letter-spacing: 0.05em; color: var(--color-neutral-500);
		white-space: nowrap; user-select: none;
	}
	.datatable-th.sortable { padding: 0; }
	.sort-btn {
		display: flex; align-items: center; justify-content: space-between;
		gap: 0.5rem; width: 100%; height: 100%; padding: 0.75rem 1rem;
		background: transparent; border: none; color: inherit;
		font: inherit; cursor: pointer; text-align: left;
	}
	.sort-btn:hover { background: var(--color-neutral-100); }
	.sort-icons { display: flex; flex-direction: column; align-items: center; gap: -2px; }
	.sort-icon { color: var(--color-neutral-300); }
	.sort-icon.active { color: var(--color-primary); }
	.resize-handle {
		position: absolute; right: 0; top: 0; bottom: 0; width: 16px;
		display: flex; align-items: center; justify-content: center;
		background: transparent; border: none; color: var(--color-neutral-300);
		cursor: col-resize; opacity: 0; transition: opacity var(--duration-fast);
	}
	.datatable-th:hover .resize-handle { opacity: 1; }
	.datatable.resizing .resize-handle { opacity: 1; }
	.datatable-body { background: white; }
	.datatable-row { transition: background-color var(--duration-fast); }
	.datatable-row:hover { background: var(--color-neutral-50); }
	.datatable-row.selected { background: var(--color-primary-light); }
	.datatable-row.clickable { cursor: pointer; }
	.datatable-cell {
		padding: 0.875rem 1rem;
		border-bottom: 1px solid var(--color-neutral-100);
		color: var(--color-neutral-700);
	}
	.col-select { width: 3rem; padding: 0.75rem; text-align: center; }
	.select-btn {
		display: inline-flex; align-items: center; justify-content: center;
		padding: 0.25rem; background: transparent; border: none;
		color: var(--color-neutral-400); cursor: pointer;
		transition: color var(--duration-fast); border-radius: var(--radius-sm);
	}
	.select-btn:hover { color: var(--color-primary); background: var(--color-neutral-100); }
	.text-primary { color: var(--color-primary); }
	.empty-cell { padding: 3rem 1rem; }
	.text-left { text-align: left; }
	.text-center { text-align: center; }
	.text-right { text-align: right; }
</style>
