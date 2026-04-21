<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ProgressBar from '$lib/components/shell/ProgressBar.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Wrench, ArrowUpFromLine, RefreshCcw, Activity, ShieldCheck, AlertCircle, ChevronRight, Clock } from 'lucide-svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/maintenance');
	const maintenance = $derived(data.maintenance);

	const activeWindows = $derived(maintenance.windows);
	const drainingNodes = $derived(maintenance.nodes);

	const isEmpty = $derived(
		activeWindows.length === 0 &&
		drainingNodes.length === 0 &&
		!maintenance.pending_actions &&
		!maintenance.upgrade_available
	);

	const upgradePostureProps = $derived([
		{ label: 'Current Version', value: maintenance.current_version || 'Unavailable' },
		{ label: 'Pending Upgrades', value: maintenance.upgrade_available ? 'Upgrades available' : 'Up to date', tone: (maintenance.upgrade_available ? 'warning' : 'healthy') as any },
		{ label: 'Reboot Required', value: maintenance.reboot_required_nodes ? maintenance.reboot_required_nodes.join(', ') : 'None', tone: (maintenance.reboot_required_nodes?.length ? 'warning' : 'healthy') as any },
		{ label: 'Orchestrator Health', value: maintenance.orchestrator_health || 'Unknown', tone: (maintenance.orchestrator_health === 'Nominal' ? 'healthy' : 'warning') as any }
	]);

	const stats = $derived([
		{ label: 'Draining Nodes', value: drainingNodes.filter(n => n.status === 'draining').length, status: 'warning' as const },
		{ label: 'In Maintenance', value: drainingNodes.filter(n => n.status === 'in_maintenance').length, status: 'neutral' as const },
		{ label: 'Pending Actions', value: maintenance.pending_actions || 0, status: 'critical' as const },
		{ label: 'Upgrade Ready', value: maintenance.upgrade_available ? 'Yes' : 'No', status: maintenance.upgrade_available ? 'warning' as const : 'healthy' as const }
	]);
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={page}>
		{#snippet actions()}
			<button class="btn-primary">
				<Wrench size={14} />
				Schedule Maintenance
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<div class="posture-strip-wrapper">
		<CompactStatStrip {stats} />
	</div>

	{#if data.error}
		<ErrorState />
	{:else if isEmpty}
		<EmptyInfrastructureState
			title="No maintenance activity"
			description="There are no active maintenance windows, draining nodes, or pending actions."
			hint="Schedule a maintenance window to safely drain nodes and apply updates."
		/>
	{:else}
		<main class="detail-grid">
		<div class="detail-main-span">
			<SectionCard title="Active Maintenance Windows" icon={Clock} badgeTone={activeWindows.length > 0 ? 'warning' : 'neutral'}>
				{#if activeWindows.length === 0}
					<p class="empty-hint">No maintenance windows currently active or scheduled.</p>
				{:else}
					<div class="active-windows">
						{#each activeWindows as window}
							<div class="window-entry">
								<div class="window-main">
									<span class="window-title">{window.title}</span>
									<div class="window-meta">
										<span class="time">Started {new Date(window.started_at).toLocaleString()}</span>
										<span class="sep">·</span>
										<span class="end">Ends {new Date(window.expected_end_at).toLocaleDateString()}</span>
									</div>
								</div>
								<StatusBadge label={window.status} tone="warning" />
							</div>
						{/each}
					</div>
				{/if}
			</SectionCard>

			<SectionCard title="Lifecycle progress" icon={ArrowUpFromLine}>
				{#if drainingNodes.length === 0}
					<p class="empty-hint">No nodes are currently draining or undergoing maintenance.</p>
				{:else}
					<div class="node-progress-list">
						{#each drainingNodes as node}
							<div class="node-op-entry">
								<div class="node-op-header">
									<div class="node-info">
										<span class="node-name">{node.name}</span>
										<span class="node-status">{node.status}</span>
									</div>
									<span class="progress-pct">{node.progress}%</span>
								</div>
								<ProgressBar progress={node.progress || 0} tone={node.status === 'draining' ? 'warning' : 'healthy'} />
							</div>
						{/each}
					</div>
				{/if}
			</SectionCard>

			<SectionCard title="System Upgrade Posture" icon={RefreshCcw}>
				<PropertyGrid properties={upgradePostureProps} columns={2} />
				<div class="upgrade-action">
					{#if maintenance.upgrade_available}
						<div class="upgrade-banner">
							<RefreshCcw size={16} class="spin-slow" />
							<div class="upgrade-text">
								<strong>Version 2.5.0-LTS Available</strong>
								<span>Includes security patches for kernel NVMe drivers.</span>
							</div>
							<button class="btn-primary btn-sm">Start Upgrade</button>
						</div>
					{:else}
						<div class="up-to-date">
							<ShieldCheck size={16} />
							<span>Cluster components are running the latest stable build.</span>
						</div>
					{/if}
				</div>
			</SectionCard>
		</div>

		<aside class="detail-side-span">
			<SectionCard title="Pending Operator Action" icon={AlertCircle} badgeTone="warning">
				{#if (maintenance.pending_operator_actions || []).length === 0}
					<p class="empty-hint">No operator actions required.</p>
				{:else}
					<ul class="action-list">
						{#each maintenance.pending_operator_actions as action}
							<li>
								<div class="action-item">
									<span class="txt">{action.summary}</span>
									<button class="btn-secondary btn-xs">Acknowledge</button>
								</div>
							</li>
						{/each}
					</ul>
				{/if}
			</SectionCard>

			<SectionCard title="Maintenance History" icon={Activity}>
				<p class="empty-hint">No recent maintenance history available.</p>
			</SectionCard>
		</aside>
	</main>
	{/if}
</div>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.posture-strip-wrapper {
		margin-top: -0.25rem;
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

	.active-windows {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.window-entry {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.75rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.25rem;
	}

	.window-main {
		display: flex;
		flex-direction: column;
	}

	.window-title {
		font-weight: 600;
		font-size: var(--text-sm);
	}

	.window-meta {
		display: flex;
		gap: 0.35rem;
		font-size: 10px;
		color: var(--shell-text-muted);
	}

	.node-progress-list {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.node-op-entry {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.node-op-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
	}

	.node-info {
		display: flex;
		flex-direction: column;
	}

	.node-name {
		font-weight: 700;
		font-size: var(--text-sm);
	}

	.node-status {
		font-size: 10px;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.progress-pct {
		font-family: var(--font-mono);
		font-size: 11px;
		font-weight: 700;
	}

	.upgrade-action {
		margin-top: 1rem;
		padding-top: 1rem;
		border-top: 1px solid var(--shell-line);
	}

	.upgrade-banner {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.75rem;
		background: var(--color-warning-light);
		border: 1px solid var(--color-warning-dark);
		border-radius: 0.35rem;
	}

	.upgrade-text {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	.upgrade-text strong { font-size: var(--text-sm); }
	.upgrade-text span { font-size: 11px; }

	.up-to-date {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: var(--color-success);
		font-size: var(--text-xs);
		font-weight: 500;
	}

	.action-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.action-item {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.25rem;
	}

	.action-item .txt {
		font-size: var(--text-xs);
		font-weight: 500;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
	}

	.spin-slow {
		animation: spin 3s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	@media (max-width: 1200px) {
		.detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
