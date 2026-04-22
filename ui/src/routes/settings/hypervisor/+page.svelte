<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		Cpu,
		MemoryStick,
		Plug,
		Shield,
		Monitor,
		Settings,
		RotateCcw,
		Check,
		ChevronDown,
		ChevronUp,
		AlertTriangle
	} from 'lucide-svelte';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import ConfirmAction from '$lib/components/modals/ConfirmAction.svelte';
	import { toast } from '$lib/stores/toast';
	import { getStoredToken, getStoredRole } from '$lib/api/client';
	import {
		updateHypervisorSettings,
		applyHypervisorProfile,
		type HypervisorSettings
	} from '$lib/bff/hypervisor-settings';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const token = getStoredToken() ?? undefined;
	const currentRole = getStoredRole();

	// Page state
	let settings = $state<HypervisorSettings | null>(null);
	let profiles = $state<{ id: string; name: string; description: string }[]>([]);
	let selectedProfileId = $state<string>('');
	let saving = $state(false);
	let confirmApplyOpen = $state(false);
	let confirmResetOpen = $state(false);
	let expandedGroups = $state<Record<string, boolean>>({
		cpu: true,
		memory: true,
		devices: true,
		security: true,
		serial: true
	});

	$effect(() => {
		settings = data.hypervisor?.settings ?? null;
		profiles = data.hypervisor?.profiles ?? [];
		selectedProfileId = settings?.profile_id ?? '';
	});

	onMount(() => {
		if (!token) {
			goto('/login');
			return;
		}
		if (currentRole !== 'admin') {
			goto('/settings');
			return;
		}
	});

	// Debounce helper
	let debounceTimer: ReturnType<typeof setTimeout>;
	function debouncedSave(patch: Partial<HypervisorSettings>) {
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => {
			doSave(patch);
		}, 300);
	}

	async function doSave(patch: Partial<HypervisorSettings>) {
		if (!settings) return;
		saving = true;
		try {
			const res = await updateHypervisorSettings(patch, token);
			settings = res.settings;
			selectedProfileId = res.settings.profile_id ?? '';
			toast.success('Settings saved');
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to save settings';
			toast.error(message);
		} finally {
			saving = false;
		}
	}

	function handleToggle(field: keyof HypervisorSettings, value: boolean) {
		if (!settings) return;
		settings = { ...settings, [field]: value };
		debouncedSave({ [field]: value });
	}

	function handleStringChange(field: keyof HypervisorSettings, value: string) {
		if (!settings) return;
		settings = { ...settings, [field]: value };
		debouncedSave({ [field]: value });
	}

	function handleNullableStringChange(field: keyof HypervisorSettings, value: string) {
		if (!settings) return;
		const val = value === '' ? null : value;
		settings = { ...settings, [field]: val };
		debouncedSave({ [field]: val });
	}

	function openApplyProfile() {
		if (!selectedProfileId) {
			toast.error('Select a profile first');
			return;
		}
		confirmApplyOpen = true;
	}

	async function handleApplyProfile() {
		if (!selectedProfileId) return;
		try {
			const res = await applyHypervisorProfile(selectedProfileId, token);
			settings = res.settings;
			selectedProfileId = res.settings.profile_id ?? '';
			toast.success('Profile applied');
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to apply profile';
			toast.error(message);
		}
	}

	async function handleResetToDefaults() {
		const defaults: Partial<HypervisorSettings> = {
			cpu_nested: true,
			cpu_amx: false,
			cpu_kvm_hyperv: false,
			memory_mergeable: false,
			memory_hugepages: false,
			memory_shared: false,
			memory_prefault: false,
			iommu: false,
			rng_src: '/dev/urandom',
			watchdog: false,
			landlock_enable: false,
			serial_mode: 'Pty',
			console_mode: 'Off',
			pvpanic: false,
			tpm_type: null,
			tpm_socket_path: null,
			profile_id: null
		};
		try {
			const res = await updateHypervisorSettings(defaults, token);
			settings = res.settings;
			selectedProfileId = '';
			toast.success('Settings reset to defaults');
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to reset settings';
			toast.error(message);
		}
	}

	function toggleGroup(group: string) {
		expandedGroups = { ...expandedGroups, [group]: !expandedGroups[group] };
	}

	const currentProfileName = $derived(
		profiles.find((p) => p.id === settings?.profile_id)?.name ?? 'Custom'
	);

	const page = {
		href: '/settings/hypervisor',
		navLabel: 'Hypervisor Settings',
		shortLabel: 'Hypervisor',
		title: 'Hypervisor Settings',
		eyebrow: 'Administration',
		description: 'Configure global hypervisor defaults and apply preset profiles.',
		icon: Settings,
		badges: [{ label: 'Admin Only', tone: 'warning' as const }, { label: 'Global', tone: 'healthy' as const }],
		summary: [],
		focusAreas: [],
		states: {
			loading: { title: '', description: '', hint: '' },
			empty: { title: '', description: '', hint: '' },
			error: { title: '', description: '', hint: '' }
		}
	};
