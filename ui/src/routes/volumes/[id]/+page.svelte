<script lang="ts">
	import { PageShell, StateBanner, Badge } from '$lib/components/system';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { mapRelatedTask } from '$lib/webui/task-helpers';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/volumes');
	const detail = $derived(data.detail);

	function toStatusTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (s === 'attached' || s === 'ready') return 'healthy';
		if (s === 'attaching' || s === 'detaching') return 'warning';
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
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if detail.state !== 'ready'}
		<StateBanner
			variant={detail.state === 'error' ? 'error' : 'empty'}
			title={detail.state === 'error' ? 'Volume detail unavailable' : 'Volume not found'}
			description="The volume summary, configuration, and task context could not be assembled from the current control-plane responses."
			hint="Keep the shell active and retry once the view model becomes available again."
		/>
	{:else}
		<article class="detail-page__hero">
			<div>
				<div class="detail-page__eyebrow">{detail.summary.nodeId}</div>
				<h1>{detail.summary.name}</h1>
				<p>Volume ID: {detail.summary.volumeId}</p>
			</div>
			<div class="detail-page__hero-badges">
				<Badge label={detail.summary.status} tone={toStatusTone(detail.summary.status)} />
				<Badge label={detail.summary.health} tone={toHealthTone(detail.summary.health)} />
			</div>
		</article>

		<div class="detail-page__summary-grid">
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Size</div>
				<div class="detail-page__summary-value">{detail.summary.size}</div>
				<p>Allocated capacity</p>
			</article>
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Status</div>
				<div class="detail-page__summary-value">{detail.summary.status}</div>
				<p>Current attachment status</p>
				<Badge label={detail.summary.status} tone={toStatusTone(detail.summary.status)} />
			</article>
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Attached VM</div>
				<div class="detail-page__summary-value">{detail.summary.attachedVmId || '-'}</div>
				<p>Workload attachment</p>
			</article>
		</div>

		<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

		{#if detail.currentTab === 'summary'}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Summary</div>
				<h2>Volume overview</h2>
				<div class="detail-page__kv-list">
					<div class="detail-page__kv-row"><div>Volume ID</div><div>{detail.summary.volumeId}</div></div>
					<div class="detail-page__kv-row"><div>Name</div><div>{detail.summary.name}</div></div>
					<div class="detail-page__kv-row"><div>Node</div><div>{detail.summary.nodeId}</div></div>
					<div class="detail-page__kv-row"><div>Size</div><div>{detail.summary.size}</div></div>
					<div class="detail-page__kv-row"><div>Status</div><div>{detail.summary.status}</div></div>
					<div class="detail-page__kv-row"><div>Health</div><div>{detail.summary.health}</div></div>
					<div class="detail-page__kv-row"><div>Attached VM</div><div>{detail.summary.attachedVmId || '-'}</div></div>
				</div>
			</article>
		{:else if detail.currentTab === 'tasks'}
			<div class="detail-page__stack">
				{#if detail.recentTasks.length > 0}
					{#each detail.recentTasks as task}
						<TaskTimelineItem task={mapRelatedTask(task, detail.summary.volumeId, 'volume')} compact />
					{/each}
				{:else}
					<StateBanner
						variant="empty"
						title="No related volume tasks"
						description="Volume mutations and lifecycle actions will appear here once the control plane persists them."
						hint="The volume list remains wired to produce task references after mutations."
					/>
				{/if}
			</div>
		{:else}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Configuration</div>
				<h2>Volume configuration</h2>
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

	.detail-page__hero-badges {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	.detail-page__summary-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1rem;
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

	@media (max-width: 1100px) {
		.detail-page__summary-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
