<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onMount } from 'svelte';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import SectionHeader from '$lib/components/shell/SectionHeader.svelte';
	import StatePanel from '$lib/components/shell/StatePanel.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import { getStoredToken } from '$lib/api/client';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/nodes');
	const detail = $derived(data.detail);

	onMount(() => {
		if (data.meta.clientRefreshRecommended && getStoredToken()) {
			invalidate(`webui:node:${detail.summary.nodeId}`);
		}
	});
</script>

<div class="detail-page">
	<SectionHeader {page} />

	{#if data.meta.deferred}
		<StatePanel
			variant="loading"
			title="Loading node detail"
			description="This route waits for the client-authenticated pass before loading protected node detail data."
			hint="The node page keeps its structure visible while the browser rehydrates."
		/>
	{:else if detail.state !== 'ready'}
		<StatePanel
			variant={detail.state === 'error' ? 'error' : 'empty'}
			title={detail.state === 'error' ? 'Node detail unavailable' : 'Node not found'}
			description="The node summary, related resources, and task context could not be assembled from the current API responses."
			hint="Keep the shell active and retry once the control-plane view model becomes available."
		/>
	{:else}
		<article class="detail-page__hero">
			<div>
				<div class="detail-page__eyebrow">{detail.summary.cluster}</div>
				<h1>{detail.summary.name}</h1>
				<p>{detail.summary.hostname} · {detail.summary.ipAddress}</p>
			</div>
			<div class="detail-page__hero-badges">
				<StatusBadge label={detail.summary.stateLabel} tone={detail.summary.stateTone} />
				<StatusBadge label={detail.summary.healthLabel} tone={detail.summary.healthTone} />
				<StatusBadge label={detail.summary.maintenanceLabel} tone={detail.summary.maintenanceTone} />
			</div>
		</article>

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
							title="No active node alerts"
							description="Warnings and failures tied directly to this node appear here."
							hint="Hosted VM issues remain visible in their own resource scopes."
						/>
					{/if}
				</article>
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Configuration</div>
					<h2>Host-facing identifiers</h2>
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
		{:else if detail.currentTab === 'vms'}
			<div class="detail-page__table-shell">
				<table class="detail-page__table">
					<thead>
						<tr>
							<th>VM</th>
							<th>Power</th>
							<th>Health</th>
							<th>CPU</th>
							<th>Memory</th>
							<th>Last task</th>
						</tr>
					</thead>
					<tbody>
						{#each detail.hostedVms as vm}
							<tr>
								<td><a href={vm.href} class="detail-page__primary-link">{vm.name}</a></td>
								<td><StatusBadge label={vm.powerStateLabel} tone={vm.powerStateTone} /></td>
								<td><StatusBadge label={vm.healthLabel} tone={vm.healthTone} /></td>
								<td>{vm.cpuLabel}</td>
								<td>{vm.memoryLabel}</td>
								<td><StatusBadge label={vm.lastTaskLabel} tone={vm.lastTaskTone} /></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else if detail.currentTab === 'volumes'}
			<div class="detail-page__table-shell">
				<table class="detail-page__table">
					<thead><tr><th>Volume</th><th>Status</th><th>Capacity</th><th>Path</th></tr></thead>
					<tbody>
						{#each detail.storagePools as pool}
							<tr>
								<td>{pool.name}</td>
								<td><StatusBadge label={pool.statusLabel} tone={pool.statusTone} /></td>
								<td>{pool.capacityLabel}</td>
								<td>{pool.path}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else if detail.currentTab === 'networks'}
			<div class="detail-page__table-shell">
				<table class="detail-page__table">
					<thead><tr><th>Network</th><th>Status</th><th>Scope</th><th>CIDR</th></tr></thead>
					<tbody>
						{#each detail.networks as network}
							<tr>
								<td>{network.name}</td>
								<td><StatusBadge label={network.statusLabel} tone={network.statusTone} /></td>
								<td>{network.scopeLabel}</td>
								<td>{network.cidr}</td>
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
						title="No related node tasks"
						description="Direct maintenance, scheduling, and node-scoped actions will appear here."
						hint="VM-specific work continues to live on each VM detail page and in the global task center."
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
						title="No node events"
						description="Failed maintenance runs, degraded discovery, and related warnings show up here."
						hint="This tab stays scoped to the node rather than every hosted VM."
					/>
				{/if}
			</div>
		{:else}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Configuration</div>
				<h2>Node configuration</h2>
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

	.detail-page__hero-badges {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
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

	.detail-page__primary-link {
		color: var(--shell-text);
		font-weight: 700;
		text-decoration: none;
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
