<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { getStoredToken } from '$lib/api/client';
	import { mutateNode } from '$lib/bff/nodes';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ActionStrip from '$lib/components/shell/ActionStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { Pause, Play, Wrench, ArrowUpFromLine, Activity, Box, Info, AlertTriangle } from 'lucide-svelte';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import NodeHealthDashboard from '$lib/components/NodeHealthDashboard.svelte';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['healthy', 'ready', 'active', 'completed', 'success', 'online'].includes(s)) return 'healthy';
		if (['warning', 'maintenance', 'bootstrapping', 'draining', 'starting', 'stopping', 'paused'].includes(s)) return 'warning';
		if (['degraded', 'offline'].includes(s)) return 'degraded';
		if (['failed', 'error', 'critical', 'crashed'].includes(s)) return 'failed';
		return 'unknown';
	}

	async function executeAction(action: string) {
		pendingAction = action;
		const token = getStoredToken() ?? undefined;
		const node_id = detail.summary.node_id;
		try {
			await mutateNode({ node_id, action }, token);
			toast.success(`Node ${action.replace('_', ' ')} accepted`);
			await invalidateAll();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Action failed';
			toast.error(message);
		} finally {
			pendingAction = null;
		}
	}

	const postureProps = $derived([
		{ label: 'State', value: detail.summary.state, tone: normalizeTone(detail.summary.state) as any },
		{ label: 'Health', value: detail.summary.health, tone: normalizeTone(detail.summary.health) as any },
		{ label: 'Storage', value: detail.summary.storage },
		{ label: 'Network', value: detail.summary.network }
	]);

	const capacityProps = $derived([
		{ label: 'CPU Usage', value: detail.summary.cpu, subtext: 'Total allocated' },
		{ label: 'Memory Usage', value: detail.summary.memory, subtext: 'Total reserved' },
		{ label: 'Scheduling', value: detail.summary.scheduling ? 'Active' : 'Paused', tone: (detail.summary.scheduling ? 'healthy' : 'warning') as any },
		{ label: 'Maintenance', value: detail.summary.maintenance ? 'Enabled' : 'Disabled', tone: (detail.summary.maintenance ? 'warning' : 'healthy') as any }
	]);

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));

	const vmColumns = [
		{ key: 'name', label: 'VM' },
		{ key: 'power_state', label: 'Power', align: 'center' as const },
		{ key: 'health', label: 'Health' },
		{ key: 'cpu', label: 'CPU', align: 'right' as const },
		{ key: 'memory', label: 'Memory', align: 'right' as const }
	];

	const vmRows = $derived(detail.hosted_vms.map(v => ({
		...v,
		power_state: { label: v.power_state, tone: normalizeTone(v.power_state) },
		health: { label: v.health, tone: normalizeTone(v.health) }
	})));

	const timelineTasks = $derived(detail.recent_tasks.map(t => ({
		...t,
		tone: normalizeTone(t.status)
	})));
</script>

