<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { PageShell, StateBanner, Badge, PostureStrip, PostureCard } from '$lib/components/system';
	import { ArrowRight, Activity, AlertTriangle, Server, Box, Blocks } from 'lucide-svelte';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/');
	const overview = $derived(data.overview);

	function severityTone(severity: string): ShellTone {
		switch (severity) {
			case 'critical':
				return 'failed';
			case 'warning':
				return 'warning';
			default:
				return 'unknown';
		}
	}

	function statusTone(status: string): ShellTone {
		switch (status) {
			case 'running':
				return 'warning';
			case 'failed':
				return 'failed';
			case 'succeeded':
				return 'healthy';
			default:
				return 'unknown';
		}
	}

	function formatTimeAgo(ms: number): string {
		const seconds = Math.max(Math.round((Date.now() - ms) / 1000), 0);
		if (seconds < 60) return `${seconds}s ago`;
		const minutes = Math.round(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.round(minutes / 60);
		if (hours < 24) return `${hours}h ago`;
		return `${Math.round(hours / 24)}d ago`;
	}

	const postureChips = $derived([
		{ label: 'Clusters', value: overview.clusters_total },
		{ label: 'Nodes', value: overview.nodes_total },
		{ label: 'VMs running', value: overview.vms_running },
		{
			label: 'Degraded',
			value: overview.clusters_degraded + overview.nodes_degraded,
			variant: overview.clusters_degraded + overview.nodes_degraded > 0 ? ('degraded' as const) : undefined
		},
		{
			label: 'Tasks',
			value: overview.active_tasks,
			variant: overview.active_tasks > 0 ? ('warning' as const) : undefined
		},
		{
			label: 'Alerts',
			value: overview.unresolved_alerts,
			variant: overview.unresolved_alerts > 0 ? ('failed' as const) : undefined
		}
	]);

	const attentionItems = $derived(
		[
			...(overview.clusters_degraded > 0
				? [
						{
							type: 'cluster' as const,
							title: `${overview.clusters_degraded} cluster${overview.clusters_degraded === 1 ? '' : 's'} degraded`,
							detail: 'Review cluster posture for pressure or version skew.',
							href: '/clusters'
						}
					]
				: []),
			...(overview.nodes_degraded > 0
				? [
						{
							type: 'node' as const,
							title: `${overview.nodes_degraded} node${overview.nodes_degraded === 1 ? '' : 's'} degraded`,
							detail: 'Check node readiness and capacity pressure.',
							href: '/nodes'
						}
					]
				: []),
			...(overview.unresolved_alerts > 0
				? [
						{
							type: 'alert' as const,
							title: `${overview.unresolved_alerts} unresolved alert${overview.unresolved_alerts === 1 ? '' : 's'}`,
							detail: 'Alerts require operator inspection or acknowledgement.',
							href: '/events'
						}
					]
				: [])
		].slice(0, 4)
	);
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if overview.state === 'error'}
		<StateBanner
			variant="error"
			title="Fleet overview unavailable"
			description="The control plane could not assemble the current fleet summary."
			hint="Navigation remains available while the overview recovers."
		/>
	{:else if overview.state === 'loading'}
		<StateBanner
			variant="loading"
			title="Loading fleet overview"
			description="Assembling cluster, node, and workload posture from the control plane."
			hint="Summary cards remain visible while data refreshes."
		/>
	{:else if overview.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No fleet data yet"
			description="Fleet posture, alerts, and task summaries will appear once clusters and nodes are enrolled."
			hint="Begin by enrolling a datacenter and cluster."
		/>
	{:else}
		<div class="overview-page">
			<div class="overview-header">
				<PostureStrip chips={postureChips} />
			</div>

			<section class="attention-panel" aria-labelledby="attention-title">
				<div class="attention-panel__header">
					<h2 id="attention-title" class="attention-panel__title">Needs attention now</h2>
					{#if attentionItems.length === 0}
						<Badge label="All healthy" tone="healthy" />
					{:else}
						<Badge label="{attentionItems.length} open" tone="warning" />
					{/if}
				</div>

				{#if attentionItems.length === 0}
					<div class="attention-panel__empty">
						<div class="attention-panel__empty-icon" aria-hidden="true">
							<Server size={24} />
						</div>
						<p class="attention-panel__empty-text">Fleet is operating normally. No immediate action required.</p>
					</div>
				{:else}
					<ul class="attention-list" role="list">
						{#each attentionItems as item}
							<li class="attention-item">
								<div class="attention-item__main">
									<div class="attention-item__title">{item.title}</div>
									<p class="attention-item__detail">{item.detail}</p>
								</div>
								<a href={item.href} class="attention-item__cta">
									Inspect
									<ArrowRight size={14} aria-hidden="true" />
								</a>
							</li>
						{/each}
					</ul>
				{/if}
			</section>

			<div class="posture-grid">
				<PostureCard
					label="Capacity pressure"
					value="{overview.capacity_hotspots} hotspots"
					note={overview.capacity_hotspots > 0 ? 'Clusters or nodes above 70% utilization.' : 'No capacity pressure detected.'}
				/>
				<PostureCard
					label="Maintenance in effect"
					value="{overview.maintenance_nodes} node{overview.maintenance_nodes === 1 ? '' : 's'}"
					note={overview.maintenance_nodes > 0 ? 'Scheduling is paused on affected nodes.' : 'No active maintenance windows.'}
				/>
				<PostureCard
					label="Recent failures"
					value="{overview.alerts.filter((a) => a.severity === 'critical').length} critical"
					note="Critical alerts from the last 24 hours."
				/>
				<PostureCard
					label="Active work"
					value="{overview.active_tasks} task{overview.active_tasks === 1 ? '' : 's'}"
					note="Currently running or recently queued tasks."
				/>
			</div>

			<div class="overview-detail-grid">
				<article class="overview-panel">
					<div class="overview-panel__header">
						<div>
							<div class="overview-panel__eyebrow">Recent alerts</div>
							<h2>Unresolved operator signals</h2>
						</div>
						<a class="overview-panel__link" href="/events">Open alerts</a>
					</div>
					{#if overview.alerts.length > 0}
						<ul class="alert-list" role="list">
							{#each overview.alerts.slice(0, 5) as alert}
								<li class="alert-item">
									<Badge label={alert.severity} tone={severityTone(alert.severity)} />
									<div class="alert-item__content">
										<p>{alert.summary}</p>
										<span class="alert-item__scope">{alert.scope}</span>
									</div>
								</li>
							{/each}
						</ul>
					{:else}
						<StateBanner
							variant="empty"
							title="No active alerts"
							description="Degradations and failures will appear here when they occur."
						/>
					{/if}
				</article>

				<article class="overview-panel">
					<div class="overview-panel__header">
						<div>
							<div class="overview-panel__eyebrow">Recent tasks</div>
							<h2>Accepted and running work</h2>
						</div>
						<a class="overview-panel__link" href="/tasks">Open task center</a>
					</div>
					{#if overview.recent_tasks.length > 0}
						<ul class="task-list" role="list">
							{#each overview.recent_tasks.slice(0, 5) as task}
								<li class="task-item">
									<div class="task-item__main">
										<div class="task-item__title">{task.summary}</div>
										<div class="task-item__meta">
											<Badge label={task.status} tone={statusTone(task.status)} />
											<span>{formatTimeAgo(task.started_unix_ms)}</span>
										</div>
									</div>
									<a href="/tasks?query={task.task_id}" class="task-item__link">
										<ArrowRight size={14} aria-hidden="true" />
									</a>
								</li>
							{/each}
						</ul>
					{:else}
						<StateBanner
							variant="empty"
							title="No recent tasks"
							description="Active operations and completed work will appear here."
						/>
					{/if}
				</article>
			</div>

			<div class="quick-links" role="region" aria-label="Quick navigation">
				<a href="/nodes" class="quick-link">
					<Server size={18} aria-hidden="true" />
					<span>Nodes</span>
				</a>
				<a href="/vms" class="quick-link">
					<Box size={18} aria-hidden="true" />
					<span>Virtual Machines</span>
				</a>
				<a href="/tasks" class="quick-link">
					<Activity size={18} aria-hidden="true" />
					<span>Tasks</span>
				</a>
				<a href="/maintenance" class="quick-link">
					<Blocks size={18} aria-hidden="true" />
					<span>Maintenance</span>
				</a>
			</div>
		</div>
	{/if}
</PageShell>

<style>
	.overview-page {
		display: grid;
		gap: 1.3rem;
	}

	.overview-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.attention-panel {
		border: 1px solid var(--shell-line-strong);
		border-radius: 1.15rem;
		background: var(--shell-surface);
		padding: 1.25rem;
	}

	.attention-panel__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		margin-bottom: 1rem;
	}

	.attention-panel__title {
		font-size: 1.1rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.attention-panel__empty {
		display: flex;
		align-items: center;
		gap: 0.85rem;
		padding: 1.5rem 0.5rem;
	}

	.attention-panel__empty-icon {
		display: grid;
		place-items: center;
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 999px;
		background: var(--status-healthy-bg);
		color: var(--status-healthy-text);
	}

	.attention-panel__empty-text {
		font-size: 0.95rem;
		color: var(--shell-text-secondary);
	}

	.attention-list {
		display: grid;
		gap: 0.6rem;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.attention-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.9rem;
		background: var(--shell-surface-muted);
		padding: 0.9rem 1rem;
	}

	.attention-item__title {
		font-weight: 700;
		color: var(--shell-text);
	}

	.attention-item__detail {
		margin: 0.2rem 0 0 0;
		font-size: 0.85rem;
		color: var(--shell-text-secondary);
	}

	.attention-item__cta {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--shell-accent);
		text-decoration: none;
		white-space: nowrap;
	}

	.attention-item__cta:hover {
		text-decoration: underline;
	}

	.posture-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1rem;
	}

	.overview-detail-grid {
		display: grid;
		grid-template-columns: minmax(0, 0.92fr) minmax(0, 1.08fr);
		gap: 1rem;
	}

	.overview-panel {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
		padding: 1.05rem;
		display: grid;
		gap: 0.95rem;
	}

	.overview-panel__header {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.8rem;
	}

	.overview-panel__eyebrow {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.overview-panel h2 {
		font-size: 1.25rem;
		color: var(--shell-text);
		margin: 0;
	}

	.overview-panel__link {
		color: var(--shell-accent);
		font-size: 0.9rem;
		font-weight: 600;
		text-decoration: none;
	}

	.overview-panel__link:hover {
		text-decoration: underline;
	}

	.alert-list,
	.task-list {
		display: grid;
		gap: 0.7rem;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.alert-item,
	.task-item {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		padding: 0.8rem;
		border-radius: 0.85rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
	}

	.alert-item__content p,
	.task-item__title {
		font-size: 0.92rem;
		color: var(--shell-text);
		margin: 0;
	}

	.alert-item__scope,
	.task-item__meta span {
		font-size: 0.8rem;
		color: var(--shell-text-muted);
	}

	.task-item {
		align-items: center;
		justify-content: space-between;
	}

	.task-item__main {
		display: grid;
		gap: 0.25rem;
	}

	.task-item__meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.task-item__link {
		color: var(--shell-accent);
	}

	.quick-links {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
	}

	.quick-link {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.75rem;
		background: var(--shell-surface);
		padding: 0.65rem 1rem;
		color: var(--shell-text);
		font-size: 0.9rem;
		font-weight: 500;
		text-decoration: none;
	}

	.quick-link:hover {
		border-color: var(--shell-line-strong);
		background: var(--shell-surface-muted);
	}

	@media (max-width: 1200px) {
		.posture-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.overview-detail-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.posture-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
