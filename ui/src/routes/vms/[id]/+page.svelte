<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onMount } from 'svelte';
	import { Play, RotateCcw, Square } from 'lucide-svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import TaskReferenceCallout from '$lib/components/webui/TaskReferenceCallout.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import Button from '$lib/components/primitives/Button.svelte';
	import SectionHeader from '$lib/components/shell/SectionHeader.svelte';
	import StatePanel from '$lib/components/shell/StatePanel.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { toast } from '$lib/stores/toast';
	import { runVmLifecycleAction, type VmLifecycleAction, type VmLifecycleActionResult } from '$lib/webui/vm-actions';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/vms');
	const detail = $derived(data.detail);
	let actionLoading = $state<VmLifecycleAction | null>(null);
	let mutationResult = $state<VmLifecycleActionResult | null>(null);

	onMount(() => {
		if (data.meta.clientRefreshRecommended && getStoredToken() && data.requestedVmId) {
			invalidate(`webui:vm:${data.requestedVmId}`);
		}
	});

	async function handleLifecycleAction(action: VmLifecycleAction) {
		const token = getStoredToken();
		if (!token || !detail.summary.vmId) {
			toast.error('Session token unavailable for lifecycle action');
			return;
		}

		const client = createAPIClient({ token });
		actionLoading = action;

		try {
			const perform =
				action === 'start'
					? () => client.startVM(detail.summary.vmId)
					: action === 'stop'
						? () => client.stopVM(detail.summary.vmId)
						: () => client.restartVM(detail.summary.vmId);

				mutationResult = await runVmLifecycleAction({
					vmId: data.requestedVmId,
					vmName: detail.summary.name,
					action,
					perform,
				listOperations: () => client.listOperations(),
				now: new Date()
			});

				toast.success(mutationResult.summary);
				await invalidate(`webui:vm:${data.requestedVmId}`);
				await invalidate('webui:vms');
				await invalidate('webui:tasks');
				await invalidate('webui:overview');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : `Failed to ${action} virtual machine`);
		} finally {
			actionLoading = null;
		}
	}

	function canStart() {
		return !['running', 'starting', 'deleting'].includes(detail.summary.powerStateLabel.toLowerCase());
	}

	function canStop() {
		return ['running', 'starting', 'rebooting'].includes(detail.summary.powerStateLabel.toLowerCase());
	}

	function canRestart() {
		return detail.summary.powerStateLabel.toLowerCase() === 'running';
	}
</script>

