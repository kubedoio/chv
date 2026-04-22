<script lang="ts">
	import type { PageData } from './$types';
	import { getStoredToken, createAPIClient } from '$lib/api/client';
	import { getVmConsoleUrl, getVmBootLog, mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { 
    Play, Square, RotateCcw, Trash2, Database, Network, Activity, 
    Info, AlertTriangle, ChevronRight, Terminal, FileText, Power,
    ShieldCheck
  } from 'lucide-svelte';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import VmConsole from '$lib/components/vms/VmConsole.svelte';
	import VMMetricsWidget from '$lib/components/vms/VMMetricsWidget.svelte';
	import VmSnapshots from '$lib/components/vms/VmSnapshots.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);
	let confirmingAction = $state<string | null>(null);
	let liveConsoleUrl = $state<string | undefined>(undefined);
	let consoleLoading = $state(false);
	let bootLog = $state<string>('');
	let bootLogLoading = $state(false);
	let snapshots = $state<import('$lib/api/types').VMSnapshot[]>([]);
	let snapshotsLoading = $state(false);
	let snapshotsError = $state<string | null>(null);

	$effect(() => {
		if (detail.currentTab === 'console' && detail.summary.vm_id) {
			consoleLoading = true;
			getVmConsoleUrl(detail.summary.vm_id, getStoredToken() ?? undefined)
				.then(res => { liveConsoleUrl = res.url; })
				.catch(() => { liveConsoleUrl = undefined; })
				.finally(() => { consoleLoading = false; });
		}
	});

	$effect(() => {
		if (detail.currentTab === 'boot-log' && detail.summary.vm_id) {
			bootLogLoading = true;
			getVmBootLog(detail.summary.vm_id, getStoredToken() ?? undefined)
				.then(res => { bootLog = res.content || '(LOG_VACUUM)'; })
				.catch(() => { bootLog = '(LOG_FAILURE)'; })
				.finally(() => { bootLogLoading = false; });
		}
	});

	async function loadSnapshots() {
		if (!detail.summary.vm_id) return;
		snapshotsLoading = true;
		try {
			const client = createAPIClient();
			snapshots = await client.listVMSnapshots(detail.summary.vm_id);
		} catch (err: any) {
			snapshotsError = err.message || 'Snapshot registry inaccessible';
		} finally {
			snapshotsLoading = false;
		}
	}

	$effect(() => {
		if (detail.currentTab === 'snapshots' && detail.summary.vm_id) {
			loadSnapshots();
		}
	});

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['running', 'healthy', 'ready', 'active', 'online'].includes(s)) return 'healthy';
		if (['warning', 'maintenance', 'starting', 'stopping', 'paused', 'rebooting'].includes(s)) return 'warning';
		if (['failed', 'error', 'critical', 'crashed', 'deleting'].includes(s)) return 'failed';
		return 'unknown';
	}

	async function executeAction(action: string) {
		confirmingAction = null;
		pendingAction = action;
		const token = getStoredToken() ?? undefined;
		const vm_id = detail.summary.vm_id;

		try {
			if (action === 'delete') {
				await deleteVm({ vm_id, requested_by: 'webui' }, token);
				toast.success(`VM ${vm_id} delete accepted`);
			} else {
				const apiAction = action === 'shutdown' ? 'stop' : action;
				await mutateVm({ vm_id, action: apiAction, force: action === 'stop' }, token);
				toast.success(`Workload ${action} accepted`);
			}
			await invalidateAll();
		} catch (err: any) {
			toast.error(err.message || 'Mutation failed');
		} finally {
			pendingAction = null;
		}
	}

	const postureProps = $derived([
		{ label: 'Power Matrix', value: detail.summary.power_state },
		{ label: 'Safety Integrity', value: detail.summary.health },
		{ label: 'Core Alloc', value: detail.summary.cpu },
		{ label: 'RAM Reserv', value: detail.summary.memory }
	]);

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));

	const volumeColumns = [
		{ key: 'name', label: 'Volume Identity' },
		{ key: 'size', label: 'Density', align: 'right' as const },
		{ key: 'device_name', label: 'Device Path' },
		{ key: 'health', label: 'Integrity', align: 'center' as const }
	];

	const nicColumns = [
		{ key: 'network_name', label: 'Fabric Registry' },
		{ key: 'ip_address', label: 'Primary IP' },
		{ key: 'mac_address', label: 'L2 Identity' },
		{ key: 'addressing_mode', label: 'DHCP/STA', align: 'center' as const }
	];

	const timelineTasks = $derived(detail.recent_tasks.map(t => ({
		...t,
		tone: normalizeTone(t.status)
	})));
