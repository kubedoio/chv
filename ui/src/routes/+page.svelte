<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import SeverityShield from '$lib/components/shell/SeverityShield.svelte';
	import ResourceLink from '$lib/components/shell/ResourceLink.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import LoadingState from '$lib/components/shell/LoadingState.svelte';
	import { 
		Activity, 
		AlertCircle, 
		Box, 
		Blocks, 
		Server, 
		ShieldAlert, 
		Zap,
		ChevronRight,
		ArrowUpRight
	} from 'lucide-svelte';
	import { getTaskStatusMeta } from '$lib/webui/tasks';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/');
	const overview = $derived(data.overview);

	const stats = $derived([
		{ label: 'Total Clusters', value: overview.clusters_total },
		{ label: 'Active Nodes', value: overview.nodes_total, status: overview.nodes_degraded > 0 ? 'warning' : 'healthy' as any },
		{ label: 'Running VMs', value: overview.vms_running },
		{ label: 'Unresolved Alerts', value: overview.unresolved_alerts, status: overview.unresolved_alerts > 0 ? 'critical' : 'neutral' as any },
		{ label: 'Active Tasks', value: overview.active_tasks, status: overview.active_tasks > 0 ? 'warning' : 'neutral' as any }
	]);

	const recentTasks = $derived(overview.recent_tasks.map(t => {
		const meta = getTaskStatusMeta(t.status);
		return {
			task_id: t.task_id,
			operation: t.operation,
			summary: t.summary,
			status: t.status,
			started_at: new Date(t.started_unix_ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
			tone: meta.tone as any
		};
	}));

	const criticalAlerts = $derived(overview.alerts.filter(a => a.severity === 'critical'));
	const warningAlerts = $derived(overview.alerts.filter(a => a.severity === 'warning'));
</script>

<div class="dashboard-page">
	{#if overview.state === 'error'}
		<ErrorState title="Global Posture Unavailable" description="Failed to retrieve fleet-wide health signals." />
	{:else if overview.state === 'loading'}
		<LoadingState title="Assembling fleet signals..." />
	{:else if overview.state === 'empty'}
		<EmptyInfrastructureState 
			title="No resources enrolled" 
			description="This fleet is currently empty. Head to Clusters to enroll your first provider."
			hint="Operational indicators will appear here once compute capacity is added."
		/>
	{:else}
		<PageHeaderWithAction page={page}>
		</PageHeaderWithAction>

		<div class="posture-strip-wrapper">
			<CompactStatStrip {stats} />
		</div>

		<main class="dashboard-grid">
			<!-- TOP WIDE: CRITICAL SIGNALS -->
			{#if criticalAlerts.length > 0}
				<div class="span-full">
					<div class="incident-banner">
						<ShieldAlert size={18} />
						<div class="incident-content">
							<strong>{criticalAlerts.length} Critical Faults Detected</strong>
							<span>Infrastructure posture is currently degraded. immediate attention required.</span>
						</div>
						<a href="/events?severity=critical" class="btn-critical-link">
							View Incident Log <ArrowUpRight size={14} />
						</a>
					</div>
				</div>
			{/if}

			<!-- LEFT COL: HEALTH & CAPACITY -->
			<div class="dashboard-main">
				<SectionCard title="Infrastructure Posture" icon={Blocks} badgeLabel="{overview.clusters_total} Clusters">
					<div class="posture-grid">
						<div class="res-summary">
							<Server size={14} />
							<div class="res-details">
								<span class="res-label">Compute Nodes</span>
								<div class="res-stats">
									<span class="val">{overview.nodes_total}</span>
									<span class="sep">/</span>
									<span class="failed" class:is-zero={overview.nodes_degraded === 0}>{overview.nodes_degraded} degraded</span>
								</div>
							</div>
						</div>
						<div class="res-summary">
							<Box size={14} />
							<div class="res-details">
								<span class="res-label">Workloads</span>
								<div class="res-stats">
									<span class="val">{overview.vms_running}</span>
									<span class="sep">/</span>
									<span class="total">{overview.vms_total} total</span>
								</div>
							</div>
						</div>
					</div>
				</SectionCard>

				<SectionCard title="Capacity Pressure" icon={Zap} badgeLabel="{overview.capacity_hotspots} Hotspots" badgeTone={overview.capacity_hotspots > 0 ? 'warning' : 'neutral'}>
					{#if overview.capacity_hotspots === 0}
						<div class="all-clear">
							<ShieldAlert size={16} class="all-clear-icon" />
							<span>No resource bottlenecks detected across the fleet.</span>
						</div>
					{:else}
						<p class="hotspot-hint">Multiple clusters are reporting resource saturation above 85%.</p>
					{/if}
					
					<div class="capacity-preview">
						<!-- Placeholder for upcoming cluster capacity bars -->
						<div class="cap-item">
							<div class="cap-header">
								<span>Fleet CPU Allocation</span>
								<span>{Math.round(overview.cpu_usage_percent || 0)}%</span>
							</div>
							<div class="cap-bar"><div class="cap-fill" style="width: {overview.cpu_usage_percent || 0}%"></div></div>
						</div>
						<div class="cap-item">
							<div class="cap-header">
								<span>Fleet Memory Reservation</span>
								<span>{Math.round(overview.memory_usage_percent || 0)}%</span>
							</div>
							<div class="cap-bar"><div class="cap-fill" style="width: {overview.memory_usage_percent || 0}%"></div></div>
						</div>
					</div>
				</SectionCard>

				<SectionCard title="Recent Activity" icon={Activity}>
					<TaskTimeline tasks={recentTasks.slice(0, 5)} />
					<div class="section-footer">
						<a href="/tasks" class="view-more">
							Open Task Center <ChevronRight size={12} />
						</a>
					</div>
				</SectionCard>
			</div>

			<!-- RIGHT COL: ALERTS & MAINTENANCE -->
			<aside class="dashboard-side">
				<SectionCard title="Priority Alerts" icon={AlertCircle} badgeLabel={String(overview.unresolved_alerts)} badgeTone={overview.unresolved_alerts > 0 ? 'warning' : 'neutral'}>
					{#if overview.alerts.length === 0}
						<p class="empty-hint">Signals Nominal. No active alerts.</p>
					{:else}
						<ul class="micro-alert-list">
							{#each overview.alerts.slice(0, 6) as alert}
								<li>
									<div class="micro-alert">
										<SeverityShield severity={alert.severity} />
										<div class="alert-content">
											<span class="alert-txt">{alert.summary}</span>
											<span class="alert-scope">{alert.scope}</span>
										</div>
									</div>
								</li>
							{/each}
						</ul>
					{/if}
					<div class="section-footer">
						<a href="/events" class="view-more">
							Open Incident Log <ChevronRight size={12} />
						</a>
					</div>
				</SectionCard>

				<SectionCard title="Maintenance" icon={Blocks} badgeLabel={String(overview.maintenance_nodes)} badgeTone={overview.maintenance_nodes > 0 ? 'warning' : 'neutral'}>
					{#if overview.maintenance_nodes === 0}
						<p class="empty-hint">No maintenance windows active.</p>
					{:else}
						<div class="maintenance-summary">
							<strong>{overview.maintenance_nodes} Nodes in Maintenance</strong>
							<p>Compute scheduling is currently paused on these hosts.</p>
						</div>
					{/if}
					<div class="section-footer">
						<a href="/maintenance" class="view-more">
							Maintenance Hub <ChevronRight size={12} />
						</a>
					</div>
				</SectionCard>
			</aside>
		</main>
	{/if}
</div>

<style>
	.dashboard-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.header-activity-hint {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--color-success);
		letter-spacing: 0.05em;
	}

	.pulse {
		animation: pulse 2s infinite;
	}

	@keyframes pulse {
		0% { opacity: 0.4; }
		50% { opacity: 1; }
		100% { opacity: 0.4; }
	}

	.posture-strip-wrapper {
		margin-top: -0.25rem;
	}

	.dashboard-grid {
		display: grid;
		grid-template-columns: 1fr 340px;
		gap: 1rem;
		align-items: start;
	}

	.span-full {
		grid-column: 1 / -1;
	}

	.dashboard-main {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.dashboard-side {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	/* Incident Banner */
	.incident-banner {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.75rem 1rem;
		background: var(--color-danger-light);
		border: 1px solid var(--color-danger);
		border-radius: 0.35rem;
		color: var(--color-danger);
	}

	.incident-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	.incident-content strong { font-size: var(--text-sm); }
	.incident-content span { font-size: 11px; color: var(--color-danger-dark); }

	.btn-critical-link {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 11px;
		font-weight: 700;
		text-decoration: none;
		color: var(--color-danger);
		padding: 0.35rem 0.75rem;
		background: rgba(239, 68, 68, 0.1);
		border-radius: 0.25rem;
		transition: background 0.15s ease;
	}

	.btn-critical-link:hover {
		background: rgba(239, 68, 68, 0.2);
	}

	/* Posture Grid */
	.posture-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
	}

	.res-summary {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		padding: 0.75rem;
		background: var(--shell-surface-muted);
		border-radius: 0.35rem;
	}

	.res-details {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.res-label {
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--shell-text-muted);
		letter-spacing: 0.05em;
	}

	.res-stats {
		display: flex;
		align-items: baseline;
		gap: 0.35rem;
		font-size: var(--text-lg);
		font-weight: 700;
	}

	.res-stats .sep {
		font-size: var(--text-sm);
		color: var(--shell-line-strong);
	}

	.res-stats .failed {
		font-size: var(--text-xs);
		color: var(--color-danger);
	}

	.res-stats .failed.is-zero {
		color: var(--shell-text-muted);
		font-weight: 400;
	}

	.res-stats .total {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		font-weight: 400;
	}

	/* Capacity */
	.all-clear {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: var(--color-success);
		font-size: var(--text-xs);
		font-weight: 500;
		padding: 0.5rem 0;
	}

	.all-clear-icon { color: var(--color-success); }

	.hotspot-hint {
		font-size: var(--text-xs);
		color: var(--color-warning-dark);
		margin-bottom: 0.75rem;
	}

	.capacity-preview {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.cap-item {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.cap-header {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		font-weight: 500;
		color: var(--shell-text-muted);
	}

	.cap-bar {
		height: 4px;
		background: var(--shell-line);
		border-radius: 999px;
		overflow: hidden;
	}

	.cap-fill {
		height: 100%;
		background: var(--shell-accent);
		border-radius: 999px;
	}

	/* Alerts Sidebar */
	.micro-alert-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.micro-alert {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border-radius: 0.25rem;
	}

	.alert-content {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.alert-txt {
		font-size: var(--text-xs);
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.alert-scope {
		font-size: 10px;
		color: var(--shell-text-muted);
	}

	/* Maintenance */
	.maintenance-summary {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0.5rem;
		background: var(--color-warning-light);
		border: 1px solid var(--color-warning);
		border-radius: 0.25rem;
	}

	.maintenance-summary strong { font-size: var(--text-xs); color: var(--color-warning-dark); }
	.maintenance-summary p { font-size: 10px; color: var(--color-warning-dark); margin: 0; }

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
	}

	.section-footer {
		margin-top: 0.75rem;
		padding-top: 0.5rem;
		border-top: 1px solid var(--shell-line);
	}

	.view-more {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		text-decoration: none;
		color: var(--shell-accent);
		letter-spacing: 0.05em;
	}

	@media (max-width: 1100px) {
		.dashboard-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
