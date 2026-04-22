import { browser } from '$app/environment';

export type SelectionType = 'datacenter' | 'cluster' | 'node' | 'vm' | 'fleet';

export interface SelectionState {
	type: SelectionType;
	id: string | null;
	parentId: string | null;
	label: string | null;
}

class SelectionStore {
	#active = $state<SelectionState>({
		type: 'fleet',
		id: null,
		parentId: null,
		label: 'Global Fleet'
	});

	get active() {
		return this.#active;
	}

	select(type: SelectionType, id: string | null, label: string | null = null, parentId: string | null = null) {
		this.#active = { type, id, label, parentId };
	}

	clear() {
		this.#active = {
			type: 'fleet',
			id: null,
			parentId: null,
			label: 'Global Fleet'
		};
	}
}

export const selection = new SelectionStore();
