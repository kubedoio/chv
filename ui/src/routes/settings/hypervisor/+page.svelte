<script lang="ts">
	import { onMount } from 'svelte';
	import { 
		Cpu, MemoryStick, Plug, Shield, Monitor, 
		Settings, RotateCcw, Check, Activity, ShieldCheck
	} from 'lucide-svelte';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
  import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
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

	let settings = $state<HypervisorSettings | null>(null);
	let profiles = $state<{ id: string; name: string; description: string }[]>([]);
	let selectedProfileId = $state<string>('');
	let saving = $state(false);
	let confirmApplyOpen = $state(false);
	let confirmResetOpen = $state(false);

	$effect(() => {
		settings = data.hypervisor?.settings ?? null;
		profiles = data.hypervisor?.profiles ?? [];
		selectedProfileId = settings?.profile_id ?? '';
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
			toast.success('Registry updated');
		} catch (err: any) {
			toast.error(err.message || 'Registry update failed');
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

	async function handleApplyProfile() {
		if (!selectedProfileId) return;
		try {
			const res = await applyHypervisorProfile(selectedProfileId, token);
			settings = res.settings;
			selectedProfileId = res.settings.profile_id ?? '';
			toast.success('Profile applied');
		} catch (err: any) {
			toast.error(err.message || 'Profile application failed');
		}
	}

	const currentProfileName = $derived(
		profiles.find((p) => p.id === settings?.profile_id)?.name ?? 'CUSTOM_VARS'
	);

	const pageDef = {
		title: 'Fabric Infrastructure',
		eyebrow: 'SET_INFRA_REGISTRY',
		description: 'Low-level hypervisor parameters and global fabric posture.',
		icon: Settings,
		badges: [{ label: 'ADMIN_ONLY', tone: 'warning' as const }, { label: 'GLOBAL_SCOPE', tone: 'healthy' as const }]
	};
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef}>
		{#snippet actions()}
			<div class="header-actions">
				{#if saving}
					<div class="save-status">
						<Activity size={12} class="animate-pulse" />
						<span>SYNCING...</span>
					</div>
				{/if}
			</div>
		{/snippet}
	</PageHeaderWithAction>

	{#if data.hypervisor?.state === 'error'}
		<ErrorState title="Fabric registry unreachable" description="Failed to retrieve hypervisor configuration from the control plane." />
	{:else if !settings}
		<div class="skeleton-grid"></div>
	{:else}
    <div class="inventory-metrics">
			<CompactMetricCard label="CPU Integrity" value={settings.cpu_nested ? 'NESTED_ON' : 'NESTED_OFF'} color={settings.cpu_nested ? 'primary' : 'neutral'} />
			<CompactMetricCard label="Security Sandbox" value={settings.landlock_enable ? 'LANDLOCK_ON' : 'LANDLOCK_OFF'} color={settings.landlock_enable ? 'primary' : 'neutral'} />
			<CompactMetricCard label="Memory Epoch" value={settings.memory_hugepages ? 'HUGE_PAGES' : 'STD_PAGES'} color="neutral" />
			<CompactMetricCard label="Policy Profile" value={currentProfileName} color="primary" />
		</div>

		<main class="inventory-main">
			<div class="settings-content">
				<SectionCard title="Compute Fabric" icon={Cpu}>
					<div class="params-grid">
						<label class="toggle-control">
							<input type="checkbox" checked={settings.cpu_nested} onchange={(e) => handleToggle('cpu_nested', e.currentTarget.checked)} />
							<div class="toggle-meta">
								<span class="label">NESTED_VIRTUALIZATION</span>
								<span class="desc">Enable recursive guest execution for hypervisor-as-a-service.</span>
							</div>
						</label>
						<label class="toggle-control">
							<input type="checkbox" checked={settings.cpu_amx} onchange={(e) => handleToggle('cpu_amx', e.currentTarget.checked)} />
							<div class="toggle-meta">
								<span class="label">AMX_ACCELERATION</span>
								<span class="desc">Enable Advanced Matrix Extensions for compute-dense workloads.</span>
							</div>
						</label>
            <label class="toggle-control">
							<input type="checkbox" checked={settings.cpu_kvm_hyperv} onchange={(e) => handleToggle('cpu_kvm_hyperv', e.currentTarget.checked)} />
							<div class="toggle-meta">
								<span class="label">HYPERV_ENLIGHTENMENTS</span>
								<span class="desc">Enable KVM Hyper-V parity for optimized guest telemetry.</span>
							</div>
						</label>
					</div>
				</SectionCard>

				<SectionCard title="Memory Architecture" icon={MemoryStick}>
					<div class="params-grid">
						<label class="toggle-control">
							<input type="checkbox" checked={settings.memory_mergeable} onchange={(e) => handleToggle('memory_mergeable', e.currentTarget.checked)} />
							<div class="toggle-meta">
								<span class="label">KSM_DEDUPLICATION</span>
								<span class="desc">Enable kernel same-page merging for memory density.</span>
							</div>
						</label>
						<label class="toggle-control">
							<input type="checkbox" checked={settings.memory_hugepages} onchange={(e) => handleToggle('memory_hugepages', e.currentTarget.checked)} />
							<div class="toggle-meta">
								<span class="label">HUGEPAGE_ALLOCATION</span>
								<span class="desc">Enable large page backings for high-performance TLB usage.</span>
							</div>
						</label>
					</div>
				</SectionCard>

				<SectionCard title="Device Fabric & IO" icon={Plug}>
					<div class="params-grid">
						<label class="toggle-control">
							<input type="checkbox" checked={settings.iommu} onchange={(e) => handleToggle('iommu', e.currentTarget.checked)} />
							<div class="toggle-meta">
								<span class="label">IOMMU_TRANSLATION</span>
								<span class="desc">Enable hardware-level I/O virtualization and mapping.</span>
							</div>
						</label>
            <div class="field-control">
							<label class="label">RNG_IDENTITY_SOURCE</label>
							<input type="text" value={settings.rng_src} onchange={(e) => handleStringChange('rng_src', e.currentTarget.value)} />
						</div>
					</div>
				</SectionCard>

        <SectionCard title="Serial & Protocol" icon={Monitor}>
          <div class="params-grid">
            <div class="field-control">
							<label class="label">SERIAL_FABRIC_MODE</label>
							<select value={settings.serial_mode} onchange={(e) => handleStringChange('serial_mode', e.currentTarget.value)}>
								<option value="Pty">Pty</option>
								<option value="File">File</option>
								<option value="Off">Off</option>
							</select>
						</div>
            <div class="field-control">
							<label class="label">CONSOLE_FABRIC_MODE</label>
							<select value={settings.console_mode} onchange={(e) => handleStringChange('console_mode', e.currentTarget.value)}>
								<option value="Pty">Pty</option>
								<option value="File">File</option>
								<option value="Off">Off</option>
							</select>
						</div>
          </div>
        </SectionCard>
			</div>

			<aside class="support-area">
				<SectionCard title="Fabric Profiles" icon={Settings}>
					<div class="profile-ops">
            <div class="field-control">
							<label class="label">REGISTRY_PRESET</label>
							<select bind:value={selectedProfileId}>
								<option value="">—</option>
								{#each profiles as profile}
									<option value={profile.id}>{profile.name}</option>
								{/each}
							</select>
						</div>
						
						<button class="btn-primary w-full" disabled={saving || !selectedProfileId} onclick={() => confirmApplyOpen = true}>
							<Check size={14} />
							APPLY_PRESET
						</button>
						<button class="btn-secondary w-full" disabled={saving} onclick={() => confirmResetOpen = true}>
							<RotateCcw size={14} />
							RESET_DEFAULTS
						</button>
					</div>
				</SectionCard>

        <SectionCard title="Audit Posture" icon={ShieldCheck}>
          <div class="safety-sign">
            <ShieldCheck size={16} />
            <span>FABRIC_LEVEL_VERIFIED</span>
          </div>
        </SectionCard>
			</aside>
		</main>
	{/if}
</div>

<ConfirmAction
	bind:open={confirmApplyOpen}
	title="Apply Fabric Profile"
	description="This will overwrite all global fabric parameters with the preset registry. Continue?"
	actionType="generic"
	confirmText="Commit Changes"
	onConfirm={handleApplyProfile}
/>

<ConfirmAction
	bind:open={confirmResetOpen}
	title="Restore Defaults"
	description="This will reset all fabric parameters to the initial safe state. Continue?"
	actionType="generic"
	confirmText="Restore Registry"
	onConfirm={() => { confirmResetOpen = false; handleResetToDefaults(); }}
/>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

  .header-actions {
    display: flex;
    align-items: center;
  }

  .save-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 10px;
    font-weight: 800;
    color: var(--color-primary);
  }

	.inventory-metrics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.75rem;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.settings-content {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

  .params-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 1.25rem;
  }

  .toggle-control {
    display: flex;
    gap: 0.75rem;
    cursor: pointer;
  }

  .toggle-control input {
    margin-top: 0.125rem;
  }

  .toggle-meta {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .toggle-meta .label {
    font-size: 11px;
    font-weight: 800;
    color: var(--color-neutral-900);
  }

  .toggle-meta .desc {
    font-size: 10px;
    color: var(--color-neutral-500);
    line-height: 1.4;
  }

  .field-control {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .field-control .label {
    font-size: 10px;
    font-weight: 800;
    color: var(--color-neutral-500);
  }

  .field-control input, .field-control select {
    background: var(--bg-surface-muted);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xs);
    padding: 0.35rem 0.5rem;
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-neutral-900);
  }

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

  .profile-ops {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .safety-sign {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		background: rgba(var(--color-success-rgb), 0.1);
		color: var(--color-success);
		font-size: 10px;
		font-weight: 800;
		border-radius: var(--radius-xs);
	}

  .w-full { width: 100%; justify-content: center; }

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>

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
