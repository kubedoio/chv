<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { getStoredToken } from '$lib/api/client';
	import { mutateVolume } from '$lib/bff/volumes';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ActionStrip from '$lib/components/shell/ActionStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import { Link2, Unlink, Maximize2, Database, Box, Activity, Info, AlertTriangle } from 'lucide-svelte';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);
	let confirmingAction = $state<string | null>(null);

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['attached', 'healthy', 'ready', 'active', 'online'].includes(s)) return 'healthy';
		if (['warning', 'maintenance', 'attaching', 'detaching', 'resizing', 'available'].includes(s)) return 'warning';
		if (['degraded', 'offline'].includes(s)) return 'degraded';
		if (['failed', 'error', 'critical'].includes(s)) return 'failed';
		return 'unknown';
	}

	function handleActionClick(action: string, needsConfirm = false) {
		if (needsConfirm) {
			confirmingAction = action;
		} else {
			executeAction(action);
		}
	}

	async function executeAction(action: string) {
		confirmingAction = null;
		pendingAction = action;
		const token = getStoredToken() ?? undefined;
		const volume_id = detail.summary.volume_id;
		try {
			await mutateVolume({ volume_id, action, force: false }, token);
			toast.success(`Volume ${action} accepted`);
			await invalidateAll();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Action failed';
			toast.error(message);
		} finally {
			pendingAction = null;
		}
	}

	const postureProps = $derived([
		{ label: 'Status', value: detail.summary.status, tone: normalizeTone(detail.summary.status) as any },
		{ label: 'Health', value: detail.summary.health, tone: normalizeTone(detail.summary.health) as any },
		{ label: 'Size', value: detail.summary.size },
		{ label: 'Node', value: detail.summary.node_id }
	]);

	const attachmentProps = $derived([
		{ label: 'Attached VM', value: detail.summary.attached_vm_name || '-', subtext: detail.summary.attached_vm_id || 'None' },
		{ label: 'Device Path', value: detail.summary.device_name || 'N/A' },
		{ label: 'Read Only', value: detail.summary.read_only ? 'Yes' : 'No' },
		{ label: 'Storage Class', value: detail.summary.storage_class || 'Standard' }
	]);

	const configProps = $derived(detail.configuration.map(c => ({ label: c.label, value: c.value })));

	const timelineTasks = $derived(detail.recent_tasks.map(t => ({
		...t,
		tone: normalizeTone(t.status)
	})));
</script>

<div class="resource-detail">
	{#if detail.state === 'error'}
		<ErrorState title="Volume Detail Unavailable" description="Failed to retrieve block device metadata." />
	{:else if detail.state === 'empty'}
		<EmptyInfrastructureState hint="Check the ID and try again" title="Volume Not Found" description="The requested volume ID is not recognized." />
	{:else}
		<ResourceDetailHeader 
			title={detail.summary.name} 
			eyebrow={detail.summary.node_id}
			statusLabel={detail.summary.status}
			tone={normalizeTone(detail.summary.status)}
			parentLabel="Volumes"
			parentHref="/volumes"
			description="Persistent block storage device."
		>
			{#snippet actions()}
				<div class="header-actions">
					<ActionStrip>
						{#if confirmingAction}
							<div class="confirm-group">
								<span class="confirm-text">Confirm <strong>{confirmingAction}</strong>?</span>
								<button class="btn-danger btn-sm" onclick={() => executeAction(confirmingAction!)}>Confirm</button>
								<button class="btn-secondary btn-sm" onclick={() => confirmingAction = null}>Cancel</button>
							</div>
						{:else}
							<button class="btn-primary btn-sm" disabled={!!detail.summary.attached_vm_id || pendingAction !== null} onclick={() => handleActionClick('attach')}>
								<Link2 size={14} />
								{pendingAction === 'attach' ? 'Attaching...' : 'Attach'}
							</button>
							<button class="btn-secondary btn-sm" disabled={!detail.summary.attached_vm_id || pendingAction !== null} onclick={() => handleActionClick('detach', true)}>
								<Unlink size={14} />
								{pendingAction === 'detach' ? 'Detaching...' : 'Detach'}
							</button>
							<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => handleActionClick('resize', true)}>
								<Maximize2 size={14} />
								{pendingAction === 'resize' ? 'Resizing...' : 'Resize'}
							</button>
						{/if}
					</ActionStrip>
				</div>
			{/snippet}
		</ResourceDetailHeader>

		<main class="detail-grid">
			<section class="detail-main-span">
				<div class="summary-top">
					<SectionCard title="Volume Posture" icon={Database}>
						<PropertyGrid properties={[...postureProps, ...attachmentProps]} columns={4} />
					</SectionCard>
				</div>

				<div class="detail-sections">
					<SectionCard title="Attachment Info" icon={Box}>
						{#if !detail.summary.attached_vm_id}
							<p class="empty-hint">This volume is currently available and not attached to any VM.</p>
						{:else}
							<div class="attachment-info">
								<p>Attached to <strong>{detail.summary.attached_vm_name}</strong> as <code>{detail.summary.device_name}</code>.</p>
								<a href="/vms/{detail.summary.attached_vm_id}" class="btn-secondary btn-sm">View VM</a>
							</div>
						{/if}
					</SectionCard>

					<SectionCard title="Recent Activity" icon={Activity}>
						<TaskTimeline tasks={timelineTasks} />SectionCard>
					</SectionCard>

					<SectionCard title="Metadata & Config" icon={Info}>
						<PropertyGrid properties={configProps} columns={2} />SectionCard>
					</SectionCard>
				</div>
			</section>

			<aside class="detail-side-span">
				<SectionCard title="Storage Backend">
					<PropertyGrid 
						columns={1}
						properties={[
							{ label: 'Kind', value: detail.summary.volume_kind || 'Block' },
							{ label: 'Pool', value: 'Default ZFS Pool' },
							{ label: 'Redundancy', value: 'RAID-10 (Mirror)' }
						]} 
					/>
				</SectionCard>

				<SectionCard title="Health Status" icon={AlertTriangle}>
					{#if detail.summary.health === 'healthy'}
						<p class="empty-hint">Volume integrity verified. No underlying hardware faults.</p>
					{:else}
						<div class="alert-box tone-warning">
							<span class="alert-label">IO Latency Detected</span>
							<p class="alert-desc">Higher than average latency detected on the backing pool. Performance may be impacted.</p>
						</div>
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

	.header-actions {
		display: flex;
		gap: 0.5rem;
	}

	.confirm-group {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		background: var(--color-danger-light);
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		border: 1px solid var(--color-danger);
	}

	.confirm-text {
		font-size: var(--text-xs);
		color: var(--color-danger-dark);
		font-weight: 600;
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

	.attachment-info {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border-radius: 0.25rem;
	}

	.attachment-info p {
		margin: 0;
		font-size: var(--text-sm);
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
