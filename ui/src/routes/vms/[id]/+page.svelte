<script lang="ts">
	import { browser } from '$app/environment';
	import type { PageData } from './$types';
	import { getStoredToken } from '$lib/api/client';
	import { getVmConsoleUrl, getVmBootLog, mutateVm, deleteVm } from '$lib/bff/vms';
	import { listVmSnapshots } from '$lib/bff/snapshots';
	import { listVmEvents } from '$lib/bff/events';
	import type { VmSnapshotItem, InfrastructureEvent } from '$lib/bff/types';
	import { toast } from '$lib/stores/toast.svelte';
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
	import VmDetailActions from '$lib/components/vms/VmDetailActions.svelte';
	import VmMigrateModal from '$lib/components/vms/VmMigrateModal.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { Terminal, FileText, Activity, BarChart3 } from 'lucide-svelte';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);
	let liveConsoleUrl = $state<string | undefined>(undefined);
	let VmConsoleComponent = $state<typeof import('$lib/components/vms/VmConsole.svelte').default | null>(null);
	let consoleLoading = $state(false);
	let bootLog = $state<string>('');
	let bootLogLoading = $state(false);
	let snapshots = $state<VmSnapshotItem[]>([]);
	let snapshotsLoading = $state(false);
	let snapshotsError = $state<string | null>(null);
	let events = $state<InfrastructureEvent[]>([]);
	let eventsLoading = $state(false);
	let eventsError = $state<string | null>(null);
	let supportRailOpen = $state(false);
	let migrateModalOpen = $state(false);
	let migrateSubmitting = $state(false);

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
		snapshotsError = null;
		try {
			const token = getStoredToken() ?? undefined;
			const res = await listVmSnapshots({ vm_id: detail.summary.vm_id }, token);
			snapshots = res.items;
		} catch (err: unknown) {
			snapshotsError = err instanceof Error ? err.message : 'Snapshot registry inaccessible';
		} finally {
			snapshotsLoading = false;
		}
	}

	$effect(() => {
		if (detail.currentTab === 'snapshots' && detail.summary.vm_id) {
			loadSnapshots();
		}
	});

	$effect(() => {
		if (detail.currentTab === 'events' && detail.summary.vm_id) {
			eventsLoading = true;
			eventsError = null;
			const token = getStoredToken() ?? undefined;
			listVmEvents(detail.summary.vm_id, token)
				.then(res => { events = res.items ?? []; })
				.catch(err => { eventsError = err instanceof Error ? err.message : 'Failed to load events'; })
				.finally(() => { eventsLoading = false; });
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
		pendingAction = action;
		const token = getStoredToken() ?? undefined;
		const vm_id = detail.summary.vm_id;

		try {
			if (action === 'delete') {
				const result = await deleteVm({ vm_id, requested_by: 'webui' }, token);
				toast.success(`VM ${vm_id} delete accepted — tracking task ${result.task_id}`);
			} else {
				const apiAction = action === 'shutdown' ? 'stop' : action;
				const isForce = action === 'poweroff';
				const result = await mutateVm({ vm_id, action: apiAction, force: isForce }, token);
				toast.success(`Workload ${action} accepted — tracking task ${result.task_id}`);
			}
			invalidatePattern('vms:');
			await invalidateAll();
			setTimeout(() => {
				invalidatePattern('vms:');
				invalidateAll();
			}, 2000);
		} catch (err: unknown) {
			toast.error(err instanceof Error ? err.message : 'Mutation failed');
		} finally {
			pendingAction = null;
		}
	}

	async function executeMigrate(targetNodeId: string) {
		migrateSubmitting = true;
		const token = getStoredToken() ?? undefined;
		const vm_id = detail.summary.vm_id;

		try {
			await mutateVm({ vm_id, action: 'migrate', force: false, target_node_id: targetNodeId }, token);
			toast.success(`Migration of VM ${vm_id} accepted`);
			migrateModalOpen = false;
			invalidatePattern('vms:');
			await invalidateAll();
		} catch (err: any) {
			toast.error(err.message || 'Migration failed');
		} finally {
			migrateSubmitting = false;
		}
	}

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));
</script>

