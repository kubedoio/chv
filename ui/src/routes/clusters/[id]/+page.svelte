<script lang="ts">
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ProgressBar from '$lib/components/shell/ProgressBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import LoadingState from '$lib/components/shell/LoadingState.svelte';
	import { 
		Blocks, 
		Activity, 
		AlertTriangle, 
		Zap, 
		Server,
		History,
		ExternalLink
	} from 'lucide-svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/clusters');
	const detail = $derived(data.detail);
	const summary = $derived(detail.summary);

	const basicProps = $derived([
		{ label: 'Datacenter', value: summary.datacenter },
		{ label: 'Cluster ID', value: summary.clusterId, isMono: true },
		{ label: 'Node Count', value: String(summary.nodeCount) },
		{ label: 'Version', value: summary.version + (summary.versionSkew ? ' (skew)' : '') }
	]);

	function capacityTone(val: number): 'healthy' | 'warning' | 'failed' {
		if (val >= 85) return 'failed';
		if (val >= 70) return 'failed';
		if (val >= 55) return 'warning';
		return 'healthy';
	}
</script>

<div class="detail-page">
	{#if detail.state === 'error'}
		<ErrorState title="Cluster Detail Unavailable" description="Failed to retrieve cluster metrics from the control plane." />
	{:else}
		<ResourceDetailHeader
			eyebrow={summary.datacenter}
			title={summary.name}
			description="Compute cluster governing {summary.nodeCount} nodes"
			statusLabel={summary.state}
			tone={summary.state === 'healthy' ? 'healthy' : 'degraded'}
		>
			{#snippet actions()}
				<button class="btn-secondary">
					<ExternalLink size={14} />
					Provider Console
				</button>
			{/snippet}
		</ResourceDetailHeader>

		<main class="detail-grid">
			<div class="detail-main-span">
				<SectionCard title="Cluster Summary" icon={Blocks}>
					<PropertyGrid properties={basicProps} columns={2} />
				</SectionCard>

				<SectionCard title="Compute & Storage Capacity" icon={Zap}>
					<div class="capacity-segments">
						<div class="cap-segment">
							<div class="cap-info">
								<span class="cap-label">Aggregate CPU</span>
								<span class="cap-val">{summary.cpuPercent}%</span>
							</div>
							<ProgressBar progress={summary.cpuPercent} tone={capacityTone(summary.cpuPercent)} />
						</div>
						<div class="cap-segment">
							<div class="cap-info">
								<span class="cap-label">Aggregate Memory</span>
								<span class="cap-val">{summary.memoryPercent}%</span>
							</div>
							<ProgressBar progress={summary.memoryPercent} tone={capacityTone(summary.memoryPercent)} />
						</div>
						<div class="cap-segment">
							<div class="cap-info">
								<span class="cap-label">Distributed Storage</span>
								<span class="cap-val">{summary.storagePercent}%</span>
							</div>
							<ProgressBar progress={summary.storagePercent} tone={capacityTone(summary.storagePercent)} />
						</div>
					</div>
				</SectionCard>

				<SectionCard title="Active Workloads" icon={Server}>
					<div class="workload-summary">
						<div class="w-stat">
							<span class="val">{summary.vmCount ?? '—'}</span>
							<span class="lbl">Total VMs</span>
						</div>
						<div class="w-stat">
							<span class="val">{summary.vmHealthy ?? '—'}</span>
							<span class="lbl">Healthy</span>
						</div>
						{#if summary.vmCount === undefined}
							<p class="w-hint">Workload rollups are currently being calculated for this cluster.</p>
						{/if}
					</div>
				</SectionCard>
			</div>

			<aside class="detail-side-span">
				<SectionCard title="Recent Tasks" icon={History} badgeLabel={String(summary.activeTasks)}>
					{#if summary.activeTasks === 0}
						<p class="empty-hint">No active tasks for this cluster.</p>
					{:else}
						<div class="mini-activity">
							<p>Multiple operations are currently running or recently completed.</p>
							<a href="/tasks?query={summary.clusterId}" class="view-link">Open Cluster Tasks</a>
						</div>
					{/if}
				</SectionCard>

				<SectionCard title="Unresolved Alerts" icon={AlertTriangle} badgeLabel={String(summary.alerts)} badgeTone={summary.alerts > 0 ? 'warning' : 'neutral'}>
					{#if summary.alerts === 0}
						<p class="empty-hint">Cluster reporting nominal health signals.</p>
					{:else}
						<div class="mini-activity">
							<p>{summary.alerts} signals require operator inspection.</p>
							<a href="/events?query={summary.clusterId}" class="view-link">Open Cluster Events</a>
						</div>
					{/if}
				</SectionCard>
			</aside>
		</main>
	{/if}
</div>

<style>
	.detail-page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.detail-grid {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.detail-main-span {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.detail-side-span {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.capacity-segments {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 0.5rem 0;
	}

	.cap-segment {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.cap-info {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}

	.cap-label {
		font-size: var(--text-xs);
		font-weight: 500;
		color: var(--shell-text-muted);
	}

	.cap-val {
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		font-weight: 700;
	}

	.workload-summary {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.75rem;
	}

	.w-stat {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 0.75rem;
		background: var(--shell-surface-muted);
		border-radius: 0.35rem;
	}

	.w-stat .val { font-size: var(--text-lg); font-weight: 700; }
	.w-stat .lbl { font-size: 10px; color: var(--shell-text-muted); text-transform: uppercase; letter-spacing: 0.05em; }

	.w-hint {
		grid-column: 1 / -1;
		font-size: 11px;
		color: var(--shell-text-muted);
		text-align: center;
		margin-top: 0.5rem;
	}

	.mini-activity {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.mini-activity p {
		font-size: var(--text-xs);
		color: var(--shell-text-secondary);
		margin: 0;
	}

	.view-link {
		font-size: 11px;
		font-weight: 700;
		text-transform: uppercase;
		text-decoration: none;
		color: var(--shell-accent);
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
	}

	@media (max-width: 1200px) {
		.detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
