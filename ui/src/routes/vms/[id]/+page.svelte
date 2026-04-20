<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { getStoredToken } from '$lib/api/client';
	import { getVmConsoleUrl, mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import { onMount } from 'svelte';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ActionStrip from '$lib/components/shell/ActionStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { Play, Square, RotateCcw, Trash2, Database, Network, Activity, Info, AlertTriangle, ChevronRight, Terminal } from 'lucide-svelte';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import VmConsole from '$lib/components/vms/VmConsole.svelte';
	import VMMetricsWidget from '$lib/components/vms/VMMetricsWidget.svelte';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);
	let confirmingAction = $state<string | null>(null);
	let actionError = $state<string | null>(null);
	let liveConsoleUrl = $state<string | undefined>(undefined);
	let consoleLoading = $state(false);

	$effect(() => {
		if (detail.currentTab === 'console' && detail.summary.vm_id) {
			consoleLoading = true;
			liveConsoleUrl = undefined;
			const token = getStoredToken() ?? undefined;
			getVmConsoleUrl(detail.summary.vm_id, token)
				.then(res => { liveConsoleUrl = res.url; })
				.catch(() => { liveConsoleUrl = undefined; })
				.finally(() => { consoleLoading = false; });
		}
	});

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['running', 'healthy', 'ready', 'active', 'online'].includes(s)) return 'healthy';
		if (['warning', 'maintenance', 'starting', 'stopping', 'paused', 'rebooting'].includes(s)) return 'warning';
		if (['degraded', 'offline'].includes(s)) return 'degraded';
		if (['failed', 'error', 'critical', 'crashed', 'deleting'].includes(s)) return 'failed';
		return 'unknown';
	}

	function handleActionClick(action: string, needsConfirm = false) {
		if (needsConfirm) {
			confirmingAction = action;
		} else {
			executeAction(action);
		}
	}

	async function executeAction(action: string) {
		confirmingAction = null;
		pendingAction = action;
		actionError = null;
		const token = getStoredToken() ?? undefined;
		const vm_id = detail.summary.vm_id;

		try {
			if (action === 'delete') {
				await deleteVm({ vm_id, requested_by: 'webui' }, token);
				toast.success(`VM ${vm_id} delete accepted`);
			} else {
				await mutateVm({ vm_id, action, force: false }, token);
				toast.success(`VM ${action} accepted`);
			}
			await invalidateAll();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Action failed';
			actionError = message;
			toast.error(message);
		} finally {
			pendingAction = null;
		}
	}

	const postureProps = $derived([
		{ label: 'Power State', value: detail.summary.power_state, tone: normalizeTone(detail.summary.power_state) as any },
		{ label: 'Health', value: detail.summary.health, tone: normalizeTone(detail.summary.health) as any },
		{ label: 'CPU', value: detail.summary.cpu },
		{ label: 'Memory', value: detail.summary.memory }
	]);

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));

	const volumeColumns = [
		{ key: 'name', label: 'Volume' },
		{ key: 'size', label: 'Size', align: 'right' as const },
		{ key: 'device_name', label: 'Device' },
		{ key: 'health', label: 'Health' }
	];

	const volumeRows = $derived((detail.summary.attached_volumes ?? []).map(v => ({
		...v,
		health: { label: v.health, tone: normalizeTone(v.health) }
	})));

	const nicColumns = [
		{ key: 'network_name', label: 'Network' },
		{ key: 'ip_address', label: 'IP Address' },
		{ key: 'mac_address', label: 'MAC Address' },
		{ key: 'nic_model', label: 'Model' },
		{ key: 'addressing_mode', label: 'Addressing' }
	];

	function addressingTone(mode: string): ShellTone {
		if (mode === 'internal') return 'healthy';
		if (mode === 'external') return 'warning';
		return 'unknown';
	}

	function addressingLabel(mode: string): string {
		if (mode === 'internal') return 'DHCP';
		if (mode === 'external') return 'External';
		return 'Static';
	}

	const nicRows = $derived((detail.summary.attached_nics ?? []).map(n => ({
		...n,
		ip_address: n.ip_address || 'No IP address reported yet',
		addressing_mode: { label: addressingLabel(n.addressing_mode), tone: addressingTone(n.addressing_mode) }
	})));

	const timelineTasks = $derived(detail.recent_tasks.map(t => ({
		...t,
		tone: normalizeTone(t.status)
	})));
</script>

