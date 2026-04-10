/**
 * Table state management utility using Svelte 5 runes
 * Provides sorting, pagination, and selection functionality
 */

export interface SortState {
	column: string;
	direction: 'asc' | 'desc';
}

export interface FilterState {
	[key: string]: unknown;
}

export interface TableOptions<T> {
	data: T[];
	initialSort?: SortState;
	initialPage?: number;
	pageSize?: number;
	clientSideSort?: boolean;
	clientSideFilter?: boolean;
}

export interface TableState<T> {
	// Data
	readonly data: T[];
	readonly sortedData: T[];
	readonly paginatedData: T[];
	readonly totalItems: number;
	readonly totalPages: number;

	// Sorting
	readonly sortColumn: string | null;
	readonly sortDirection: 'asc' | 'desc' | null;
	sort: (column: string) => void;
	setSort: (column: string, direction: 'asc' | 'desc') => void;
	clearSort: () => void;

	// Pagination
	readonly page: number;
	readonly pageSize: number;
	setPage: (page: number) => void;
	setPageSize: (size: number) => void;
	nextPage: () => void;
	prevPage: () => void;
	firstPage: () => void;
	lastPage: () => void;

	// Selection
	readonly selectedIds: Set<string>;
	readonly selectedCount: number;
	readonly isAllSelected: boolean;
	readonly isPartiallySelected: boolean;
	toggleSelect: (id: string) => void;
	select: (id: string) => void;
	deselect: (id: string) => void;
	selectAll: () => void;
	selectNone: () => void;
	selectRange: (startId: string, endId: string, getId: (item: T) => string) => void;

	// Filtering
	readonly filters: FilterState;
	setFilter: (key: string, value: unknown) => void;
	clearFilter: (key: string) => void;
	clearAllFilters: () => void;
	readonly activeFilterCount: number;

	// Helpers
	isSelected: (id: string) => boolean;
	getVisibleIds: (getId: (item: T) => string) => string[];
}

/**
 * Default value getter for sorting
 */
function getValue<T>(obj: T, path: string): unknown {
	const keys = path.split('.');
	let value: unknown = obj;
	for (const key of keys) {
		if (value === null || value === undefined) return undefined;
		value = (value as Record<string, unknown>)[key];
	}
	return value;
}

/**
 * Compare two values for sorting
 */
function compareValues(a: unknown, b: unknown, direction: 'asc' | 'desc'): number {
	// Handle null/undefined
	if (a === null || a === undefined) return direction === 'asc' ? -1 : 1;
	if (b === null || b === undefined) return direction === 'asc' ? 1 : -1;

	// Handle dates
	if (a instanceof Date && b instanceof Date) {
		return direction === 'asc' 
			? a.getTime() - b.getTime() 
			: b.getTime() - a.getTime();
	}

	// Handle strings
	if (typeof a === 'string' && typeof b === 'string') {
		return direction === 'asc' 
			? a.localeCompare(b) 
			: b.localeCompare(a);
	}

	// Handle numbers
	if (typeof a === 'number' && typeof b === 'number') {
		return direction === 'asc' ? a - b : b - a;
	}

	// Handle booleans
	if (typeof a === 'boolean' && typeof b === 'boolean') {
		if (a === b) return 0;
		return direction === 'asc' 
			? (a ? 1 : -1) 
			: (a ? -1 : 1);
	}

	// Default to string comparison
	const aStr = String(a);
	const bStr = String(b);
	return direction === 'asc' ? aStr.localeCompare(bStr) : bStr.localeCompare(aStr);
}

/**
 * Create a reactive table state manager
 */
