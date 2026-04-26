<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import type { PageData } from './$types';
	import { getStoredToken, createAPIClient } from '$lib/api/client';
	import { getVmConsoleUrl, getVmBootLog, mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import { invalidatePattern } from '$lib/stores/api-cache.svelte';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { 
    Play, Square, RotateCcw, Trash2, Database, Network, Activity, 
    Info, AlertTriangle, ChevronRight, Terminal, FileText, Power,
    ShieldCheck, ArrowLeft, ChevronLeft
  } from 'lucide-svelte';
	import DetailTabs from '$lib/components/shared/DetailTabs.svelte';
	import VMMetricsWidget from '$lib/components/vms/VMMetricsWidget.svelte';
	import VmSnapshots from '$lib/components/vms/VmSnapshots.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);
	let confirmingAction = $state<string | null>(null);
	let liveConsoleUrl = $state<string | undefined>(undefined);
	let VmConsoleComponent = $state<typeof import('$lib/components/vms/VmConsole.svelte').default | null>(null);
	let consoleLoading = $state(false);
	let bootLog = $state<string>('');
	let bootLogLoading = $state(false);
	let snapshots = $state<import('$lib/api/types').VMSnapshot[]>([]);
	let snapshotsLoading = $state(false);
	let snapshotsError = $state<string | null>(null);
	let supportRailOpen = $state(false);

	async function ensureVmConsole() {
		if (!browser || VmConsoleComponent) return;
		const module = await import('$lib/components/vms/VmConsole.svelte');
		VmConsoleComponent = module.default;
	}

	$effect(() => {
		if (detail.currentTab === 'console' && detail.summary.vm_id) {
			ensureVmConsole();
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

	async function retryDetailLoad() {
		await invalidateAll();
	}

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
				const isForce = action === 'poweroff';
				await mutateVm({ vm_id, action: apiAction, force: isForce }, token);
				toast.success(`Workload ${action} accepted`);
			}
			invalidatePattern('vms:');
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
		<ResourceDetailHeader
			title="Requested workload unreachable"
			eyebrow={`VM ID ${data.requestedVmId ?? detail.summary.vm_id}`}
			statusLabel="unreachable"
			tone="failed"
			description="The control plane could not assemble a reliable VM record for this route. Keep the operator in a recovery workflow instead of a blank dead end."
			parentLabel="Virtual machines"
			parentHref="/vms"
		>
			{#snippet actions()}
				<div class="header-actions">
					<Button variant="secondary" onclick={() => goto('/vms')}>
						<ArrowLeft size={14} />
						Back to Catalog
					</Button>
					<Button variant="primary" onclick={retryDetailLoad}>
						<RotateCcw size={14} />
						Retry Lookup
					</Button>
				</div>
			{/snippet}
		</ResourceDetailHeader>

		<div class="detail-recovery">
			<section class="detail-recovery__lead">
				<div class="recovery-hero">
					<div class="recovery-hero__icon">
						<AlertTriangle size={18} />
					</div>
					<div class="recovery-hero__copy">
						<h2>Workload telemetry could not be resolved.</h2>
						<p>
							The requested VM may still exist, but the control plane could not join live guest
							signals, placement data, or recent task state into a usable record.
						</p>
						<span>Most often this is a transient API gap, a stale route, or a node-side reporting interruption.</span>
					</div>
				</div>

				<SectionCard title="Recovery Paths" icon={ChevronRight} badgeLabel="Operator Actions">
					<div class="recovery-actions-grid">
						<a href="/vms" class="recovery-action">
							<strong>Return to virtual machines</strong>
							<span>Check whether the workload is still listed and whether its posture changed.</span>
						</a>
						<a href="/" class="recovery-action">
							<strong>Open fleet overview</strong>
							<span>Look for node degradation, alert spikes, or task backlog before retrying.</span>
						</a>
						<a href="/events" class="recovery-action">
							<strong>Inspect event stream</strong>
							<span>Use the incident feed to confirm whether this is a routing problem or a guest-side failure.</span>
						</a>
					</div>
				</SectionCard>
			</section>

			<aside class="detail-recovery__rail">
				<SectionCard title="Requested Object" icon={Info}>
					<PropertyGrid
						columns={1}
						properties={[
							{ label: 'Requested VM ID', value: data.requestedVmId ?? (detail.summary.vm_id || 'Unknown') },
							{ label: 'Requested tab', value: detail.currentTab || 'summary' },
							{ label: 'Known host', value: detail.summary.node_id || 'Not available' }
						]}
					/>
				</SectionCard>

				<SectionCard title="Operator Checklist" icon={ShieldCheck}>
					<ul class="recovery-checklist">
						<li>Confirm the workload still exists in the VM catalog.</li>
						<li>Verify the host node is still reporting into the control plane.</li>
						<li>Retry the lookup after the fleet event queue settles.</li>
					</ul>
				</SectionCard>

				<SectionCard title="Failure Shape" icon={Activity}>
					<div class="recovery-facts">
						<p>No guest summary was returned for this route.</p>
						<span>The page stayed in recovery mode rather than showing stale runtime controls.</span>
					</div>
				</SectionCard>
			</aside>
		</div>
	{:else if detail.state === 'empty'}
		<EmptyInfrastructureState title="Workload Identity Unknown" description="The requested virtual entity is not recognized." hint="Return to the VM catalog and refresh the workload inventory." />
	{:else}
		<ResourceDetailHeader 
			title={detail.summary.name} 
			eyebrow={`VM ID ${detail.summary.vm_id}`}
			statusLabel={detail.summary.power_state}
			tone={normalizeTone(detail.summary.power_state)}
			parentLabel="Virtual machines"
			parentHref="/vms"
		>
			{#snippet actions()}
				<div class="header-actions">
           {#if confirmingAction}
							<div class="confirm-group">
								<span class="confirm-text">Confirm {confirmingAction}?</span>
								<Button variant="primary" size="sm" onclick={() => executeAction(confirmingAction!)}>Confirm</Button>
								<Button variant="secondary" size="sm" onclick={() => confirmingAction = null}>Cancel</Button>
							</div>
							{:else}
								{@const ps = detail.summary.power_state.toLowerCase()}
								<button class="vm-action vm-action--primary" type="button" disabled={ps === 'running' || pendingAction !== null} onclick={() => executeAction('start')} title={pendingAction === 'start' ? 'Starting' : 'Start VM'} aria-label={pendingAction === 'start' ? 'Starting VM' : 'Start VM'}>
									<Play size={13} />
								</button>
								<button class="vm-action" type="button" disabled={ps !== 'running' || pendingAction !== null} onclick={() => confirmingAction = 'shutdown'} title={pendingAction === 'shutdown' ? 'Shutting down' : 'Shutdown VM'} aria-label={pendingAction === 'shutdown' ? 'Shutting down VM' : 'Shutdown VM'}>
									<Power size={13} />
								</button>
								<button class="vm-action" type="button" disabled={ps !== 'running' || pendingAction !== null} onclick={() => confirmingAction = 'poweroff'} title={pendingAction === 'poweroff' ? 'Powering off' : 'Poweroff VM'} aria-label={pendingAction === 'poweroff' ? 'Powering off VM' : 'Poweroff VM'}>
									<Square size={13} />
								</button>
								<button class="vm-action" type="button" disabled={ps !== 'running' || pendingAction !== null} onclick={() => confirmingAction = 'restart'} title={pendingAction === 'restart' ? 'Rebooting' : 'Reboot VM'} aria-label={pendingAction === 'restart' ? 'Rebooting VM' : 'Reboot VM'}>
									<RotateCcw size={13} />
								</button>
								<button class="vm-action vm-action--danger" type="button" disabled={pendingAction !== null} onclick={() => confirmingAction = 'delete'} title={pendingAction === 'delete' ? 'Deleting' : 'Delete VM'} aria-label={pendingAction === 'delete' ? 'Deleting VM' : 'Delete VM'}>
									<Trash2 size={13} />
								</button>
						{/if}
				</div>
			{/snippet}
		</ResourceDetailHeader>

    <div class="tabs-area">
		  <DetailTabs tabs={detail.sections} currentId={detail.currentTab} />
    </div>

		<main class="inventory-main" class:inventory-main--rail-open={supportRailOpen}>
			<section class="detail-content">
				{#if detail.currentTab === 'console'}
					<SectionCard title="Direct Fabric Console" icon={Terminal}>
						{#if consoleLoading}
							<p class="empty-hint">Establishing encrypted bypass tunnel...</p>
						{:else if liveConsoleUrl && VmConsoleComponent}
							<VmConsoleComponent
								vmId={detail.summary.vm_id}
								consoleUrl={liveConsoleUrl}
								running={detail.summary.power_state.toLowerCase() === 'running'}
								getConsoleUrl={async () => {
									const res = await getVmConsoleUrl(detail.summary.vm_id, getStoredToken() ?? undefined);
									return res.url;
								}}
							/>
						{:else if liveConsoleUrl}
							<p class="empty-hint">Loading console workspace...</p>
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
										 {@const health = row.health as { label: string; tone: ShellTone }}
                     <StatusBadge label={health.label} tone={health.tone} />
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
										 {@const addressingMode = row.addressing_mode as { label: string; tone: ShellTone }}
                     <StatusBadge label={addressingMode.label} tone={addressingMode.tone} />
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

			<aside class="support-area" class:support-area--collapsed={!supportRailOpen}>
				<div class="support-rail-control">
					<button
						class="support-toggle"
						type="button"
						onclick={() => supportRailOpen = !supportRailOpen}
						title={supportRailOpen ? 'Minimize details' : 'Expand details'}
						aria-label={supportRailOpen ? 'Minimize details' : 'Expand details'}
					>
						{#if supportRailOpen}
							<ChevronRight size={14} />
						{:else}
							<ChevronLeft size={14} />
						{/if}
					</button>

					{#if !supportRailOpen}
						<button class="support-rail-tab" type="button" onclick={() => supportRailOpen = true} title="Placement audit" aria-label="Expand placement audit">
							<ChevronRight size={13} />
						</button>
						<button class="support-rail-tab" type="button" onclick={() => supportRailOpen = true} title="Workload meta" aria-label="Expand workload meta">
							<Info size={13} />
						</button>
						<button class="support-rail-tab" type="button" onclick={() => supportRailOpen = true} title="Safety integrity" aria-label="Expand safety integrity">
							<ShieldCheck size={13} />
						</button>
					{/if}
				</div>

				{#if supportRailOpen}
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
				{/if}
			</aside>
		</main>
	{/if}
</div>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.header-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
		align-items: center;
		justify-content: flex-end;
	}

	.vm-action {
		display: inline-grid;
		place-items: center;
		width: 1.85rem;
		height: 1.85rem;
		padding: 0;
		border: 1px solid var(--shell-line);
		border-radius: var(--radius-xs);
		background: var(--shell-surface);
		color: var(--shell-text-secondary);
		cursor: pointer;
		transition:
			background 120ms ease,
			border-color 120ms ease,
			color 120ms ease;
	}

	.vm-action:hover:not(:disabled) {
		border-color: var(--shell-accent);
		background: var(--shell-accent-soft);
		color: var(--shell-text);
	}

	.vm-action:disabled {
		cursor: not-allowed;
		opacity: 0.42;
	}

	.vm-action--primary {
		background: var(--shell-accent);
		border-color: var(--shell-accent);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.vm-action--primary:hover:not(:disabled) {
		background: var(--color-primary-active);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.vm-action--danger:hover:not(:disabled) {
		border-color: var(--color-danger);
		background: var(--color-danger-light);
		color: var(--color-danger-dark);
	}

	.tabs-area {
		margin-top: -0.25rem;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 2.4rem;
		gap: 1rem;
		align-items: start;
	}

	.inventory-main--rail-open {
		grid-template-columns: minmax(0, 1.65fr) minmax(17rem, 0.9fr);
	}

	.detail-content,
	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		min-width: 0;
	}

	.support-area {
		position: sticky;
		top: 0.75rem;
	}

	.support-area--collapsed {
		align-items: stretch;
		gap: 0.35rem;
	}

	.support-rail-control {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.support-toggle,
	.support-rail-tab {
		display: grid;
		place-items: center;
		width: 2.2rem;
		height: 2.2rem;
		border: 1px solid var(--shell-line);
		border-radius: var(--radius-xs);
		background: var(--shell-surface);
		color: var(--shell-text-muted);
		cursor: pointer;
	}

	.support-toggle:hover,
	.support-rail-tab:hover {
		border-color: var(--shell-accent);
		background: var(--shell-accent-soft);
		color: var(--shell-text);
	}

	.summary-top,
	.vital-metrics {
		display: flex;
		flex-direction: column;
	}

	.detail-recovery {
		display: grid;
		grid-template-columns: minmax(0, 1.55fr) minmax(18rem, 0.85fr);
		gap: 1rem;
		align-items: start;
	}

	.detail-recovery__lead,
	.detail-recovery__rail {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		min-width: 0;
	}

	.recovery-hero {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 1rem;
		padding: 1rem 1.1rem;
		border-radius: var(--radius-md);
		border: 1px solid var(--color-danger);
		background: linear-gradient(180deg, var(--color-danger-light), color-mix(in srgb, var(--color-danger-light) 35%, white));
	}

	.recovery-hero__icon {
		display: grid;
		place-items: center;
		width: 2.8rem;
		height: 2.8rem;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-danger);
		color: var(--color-danger-dark);
		background: var(--bg-surface);
	}

	.recovery-hero__copy h2,
	.recovery-hero__copy p,
	.recovery-hero__copy span {
		margin: 0;
	}

	.recovery-hero__copy h2 {
		font-size: var(--text-lg);
		line-height: 1.2;
		color: var(--shell-text);
	}

	.recovery-hero__copy p {
		margin-top: 0.4rem;
		font-size: var(--text-sm);
		line-height: 1.55;
		color: var(--shell-text);
		max-width: 46rem;
	}

	.recovery-hero__copy span {
		display: block;
		margin-top: 0.55rem;
		font-size: var(--text-xs);
		line-height: 1.5;
		color: var(--shell-text-muted);
	}

	.recovery-actions-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.75rem;
	}

	.recovery-action {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0.85rem 0.9rem;
		border-radius: var(--radius-sm);
		border: 1px solid var(--shell-line);
		background: var(--shell-surface-muted);
		text-decoration: none;
		color: inherit;
	}

	.recovery-action strong {
		font-size: var(--text-sm);
		color: var(--shell-text);
	}

	.recovery-action span {
		font-size: var(--text-xs);
		line-height: 1.5;
		color: var(--shell-text-muted);
	}

	.recovery-action:hover {
		border-color: var(--shell-accent);
		background: color-mix(in srgb, var(--shell-surface-muted) 70%, var(--color-primary-light));
	}

	.recovery-checklist {
		display: flex;
		flex-direction: column;
		gap: 0.55rem;
		padding-left: 1rem;
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--shell-text);
	}

	.recovery-facts {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.recovery-facts p,
	.recovery-facts span {
		margin: 0;
	}

	.recovery-facts p {
		font-size: var(--text-sm);
		color: var(--shell-text);
	}

	.recovery-facts span {
		font-size: var(--text-xs);
		line-height: 1.5;
		color: var(--shell-text-muted);
	}

	.confirm-group {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.5rem;
		background: var(--color-danger-light);
		padding: 0.45rem 0.65rem;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-danger);
	}

	.confirm-text {
		font-size: var(--text-sm);
		color: var(--color-danger-dark);
		font-weight: 600;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
	}

	.safety-sign {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.65rem 0.75rem;
		border-radius: var(--radius-sm);
		background: var(--color-success-light);
		color: var(--color-success-dark);
		font-size: var(--text-sm);
		font-weight: 600;
	}

	@media (max-width: 1200px) {
		.detail-recovery,
		.inventory-main {
			grid-template-columns: 1fr;
		}

		.recovery-actions-grid {
			grid-template-columns: 1fr;
		}

		.support-area {
			order: -1;
			position: static;
			align-items: flex-start;
		}

		.support-rail-control {
			flex-direction: row;
		}
	}

	@media (max-width: 720px) {
		.tabs-area {
			margin-top: 0;
		}

		.confirm-group {
			align-items: stretch;
		}

		.header-actions :global(button),
		.confirm-group :global(button) {
			flex: 0 0 auto;
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