<div class="detail-page">
	<SectionHeader {page} />

	{#if data.meta.deferred}
		<StatePanel
			variant="loading"
			title="Loading virtual machine detail"
			description="This route waits for the client-authenticated pass before loading protected VM data."
			hint="Summary, tabs, and lifecycle controls stay shell-first while the browser rehydrates."
		/>
	{:else if detail.state !== 'ready'}
		<StatePanel
			variant={detail.state === 'error' ? 'error' : 'empty'}
			title={detail.state === 'error' ? 'VM detail unavailable' : 'Virtual machine not found'}
			description="The VM summary, configuration, and task context could not be assembled from the current control-plane responses."
			hint="Keep the shell active and retry once the view model becomes available again."
		/>
	{:else}
		<article class="detail-page__hero">
			<div>
				<div class="detail-page__eyebrow">{detail.summary.nodeName}</div>
				<h1>{detail.summary.name}</h1>
				<p>{detail.summary.ipAddress} · {detail.summary.consoleLabel}</p>
			</div>
			<div class="detail-page__hero-side">
				<div class="detail-page__hero-badges">
					<StatusBadge label={detail.summary.powerStateLabel} tone={detail.summary.powerStateTone} />
					<StatusBadge label={detail.summary.healthLabel} tone={detail.summary.healthTone} />
				</div>
				<div class="detail-page__action-row">
					<Button variant="primary" size="sm" disabled={!canStart()} loading={actionLoading === 'start'} onclick={() => handleLifecycleAction('start')}>
						<Play size={14} />
						Start
					</Button>
					<Button variant="secondary" size="sm" disabled={!canStop()} loading={actionLoading === 'stop'} onclick={() => handleLifecycleAction('stop')}>
						<Square size={14} />
						Stop
					</Button>
					<Button variant="secondary" size="sm" disabled={!canRestart()} loading={actionLoading === 'restart'} onclick={() => handleLifecycleAction('restart')}>
						<RotateCcw size={14} />
						Restart
					</Button>
				</div>
			</div>
		</article>

		{#if mutationResult}
			<TaskReferenceCallout result={mutationResult} />
		{/if}

		<div class="detail-page__summary-grid">
			{#each detail.summaryCards as card}
				<article class="detail-page__summary-card">
					<div class="detail-page__eyebrow">{card.label}</div>
					<div class="detail-page__summary-value">{card.value}</div>
					<p>{card.note}</p>
					{#if card.tone}
						<StatusBadge label={card.tone} tone={card.tone} />
					{/if}
				</article>
			{/each}
		</div>

		<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

		{#if detail.currentTab === 'summary'}
			<div class="detail-page__panel-grid">
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Alerts</div>
					<h2>Current operator signals</h2>
					{#if detail.alerts.length > 0}
						<div class="detail-page__stack">
							{#each detail.alerts as alert}
								<div class="detail-page__notice-row">
									<StatusBadge label="attention" tone="failed" />
									<p>{alert}</p>
								</div>
							{/each}
						</div>
					{:else}
						<StatePanel
							variant="empty"
							title="No VM alerts"
							description="Lifecycle failures and guest-state issues tied to this VM appear here."
							hint="Every mutation still emits a task even when the alert surface is quiet."
						/>
					{/if}
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
					<div class="detail-page__kv-row"><div>Console</div><div>{detail.summary.consoleLabel}</div></div>
					<div class="detail-page__kv-row"><div>Guest IP</div><div>{detail.summary.ipAddress}</div></div>
					<div class="detail-page__kv-row"><div>Node</div><div>{detail.summary.nodeName}</div></div>
				</div>
			</article>
		{:else if detail.currentTab === 'volumes'}
			<div class="detail-page__table-shell">
				<table class="detail-page__table">
					<thead><tr><th>Volume</th><th>Status</th><th>Size</th><th>Path</th></tr></thead>
					<tbody>
						{#each detail.storageItems as item}
							<tr>
								<td>{item.name}</td>
								<td><StatusBadge label={item.statusLabel} tone={item.statusTone} /></td>
								<td>{item.sizeLabel}</td>
								<td>{item.path}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else if detail.currentTab === 'networks'}
			<div class="detail-page__table-shell">
				<table class="detail-page__table">
					<thead><tr><th>Network</th><th>Status</th><th>Scope</th><th>CIDR</th><th>Gateway</th></tr></thead>
					<tbody>
						{#each detail.networkItems as item}
							<tr>
								<td>{item.name}</td>
								<td><StatusBadge label={item.statusLabel} tone={item.statusTone} /></td>
								<td>{item.scopeLabel}</td>
								<td>{item.cidr}</td>
								<td>{item.gateway}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else if detail.currentTab === 'tasks'}
			<div class="detail-page__stack">
				{#if detail.recentTasks.length > 0}
					{#each detail.recentTasks as task}
						<TaskTimelineItem {task} compact />
					{/each}
				{:else}
					<StatePanel
						variant="empty"
						title="No related VM tasks"
						description="Start, stop, restart, and other VM actions will appear here once the control plane persists them."
						hint="The lifecycle action bar above is wired to produce task references after mutations."
					/>
				{/if}
			</div>
		{:else if detail.currentTab === 'events'}
			<div class="detail-page__stack">
				{#if detail.events.length > 0}
					{#each detail.events as event}
						<article class="detail-page__event-card">
							<div class="detail-page__event-topline">
								<StatusBadge label={event.label} tone={event.tone} />
								<span>{event.timestampLabel}</span>
							</div>
							<p>{event.message}</p>
						</article>
					{/each}
				{:else}
					<StatePanel
						variant="empty"
						title="No VM events"
						description="Guest failures, lifecycle warnings, and event rollups scoped to this VM land here."
						hint="Task history remains the primary audit surface after operator mutations."
					/>
				{/if}
			</div>
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
</div>

<style>
	.detail-page {
		display: grid;
		gap: 1.2rem;
	}

	.detail-page__hero,
	.detail-page__summary-card,
	.detail-page__panel,
	.detail-page__table-shell,
	.detail-page__event-card {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.detail-page__hero,
	.detail-page__summary-card,
	.detail-page__panel,
	.detail-page__event-card {
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
	.detail-page__summary-card p,
	.detail-page__event-card p {
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

	.detail-page__notice-row {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.7rem;
		padding: 0.85rem;
		border-radius: 0.9rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
	}

	.detail-page__notice-row p {
		margin: 0;
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

	.detail-page__table-shell {
		overflow-x: auto;
	}

	.detail-page__table {
		width: 100%;
		min-width: 780px;
		border-collapse: collapse;
	}

	.detail-page__table th,
	.detail-page__table td {
		padding: 0.95rem 1rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
		color: var(--shell-text-secondary);
		text-align: left;
	}

	.detail-page__table th {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
		background: rgba(247, 242, 234, 0.75);
	}

	.detail-page__event-topline {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.7rem;
		font-size: 0.82rem;
		color: var(--shell-text-muted);
	}

	@media (max-width: 1100px) {
		.detail-page__summary-grid,
		.detail-page__panel-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