export function useTable<T>(options: TableOptions<T>): TableState<T> {
	// Reactive state
	let data = $state(options.data);
	let sortColumn = $state<string | null>(options.initialSort?.column ?? null);
	let sortDirection = $state<'asc' | 'desc' | null>(options.initialSort?.direction ?? null);
	let page = $state(options.initialPage ?? 1);
	let pageSize = $state(options.pageSize ?? 10);
	let selectedIds = $state<Set<string>>(new Set());
	let filters = $state<FilterState>({});

	// Sync data when options change
	$effect(() => {
		data = options.data;
	});

	// Derived: sorted data
	const sortedData = $derived.by(() => {
		if (!sortColumn || !sortDirection) return data;

		return [...data].sort((a, b) => {
			const aVal = getValue(a, sortColumn!);
			const bVal = getValue(b, sortColumn!);
			return compareValues(aVal, bVal, sortDirection!);
		});
	});

	// Derived: filtered data
	const filteredData = $derived.by(() => {
		const activeFilters = Object.entries(filters).filter(([_, v]) => v !== undefined && v !== null && v !== '');
		if (activeFilters.length === 0) return sortedData;

		return sortedData.filter(item => {
			return activeFilters.every(([key, value]) => {
				const itemValue = getValue(item, key);
				if (typeof value === 'string') {
					return String(itemValue).toLowerCase().includes(value.toLowerCase());
				}
				return itemValue === value;
			});
		});
	});

	// Derived: paginated data
	const totalItems = $derived(filteredData.length);
	const totalPages = $derived(Math.max(1, Math.ceil(totalItems / pageSize)));
	
	// Clamp page to valid range
	const safePage = $derived(Math.min(page, totalPages));
	
	const paginatedData = $derived.by(() => {
		const start = (safePage - 1) * pageSize;
		const end = start + pageSize;
		return filteredData.slice(start, end);
	});

	// Helper: get row id (placeholder, will be replaced by user-provided function)
	function getRowId(row: T): string {
		return (row as unknown as { id: string }).id;
	}

	// Derived: selection state
	const selectedCount = $derived(selectedIds.size);
	const isAllSelected = $derived(
		paginatedData.length > 0 && paginatedData.every(item => selectedIds.has(getRowId(item)))
	);
	const isPartiallySelected = $derived(
		paginatedData.some(item => selectedIds.has(getRowId(item))) && 
		!paginatedData.every(item => selectedIds.has(getRowId(item)))
	);

	// Active filter count
	const activeFilterCount = $derived(
		Object.values(filters).filter(v => v !== undefined && v !== null && v !== '').length
	);

	// Actions
	function sort(column: string) {
		if (sortColumn === column) {
			// Cycle: asc -> desc -> none
			if (sortDirection === 'asc') {
				sortDirection = 'desc';
			} else if (sortDirection === 'desc') {
				sortColumn = null;
				sortDirection = null;
			}
		} else {
			sortColumn = column;
			sortDirection = 'asc';
		}
		page = 1; // Reset to first page on sort change
	}

	function setSort(column: string, direction: 'asc' | 'desc') {
		sortColumn = column;
		sortDirection = direction;
		page = 1;
	}

	function clearSort() {
		sortColumn = null;
		sortDirection = null;
	}

	function setPage(newPage: number) {
		page = Math.max(1, Math.min(newPage, totalPages));
	}

	function setPageSize(size: number) {
		pageSize = size;
		page = 1; // Reset to first page
	}

	function nextPage() {
		if (page < totalPages) page++;
	}

	function prevPage() {
		if (page > 1) page--;
	}

	function firstPage() {
		page = 1;
	}

	function lastPage() {
		page = totalPages;
	}

	function toggleSelect(id: string) {
		const newSet = new Set(selectedIds);
		if (newSet.has(id)) {
			newSet.delete(id);
		} else {
			newSet.add(id);
		}
		selectedIds = newSet;
	}

	function select(id: string) {
		const newSet = new Set(selectedIds);
		newSet.add(id);
		selectedIds = newSet;
	}

	function deselect(id: string) {
		const newSet = new Set(selectedIds);
		newSet.delete(id);
		selectedIds = newSet;
	}

	function selectAll() {
		const newSet = new Set(selectedIds);
		paginatedData.forEach(item => {
			newSet.add(getRowId(item));
		});
		selectedIds = newSet;
	}

	function selectNone() {
		paginatedData.forEach(item => {
			selectedIds.delete(getRowId(item));
		});
		selectedIds = new Set(selectedIds); // Trigger reactivity
	}

	function selectRange(startId: string, endId: string, getId: (item: T) => string) {
		const visibleIds = paginatedData.map(getId);
		const startIndex = visibleIds.indexOf(startId);
		const endIndex = visibleIds.indexOf(endId);
		
		if (startIndex === -1 || endIndex === -1) return;
		
		const [min, max] = startIndex < endIndex ? [startIndex, endIndex] : [endIndex, startIndex];
		const newSet = new Set(selectedIds);
		
		for (let i = min; i <= max; i++) {
			newSet.add(visibleIds[i]);
		}
		
		selectedIds = newSet;
	}

	function setFilter(key: string, value: unknown) {
		filters = { ...filters, [key]: value };
		page = 1; // Reset to first page on filter change
	}

	function clearFilter(key: string) {
		const { [key]: _, ...rest } = filters;
		filters = rest;
		page = 1;
	}

	function clearAllFilters() {
		filters = {};
		page = 1;
	}

	function isSelected(id: string): boolean {
		return selectedIds.has(id);
	}

	function getVisibleIds(getId: (item: T) => string): string[] {
		return paginatedData.map(getId);
	}

	return {
		// Data (readonly getters)
		get data() { return data; },
		get sortedData() { return sortedData; },
		get paginatedData() { return paginatedData; },
		get totalItems() { return totalItems; },
		get totalPages() { return totalPages; },

		// Sorting
		get sortColumn() { return sortColumn; },
		get sortDirection() { return sortDirection; },
		sort,
		setSort,
		clearSort,

		// Pagination
		get page() { return safePage; },
		get pageSize() { return pageSize; },
		setPage,
		setPageSize,
		nextPage,
		prevPage,
		firstPage,
		lastPage,

		// Selection
		get selectedIds() { return selectedIds; },
		get selectedCount() { return selectedCount; },
		get isAllSelected() { return isAllSelected; },
		get isPartiallySelected() { return isPartiallySelected; },
		toggleSelect,
		select,
		deselect,
		selectAll,
		selectNone,
		selectRange,

		// Filtering
		get filters() { return filters; },
		setFilter,
		clearFilter,
		clearAllFilters,
		get activeFilterCount() { return activeFilterCount; },

		// Helpers
		isSelected,
		getVisibleIds,
	};
}

