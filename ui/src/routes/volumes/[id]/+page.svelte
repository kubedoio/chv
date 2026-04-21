<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { getStoredToken } from '$lib/api/client';
	import { mutateVolume } from '$lib/bff/volumes';
	import { listVms } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { invalidateAll } from '$app/navigation';
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import ActionStrip from '$lib/components/shell/ActionStrip.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import TaskTimeline from '$lib/components/shell/TaskTimeline.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import Modal from '$lib/components/modals/Modal.svelte';
	import FormField from '$lib/components/forms/FormField.svelte';
	import Input from '$lib/components/Input.svelte';
	import { Link2, Unlink, Maximize2, Database, Box, Activity, Info, AlertTriangle } from 'lucide-svelte';

	let { data }: { data: PageData } = $props();

	const detail = $derived(data.detail);
	let pendingAction = $state<string | null>(null);
	let confirmingAction = $state<string | null>(null);

	// Attach modal state
	let attachModalOpen = $state(false);
	let availableVms = $state<{ vm_id: string; name: string; node_id: string }[]>([]);
	let selectedVmId = $state('');
	let loadingVms = $state(false);

	// Resize modal state
	let resizeModalOpen = $state(false);
	let resizeSizeGb = $state(10);
	let resizeError = $state('');

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
		} else if (action === 'attach') {
			openAttachModal();
		} else if (action === 'resize') {
			openResizeModal();
		} else {
			executeAction(action);
		}
	}

	async function openAttachModal() {
		selectedVmId = '';
		attachModalOpen = true;
		loadingVms = true;
		try {
			const token = getStoredToken() ?? undefined;
			const res = await listVms({ page: 1, page_size: 200, filters: {} }, token);
			const allVms = (res.items as any[]).map((v) => ({
				vm_id: v.vm_id as string,
				name: v.name as string,
				node_id: v.node_id as string
			}));
			// Filter VMs on the same node as the volume, excluding already-attached
			availableVms = allVms.filter(
				(v) => v.node_id === detail.summary.node_id && v.vm_id !== detail.summary.attached_vm_id
			);
		} catch (err) {
			toast.error('Failed to load VMs');
		} finally {
			loadingVms = false;
		}
	}

	function openResizeModal() {
		const currentBytes = detail.summary.capacity_bytes ?? 0;
		resizeSizeGb = currentBytes > 0 ? Math.ceil(currentBytes / (1024 * 1024 * 1024)) : 10;
		resizeError = '';
		resizeModalOpen = true;
	}

	async function executeAttach() {
		if (!selectedVmId) {
			toast.error('Please select a VM');
			return;
		}
		attachModalOpen = false;
		pendingAction = 'attach';
		const token = getStoredToken() ?? undefined;
		const volume_id = detail.summary.volume_id;
		try {
			await mutateVolume({ volume_id, action: 'attach', force: false, vm_id: selectedVmId }, token);
			toast.success('Volume attach accepted');
			await invalidateAll();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Attach failed';
			toast.error(message);
		} finally {
			pendingAction = null;
		}
	}

	async function executeResize() {
		if (resizeSizeGb < 1) {
			resizeError = 'Size must be at least 1 GB';
			return;
		}
		const currentBytes = detail.summary.capacity_bytes ?? 0;
		const newBytes = resizeSizeGb * 1024 * 1024 * 1024;
		if (newBytes <= currentBytes) {
			resizeError = 'New size must be larger than current size';
			return;
		}
		resizeModalOpen = false;
		pendingAction = 'resize';
		const token = getStoredToken() ?? undefined;
		const volume_id = detail.summary.volume_id;
		try {
			await mutateVolume({ volume_id, action: 'resize', force: false, resize_bytes: newBytes }, token);
			toast.success('Volume resize accepted');
			await invalidateAll();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Resize failed';
			toast.error(message);
		} finally {
			pendingAction = null;
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
							<button class="btn-secondary btn-sm" disabled={pendingAction !== null} onclick={() => handleActionClick('resize')}>
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
						<TaskTimeline tasks={timelineTasks} />
					</SectionCard>

					<SectionCard title="Metadata & Config" icon={Info}>
						<PropertyGrid properties={configProps} columns={2} />
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

<!-- Attach Modal -->
<Modal bind:open={attachModalOpen} title="Attach Volume to VM">
	{#snippet children()}
		<div class="modal-body">
			<p class="modal-desc">Select a VM on node <strong>{detail.summary.node_id}</strong> to attach this volume to.</p>
			<FormField label="Target VM" required>
				{#if loadingVms}
					<p class="text-sm text-muted">Loading VMs...</p>
				{:else if availableVms.length === 0}
					<p class="text-sm text-muted">No available VMs on this node.</p>
				{:else}
					<select bind:value={selectedVmId} class="vm-select">
						<option value="">-- Select a VM --</option>
						{#each availableVms as vm}
							<option value={vm.vm_id}>{vm.name} ({vm.vm_id})</option>
						{/each}
					</select>
				{/if}
			</FormField>
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="modal-footer">
			<button class="btn-secondary btn-sm" onclick={() => attachModalOpen = false}>Cancel</button>
			<button class="btn-primary btn-sm" disabled={!selectedVmId || loadingVms} onclick={executeAttach}>Attach</button>
		</div>
	{/snippet}
</Modal>

<!-- Resize Modal -->
<Modal bind:open={resizeModalOpen} title="Resize Volume">
	{#snippet children()}
		<div class="modal-body">
			<p class="modal-desc">Current size: <strong>{detail.summary.size}</strong>. Enter the new size below.</p>
			<FormField label="New Size (GB)" required error={resizeError}>
				<Input type="number" bind:value={resizeSizeGb} min={1} />
			</FormField>
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="modal-footer">
			<button class="btn-secondary btn-sm" onclick={() => resizeModalOpen = false}>Cancel</button>
			<button class="btn-primary btn-sm" onclick={executeResize}>Resize</button>
		</div>
	{/snippet}
</Modal>

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
		font-size: 0.875rem;
		color: var(--color-ink);
	}

	.detail-grid {
		display: grid;
		grid-template-columns: 1fr 320px;
		gap: 1.5rem;
		padding: 1.5rem;
	}

	.detail-main-span {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}

	.detail-sections {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
	}

	.summary-top {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.attachment-info {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.empty-hint {
		color: var(--color-muted);
		font-size: 0.875rem;
	}

	.alert-box {
		padding: 0.75rem;
		border-radius: 0.375rem;
		border: 1px solid;
	}

	.alert-box.tone-warning {
		background: var(--color-warning-light);
		border-color: var(--color-warning);
	}

	.alert-label {
		font-weight: 600;
		font-size: 0.875rem;
		color: var(--color-ink);
	}

	.alert-desc {
		font-size: 0.875rem;
		color: var(--color-muted);
		margin-top: 0.25rem;
	}

	.modal-body {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.modal-desc {
		font-size: 0.875rem;
		color: var(--color-muted);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
	}

	.vm-select {
		height: 2.25rem;
		width: 100%;
		border-radius: 0.375rem;
		border: 1px solid var(--color-border, #ccc);
		padding: 0 0.75rem;
		font-size: 0.875rem;
		background: white;
	}
</style>
