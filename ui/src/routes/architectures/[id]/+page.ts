import type { PageLoad } from './$types';
import { error } from '@sveltejs/kit';
import { getStoredToken } from '$lib/api/client';
import { getArchitecture, type Architecture } from '$lib/bff/architectures';

export type ArchitectureDetailModel =
	| {
			state: 'ready';
			architecture: Architecture;
			designGraphJson: string | null;
			latestYaml: string | null;
	  }
	| { state: 'error'; id: string; errorMessage: string };

export const load: PageLoad = async ({ params }) => {
	const token = getStoredToken() ?? undefined;

	try {
		const res = await getArchitecture({ id: params.id }, token);
		if (!res.architecture) {
			error(404, 'Architecture not found');
		}
		const model: ArchitectureDetailModel = {
			state: 'ready',
			architecture: res.architecture,
			designGraphJson: res.design_graph_json,
			latestYaml: res.latest_yaml
		};
		return { detail: model };
	} catch (err) {
		const message = err instanceof Error ? err.message : 'Failed to load architecture';
		// eslint-disable-next-line no-console
		console.error('[architecture-detail-loader]', message, err);
		const model: ArchitectureDetailModel = {
			state: 'error',
			id: params.id,
			errorMessage: message
		};
		return { detail: model };
	}
};
