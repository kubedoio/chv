<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
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
		Zap
	} from 'lucide-svelte';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import { inventory } from '$lib/stores/inventory.svelte';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/');
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
			<!-- TOP BAR: REAL-TIME METRICS -->
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

			<!-- MAIN AREA: TOPOLOGY CANVAS -->
			<div class="cockpit-main">
				<TopologyCanvas />
			</div>

			<!-- LOWER STRIP: RECENT TASKS & ALERTS -->
			<div class="cockpit-bottom-grid">
				<SectionCard title="Operation Pipeline" icon={Activity} badgeLabel="Live">
					<TaskTimeline tasks={recentTasks.slice(0, 3)} />
				</SectionCard>

				<SectionCard title="Active Incidents" icon={AlertCircle} badgeLabel={String(overview.unresolved_alerts)}>
					<ul class="micro-alert-list">
						{#each overview.alerts.slice(0, 3) as alert}
							<li class="micro-alert-compact">
								<SeverityShield severity={alert.severity} />
								<span class="alert-txt">{alert.summary}</span>
								<span class="alert-scope">{alert.scope}</span>
							</li>
						{/each}
						{#if overview.alerts.length === 0}
							<li class="empty-signals">Signals Nominal. No active incidents.</li>
						{/if}
					</ul>
				</SectionCard>

				<SectionCard title="Resource Saturation" icon={Zap}>
					<div class="capacity-preview">
						<div class="cap-item">
							<div class="cap-header"><span>Storage Pool Utilization</span><span>{Math.round(overview.storage_usage_percent || 0)}%</span></div>
							<div class="cap-bar"><div class="cap-fill" style="width: {overview.storage_usage_percent || 0}%"></div></div>
						</div>
						<div class="cap-item">
							<div class="cap-header"><span>Network Throughput Index</span><span>Nominal</span></div>
							<div class="cap-bar"><div class="cap-fill" style="width: 35%"></div></div>
						</div>
					</div>
				</SectionCard>
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

	.cockpit-main {
		flex: 1;
		min-height: 450px;
	}

	.cockpit-bottom-grid {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 0.75rem;
	}

	.micro-alert-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.micro-alert-compact {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.35rem 0.5rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
		font-size: 11px;
	}

	.alert-txt {
		flex: 1;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--color-neutral-800);
	}

	.alert-scope {
		color: var(--color-neutral-400);
		font-size: 9px;
	}

	.empty-signals {
		padding: 1rem;
		text-align: center;
		font-size: 11px;
		color: var(--color-neutral-400);
	}

	.capacity-preview {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.cap-item {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.cap-header {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		font-weight: 600;
		color: var(--color-neutral-500);
		text-transform: uppercase;
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
</style>
