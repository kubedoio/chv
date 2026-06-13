import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { listArchitectures, type ArchitectureSummary } from '$lib/bff/architectures';

export type ArchitecturesListModel = {
	items: ArchitectureSummary[];
	state: 'ready' | 'empty' | 'error';
	errorMessage?: string;
};

export const load: PageLoad = async () => {
	const token = getStoredToken() ?? undefined;

	try {
		const res = await listArchitectures({}, token);
		const items = res.items ?? [];
		const model: ArchitecturesListModel = {
			items,
			state: items.length === 0 ? 'empty' : 'ready'
		};
		return { architectures: model };
	} catch (err) {
		const message = err instanceof Error ? err.message : 'Failed to load architectures';
		// eslint-disable-next-line no-console
		console.error('[architectures-loader]', message, err);
		const model: ArchitecturesListModel = {
			items: [],
			state: 'error',
			errorMessage: message
		};
		return { architectures: model };
	}
};
