export interface UseTableSelectionOptions<T> {
	data: T[];
	rowId: (row: T) => string;
	selectedIds: string[];
	onSelect?: (ids: string[]) => void;
}

export interface UseTableSelectionResult<T> {
	isRowSelected: (row: T) => boolean;
	isAllSelected: boolean;
	isIndeterminate: boolean;
	toggleRow: (rowId: string, event?: MouseEvent) => void;
	toggleAll: () => void;
}

export function useTableSelection<T>(
	options: UseTableSelectionOptions<T>
): UseTableSelectionResult<T> {
	let lastSelectedId: string | null = null;

	function isRowSelected(row: T): boolean {
		return options.selectedIds.includes(options.rowId(row));
	}

	function getIsAllSelected(): boolean {
		return options.data.length > 0 && options.data.every((row) =>
			options.selectedIds.includes(options.rowId(row))
		);
	}

	function getIsIndeterminate(): boolean {
		if (options.data.length === 0) return false;
		const someSelected = options.data.some((row) =>
			options.selectedIds.includes(options.rowId(row))
		);
		const allSelected = options.data.every((row) =>
			options.selectedIds.includes(options.rowId(row))
		);
		return someSelected && !allSelected;
	}

	function toggleRow(rowId: string, event?: MouseEvent) {
		if (!options.onSelect) return;

		const newSelected = new Set(options.selectedIds);

		if (event?.shiftKey && lastSelectedId) {
			const ids = options.data.map((r) => options.rowId(r));
			const startIdx = ids.indexOf(lastSelectedId);
			const endIdx = ids.indexOf(rowId);

			if (startIdx !== -1 && endIdx !== -1) {
				const [min, max] =
					startIdx < endIdx ? [startIdx, endIdx] : [endIdx, startIdx];
				for (let i = min; i <= max; i++) {
					newSelected.add(ids[i]);
				}
			}
		} else {
			if (newSelected.has(rowId)) {
				newSelected.delete(rowId);
			} else {
				newSelected.add(rowId);
			}
		}

		lastSelectedId = rowId;
		options.onSelect(Array.from(newSelected));
	}

	function toggleAll() {
		if (!options.onSelect) return;

		if (getIsAllSelected()) {
			options.onSelect([]);
		} else {
			const allIds = options.data.map((row) => options.rowId(row));
			options.onSelect(allIds);
		}
	}

	return {
		isRowSelected,
		get isAllSelected() {
			return getIsAllSelected();
		},
		get isIndeterminate() {
			return getIsIndeterminate();
		},
		toggleRow,
		toggleAll,
	};
}