<div class="resource-detail">
	{#if detail.state === 'error'}
		<ErrorState title="Node Detail Unavailable" description="The control plane could not assemble the requested host details." />
	{:else if detail.state === 'empty'}
		<EmptyInfrastructureState title="Node Not Found" description="The requested node ID does not exist in the current inventory." hint="Verify the Node ID is correct and the node has been enrolled." />
	{:else}
		<ResourceDetailHeader 
			title={detail.summary.name} 
			eyebrow={detail.summary.cluster}
			statusLabel={detail.summary.state}
			tone={normalizeTone(detail.summary.state)}
			parentLabel="Nodes"
			parentHref="/nodes"
			description="Physical compute host providing hypervisor resources."
		>
			{#snippet actions()}
				<div class="header-actions">
					<ActionStrip>
						{#if detail.summary.maintenance}
							<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => executeAction('exit_maintenance')}>
								<Wrench size={14} />
								{pendingAction === 'exit_maintenance' ? 'Exiting...' : 'Exit Maintenance'}
							</button>
						{:else}
							<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => executeAction('enter_maintenance')}>
								<Wrench size={14} />
								{pendingAction === 'enter_maintenance' ? 'Entering...' : 'Enter Maintenance'}
							</button>
						{/if}

						{#if detail.summary.scheduling}
							<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => executeAction('pause_scheduling')}>
								<Pause size={14} />
								{pendingAction === 'pause_scheduling' ? 'Pausing...' : 'Pause Scheduling'}
							</button>
						{:else}
							<button class="btn-primary btn-sm" disabled={pendingAction !== null} onclick={() => executeAction('resume_scheduling')}>
								<Play size={14} />
								{pendingAction === 'resume_scheduling' ? 'Resuming...' : 'Resume Scheduling'}
							</button>
						{/if}

						<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => executeAction('drain')}>
							<ArrowUpFromLine size={14} />
							{pendingAction === 'drain' ? 'Draining...' : 'Drain'}
						</button>
					</ActionStrip>
				</div>
			{/snippet}
		</ResourceDetailHeader>

		<main class="detail-grid">
			<section class="detail-main-span">
				<div class="summary-top">
					<SectionCard title="Current Posture" icon={Activity}>
						<PropertyGrid properties={[...postureProps, ...capacityProps]} columns={4} />SectionCard>
					</SectionCard>
				</div>

				<div class="detail-sections">
					<SectionCard title="Node Health" icon={Activity}>
						<NodeHealthDashboard />
					</SectionCard>

					<SectionCard title="Hosted Workloads" icon={Box} badgeLabel={String(detail.hosted_vms.length)}>
						{#if detail.hosted_vms.length === 0}
							<p class="empty-hint">No virtual machines currently placed on this node.</p>
						{:else}
							<InventoryTable 
								columns={vmColumns} 
								rows={vmRows} 
								rowHref={(row) => `/vms/${row.vm_id}`} 
							/>
						{/if}
					</SectionCard>

					<SectionCard title="Recent History" icon={Activity}>
						<TaskTimeline tasks={timelineTasks} />
					</SectionCard>

					<SectionCard title="Configuration" icon={Info}>
						<PropertyGrid properties={configProps} columns={2} />
					</SectionCard>
				</div>
			</section>

			<aside class="detail-side-span">
				<SectionCard title="Attention Required" icon={AlertTriangle} badgeTone={detail.summary.health !== 'healthy' ? 'warning' : 'neutral'}>
					{#if detail.summary.health === 'healthy'}
						<p class="empty-hint">No active alerts or hardware degradation detected.</p>
					{:else}
						<p class="empty-hint">Host infrastructure signals indicate degradation. Review events for details.</p>
					{/if}
				</SectionCard>

				<SectionCard title="Node Metadata">
					<PropertyGrid 
						columns={1}
						properties={[
							{ label: 'OS Version', value: detail.summary.version },
							{ label: 'Uptime', value: detail.summary.uptime || '—' },
							{ label: 'Last Check-in', value: detail.summary.last_checkin || '—' }
						]} 
					/>
				</SectionCard>
			</aside>
		</main>
	{/if}
</div>

<style>
	.resource-detail {
		display: flex;
		flex-direction: column;
	}

	.header-actions {
		display: flex;
		gap: 0.5rem;
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

	.detail-sections {
		display: grid;
		grid-template-columns: 1fr;
		gap: 1rem;
	}

	.detail-side-span {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
	}

	.alert-box {
		padding: 0.75rem;
		border-radius: 0.25rem;
		background: var(--color-warning-light);
		border: 1px solid var(--color-warning-dark);
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.alert-label {
		font-weight: 700;
		font-size: var(--text-xs);
		color: var(--color-warning-dark);
	}

	.alert-desc {
		font-size: var(--text-xs);
		color: var(--color-warning-dark);
		margin: 0;
	}

	@media (max-width: 1200px) {
		.detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
