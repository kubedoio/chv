<script lang="ts">
	import {
		Cpu, MemoryStick, Plug, Shield, Monitor,
		Settings, Activity, ShieldCheck
	} from 'lucide-svelte';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import HypervisorToggle from '$lib/components/hypervisor/HypervisorToggle.svelte';
	import HypervisorSelectField from '$lib/components/hypervisor/HypervisorSelectField.svelte';
	import HypervisorTextField from '$lib/components/hypervisor/HypervisorTextField.svelte';
	import HypervisorProfilePanel from '$lib/components/hypervisor/HypervisorProfilePanel.svelte';
	import { toast } from '$lib/stores/toast';
	import { getStoredToken } from '$lib/api/client';
	import {
		updateHypervisorSettings,
		applyHypervisorProfile,
		type HypervisorSettings,
		type HypervisorProfile
	} from '$lib/bff/hypervisor-settings';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const token = getStoredToken() ?? undefined;

	let settings = $state<HypervisorSettings | null>(null);
	let profiles = $state<HypervisorProfile[]>([]);
	let selectedProfileId = $state<string>('');

	$effect(() => {
		const s = data.hypervisor?.settings ?? null;
		const p = data.hypervisor?.profiles ?? [];
		settings = s;
		profiles = p;
		selectedProfileId = s?.profile_id ?? '';
	});
	let saving = $state(false);

	let debounceTimer: ReturnType<typeof setTimeout>;
	function debouncedSave(patch: Partial<HypervisorSettings>) {
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => doSave(patch), 300);
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

	async function handleApplyProfile(profileId: string) {
		if (!profileId) return;
		try {
			const res = await applyHypervisorProfile(profileId, token);
			settings = res.settings;
			selectedProfileId = res.settings.profile_id ?? '';
			toast.success('Profile applied');
		} catch (err: any) {
			toast.error(err.message || 'Profile application failed');
		}
	}

	async function handleResetToDefaults() {
		if (!settings) return;
		const defaults: Partial<HypervisorSettings> = {
			cpu_nested: false,
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
			console_mode: 'Pty',
			pvpanic: false,
			tpm_type: null,
			tpm_socket_path: null,
			profile_id: null
		};
		settings = { ...settings, ...defaults };
		await doSave(defaults);
	}

	const pageDef = {
		href: '/settings/hypervisor',
		navLabel: 'Hypervisor',
		shortLabel: 'Hypervisor',
		title: 'Fabric Infrastructure',
		eyebrow: 'SET_INFRA_REGISTRY',
		description: 'Low-level hypervisor parameters and global fabric posture.',
		icon: Settings,
		badges: [{ label: 'ADMIN_ONLY', tone: 'warning' as const }, { label: 'GLOBAL_SCOPE', tone: 'healthy' as const }],
		summary: [],
		focusAreas: [],
		states: {
			loading: { title: 'Loading', description: 'Loading hypervisor settings.', hint: '' },
			empty: { title: 'No data', description: 'No hypervisor settings available.', hint: '' },
			error: { title: 'Error', description: 'Failed to load hypervisor settings.', hint: '' }
		}
	};
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef}>
		{#snippet actions()}
			{#if saving}
				<div class="save-status">
					<Activity size={12} class="animate-pulse" />
					<span>SYNCING...</span>
				</div>
			{/if}
		{/snippet}
	</PageHeaderWithAction>

	{#if data.hypervisor?.state === 'error'}
		<ErrorState title="Fabric registry unreachable" description="Failed to retrieve hypervisor configuration from the control plane." />
	{:else if !settings}
		<div class="skeleton-grid" aria-busy="true" aria-label="Loading hypervisor settings"></div>
	{:else}
		<div class="inventory-metrics">
			<CompactMetricCard label="CPU Integrity" value={settings.cpu_nested ? 'NESTED_ON' : 'NESTED_OFF'} color={settings.cpu_nested ? 'primary' : undefined} />
			<CompactMetricCard label="Security Sandbox" value={settings.landlock_enable ? 'LANDLOCK_ON' : 'LANDLOCK_OFF'} color={settings.landlock_enable ? 'primary' : undefined} />
			<CompactMetricCard label="Memory Epoch" value={settings.memory_hugepages ? 'HUGE_PAGES' : 'STD_PAGES'} />
			<CompactMetricCard label="Policy Profile" value={profiles.find(p => p.id === settings?.profile_id)?.name ?? 'CUSTOM_VARS'} color="primary" />
		</div>

		<main class="inventory-main">
			<div class="settings-content">
				<SectionCard title="Compute Fabric" icon={Cpu}>
					<div class="params-grid">
						<HypervisorToggle checked={settings.cpu_nested} label="NESTED_VIRTUALIZATION" description="Enable recursive guest execution for hypervisor-as-a-service." onchange={(v) => handleToggle('cpu_nested', v)} />
						<HypervisorToggle checked={settings.cpu_amx} label="AMX_ACCELERATION" description="Enable Advanced Matrix Extensions for compute-dense workloads." onchange={(v) => handleToggle('cpu_amx', v)} />
						<HypervisorToggle checked={settings.cpu_kvm_hyperv} label="HYPERV_ENLIGHTENMENTS" description="Enable KVM Hyper-V parity for optimized guest telemetry." onchange={(v) => handleToggle('cpu_kvm_hyperv', v)} />
					</div>
				</SectionCard>

				<SectionCard title="Memory Architecture" icon={MemoryStick}>
					<div class="params-grid">
						<HypervisorToggle checked={settings.memory_mergeable} label="KSM_DEDUPLICATION" description="Enable kernel same-page merging for memory density." onchange={(v) => handleToggle('memory_mergeable', v)} />
						<HypervisorToggle checked={settings.memory_hugepages} label="HUGEPAGE_ALLOCATION" description="Enable large page backings for high-performance TLB usage." onchange={(v) => handleToggle('memory_hugepages', v)} />
						<HypervisorToggle checked={settings.memory_shared} label="SHARED_MEMORY" description="Allow memory sharing between host and guest processes." onchange={(v) => handleToggle('memory_shared', v)} />
						<HypervisorToggle checked={settings.memory_prefault} label="MEMORY_PREFAULT" description="Pre-fault guest memory at allocation time to avoid runtime faults." onchange={(v) => handleToggle('memory_prefault', v)} />
					</div>
				</SectionCard>

				<SectionCard title="Device Fabric & IO" icon={Plug}>
					<div class="params-grid">
						<HypervisorToggle checked={settings.iommu} label="IOMMU_TRANSLATION" description="Enable hardware-level I/O virtualization and mapping." onchange={(v) => handleToggle('iommu', v)} />
						<HypervisorToggle checked={settings.watchdog} label="WATCHDOG_TIMER" description="Enable guest watchdog for automatic crash recovery." onchange={(v) => handleToggle('watchdog', v)} />
						<HypervisorToggle checked={settings.pvpanic} label="PVPANIC_DEVICE" description="Enable paravirtualized panic notification for clean shutdowns." onchange={(v) => handleToggle('pvpanic', v)} />
						<HypervisorTextField label="RNG_IDENTITY_SOURCE" value={settings.rng_src} onchange={(v) => handleStringChange('rng_src', v)} />
					</div>
				</SectionCard>

				<SectionCard title="Serial & Protocol" icon={Monitor}>
					<div class="params-grid">
						<HypervisorSelectField label="SERIAL_FABRIC_MODE" value={settings.serial_mode} options={[{ value: 'Pty', label: 'Pty' }, { value: 'File', label: 'File' }, { value: 'Off', label: 'Off' }]} onchange={(v) => handleStringChange('serial_mode', v)} />
						<HypervisorSelectField label="CONSOLE_FABRIC_MODE" value={settings.console_mode} options={[{ value: 'Pty', label: 'Pty' }, { value: 'File', label: 'File' }, { value: 'Off', label: 'Off' }]} onchange={(v) => handleStringChange('console_mode', v)} />
						<HypervisorTextField label="TPM_TYPE" value={settings.tpm_type ?? ''} onchange={(v) => handleStringChange('tpm_type', v || null as any)} />
						<HypervisorTextField label="TPM_SOCKET_PATH" value={settings.tpm_socket_path ?? ''} onchange={(v) => handleStringChange('tpm_socket_path', v || null as any)} />
					</div>
				</SectionCard>

				<SectionCard title="Security Hardening" icon={Shield}>
					<div class="params-grid">
						<HypervisorToggle checked={settings.landlock_enable} label="LANDLOCK_SANDBOX" description="Enable Landlock LSM sandboxing for workload isolation." onchange={(v) => handleToggle('landlock_enable', v)} />
					</div>
				</SectionCard>
			</div>

			<aside class="support-area">
				<HypervisorProfilePanel
					{profiles}
					currentProfileId={settings.profile_id}
					bind:selectedProfileId
					{saving}
					onApply={handleApplyProfile}
					onReset={handleResetToDefaults}
				/>
			</aside>
		</main>
	{/if}
</div>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
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

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.skeleton-grid {
		min-height: 200px;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-sm);
		animation: pulse 1.5s infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
