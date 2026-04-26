import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

export type HypervisorSettings = {
	cpu_nested: boolean;
	cpu_amx: boolean;
	cpu_kvm_hyperv: boolean;
	memory_mergeable: boolean;
	memory_hugepages: boolean;
	memory_shared: boolean;
	memory_prefault: boolean;
	iommu: boolean;
	rng_src: string;
	watchdog: boolean;
	landlock_enable: boolean;
	serial_mode: string;
	console_mode: string;
	pvpanic: boolean;
	tpm_type: string | null;
	tpm_socket_path: string | null;
	profile_id: string | null;
};

export type HypervisorProfile = {
	id: string;
	name: string;
	description: string | null;
	cpu_nested: boolean | null;
	cpu_amx: boolean | null;
	cpu_kvm_hyperv: boolean | null;
	memory_mergeable: boolean | null;
	memory_hugepages: boolean | null;
	memory_shared: boolean | null;
	memory_prefault: boolean | null;
	iommu: boolean | null;
	rng_src: string | null;
	watchdog: boolean | null;
	landlock_enable: boolean | null;
	serial_mode: string | null;
	console_mode: string | null;
	pvpanic: boolean | null;
	tpm_type: string | null;
	tpm_socket_path: string | null;
	is_builtin: boolean;
};

export type HypervisorSettingsResponse = {
	settings: HypervisorSettings;
	profiles: HypervisorProfile[];
};

export type HypervisorSettingsPatch = Partial<HypervisorSettings>;

export async function getHypervisorSettings(token?: string): Promise<HypervisorSettingsResponse> {
	return bffFetch(BFFEndpoints.getHypervisorSettings, {
		method: 'GET',
		token
	});
}

export async function updateHypervisorSettings(
	payload: HypervisorSettingsPatch,
	token?: string
): Promise<HypervisorSettingsResponse> {
	return bffFetch(BFFEndpoints.updateHypervisorSettings, {
		method: 'PATCH',
		body: JSON.stringify(payload),
		token
	});
}

export async function applyHypervisorProfile(
	profileId: string,
	token?: string
): Promise<HypervisorSettingsResponse> {
	return bffFetch(`${BFFEndpoints.applyHypervisorProfile}/${profileId}`, {
		method: 'POST',
		token
	});
}

export async function listHypervisorProfiles(token?: string): Promise<{ profiles: HypervisorProfile[] }> {
	return bffFetch(BFFEndpoints.listHypervisorProfiles, {
		method: 'GET',
		token
	});
}
