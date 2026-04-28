<script lang="ts">
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import type { PageData } from './$types';
	import { getStoredToken, createAPIClient } from '$lib/api/client';
	import { getVmConsoleUrl, getVmBootLog, mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import { invalidatePattern } from '$lib/stores/api-cache.svelte';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import DetailTabs from '$lib/components/shared/DetailTabs.svelte';
	import VmSnapshots from '$lib/components/vms/VmSnapshots.svelte';
	import VmDetailErrorState from '$lib/components/vms/VmDetailErrorState.svelte';
	import VmDetailSummaryTab from '$lib/components/vms/VmDetailSummaryTab.svelte';
	import VmDetailSupportRail from '$lib/components/vms/VmDetailSupportRail.svelte';
	import Button from '$lib/components/primitives/Button.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';
	import {
		Play, Square, RotateCcw, Trash2, Power,
		Terminal, FileText
	} from 'lucide-svelte';

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

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));

	const timelineTasks = $derived(detail.recent_tasks.map(t => ({
		...t,
		tone: normalizeTone(t.status)
	})));
</script>

<div class="inventory-page">
	{#if detail.state === 'error'}
		<VmDetailErrorState
			vmId={detail.summary.vm_id}
			requestedVmId={data.requestedVmId}
			currentTab={detail.currentTab}
			nodeId={detail.summary.node_id}
			onRetry={retryDetailLoad}
		/>
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
					<VmDetailSummaryTab
						powerState={detail.summary.power_state}
						health={detail.summary.health}
						cpu={detail.summary.cpu}
						memory={detail.summary.memory}
						volumes={detail.summary.attached_volumes?.map(v => ({
							...v,
							health: { label: v.health, tone: normalizeTone(v.health) }
						})) ?? []}
						nics={detail.summary.attached_nics?.map(n => ({
							...n,
							ip_address: n.ip_address || 'UNASSIGNED',
							addressing_mode: { label: n.addressing_mode === 'internal' ? 'DHCP' : 'STATIC', tone: n.addressing_mode === 'internal' ? 'healthy' : 'warning' }
						})) ?? []}
						tasks={timelineTasks}
					/>
				{/if}
			</section>

			<VmDetailSupportRail
				nodeId={detail.summary.node_id}
				{configProps}
				open={supportRailOpen}
				onToggle={() => supportRailOpen = !supportRailOpen}
			/>
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

	.detail-content {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		min-width: 0;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
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

	@media (max-width: 1200px) {
		.inventory-main {
			grid-template-columns: 1fr;
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
</style>
