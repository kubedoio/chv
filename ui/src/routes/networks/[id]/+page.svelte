<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ActionStrip from '$lib/components/shell/ActionStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { Shield, ShieldAlert, Network, Box, Activity, Info, AlertTriangle } from 'lucide-svelte';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['healthy', 'ready', 'active', 'online'].includes(s)) return 'healthy';
		if (['warning', 'maintenance', 'starting', 'stopping', 'paused', 'nat'].includes(s)) return 'warning';
		if (['degraded', 'offline', 'public'].includes(s)) return 'degraded';
		if (['failed', 'error', 'critical', 'crashed'].includes(s)) return 'failed';
		return 'unknown';
	}

	const postureProps = $derived([
		{ label: 'Infrastructure Health', value: detail.health, tone: normalizeTone(detail.health) as any },
		{ label: 'Exposure Status', value: detail.exposure, tone: normalizeTone(detail.exposure) as any },
		{ label: 'CIDR', value: detail.cidr },
		{ label: 'Gateway', value: detail.gateway }
	]);

	const policyProps = $derived([
		{ label: 'Active Policy', value: detail.policy },
		{ label: 'Scope', value: detail.scope },
		{ label: 'VLAN ID', value: '402' },
		{ label: 'Created', value: new Date(detail.created_at).toLocaleDateString() }
	]);

	const vmColumns = [
		{ key: 'name', label: 'VM' },
		{ key: 'ip', label: 'IP Address' },
		{ key: 'state', label: 'State' }
	];

	const vmRows = $derived(detail.attached_vms.map(v => ({
		...v,
		ip: v.ip || 'DHCP-pending',
		state: { label: 'connected', tone: 'healthy' as const }
	})));

	const timelineTasks = $derived([
		{ task_id: 't-01', summary: 'Policy modified', status: 'completed', operation: 'update_networks', tone: 'healthy' as const, started_at: '2h ago' }
	]);
</script>

<div class="resource-detail">
	{#if detail.state === 'error'}
		<ErrorState title="Network Detail Unavailable" description="The SDN controller could not provide synchronization data." />
	{:else}
		<ResourceDetailHeader 
			title={detail.name} 
			eyebrow={detail.scope}
			statusLabel={detail.exposure}
			tone={normalizeTone(detail.exposure)}
			parentLabel="Networks"
			parentHref="/networks"
			description="Defined software network segment."
		>
			{#snippet actions()}
				<ActionStrip>
					{#if detail.exposure === 'public'}
						<button class="btn-secondary btn-sm">
							<ShieldAlert size={14} />
							Withdraw Exposure
						</button>
					{:else}
						<button class="btn-secondary btn-sm">
							<Shield size={14} />
							Expose Publicly
						</button>
					{/if}
					<button class="btn-secondary btn-sm">
						<Activity size={14} />
						Edit Policy
					</button>
				</ActionStrip>
			{/snippet}
		</ResourceDetailHeader>

		<main class="detail-grid">
			<section class="detail-main-span">
				<div class="summary-top">
					<SectionCard title="Network Posture" icon={Activity}>
						<PropertyGrid properties={[...postureProps, ...policyProps]} columns={4} />
					</SectionCard>
				</div>

				<div class="detail-sections">
					<SectionCard title="Connected Workloads" icon={Box} badgeLabel={String(detail.attached_vms.length)}>
						{#if detail.attached_vms.length === 0}
							<p class="empty-hint">No virtual machines currently attached to this subnet.</p>
						{:else}
							<InventoryTable 
								columns={vmColumns} 
								rows={vmRows} 
								rowHref={(row) => `/vms/${row.vm_id}`} 
							/>
						{/if}
					</SectionCard>

					<SectionCard title="Policy History" icon={Activity}>
						<TaskTimeline tasks={timelineTasks} />SectionCard>
					</SectionCard>
				</div>
			</section>

			<aside class="detail-side-span">
				<SectionCard title="L3 Configuration" icon={Network}>
					<PropertyGrid 
						columns={1}
						properties={[
							{ label: 'Subnet ID', value: detail.network_id },
							{ label: 'Gateway IP', value: detail.gateway },
							{ label: 'DNS Servers', value: '1.1.1.1, 8.8.8.8' }
						]} 
					/>
				</SectionCard>

				<SectionCard title="Ingress Audit" icon={AlertTriangle}>
					{#if detail.exposure === 'public'}
						<div class="alert-box tone-warning">
							<span class="alert-label">Public Exposure Active</span>
							<p class="alert-desc">This network is reachable from external gateways. Ensure security policies are restricted.</p>
						</div>
					{:else}
						<p class="empty-hint">Network is private. Only peered cluster traffic allowed.</p>
					{/if}
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