</script>

<div class="inventory-page">
	{#if detail.state === 'error'}
		<ErrorState title="Workload record unreachable" description="Failed to retrieve guest telemetry from the hypervisor fabric." />
	{:else if detail.state === 'empty'}
		<EmptyInfrastructureState title="Workload Identity Unknown" description="The requested virtual entity is not recognized." />
	{:else}
		<ResourceDetailHeader 
			title={detail.summary.name} 
			eyebrow="WORKLOAD_INST // {detail.summary.vm_id}"
			statusLabel={detail.summary.power_state}
			tone={normalizeTone(detail.summary.power_state)}
			parentLabel="Virtual Machines"
			parentHref="/vms"
		>
			{#snippet actions()}
				<div class="header-actions">
           {#if confirmingAction}
							<div class="confirm-group">
								<span class="confirm-text">CONFIRM_{confirmingAction.toUpperCase()}?</span>
								<button class="btn-primary" onclick={() => executeAction(confirmingAction!)}>COMMIT</button>
								<button class="btn-secondary" onclick={() => confirmingAction = null}>ABORT</button>
							</div>
							{:else}
								{@const ps = detail.summary.power_state.toLowerCase()}
								<button class="btn-primary" disabled={ps === 'running' || pendingAction !== null} onclick={() => executeAction('start')}>
									<Play size={14} />
									{pendingAction === 'start' ? 'EXECUTING...' : 'START'}
								</button>
								<button class="btn-secondary" disabled={ps !== 'running' || pendingAction !== null} onclick={() => confirmingAction = 'shutdown'}>
									<Power size={14} />
									{pendingAction === 'shutdown' ? 'EXECUTING...' : 'SHUTDOWN'}
								</button>
								<button class="btn-secondary" disabled={ps !== 'running' || pendingAction !== null} onclick={() => confirmingAction = 'restart'}>
									<RotateCcw size={14} />
									{pendingAction === 'restart' ? 'EXECUTING...' : 'REBOOT'}
								</button>
								<button class="btn-secondary" disabled={pendingAction !== null} onclick={() => confirmingAction = 'delete'}>
									<Trash2 size={14} />
									{pendingAction === 'delete' ? 'EXECUTING...' : 'DELETE'}
								</button>
						{/if}
				</div>
			{/snippet}
		</ResourceDetailHeader>

    <div class="tabs-area">
		  <DetailTabs tabs={detail.sections} currentId={detail.currentTab} />
    </div>

		<main class="inventory-main">
			<section class="detail-content">
				{#if detail.currentTab === 'console'}
					<SectionCard title="Direct Fabric Console" icon={Terminal}>
						{#if consoleLoading}
							<p class="empty-hint">Establishing encrypted bypass tunnel...</p>
						{:else if liveConsoleUrl}
							<VmConsole
								vmId={detail.summary.vm_id}
								consoleUrl={liveConsoleUrl}
								running={detail.summary.power_state.toLowerCase() === 'running'}
								getConsoleUrl={async () => {
									const res = await getVmConsoleUrl(detail.summary.vm_id, getStoredToken() ?? undefined);
									return res.url;
								}}
							/>
						{:else}
							<p class="empty-hint">Console registry inaccessible. Instance state may prevent access.</p>
						{/if}
					</SectionCard>
				{:else if detail.currentTab === 'boot-log'}
					<SectionCard title="Serial Boot Sequence" icon={FileText}>
						{#if bootLogLoading}
							<p class="empty-hint">Streaming boot sequence records...</p>
						{:else}
							<pre class="boot-log">{bootLog}</pre>
						{/if}
					</SectionCard>
				{:else if detail.currentTab === 'snapshots'}
					<VmSnapshots vmId={detail.summary.vm_id} {snapshots} loading={snapshotsLoading} error={snapshotsError} />
				{:else}
					<div class="summary-top">
						<SectionCard title="Workload Posture" icon={Activity}>
							<PropertyGrid properties={postureProps} columns={4} />
						</SectionCard>
					</div>

					<div class="vital-metrics">
						<VMMetricsWidget vms={{
							total: 1,
							running: detail.summary.power_state.toLowerCase() === 'running' ? 1 : 0,
							stopped: detail.summary.power_state.toLowerCase() === 'stopped' ? 1 : 0,
							error: detail.summary.health.toLowerCase() === 'error' ? 1 : 0
						}} />
					</div>

					<SectionCard title="Storage Fabric" icon={Database} badgeLabel={String(detail.summary.attached_volumes?.length ?? 0)}>
						{#if !detail.summary.attached_volumes || detail.summary.attached_volumes.length === 0}
							<p class="empty-hint">No storage volumes mapped to this workload fabric.</p>
						{:else}
							<InventoryTable 
								columns={volumeColumns} 
								rows={detail.summary.attached_volumes.map(v => ({
                  ...v,
                  health: { label: v.health, tone: normalizeTone(v.health) }
                }))} 
								rowHref={(row) => `/volumes/${row.volume_id}`} 
							>
                {#snippet cell({ column, row })}
                   {#if column.key === 'name'}
                     <span class="workload-name">{row.name}</span>
                   {:else if column.key === 'health'}
                     <StatusBadge label={row.health.label} tone={row.health.tone} />
                   {:else}
                     <span class="cell-text">{row[column.key]}</span>
                   {/if}
                {/snippet}
              </InventoryTable>
						{/if}
					</SectionCard>

					<SectionCard title="Network Mesh" icon={Network} badgeLabel={String(detail.summary.attached_nics?.length ?? 0)}>
						{#if !detail.summary.attached_nics || detail.summary.attached_nics.length === 0}
							<p class="empty-hint">No L2 interfaces defined for this virtual entity.</p>
						{:else}
							<InventoryTable 
								columns={nicColumns} 
								rows={detail.summary.attached_nics.map(n => ({
                  ...n,
                  ip_address: n.ip_address || 'UNASSIGNED',
                  addressing_mode: { label: n.addressing_mode === 'internal' ? 'DHCP' : 'STATIC', tone: n.addressing_mode === 'internal' ? 'healthy' : 'warning' }
                }))} 
							>
                {#snippet cell({ column, row })}
                  {#if column.key === 'addressing_mode'}
                     <StatusBadge label={row.addressing_mode.label} tone={row.addressing_mode.tone} />
                  {:else}
                     <span class="cell-text">{row[column.key]}</span>
                  {/if}
                {/snippet}
              </InventoryTable>
						{/if}
					</SectionCard>

					<SectionCard title="Operational History" icon={Activity}>
						<TaskTimeline tasks={timelineTasks} />
					</SectionCard>
				{/if}
			</section>

			<aside class="support-area">
				<SectionCard title="Placement Audit" icon={ChevronRight}>
					<PropertyGrid 
						columns={1}
						properties={[
							{ label: 'Fabric Host', value: detail.summary.node_id },
							{ label: 'Security Domain', value: 'BALANCED' },
							{ label: 'Hypervisor Sub', value: 'CLOUD_HYPERVISOR_v3' }
						]} 
					/>
				</SectionCard>

				<SectionCard title="Workload Meta" icon={Info}>
					<PropertyGrid properties={configProps} columns={1} />
				</SectionCard>

				<SectionCard title="Safety Integrity" icon={ShieldCheck}>
					<div class="safety-sign">
						<ShieldCheck size={16} />
						<span>GUEST_LEVEL_NOMINAL</span>
					</div>
				</SectionCard>
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

	.boot-log {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		line-height: 1.5;
		background: var(--color-neutral-50);
		border: 1px solid var(--shell-line);
		border-radius: var(--radius-md);
		padding: var(--space-4);
		overflow-x: auto;
		max-height: 600px;
		overflow-y: auto;
		white-space: pre;
		color: var(--shell-text);
		margin: 0;
	}
</style>