<div class="inventory-page">
	{#if detail.state === 'error'}
		<VmDetailErrorState
			vmId={detail.summary.vm_id}
			requestedVmId={data.requestedVmId}
			currentTab={detail.currentTab}
			nodeId={detail.summary.node_id}
			errorMessage={detail.errorMessage}
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
				<VmDetailActions {pendingAction} powerState={detail.summary.power_state} onExecute={executeAction} onMigrate={() => { migrateModalOpen = true; }} />
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
				{:else if detail.currentTab === 'events'}
					<SectionCard title="VM Events" icon={Activity}>
						{#if eventsLoading}
							<p class="empty-hint">Loading event stream...</p>
						{:else if eventsError}
							<p class="empty-hint">Event registry inaccessible: {eventsError}</p>
						{:else if events.length === 0}
							<p class="empty-hint">No events recorded for this workload.</p>
						{:else}
							<div class="events-table-wrap">
								<table class="events-table">
									<thead>
										<tr>
											<th>Timestamp</th>
											<th>Type</th>
											<th>Severity</th>
											<th>Message</th>
										</tr>
									</thead>
									<tbody>
										{#each events as event}
											<tr>
												<td class="events-ts">{new Date(event.occurred_at).toLocaleString('en-US', { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })}</td>
												<td>{event.type}</td>
												<td><span class="severity-badge severity-badge--{event.severity}">{event.severity}</span></td>
												<td>{event.summary}</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</div>
						{/if}
					</SectionCard>
				{:else if detail.currentTab === 'metrics'}
					<SectionCard title="VM Metrics" icon={BarChart3}>
						<div class="metrics-grid">
							<div class="metric-card">
								<span class="metric-label">CPU Assigned</span>
								<span class="metric-value">{detail.summary.cpu || '—'}</span>
							</div>
							<div class="metric-card">
								<span class="metric-label">Memory Assigned</span>
								<span class="metric-value">{detail.summary.memory || '—'}</span>
							</div>
							<div class="metric-card">
								<span class="metric-label">Power State</span>
								<span class="metric-value">{detail.summary.power_state || '—'}</span>
							</div>
							<div class="metric-card">
								<span class="metric-label">Health</span>
								<span class="metric-value">{detail.summary.health || '—'}</span>
							</div>
							<div class="metric-card">
								<span class="metric-label">Attached Volumes</span>
								<span class="metric-value">{detail.summary.attached_volumes?.length ?? 0}</span>
							</div>
							<div class="metric-card">
								<span class="metric-label">Attached NICs</span>
								<span class="metric-value">{detail.summary.attached_nics?.length ?? 0}</span>
							</div>
						</div>
					</SectionCard>
				{:else}
					<VmDetailSummaryTab
						powerState={detail.summary.power_state}
						health={detail.summary.health}
						cpu={detail.summary.cpu}
						memory={detail.summary.memory}
						volumes={detail.summary.attached_volumes ?? []}
						nics={detail.summary.attached_nics ?? []}
						recentTasks={detail.recent_tasks}
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

<VmMigrateModal
	bind:open={migrateModalOpen}
	vmId={detail.summary.vm_id}
	currentNodeId={detail.summary.node_id}
	submitting={migrateSubmitting}
	onmigrate={executeMigrate}
	onclose={() => { migrateModalOpen = false; }}
/>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
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

	}

	.events-table-wrap {
		overflow-x: auto;
	}

	.events-table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-xs);
	}

	.events-table th {
		text-align: left;
		font-weight: 700;
		color: var(--shell-text-muted);
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		white-space: nowrap;
	}

	.events-table td {
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		color: var(--shell-text);
	}

	.events-ts {
		white-space: nowrap;
		color: var(--shell-text-muted);
	}

	.severity-badge {
		display: inline-block;
		font-size: 9px;
		font-weight: 700;
		text-transform: uppercase;
		padding: 2px 6px;
		border-radius: 3px;
	}

	.severity-badge--critical {
		background: var(--color-danger-light, #fee2e2);
		color: var(--color-danger, #dc2626);
	}

	.severity-badge--warning {
		background: var(--color-warning-light, #fef3c7);
		color: var(--color-warning-dark, #92400e);
	}

	.severity-badge--info {
		background: var(--color-neutral-100, #f3f4f6);
		color: var(--color-neutral-600, #4b5563);
	}

	.metrics-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
		gap: 0.75rem;
	}

	.metric-card {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 1rem;
		background: var(--color-neutral-50, #f9fafb);
		border: 1px solid var(--shell-line);
		border-radius: var(--radius-md);
	}

	.metric-label {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		font-weight: 600;
	}

	.metric-value {
		font-size: var(--text-lg, 1.125rem);
		font-weight: 800;
		color: var(--shell-text);
	}
</style>
