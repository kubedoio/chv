import type { PageLoad } from './$types';
import { getStoredToken } from '#lib/api/client.ts';
import { getHypervisorSettings } from '#lib/bff/hypervisor-settings.ts';
import type { HypervisorSettingsResponse } from '#lib/bff/types.ts';

export type HypervisorPageModel = {
	settings: HypervisorSettingsResponse['settings'] | null;
	profiles: HypervisorSettingsResponse['profiles'];
	state: 'ready' | 'error';
};

export const load: PageLoad = async () => {
	const token = getStoredToken() ?? undefined;
	try {
		const res = await getHypervisorSettings(token);
		return {
			hypervisor: {
				settings: res.settings,
				profiles: res.profiles,
				state: 'ready' as const
			}
		};
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF getHypervisorSettings error:', err);
		return {
			hypervisor: {
				settings: null,
				profiles: [],
				state: 'error' as const
			}
		};
	}
};
