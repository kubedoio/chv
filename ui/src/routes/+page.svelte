<script lang="ts">
	import type { PageData } from './$types';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import SeverityShield from '$lib/components/shell/SeverityShield.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import LoadingState from '$lib/components/shell/LoadingState.svelte';
	import TopologyCanvas from '$lib/components/topology/TopologyCanvas.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import { 
		Activity, 
		AlertCircle, 
		Server,
		ShieldCheck,
		Zap
	} from 'lucide-svelte';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import { inventory } from '$lib/stores/inventory.svelte';

	let { data }: { data: PageData } = $props();

	const overview = $derived(data.overview);

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

	const fleetBriefing = $derived([
		{
			label: 'Control-plane reach',
			value: `${inventory.nodes.length} reporting nodes`,
			note:
				overview.nodes_degraded > 0
					? `${overview.nodes_degraded} node signals need review`
					: 'Fleet reporting is stable'
		},
		{
			label: 'Workload posture',
			value: `${overview.vms_running} active of ${overview.vms_total || inventory.vms.length}`,
			note:
				overview.unresolved_alerts > 0
					? `${overview.unresolved_alerts} unresolved operator alerts`
					: 'No blocking workload alarms'
		},
		{
			label: 'Execution queue',
			value: `${overview.active_tasks || recentTasks.length} active operations`,
			note:
				recentTasks.length > 0
					? `Latest activity: ${recentTasks[0].operation}`
					: 'No recent task churn'
		}
	]);

	const pressureCards = $derived([
		{
			label: 'CPU envelope',
			value: `${Math.round(overview.cpu_usage_percent || 0)}%`,
			width: overview.cpu_usage_percent || 0
		},
		{
			label: 'Memory envelope',
			value: `${Math.round(overview.memory_usage_percent || 0)}%`,
			width: overview.memory_usage_percent || 0
		},
		{
			label: 'Storage pressure',
			value: `${Math.round(overview.storage_usage_percent || 0)}%`,
			width: overview.storage_usage_percent || 0
		}
	]);
</script>

