export interface UseTableSortingOptions {
	sortColumn: string | null;
	sortDirection: 'asc' | 'desc' | null;
	onSort?: (column: string, direction: 'asc' | 'desc' | null) => void;
}

export interface UseTableSortingResult {
	handleSort: (columnKey: string) => void;
	getSortDirection: (columnKey: string) => 'asc' | 'desc' | null;
}

export function useTableSorting(options: UseTableSortingOptions): UseTableSortingResult {
	function handleSort(columnKey: string) {
		if (!options.onSort) return;

		let newDirection: 'asc' | 'desc' | null;
		if (options.sortColumn === columnKey) {
			if (options.sortDirection === 'asc') {
				newDirection = 'desc';
			} else if (options.sortDirection === 'desc') {
				newDirection = null;
			} else {
				newDirection = 'asc';
			}
		} else {
			newDirection = 'asc';
		}

		options.onSort(columnKey, newDirection);
	}

	function getSortDirection(columnKey: string): 'asc' | 'desc' | null {
		if (options.sortColumn !== columnKey) return null;
		return options.sortDirection;
	}

	return {
		handleSort,
		getSortDirection,
	};
}
