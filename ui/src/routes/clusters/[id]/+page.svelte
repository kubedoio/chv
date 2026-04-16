<script lang="ts">
	import { Badge, PageShell, StateBanner } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/clusters');
	const detail = $derived(data.detail);

	function mapTone(state: string): ShellTone {
		switch (state.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'warning':
			case 'maintenance':
				return 'warning';
			case 'degraded':
				return 'degraded';
			case 'failed':
				return 'failed';
			default:
				return 'unknown';
		}
	}

	function capacityTone(value: number): ShellTone {
		if (value >= 85) return 'failed';
		if (value >= 70) return 'degraded';
		if (value >= 55) return 'warning';
		return 'healthy';
	}
</script>

<PageShell title="Cluster Detail" eyebrow={page.eyebrow} description={page.description}>
	{#if detail.state === 'error'}
		<StateBanner
			variant="error"
			title="Cluster detail unavailable"
			description="The cluster detail view could not be loaded from the BFF."
			hint="Return to the cluster list and retry once the control plane is reachable."
		/>
	{:else if detail.state === 'not_found'}
		<StateBanner
			variant="empty"
			title="Cluster not found"
			description={`Cluster ${detail.summary.clusterId} was not found in the current fleet.`}
			hint="Return to the cluster list to pick an available cluster."
		/>
	{:else}
		<header class="detail-header">
			<div>
				<div class="detail-header__eyebrow">{detail.summary.datacenter}</div>
				<h1>{detail.summary.name}</h1>
				<p>Cluster ID: {detail.summary.clusterId}</p>
			</div>
			<div class="detail-header__badges">
				<Badge label={detail.summary.state} tone={mapTone(detail.summary.state)} />
				{#if detail.summary.maintenance}
					<Badge label="Maintenance" tone="warning" />
				{/if}
			</div>
		</header>

		<div class="detail-grid">
			<article class="detail-card">
				<div class="detail-card__label">Nodes</div>
				<div class="detail-card__value">{detail.summary.nodeCount}</div>
				<p>Total registered nodes in this cluster.</p>
			</article>
			<article class="detail-card">
				<div class="detail-card__label">Version</div>
				<div class="detail-card__value">
					{detail.summary.version}
					{#if detail.summary.versionSkew}
						<span class="detail-card__inline"> (skew)</span>
					{/if}
				</div>
				<p>Current version posture for this cluster.</p>
			</article>
			<article class="detail-card">
				<div class="detail-card__label">Tasks</div>
				<div class="detail-card__value">{detail.summary.activeTasks}</div>
				<p>Accepted or running tasks targeting this cluster.</p>
			</article>
			<article class="detail-card">
				<div class="detail-card__label">Alerts</div>
				<div class="detail-card__value">{detail.summary.alerts}</div>
				<p>Unresolved events linked to this cluster.</p>
			</article>
		</div>

		<section class="capacity-section" aria-label="Capacity">
			<div class="capacity-row">
				<div>CPU</div>
				<div class="capacity-row__value">
					<Badge label={`${detail.summary.cpuPercent}%`} tone={capacityTone(detail.summary.cpuPercent)} />
				</div>
			</div>
			<div class="capacity-row">
				<div>Memory</div>
				<div class="capacity-row__value">
					<Badge
						label={`${detail.summary.memoryPercent}%`}
						tone={capacityTone(detail.summary.memoryPercent)}
					/>
				</div>
			</div>
			<div class="capacity-row">
				<div>Storage</div>
				<div class="capacity-row__value">
					<Badge
						label={`${detail.summary.storagePercent}%`}
						tone={capacityTone(detail.summary.storagePercent)}
					/>
				</div>
			</div>
		</section>

		{#if detail.summary.topIssue}
			<StateBanner
				variant="degraded"
				title="Top issue"
				description={detail.summary.topIssue}
				hint="Use Events and Tasks pages for full timeline context."
			/>
		{/if}

		<div class="detail-links">
			<a href="/clusters">Back to clusters</a>
			<a href="/nodes">Open nodes</a>
			<a href="/tasks">Open tasks</a>
			<a href="/events">Open events</a>
		</div>
	{/if}
</PageShell>

<style>
	.detail-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 1rem;
	}

	.detail-header h1 {
		margin: 0;
		font-size: 1.5rem;
		color: var(--shell-text);
	}

	.detail-header p {
		margin: 0.35rem 0 0;
		color: var(--shell-text-muted);
		font-size: 0.9rem;
	}

	.detail-header__eyebrow {
		font-size: 0.72rem;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
		font-weight: 700;
	}

	.detail-header__badges {
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.detail-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1rem;
	}

	.detail-card {
		border: 1px solid var(--shell-line);
		background: var(--shell-surface);
		border-radius: 1rem;
		padding: 1rem;
	}

	.detail-card__label {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		color: var(--shell-text-muted);
		font-weight: 700;
	}

	.detail-card__value {
		margin-top: 0.35rem;
		font-size: 1.45rem;
		color: var(--shell-text);
		font-weight: 700;
	}

	.detail-card__inline {
		font-size: 0.95rem;
		font-weight: 600;
		color: var(--shell-text-muted);
	}

	.detail-card p {
		margin: 0.45rem 0 0;
		color: var(--shell-text-secondary);
		font-size: 0.85rem;
	}

	.capacity-section {
		border: 1px solid var(--shell-line);
		background: var(--shell-surface);
		border-radius: 1rem;
		padding: 0.75rem 1rem;
	}

	.capacity-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.6rem 0;
		border-bottom: 1px solid var(--shell-line);
		color: var(--shell-text);
	}

	.capacity-row:last-child {
		border-bottom: none;
	}

	.capacity-row__value {
		display: flex;
	}

	.detail-links {
		display: flex;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.detail-links a {
		color: var(--shell-accent);
		text-decoration: none;
		font-weight: 600;
	}

	@media (max-width: 960px) {
		.detail-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 640px) {
		.detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