/**
 * Generate page numbers with ellipsis for pagination display
 * Returns array of numbers and null for ellipsis
 */
export function generatePageNumbers(currentPage: number, totalPages: number, maxVisible: number = 7): (number | null)[] {
	if (totalPages <= maxVisible) {
		return Array.from({ length: totalPages }, (_, i) => i + 1);
	}

	const pages: (number | null)[] = [];
	
	// Always show first page
	pages.push(1);
	
	// Calculate middle range
	const leftBound = Math.max(2, currentPage - 1);
	const rightBound = Math.min(totalPages - 1, currentPage + 1);
	
	// Add ellipsis after first page if needed
	if (leftBound > 2) {
		pages.push(null);
	}
	
	// Add middle pages
	for (let i = leftBound; i <= rightBound; i++) {
		pages.push(i);
	}
	
	// Add ellipsis before last page if needed
	if (rightBound < totalPages - 1) {
		pages.push(null);
	}
	
	// Always show last page
	pages.push(totalPages);
	
	return pages;
}

/**
 * Format bytes to human-readable string
 */
export function formatBytes(bytes: number, decimals = 1): string {
	if (bytes === 0) return '0 B';
	
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(decimals))} ${sizes[i]}`;
}

/**
 * Debounce function for filter inputs
 */
export function debounce<T extends (...args: unknown[]) => unknown>(
	fn: T,
	delay: number
): (...args: Parameters<T>) => void {
	let timeoutId: ReturnType<typeof setTimeout>;
	return (...args: Parameters<T>) => {
		clearTimeout(timeoutId);
		timeoutId = setTimeout(() => fn(...args), delay);
	};
}