</script>

<div class="hypervisor-page">
	<PageHeaderWithAction {page}>
		{#snippet actions()}
			{#if saving}
				<span class="text-xs text-muted flex items-center gap-1">
					<svg class="animate-spin h-3 w-3" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
					</svg>
					Saving...
				</span>
			{/if}
		{/snippet}
	</PageHeaderWithAction>

	{#if data.hypervisor?.state === 'error'}
		<ErrorState title="Hypervisor Settings Unavailable" description="Failed to retrieve hypervisor configuration from the control plane." />
	{:else if !settings}
		<div class="loading-state">Loading hypervisor settings...</div>
	{:else}
		<main class="settings-grid">
			<div class="settings-main">
				<!-- CPU -->
				<SectionCard title="CPU" icon={Cpu}>
					<div class="form-group">
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.cpu_nested}
								onchange={(e) => handleToggle('cpu_nested', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Nested Virtualization</span>
						</label>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.cpu_amx}
								onchange={(e) => handleToggle('cpu_amx', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">AMX (Advanced Matrix Extensions)</span>
						</label>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.cpu_kvm_hyperv}
								onchange={(e) => handleToggle('cpu_kvm_hyperv', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">KVM HyperV Enlightenments</span>
						</label>
					</div>
				</SectionCard>

				<!-- Memory -->
				<SectionCard title="Memory" icon={MemoryStick}>
					<div class="form-group">
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.memory_mergeable}
								onchange={(e) => handleToggle('memory_mergeable', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Mergeable Memory (KSM)</span>
						</label>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.memory_hugepages}
								onchange={(e) => handleToggle('memory_hugepages', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Hugepages</span>
						</label>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.memory_shared}
								onchange={(e) => handleToggle('memory_shared', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Shared Memory</span>
						</label>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.memory_prefault}
								onchange={(e) => handleToggle('memory_prefault', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Prefault Memory</span>
						</label>
					</div>
				</SectionCard>

				<!-- Devices -->
				<SectionCard title="Devices" icon={Plug}>
					<div class="form-group">
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.iommu}
								onchange={(e) => handleToggle('iommu', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">IOMMU</span>
						</label>
						<div class="field-row">
							<label for="rng-src" class="field-label">RNG Source</label>
							<input
								id="rng-src"
								type="text"
								value={settings.rng_src}
								onchange={(e) => handleStringChange('rng_src', e.currentTarget.value)}
								class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
							/>
						</div>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.watchdog}
								onchange={(e) => handleToggle('watchdog', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Watchdog</span>
						</label>
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.pvpanic}
								onchange={(e) => handleToggle('pvpanic', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">PvPanic</span>
						</label>
					</div>
				</SectionCard>

				<!-- Security -->
				<SectionCard title="Security" icon={Shield}>
					<div class="form-group">
						<label class="toggle-row">
							<input
								type="checkbox"
								checked={settings.landlock_enable}
								onchange={(e) => handleToggle('landlock_enable', e.currentTarget.checked)}
								class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
							/>
							<span class="toggle-label">Landlock Sandbox</span>
						</label>
						<div class="field-row">
							<label for="tpm-type" class="field-label">TPM Type</label>
							<select
								id="tpm-type"
								value={settings.tpm_type ?? ''}
								onchange={(e) => handleNullableStringChange('tpm_type', e.currentTarget.value)}
								class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
							>
								<option value="">None</option>
								<option value="swtpm">swtpm</option>
							</select>
						</div>
						{#if settings.tpm_type}
							<div class="field-row">
								<label for="tpm-socket-path" class="field-label">TPM Socket Path</label>
								<input
									id="tpm-socket-path"
									type="text"
									value={settings.tpm_socket_path ?? ''}
									onchange={(e) => handleNullableStringChange('tpm_socket_path', e.currentTarget.value)}
									class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
								/>
							</div>
						{/if}
					</div>
				</SectionCard>

				<!-- Serial / Console -->
				<SectionCard title="Serial / Console" icon={Monitor}>
					<div class="form-group">
						<div class="field-row">
							<label for="serial-mode" class="field-label">Serial Mode</label>
							<select
								id="serial-mode"
								value={settings.serial_mode}
								onchange={(e) => handleStringChange('serial_mode', e.currentTarget.value)}
								class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
							>
								<option value="Pty">Pty</option>
								<option value="File">File</option>
								<option value="Off">Off</option>
								<option value="Null">Null</option>
							</select>
						</div>
						<div class="field-row">
							<label for="console-mode" class="field-label">Console Mode</label>
							<select
								id="console-mode"
								value={settings.console_mode}
								onchange={(e) => handleStringChange('console_mode', e.currentTarget.value)}
								class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
							>
								<option value="Pty">Pty</option>
								<option value="File">File</option>
								<option value="Off">Off</option>
								<option value="Null">Null</option>
							</select>
						</div>
					</div>
				</SectionCard>
			</div>

			<aside class="settings-side">
				<SectionCard title="Profile" icon={Settings} badgeLabel={currentProfileName}>
					<div class="side-content">
						<div class="field-row">
							<label for="profile-select" class="field-label">Select Profile</label>
							<select
								id="profile-select"
								bind:value={selectedProfileId}
								class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
							>
								<option value="">—</option>
								{#each profiles as profile}
									<option value={profile.id}>{profile.name}</option>
								{/each}
							</select>
							{#if selectedProfileId}
								{@const profile = profiles.find((p) => p.id === selectedProfileId)}
								{#if profile}
									<p class="profile-description" title={profile.description}>{profile.description}</p>
								{/if}
							{/if}
						</div>

						<button
							onclick={openApplyProfile}
							disabled={saving || !selectedProfileId}
							class="btn-primary w-full justify-center"
						>
							<Check size={14} />
							Apply Profile
						</button>

						<button
							onclick={() => (confirmResetOpen = true)}
							disabled={saving}
							class="btn-secondary w-full justify-center"
						>
							<RotateCcw size={14} />
							Reset to Defaults
						</button>
					</div>
				</SectionCard>
			</aside>
		</main>
	{/if}
</div>

<ConfirmAction
	bind:open={confirmApplyOpen}
	title="Apply Profile"
	description="This will overwrite your current global hypervisor settings with the selected profile values. Continue?"
	actionType="generic"
	confirmText="Apply Profile"
	onConfirm={handleApplyProfile}
/>

<ConfirmAction
	bind:open={confirmResetOpen}
	title="Reset to Defaults"
	description="This will reset all hypervisor settings to their factory defaults. Continue?"
	actionType="generic"
	confirmText="Reset"
	onConfirm={() => {
		confirmResetOpen = false;
		handleResetToDefaults();
	}}
/>

<style>
	.hypervisor-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.settings-main {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.settings-side {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.loading-state {
		padding: 2rem;
		text-align: center;
		color: var(--shell-text-muted);
		font-size: var(--text-sm);
	}

	.form-group {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.toggle-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		font-size: var(--text-sm);
		color: var(--shell-text);
	}

	.toggle-label {
		user-select: none;
	}

	.field-row {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.field-label {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--shell-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.side-content {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.profile-description {
		font-size: 11px;
		color: var(--shell-text-muted);
		margin: 0;
		line-height: 1.4;
	}

	.w-full {
		width: 100%;
	}

	.justify-center {
		justify-content: center;
	}

	.btn-primary {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		padding: 0.4rem 0.875rem;
		border-radius: 0.25rem;
		background: var(--shell-accent);
		color: white;
		font-size: var(--text-sm);
		font-weight: 500;
		border: none;
		cursor: pointer;
		transition: opacity 0.1s;
	}

	.btn-primary:hover {
		opacity: 0.88;
	}

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-secondary {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		padding: 0.4rem 0.875rem;
		border-radius: 0.25rem;
		background: transparent;
		color: var(--shell-text);
		font-size: var(--text-sm);
		font-weight: 500;
		border: 1px solid var(--shell-line);
		cursor: pointer;
		transition: background 0.1s;
	}

	.btn-secondary:hover {
		background: var(--shell-surface-muted);
	}

	.btn-secondary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@media (max-width: 1100px) {
		.settings-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
