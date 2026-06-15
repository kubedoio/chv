import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { getApplyRun, getArchitecture, type ApplyRunDetail, type Architecture } from '$lib/bff/architectures';

export type RunDetailModel =
	| { state: 'ready'; architecture: Architecture; run: ApplyRunDetail }
	| { state: 'error'; architectureId: string; runId: string; errorMessage: string };

export const load: PageLoad = async ({ params }) => {
	const token = getStoredToken() ?? undefined;
	try {
		const detail = await getArchitecture({ id: params.id }, token);
		const run = await getApplyRun(params.id, params.run_id, token);
		const model: RunDetailModel = {
			state: 'ready',
			architecture: detail.architecture,
			run
		};
		return { model };
	} catch (err) {
		const message = err instanceof Error ? err.message : 'Failed to load run';
		// eslint-disable-next-line no-console
		console.error('[architecture-run-loader]', message, err);
		const model: RunDetailModel = {
			state: 'error',
			architectureId: params.id,
			runId: params.run_id,
			errorMessage: message
		};
		return { model };
	}
};