<div class="resource-detail">
	{#if detail.state === 'error'}
		<ErrorState title="VM Detail Unavailable" description="Failed to retrieve guest state from the hypervisor." />
	{:else if detail.state === 'empty'}
		<EmptyInfrastructureState hint="Check the ID and try again" title="VM Not Found" description="The requested virtual machine ID is not recognized." />
	{:else}
		<ResourceDetailHeader 
			title={detail.summary.name} 
			eyebrow={detail.summary.node_id}
			statusLabel={detail.summary.power_state}
			tone={normalizeTone(detail.summary.power_state)}
			parentLabel="Virtual Machines"
			parentHref="/vms"
			description="Virtual machine workload."
		>
			{#snippet actions()}
				<div class="header-actions">
					<ActionStrip>
						{#if confirmingAction}
							<div class="confirm-group">
								<span class="confirm-text">Confirm <strong>{confirmingAction}</strong>?</span>
								<button class="btn-danger btn-sm" onclick={() => executeAction(confirmingAction!)}>Confirm</button>
								<button class="btn-secondary btn-sm" onclick={() => confirmingAction = null}>Cancel</button>
							</div>
							{:else}
								{@const ps = detail.summary.power_state.toLowerCase()}
								<button class="btn-primary btn-sm" disabled={ps === 'running' || pendingAction !== null} onclick={() => handleActionClick('start')}>
									<Play size={14} />
									{pendingAction === 'start' ? 'Starting...' : 'Start'}
								</button>
								<button class="btn-secondary btn-sm" disabled={ps !== 'running' || pendingAction !== null} onclick={() => handleActionClick('stop', true)}>
									<Square size={14} />
									{pendingAction === 'stop' ? 'Stopping...' : 'Stop'}
								</button>
								<button class="btn-secondary btn-sm" disabled={ps !== 'running' || pendingAction !== null} onclick={() => handleActionClick('restart', true)}>
									<RotateCcw size={14} />
									{pendingAction === 'restart' ? 'Restarting...' : 'Reboot'}
								</button>
								<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => handleActionClick('delete', true)}>
									<Trash2 size={14} />
									{pendingAction === 'delete' ? 'Deleting...' : 'Delete'}
								</button>
						{/if}
					</ActionStrip>
				</div>
			{/snippet}
		</ResourceDetailHeader>

		<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

		<main class="detail-grid">
			{#if detail.currentTab === 'console'}
				<section class="detail-main-span">
					<SectionCard title="Serial Console" icon={Terminal}>
						{#if consoleLoading}
							<p class="empty-hint">Connecting to console...</p>
						{:else if liveConsoleUrl}
							<VmConsole
								vmId={detail.summary.vm_id}
								consoleUrl={liveConsoleUrl}
								getConsoleUrl={async () => {
									const res = await getVmConsoleUrl(detail.summary.vm_id, getStoredToken() ?? undefined);
									return res.url;
								}}
							/>
						{:else}
							<p class="empty-hint">Console URL unavailable. The VM may not be running.</p>
						{/if}
					</SectionCard>
				</section>
			{:else}
				<section class="detail-main-span">
					<div class="summary-top">
						<SectionCard title="Guest Posture" icon={Activity}>
							<PropertyGrid properties={postureProps} columns={4} />
						</SectionCard>
					</div>

					<div class="detail-sections">
						<VMMetricsWidget vms={{
							total: 1,
							running: detail.summary.power_state.toLowerCase() === 'running' ? 1 : 0,
							stopped: detail.summary.power_state.toLowerCase() === 'stopped' ? 1 : 0,
							error: detail.summary.health.toLowerCase() === 'error' ? 1 : 0
						}} />

						<SectionCard title="Storage Attachments" icon={Database} badgeLabel={String(detail.summary.attached_volumes?.length ?? 0)}>
							{#if !detail.summary.attached_volumes || detail.summary.attached_volumes.length === 0}
								<p class="empty-hint">No storage volumes attached to this guest.</p>
							{:else}
								<InventoryTable 
									columns={volumeColumns} 
									rows={volumeRows} 
									rowHref={(row) => `/volumes/${row.volume_id}`} 
								/>
							{/if}
						</SectionCard>

						<SectionCard title="Network Interfaces" icon={Network} badgeLabel={String(detail.summary.attached_nics?.length ?? 0)}>
							{#if !detail.summary.attached_nics || detail.summary.attached_nics.length === 0}
								<p class="empty-hint">No NICs defined for this guest.</p>
							{:else}
								<InventoryTable 
									columns={nicColumns} 
									rows={nicRows} 
								/>
							{/if}
						</SectionCard>

						<SectionCard title="Operational History" icon={Activity}>
							<TaskTimeline tasks={timelineTasks} />
						</SectionCard>

						<SectionCard title="Guest Configuration" icon={Info}>
							<PropertyGrid properties={configProps} columns={2} />
						</SectionCard>
					</div>
				</section>

				<aside class="detail-side-span">
					<SectionCard title="Hypervisor Placement" icon={ChevronRight}>
						<PropertyGrid 
							columns={1}
							properties={[
								{ label: 'Host Node', value: detail.summary.node_id },
								{ label: 'Placement Policy', value: 'Balanced' },
								{ label: 'Hypervisor', value: 'KVM / QEMU' }
							]} 
						/>
					</SectionCard>

					<SectionCard title="System Alerts" icon={AlertTriangle}>
						<p class="empty-hint">No active hypervisor alerts for this workload.</p>
					</SectionCard>
				</aside>
			{/if}
		</main>
	{/if}
</div>

<style>
	.resource-detail {
		display: flex;
		flex-direction: column;
	}

	.header-actions {
		display: flex;
		gap: 0.5rem;
	}

	.confirm-group {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		background: var(--color-danger-light);
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		border: 1px solid var(--color-danger);
	}

	.confirm-text {
		font-size: var(--text-xs);
		color: var(--color-danger-dark);
		font-weight: 600;
	}

	.detail-grid {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.detail-main-span {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.detail-sections {
		display: grid;
		grid-template-columns: 1fr;
		gap: 1rem;
	}

	.detail-side-span {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
	}

	@media (max-width: 1200px) {
		.detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