<div class="cockpit-dashboard">
	{#if overview.state === 'error'}
		<ErrorState title="Telemetry Failure" description="Fleet-wide health signals are currently unreachable." />
	{:else if overview.state === 'loading' || inventory.isLoading}
		<LoadingState title="Indexing topology..." />
	{:else if overview.state === 'empty' && inventory.nodes.length === 0}
		<EmptyInfrastructureState 
			title="Empty Fleet" 
			description="No clusters or nodes are currently indexed."
			hint="Enroll infrastructure to see real-time topology."
		/>
	{:else}
		<div class="cockpit-layout">
			<div class="cockpit-metrics">
				<CompactMetricCard 
					label="Managed Nodes" 
					value={inventory.nodes.length} 
					trend={0}
					color="primary"
				/>
				<CompactMetricCard 
					label="Running Workloads" 
					value={inventory.vms.filter(v => v.actual_state === 'running').length} 
					unit={`/ ${inventory.vms.length}`}
					trend={+2}
					points={[10, 12, 11, 14, 15, 14, 16]}
					color="accent"
				/>
				<CompactMetricCard 
					label="Fleet CPU" 
					value={Math.round(overview.cpu_usage_percent || 0)} 
					unit="%"
					trend={-5}
					points={[45, 42, 48, 50, 47, 45]}
					color={overview.cpu_usage_percent > 80 ? 'danger' : 'primary'}
				/>
				<CompactMetricCard 
					label="Fleet memory" 
					value={Math.round(overview.memory_usage_percent || 0)} 
					unit="%"
					trend={+1}
					points={[65, 68, 70, 72, 71, 72]}
					color={overview.memory_usage_percent > 85 ? 'warning' : 'primary'}
				/>
			</div>

			<div class="cockpit-briefing-grid">
				<SectionCard title="Fleet Briefing" icon={ShieldCheck} badgeLabel="Shift View">
					<div class="briefing-grid">
						{#each fleetBriefing as item}
							<article class="briefing-card">
								<p class="briefing-label">{item.label}</p>
								<p class="briefing-value">{item.value}</p>
								<p class="briefing-note">{item.note}</p>
							</article>
						{/each}
					</div>
				</SectionCard>

				<SectionCard
					title="Immediate Attention"
					icon={AlertCircle}
					badgeLabel={overview.unresolved_alerts > 0 ? String(overview.unresolved_alerts) : 'Clear'}
					badgeTone={overview.unresolved_alerts > 0 ? 'warning' : 'healthy'}
				>
					<ul class="attention-list">
						{#each overview.alerts.slice(0, 4) as alert}
							<li class="attention-item">
								<div class="attention-item__header">
									<SeverityShield severity={alert.severity} />
									<span class="attention-scope">{alert.scope}</span>
								</div>
								<p>{alert.summary}</p>
							</li>
						{/each}
						{#if overview.alerts.length === 0}
							<li class="attention-item attention-item--quiet">
								<Server size={15} />
								<div>
									<p>Signals nominal across the indexed fleet.</p>
									<span>No active incidents are crowding the queue.</span>
								</div>
							</li>
						{/if}
					</ul>
				</SectionCard>
			</div>

			<div class="cockpit-workspace">
				<section class="cockpit-topology">
					<TopologyCanvas />
				</section>

				<aside class="cockpit-rail">
					<SectionCard title="Operation Pipeline" icon={Activity} badgeLabel="Live">
						<TaskTimeline tasks={recentTasks.slice(0, 4)} />
					</SectionCard>

					<SectionCard title="Capacity Pressure" icon={Zap}>
						<div class="capacity-preview">
							{#each pressureCards as item}
								<div class="cap-item">
									<div class="cap-header"><span>{item.label}</span><span>{item.value}</span></div>
									<div class="cap-bar"><div class="cap-fill" style={`width: ${item.width}%`}></div></div>
								</div>
							{/each}
							<div class="capacity-footnote">
								<span>Network throughput index</span>
								<strong>Nominal</strong>
							</div>
						</div>
					</SectionCard>
				</aside>
			</div>
		</div>
	{/if}
</div>

<style>
	.cockpit-dashboard {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.cockpit-layout {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		height: 100%;
	}

	.cockpit-metrics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.75rem;
	}

	.cockpit-briefing-grid {
		display: grid;
		grid-template-columns: minmax(0, 1.3fr) minmax(18rem, 0.9fr);
		gap: 0.75rem;
	}

	.briefing-grid {
		display: grid;
		gap: 0.75rem;
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}

	.briefing-card {
		padding: 0.85rem 0.9rem;
		border-radius: var(--radius-sm);
		border: 1px solid var(--shell-line);
		background: var(--shell-surface-muted);
	}

	.briefing-label {
		margin: 0;
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--shell-text-muted);
	}

	.briefing-value {
		margin: 0.45rem 0 0;
		font-size: var(--text-lg);
		font-weight: 700;
		color: var(--shell-text);
	}

	.briefing-note {
		margin: 0.3rem 0 0;
		font-size: var(--text-xs);
		line-height: 1.5;
		color: var(--shell-text-secondary);
	}

	.cockpit-workspace {
		display: grid;
		grid-template-columns: minmax(0, 1.65fr) minmax(19rem, 0.85fr);
		gap: 0.75rem;
		align-items: start;
		flex: 1;
		min-height: 0;
	}

	.cockpit-topology {
		min-width: 0;
	}

	.cockpit-topology :global(.topology-canvas) {
		min-height: 34rem;
	}

	.cockpit-topology :global(.svg-container) {
		min-height: 28rem;
	}

	.cockpit-rail {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		min-width: 0;
	}

	.attention-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.attention-item {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0.7rem 0.75rem;
		background: var(--bg-surface-muted);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
	}

	.attention-item__header {
		display: flex;
		align-items: center;
		gap: 0.45rem;
	}

	.attention-item p {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.45;
		color: var(--shell-text);
	}

	.attention-scope,
	.attention-item span {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.attention-item--quiet {
		display: grid;
		grid-template-columns: auto 1fr;
		align-items: start;
		color: var(--color-success);
	}

	.capacity-preview {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.cap-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.cap-header {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		font-weight: 600;
		color: var(--color-neutral-500);
		text-transform: uppercase;
	}

	.capacity-footnote {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding-top: 0.25rem;
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.cap-bar {
		height: 4px;
		background: var(--color-neutral-100);
		border-radius: 999px;
		overflow: hidden;
	}

	.cap-fill {
		height: 100%;
		background: var(--color-primary);
	}

	@media (max-width: 1100px) {
		.cockpit-briefing-grid,
		.cockpit-workspace {
			grid-template-columns: 1fr;
		}

		.briefing-grid {
			grid-template-columns: 1fr;
		}

		.cockpit-topology :global(.topology-canvas) {
			min-height: 28rem;
		}
	}
</style>
