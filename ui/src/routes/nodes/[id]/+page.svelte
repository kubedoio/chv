<script lang="ts">
	import type { PageData } from './$types';
	import { getStoredToken } from '$lib/api/client';
	import { mutateNode } from '$lib/bff/nodes';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { Pause, Play, Wrench, ArrowUpFromLine, Activity, Box, Info, AlertTriangle, ShieldCheck } from 'lucide-svelte';
	import NodeHealthDashboard from '$lib/components/NodeHealthDashboard.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['healthy', 'ready', 'active', 'online'].includes(s)) return 'healthy';
		if (['warning', 'maintenance', 'draining', 'starting', 'paused'].includes(s)) return 'warning';
		if (['degraded', 'offline', 'failed', 'error'].includes(s)) return 'failed';
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
		} catch (err: any) {
			toast.error(err.message || 'Action failed');
		} finally {
			pendingAction = null;
		}
	}

	const postureProps = $derived([
		{ label: 'Control State', value: detail.summary.state },
		{ label: 'Safety Integrity', value: detail.summary.health },
		{ label: 'Storage Fabric', value: detail.summary.storage },
		{ label: 'Network Fabric', value: detail.summary.network }
	]);

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));

	const vmColumns = [
		{ key: 'name', label: 'Workload Identity' },
		{ key: 'power_state', label: 'Power', align: 'center' as const },
		{ key: 'health', label: 'Safety', align: 'center' as const },
		{ key: 'cpu', label: 'Core Alloc', align: 'right' as const },
		{ key: 'memory', label: 'RAM Reserv', align: 'right' as const }
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

<div class="inventory-page">
	{#if detail.state === 'error'}
		<ErrorState title="Host record unreachable" description="The control plane could not assemble host telemetry." />
	{:else if detail.state === 'empty'}
		<EmptyInfrastructureState title="Host Identity Unknown" description="The requested node ID does not exist." />
	{:else}
		<ResourceDetailHeader 
			title={detail.summary.name} 
			eyebrow="PHYSICAL_NODE // {detail.summary.node_id}"
			statusLabel={detail.summary.state}
			tone={normalizeTone(detail.summary.state)}
			parentLabel="Nodes"
			parentHref="/nodes"
		>
			{#snippet actions()}
				<div class="header-actions">
           {#if detail.summary.maintenance}
							<button class="btn-secondary" disabled={pendingAction !== null} onclick={() => executeAction('exit_maintenance')}>
								<Wrench size={14} />
								{pendingAction === 'exit_maintenance' ? 'EXECUTING...' : 'EXIT_MAINTENANCE'}
							</button>
						{:else}
							<button class="btn-secondary" disabled={pendingAction !== null} onclick={() => executeAction('enter_maintenance')}>
								<Wrench size={14} />
								{pendingAction === 'enter_maintenance' ? 'EXECUTING...' : 'ENTER_MAINTENANCE'}
							</button>
						{/if}

						<button class="btn-primary" disabled={pendingAction !== null} onclick={() => executeAction('drain')}>
							<ArrowUpFromLine size={14} />
							{pendingAction === 'drain' ? 'EXECUTING...' : 'DRAIN_FABRIC'}
						</button>
				</div>
			{/snippet}
		</ResourceDetailHeader>

		<div class="inventory-metrics">
			<CompactMetricCard label="CPU Pressure" value={detail.summary.cpu} color="primary" />
			<CompactMetricCard label="Memory Entropy" value={detail.summary.memory} color="primary" />
			<CompactMetricCard label="IOPS Density" value={detail.summary.storage} color="neutral" />
			<CompactMetricCard label="Net Throughput" value={detail.summary.network} color="neutral" />
		</div>

		<main class="inventory-main">
			<section class="detail-content">
				<SectionCard title="Compute Posture" icon={Activity}>
					<PropertyGrid properties={postureProps} columns={2} />
				</SectionCard>

				<SectionCard title="Hardware Fabric" icon={Activity}>
					<NodeHealthDashboard />
				</SectionCard>

				<SectionCard title="Resident Workloads" icon={Box} badgeLabel={String(detail.hosted_vms.length)}>
					{#if detail.hosted_vms.length === 0}
						<p class="empty-hint">No virtual instances currently pinned to this host fabric.</p>
					{:else}
						<InventoryTable 
							columns={vmColumns} 
							rows={vmRows} 
							rowHref={(row) => `/vms/${row.vm_id}`} 
						>
               {#snippet cell({ column, row })}
                 {#if column.key === 'name'}
                   <span class="workload-name">{row.name}</span>
                 {:else if column.key === 'power_state' || column.key === 'health'}
                   <StatusBadge label={row[column.key].label} tone={row[column.key].tone} />
                 {:else}
                    <span class="cell-text">{row[column.key]}</span>
                 {/if}
               {/snippet}
            </InventoryTable>
					{/if}
				</SectionCard>

				<SectionCard title="Mutation Audit" icon={Activity}>
					<TaskTimeline tasks={timelineTasks} />
				</SectionCard>
			</section>

			<aside class="support-area">
				<SectionCard title="Safety Integrity" icon={ShieldCheck}>
					{#if detail.summary.health === 'healthy'}
						<div class="safety-sign">
							<ShieldCheck size={16} />
							<span>HOST_LEVEL_VERIFIED</span>
						</div>
					{:else}
						<div class="safety-sign alert">
							<AlertTriangle size={16} />
							<span>DEGRADATION_DETECTED</span>
						</div>
					{/if}
				</SectionCard>

				<SectionCard title="Host Metadata">
					<PropertyGrid 
						columns={1}
						properties={[
							{ label: 'OS Build', value: detail.summary.version },
							{ label: 'Uptime Seq', value: detail.summary.uptime || '—' },
							{ label: 'Last Sync', value: detail.summary.last_checkin || '—' }
						]} 
					/>
				</SectionCard>

				<SectionCard title="Configuration" icon={Info}>
					<PropertyGrid properties={configProps} columns={1} />
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

	@media (max-width: 1200px) {
		.detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
