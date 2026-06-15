import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { getArchitecture, listApplyRuns, type ApplyRunDetail, type Architecture } from '$lib/bff/architectures';

export type RunsListModel =
	| { state: 'ready'; architecture: Architecture; runs: ApplyRunDetail[] }
	| { state: 'error'; id: string; errorMessage: string };

export const load: PageLoad = async ({ params }) => {
	const token = getStoredToken() ?? undefined;
	try {
		const detail = await getArchitecture({ id: params.id }, token);
		const runs = await listApplyRuns(params.id, token);
		const model: RunsListModel = {
			state: 'ready',
			architecture: detail.architecture,
			runs
		};
		return { model };
	} catch (err) {
		const message = err instanceof Error ? err.message : 'Failed to load apply runs';
		// eslint-disable-next-line no-console
		console.error('[architecture-runs-loader]', message, err);
		const model: RunsListModel = {
			state: 'error',
			id: params.id,
			errorMessage: message
		};
		return { model };
	}
};
