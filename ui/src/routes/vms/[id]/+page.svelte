<script lang="ts">
	import { enhance } from '$app/forms';
	import { Play, RotateCcw, Square } from 'lucide-svelte';
	import { PageShell, StateBanner, Badge, ResourceTable } from '$lib/components/system';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import TaskReferenceCallout from '$lib/components/webui/TaskReferenceCallout.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import Button from '$lib/components/primitives/Button.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData, ActionData } from './$types';
	import type { VmLifecycleAction, VmLifecycleActionResult, VmEvent } from '$lib/bff/types';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import { mapRelatedTask } from '$lib/webui/task-helpers';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { BFFEndpoints } from '$lib/bff/endpoints';
	import { bffFetch } from '$lib/bff/client';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	const page = getPageDefinition('/vms');
	const detail = $derived(data.detail);
	let pendingAction = $state<VmLifecycleAction | null>(null);
	let confirmingAction = $state<VmLifecycleAction | null>(null);
	let actionInput = $state<HTMLInputElement | null>(null);

	let vmEvents = $state<VmEvent[]>([]);
	let eventsLoading = $state(false);
	let eventsLoadedForVm = $state('');

	async function loadVmEvents(vmId: string) {
		if (eventsLoadedForVm === vmId || eventsLoading) return;
		eventsLoading = true;
		try {
			const res = await bffFetch<{ items: VmEvent[] }>(BFFEndpoints.listVmEvents, {
				method: 'POST',
				body: JSON.stringify({ vm_id: vmId })
			});
			vmEvents = res?.items ?? [];
		} catch {
			vmEvents = [];
		} finally {
			eventsLoading = false;
			eventsLoadedForVm = vmId;
		}
	}

	$effect(() => {
		if (detail.currentTab === 'events' && detail.state === 'ready' && eventsLoadedForVm !== detail.summary.vmId) {
			loadVmEvents(detail.summary.vmId);
		}
	});

	function toPowerStateTone(state: string): ShellTone {
		const s = state.toLowerCase();
		if (s === 'running') return 'healthy';
		if (s === 'starting' || s === 'stopping') return 'warning';
		if (s === 'rebooting' || s === 'deleting') return 'degraded';
		if (s === 'failed' || s === 'error') return 'failed';
		return 'unknown';
	}

	function toHealthTone(health: string): ShellTone {
		const h = health.toLowerCase();
		if (h === 'healthy') return 'healthy';
		if (h === 'degraded') return 'degraded';
		if (h === 'warning') return 'warning';
		if (h === 'failed') return 'failed';
		return 'unknown';
	}

	function toSeverityTone(severity: string): ShellTone {
		switch (severity.toLowerCase()) {
			case 'critical': return 'failed';
			case 'warning': return 'warning';
			case 'info': return 'unknown';
			default: return 'unknown';
		}
	}

	const mutationResult = $derived<VmLifecycleActionResult | null>(
		form && typeof form === 'object' && 'accepted' in form && form.accepted === true
			? {
					accepted: true,
					action: (form as unknown as { action: string }).action as VmLifecycleAction,
					summary: (form as unknown as { summary: string }).summary,
					taskId: (form as unknown as { task_id: string | undefined }).task_id ?? null,
					taskLabel: (form as unknown as { task_id: string | undefined }).task_id
						? getTaskStatusMeta('queued').label
						: getTaskStatusMeta('failed').label,
					taskTone: (form as unknown as { task_id: string | undefined }).task_id
						? getTaskStatusMeta('queued').tone
						: getTaskStatusMeta('failed').tone,
					taskHref: (form as unknown as { task_id: string | undefined }).task_id
						? `/tasks?query=${(form as unknown as { task_id: string }).task_id}`
						: null
				}
			: null
	);

	function canStart() {
		return !['running', 'starting', 'deleting'].includes(detail.summary.powerState.toLowerCase());
	}

	function canStop() {
		return ['running', 'starting', 'rebooting'].includes(detail.summary.powerState.toLowerCase())
			&& detail.summary.powerState.toLowerCase() !== 'deleting';
	}

	function canRestart() {
		return detail.summary.powerState.toLowerCase() === 'running'
			&& detail.summary.powerState.toLowerCase() !== 'deleting';
	}

	function isDestructive(action: VmLifecycleAction): boolean {
		return action === 'stop' || action === 'restart';
	}

	function handleActionClick(action: VmLifecycleAction) {
		if (isDestructive(action)) {
			confirmingAction = action;
			pendingAction = null;
		} else {
			confirmingAction = null;
			pendingAction = action;
			submitAction(action);
		}
	}

	function submitAction(action: VmLifecycleAction) {
		confirmingAction = null;
		if (actionInput) {
			actionInput.value = action;
		}
		actionInput?.form?.requestSubmit();
	}

	function cancelConfirmation() {
		confirmingAction = null;
		pendingAction = null;
	}

	// Volume table
	const volumeColumns = [
		{ key: 'name', label: 'Name' },
		{ key: 'size', label: 'Size' },
		{ key: 'device', label: 'Device' },
		{ key: 'read_only', label: 'Read-only' },
		{ key: 'health', label: 'Health' }
	];

	const volumeRows = $derived(
		(detail.summary.attachedVolumes ?? []).map((v) => ({
			volume_id: v.volume_id,
			name: v.name,
			size: v.size,
			device: v.device_name,
			read_only: v.read_only ? 'Yes' : 'No',
			health: { label: v.health, tone: toHealthTone(v.health) }
		}))
	);

	// NIC table
	const nicColumns = [
		{ key: 'nic_id', label: 'NIC ID' },
		{ key: 'network', label: 'Network' },
		{ key: 'mac_address', label: 'MAC Address' },
		{ key: 'ip_address', label: 'IP Address' },
		{ key: 'model', label: 'Model' }
	];

	const nicRows = $derived(
		(detail.summary.attachedNics ?? []).map((n) => ({
			nic_id: n.nic_id,
			network: n.network_name || n.network_id,
			mac_address: n.mac_address,
			ip_address: n.ip_address,
			model: n.nic_model
		}))
	);

	// Events table
	const eventColumns = [
		{ key: 'severity', label: 'Severity' },
		{ key: 'type', label: 'Type' },
		{ key: 'summary', label: 'Summary' },
		{ key: 'occurred_at', label: 'Occurred At' }
	];

	const eventRows = $derived(
		vmEvents.map((e) => ({
			event_id: e.event_id,
			severity: { label: e.severity, tone: toSeverityTone(e.severity) },
			type: e.type,
			summary: e.summary,
			occurred_at: (() => {
				try {
					return new Date(e.occurred_at).toLocaleString('en-US', {
						month: 'short',
						day: 'numeric',
						hour: 'numeric',
						minute: '2-digit'
					});
				} catch {
					return e.occurred_at;
				}
			})()
		}))
	);
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if detail.state !== 'ready'}
		<StateBanner
			variant={detail.state === 'error' ? 'error' : 'empty'}
			title={detail.state === 'error' ? 'VM detail unavailable' : 'Virtual machine not found'}
			description="The VM summary, configuration, and task context could not be assembled from the current control-plane responses."
			hint="Keep the shell active and retry once the view model becomes available again."
		/>
	{:else}
		<article class="detail-page__hero">
			<div>
				<div class="detail-page__eyebrow">{detail.summary.nodeId}</div>
				<h1>{detail.summary.name}</h1>
				<p>VM ID: {detail.summary.vmId}</p>
			</div>
			<div class="detail-page__hero-side">
				<div class="detail-page__hero-badges">
					<Badge label={detail.summary.powerState} tone={toPowerStateTone(detail.summary.powerState)} />
					<Badge label={detail.summary.health} tone={toHealthTone(detail.summary.health)} />
				</div>
				<form
					method="POST"
					use:enhance={() => {
						return async ({ update }) => {
							pendingAction = null;
							confirmingAction = null;
							await update();
						};
					}}
					class="detail-page__action-row"
				>
					<input type="hidden" name="vm_id" value={detail.summary.vmId} />
					<input type="hidden" name="action" bind:this={actionInput} value="" />
					<Button
						variant="primary"
						size="sm"
						disabled={!canStart()}
						loading={pendingAction === 'start'}
						onclick={() => handleActionClick('start')}
						type="button"
					>
						<Play size={14} />
						Start
					</Button>
					<Button
						variant="secondary"
						size="sm"
						disabled={!canStop()}
						loading={pendingAction === 'stop'}
						onclick={() => handleActionClick('stop')}
						type="button"
					>
						<Square size={14} />
						Stop
					</Button>
					<Button
						variant="secondary"
						size="sm"
						disabled={!canRestart()}
						loading={pendingAction === 'restart'}
						onclick={() => handleActionClick('restart')}
						type="button"
					>
						<RotateCcw size={14} />
						Restart
					</Button>
				</form>
				{#if confirmingAction}
					{@const action = confirmingAction}
					<div class="detail-page__confirm-bar">
						<span>
							Are you sure you want to <strong>{confirmingAction}</strong> {detail.summary.name}?
						</span>
						<div class="detail-page__confirm-actions">
							<Button variant="danger" size="sm" onclick={() => submitAction(action)}>
								Confirm {action}
							</Button>
							<Button variant="ghost" size="sm" onclick={cancelConfirmation}>Cancel</Button>
						</div>
					</div>
				{/if}
			</div>
		</article>

		{#if mutationResult}
			<TaskReferenceCallout result={mutationResult} />
		{/if}

		<div class="detail-page__summary-grid">
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">CPU</div>
				<div class="detail-page__summary-value">{detail.summary.cpu}</div>
				<p>Allocated processors</p>
			</article>
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Memory</div>
				<div class="detail-page__summary-value">{detail.summary.memory}</div>
				<p>Allocated memory</p>
			</article>
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Power State</div>
				<div class="detail-page__summary-value">{detail.summary.powerState}</div>
				<p>Current power state</p>
				<Badge label={detail.summary.powerState} tone={toPowerStateTone(detail.summary.powerState)} />
			</article>
		</div>

		<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

		{#if detail.currentTab === 'summary'}
			<div class="detail-page__panel-grid">
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Alerts</div>
					<h2>Current operator signals</h2>
					<StateBanner
						variant="empty"
						title="No VM alerts"
						description="Lifecycle failures and guest-state issues tied to this VM appear here."
						hint="Every mutation still emits a task even when the alert surface is quiet."
					/>
				</article>
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Configuration</div>
					<h2>Guest-facing configuration</h2>
					<div class="detail-page__kv-list">
						{#each detail.configuration as item}
							<div class="detail-page__kv-row">
								<div>{item.label}</div>
								<div>{item.value}</div>
							</div>
						{/each}
					</div>
				</article>
			</div>
		{:else if detail.currentTab === 'console'}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Console / Access</div>
				<h2>Access surface</h2>
				<div class="detail-page__kv-list">
					<div class="detail-page__kv-row"><div>VM ID</div><div>{detail.summary.vmId}</div></div>
					<div class="detail-page__kv-row"><div>Name</div><div>{detail.summary.name}</div></div>
					<div class="detail-page__kv-row"><div>Node</div><div>{detail.summary.nodeId}</div></div>
				</div>
			</article>
		{:else if detail.currentTab === 'volumes'}
			<ResourceTable
				columns={volumeColumns}
				rows={volumeRows}
				emptyTitle="No volumes attached."
			/>
		{:else if detail.currentTab === 'networks'}
			<ResourceTable
				columns={nicColumns}
				rows={nicRows}
				emptyTitle="No NICs attached."
			/>
		{:else if detail.currentTab === 'tasks'}
			<div class="detail-page__stack">
				{#if detail.recentTasks.length > 0}
					{#each detail.recentTasks as task}
						<TaskTimelineItem task={mapRelatedTask(task, detail.summary.vmId, 'vm')} compact />
					{/each}
				{:else}
					<StateBanner
						variant="empty"
						title="No related VM tasks"
						description="Start, stop, restart, and other VM actions will appear here once the control plane persists them."
						hint="The lifecycle action bar above is wired to produce task references after mutations."
					/>
				{/if}
			</div>
		{:else if detail.currentTab === 'events'}
			{#if eventsLoading}
				<div class="detail-page__loading">Loading events…</div>
			{:else}
				<ResourceTable
					columns={eventColumns}
					rows={eventRows}
					emptyTitle="No events."
				/>
			{/if}
		{:else}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Configuration</div>
				<h2>VM configuration</h2>
				<div class="detail-page__kv-list">
					{#each detail.configuration as item}
						<div class="detail-page__kv-row">
							<div>{item.label}</div>
							<div>{item.value}</div>
						</div>
					{/each}
				</div>
			</article>
		{/if}
	{/if}
</PageShell>

<style>
	.detail-page__hero,
	.detail-page__summary-card,
	.detail-page__panel {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.detail-page__hero {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.detail-page__hero-side {
		display: grid;
		gap: 0.75rem;
	}

	.detail-page__hero-badges,
	.detail-page__action-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	.detail-page__eyebrow {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	h1 {
		margin-top: 0.25rem;
		font-size: 2rem;
		color: var(--shell-text);
	}

	h2 {
		font-size: 1.2rem;
		color: var(--shell-text);
	}

	.detail-page__hero p,
	.detail-page__summary-card p {
		margin-top: 0.35rem;
		color: var(--shell-text-secondary);
		line-height: 1.5;
	}

	.detail-page__summary-grid,
	.detail-page__panel-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1rem;
	}

	.detail-page__panel-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.detail-page__summary-value {
		margin-top: 0.8rem;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.detail-page__stack {
		display: grid;
		gap: 0.8rem;
	}

	.detail-page__kv-list {
		display: grid;
		gap: 0.65rem;
		margin-top: 0.9rem;
	}

	.detail-page__kv-row {
		display: grid;
		grid-template-columns: minmax(10rem, 0.75fr) minmax(0, 1fr);
		gap: 0.9rem;
		padding-bottom: 0.65rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
	}

	.detail-page__kv-row div:first-child {
		color: var(--shell-text-muted);
	}

	.detail-page__confirm-bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.85rem 1rem;
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface-muted);
	}

	.detail-page__confirm-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.detail-page__loading {
		padding: 2rem;
		text-align: center;
		color: var(--shell-text-muted);
		font-size: 0.92rem;
	}

	@media (max-width: 1100px) {
		.detail-page__summary-grid,
		.detail-page__panel-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
