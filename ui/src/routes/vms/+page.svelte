<script lang="ts">
	import { enhance } from '$app/forms';
	import { PageShell, FilterPanel, StateBanner, ResourceTable } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData, ActionData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	const page = getPageDefinition('/vms');
	const model = $derived(data.vms);

	type MutationResult = { accepted: boolean; task_id: string; vm_id: string; summary: string };

	let lastMutation = $state<MutationResult | null>(null);
	let lastError = $state<string | null>(null);

	$effect(() => {
		if (form && 'accepted' in form && form.accepted === true) {
			lastMutation = form as unknown as MutationResult;
			lastError = null;
		} else if (form && 'message' in form) {
			lastError = String(form.message);
			lastMutation = null;
		}
	});

	function powerStateTone(state: string): ShellTone {
		switch (state.toLowerCase()) {
			case 'running':
				return 'healthy';
			case 'stopped':
				return 'unknown';
			case 'failed':
				return 'failed';
			default:
				return 'unknown';
		}
	}

	function healthTone(health: string): ShellTone {
		switch (health.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'degraded':
				return 'degraded';
			case 'failed':
				return 'failed';
			case 'warning':
				return 'warning';
			default:
				return 'unknown';
		}
	}

	function lastTaskTone(task: string): ShellTone {
		const t = task.toLowerCase();
		if (t.includes('fail') || t.includes('error')) return 'failed';
		if (t.includes('success') || t.includes('complete')) return 'healthy';
		return 'unknown';
	}

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'powerState',
			label: 'Power state',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				{ value: 'creating', label: 'Creating' },
				{ value: 'stopped', label: 'Stopped' },
				{ value: 'starting', label: 'Starting' },
				{ value: 'running', label: 'Running' },
				{ value: 'stopping', label: 'Stopping' },
				{ value: 'rebooting', label: 'Rebooting' },
				{ value: 'deleting', label: 'Deleting' },
				{ value: 'failed', label: 'Failed' },
				{ value: 'unknown', label: 'Unknown' }
			]
		},
		{
			name: 'health',
			label: 'Health',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All health' },
				{ value: 'healthy', label: 'Healthy' },
				{ value: 'degraded', label: 'Degraded' },
				{ value: 'failed', label: 'Failed' },
				{ value: 'unknown', label: 'Unknown' }
			]
		},
		{ name: 'nodeId', label: 'Node', type: 'search' as const }
	];

	const columns = [
		{ key: 'name', label: 'VM' },
		{ key: 'node_id', label: 'Node' },
		{ key: 'power_state', label: 'Power state' },
		{ key: 'health', label: 'Health' },
		{ key: 'cpu', label: 'CPU' },
		{ key: 'memory', label: 'Memory' },
		{ key: 'storage', label: 'Storage' },
		{ key: 'networks', label: 'Networks' },
		{ key: 'tags', label: 'Tags' },
		{ key: 'last_task', label: 'Last task' }
	];

	const rows = $derived(
		model.items.map((item) => ({
			vm_id: item.vm_id,
			name: item.name,
			node_id: item.node_id,
			power_state: { label: item.power_state, tone: powerStateTone(item.power_state) },
			health: { label: item.health, tone: healthTone(item.health) },
			cpu: item.cpu,
			memory: item.memory,
			storage: item.volume_count,
			networks: item.nic_count,
			tags: '-',
			last_task: { label: item.last_task, tone: lastTaskTone(item.last_task) }
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.vm_id;
		return typeof id === 'string' ? `/vms/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<FilterPanel filters={filterConfig} values={model.filters.current} />

	{#if lastMutation}
		<StateBanner
			variant="success"
			title={lastMutation.summary}
			description={`Task ${lastMutation.task_id} accepted for VM ${lastMutation.vm_id}`}
			hint="The task will appear in the task timeline once it begins processing."
		/>
	{/if}

	{#if lastError}
		<StateBanner variant="error" title="Action failed" description={lastError} />
	{/if}

	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="VM inventory unavailable"
			description="The VM roster could not be loaded from the BFF."
			hint="The UI keeps power state, health, and task transparency ready for the next healthy refresh."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No virtual machines match the current view"
			description="Widen the filters or create a VM to populate this page."
			hint="The list remains URL-backed so a filtered VM view can be shared between operators."
		/>
	{:else}
		<ResourceTable {columns} {rows} {rowHref} emptyTitle="No virtual machines">
			{#snippet actionCell(row)}
				<form
					method="POST"
					use:enhance={() => {
						return async ({ update }) => {
							await update();
						};
					}}
					class="action-form"
				>
					<input type="hidden" name="vm_id" value={row.vm_id as string} />
					<select name="action" class="action-select" required>
						<option value="" disabled selected>Action</option>
						<option value="start">Start</option>
						<option value="stop">Stop</option>
						<option value="restart">Restart</option>
					</select>
					<button type="submit" class="action-button">Run</button>
				</form>
			{/snippet}
		</ResourceTable>
	{/if}
</PageShell>

<style>
	.action-form {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}

	.action-select {
		min-height: 2rem;
		padding: 0.35rem 0.5rem;
		border-radius: 0.6rem;
		border: 1px solid var(--shell-line-strong);
		background: var(--shell-surface-muted);
		color: var(--shell-text);
		font-size: 0.85rem;
	}

	.action-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2rem;
		padding: 0 0.75rem;
		border-radius: 999px;
		font-size: 0.85rem;
		font-weight: 600;
		border: 1px solid transparent;
		background: var(--shell-accent);
		color: #fff9f2;
		cursor: pointer;
	}
</style>
