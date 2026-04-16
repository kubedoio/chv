<script lang="ts">
	import { enhance } from '$app/forms';
	import { Link2, Unlink, Maximize2 } from 'lucide-svelte';
	import { PageShell, StateBanner, Badge } from '$lib/components/system';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import TaskReferenceCallout from '$lib/components/webui/TaskReferenceCallout.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import Button from '$lib/components/primitives/Button.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData, ActionData } from './$types';
	import type { MutationActionResult } from '$lib/bff/types';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import { mapRelatedTask } from '$lib/webui/task-helpers';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	const page = getPageDefinition('/volumes');
	const detail = $derived(data.detail);

	type VolumeAction = 'attach' | 'detach' | 'resize';
	let pendingAction = $state<VolumeAction | null>(null);
	let confirmingAction = $state<VolumeAction | null>(null);
	let actionInput = $state<HTMLInputElement | null>(null);

	function toStatusTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (s === 'attached' || s === 'ready') return 'healthy';
		if (s === 'attaching' || s === 'detaching') return 'warning';
		if (s === 'failed' || s === 'error') return 'failed';
		return 'unknown';
	}

	function toHealthTone(health: string): ShellTone {
		const h = health.toLowerCase();
		if (h === 'healthy') return 'healthy';
		if (h === 'degraded') return 'degraded';
		if (h === 'warning') return 'warning';
		if (h === 'failed') return 'failed';
		return 'unknown';
	}

	const mutationResult = $derived<MutationActionResult | null>(
		form && typeof form === 'object' && 'accepted' in form && form.accepted === true
			? {
					accepted: true,
					action: (form as unknown as { action: string }).action,
					summary: (form as unknown as { summary: string }).summary,
					taskId: (form as unknown as { task_id: string | undefined }).task_id ?? null,
					taskLabel: (form as unknown as { task_id: string | undefined }).task_id
						? getTaskStatusMeta('queued').label
						: getTaskStatusMeta('failed').label,
					taskTone: (form as unknown as { task_id: string | undefined }).task_id
						? getTaskStatusMeta('queued').tone
						: getTaskStatusMeta('failed').tone,
					taskHref: (form as unknown as { task_id: string | undefined }).task_id
						? `/tasks?query=${(form as unknown as { task_id: string }).task_id}`
						: null
				}
			: null
	);

	function isDestructive(action: VolumeAction): boolean {
		return action === 'detach' || action === 'resize';
	}

	function handleActionClick(action: VolumeAction) {
		if (isDestructive(action)) {
			confirmingAction = action;
			pendingAction = null;
		} else {
			confirmingAction = null;
			pendingAction = action;
			submitAction(action);
		}
	}

	function submitAction(action: VolumeAction) {
		confirmingAction = null;
		if (actionInput) {
			actionInput.value = action;
		}
		actionInput?.form?.requestSubmit();
	}

	function cancelConfirmation() {
		confirmingAction = null;
		pendingAction = null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if detail.state !== 'ready'}
		<StateBanner
			variant={detail.state === 'error' ? 'error' : 'empty'}
			title={detail.state === 'error' ? 'Volume detail unavailable' : 'Volume not found'}
			description="The volume summary, configuration, and task context could not be assembled from the current control-plane responses."
			hint="Keep the shell active and retry once the view model becomes available again."
		/>
	{:else}
		<article class="detail-page__hero">
			<div>
				<div class="detail-page__eyebrow">{detail.summary.nodeId}</div>
				<h1>{detail.summary.name}</h1>
				<p>Volume ID: {detail.summary.volumeId}</p>
			</div>
			<div class="detail-page__hero-side">
				<div class="detail-page__hero-badges">
					<Badge label={detail.summary.status} tone={toStatusTone(detail.summary.status)} />
					<Badge label={detail.summary.health} tone={toHealthTone(detail.summary.health)} />
				</div>
				<form
					method="POST"
					use:enhance={() => {
						return async ({ update }) => {
							pendingAction = null;
							confirmingAction = null;
							await update();
						};
					}}
					class="detail-page__action-row"
				>
					<input type="hidden" name="volume_id" value={detail.summary.volumeId} />
					<input type="hidden" name="action" bind:this={actionInput} value="" />
					<Button
						variant="primary"
						size="sm"
						disabled={!!detail.summary.attachedVmId}
						loading={pendingAction === 'attach'}
						onclick={() => handleActionClick('attach')}
						type="button"
					>
						<Link2 size={14} />
						Attach
					</Button>
					<Button
						variant="secondary"
						size="sm"
						disabled={!detail.summary.attachedVmId}
						loading={pendingAction === 'detach'}
						onclick={() => handleActionClick('detach')}
						type="button"
					>
						<Unlink size={14} />
						Detach
					</Button>
					<Button
						variant="secondary"
						size="sm"
						disabled={false}
						loading={pendingAction === 'resize'}
						onclick={() => handleActionClick('resize')}
						type="button"
					>
						<Maximize2 size={14} />
						Resize
					</Button>
				</form>
				{#if confirmingAction}
					{@const action = confirmingAction}
					<div class="detail-page__confirm-bar">
						<span>
							Are you sure you want to <strong>{confirmingAction}</strong> {detail.summary.name}?
						</span>
						<div class="detail-page__confirm-actions">
							<Button variant="danger" size="sm" onclick={() => submitAction(action)}>
								Confirm {action}
							</Button>
							<Button variant="ghost" size="sm" onclick={cancelConfirmation}>Cancel</Button>
						</div>
					</div>
				{/if}
			</div>
		</article>

		{#if mutationResult}
			<TaskReferenceCallout result={mutationResult} />
		{/if}

		<div class="detail-page__summary-grid">
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Size</div>
				<div class="detail-page__summary-value">{detail.summary.size}</div>
				<p>Allocated capacity</p>
			</article>
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Status</div>
				<div class="detail-page__summary-value">{detail.summary.status}</div>
				<p>Current attachment status</p>
				<Badge label={detail.summary.status} tone={toStatusTone(detail.summary.status)} />
			</article>
			<article class="detail-page__summary-card">
				<div class="detail-page__eyebrow">Attached VM</div>
				<div class="detail-page__summary-value">{detail.summary.attachedVmName || detail.summary.attachedVmId || '-'}</div>
				<p>Workload attachment</p>
			</article>
		</div>

		<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

		{#if detail.currentTab === 'summary'}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Summary</div>
				<h2>Volume overview</h2>
				<div class="detail-page__kv-list">
					<div class="detail-page__kv-row"><div>Volume ID</div><div>{detail.summary.volumeId}</div></div>
					<div class="detail-page__kv-row"><div>Name</div><div>{detail.summary.name}</div></div>
					<div class="detail-page__kv-row"><div>Node</div><div>{detail.summary.nodeId}</div></div>
					<div class="detail-page__kv-row"><div>Size</div><div>{detail.summary.size}</div></div>
					<div class="detail-page__kv-row"><div>Status</div><div>{detail.summary.status}</div></div>
					<div class="detail-page__kv-row"><div>Health</div><div>{detail.summary.health}</div></div>
					<div class="detail-page__kv-row"><div>Attached VM</div><div>{detail.summary.attachedVmName || detail.summary.attachedVmId || '-'}</div></div>
				</div>
			</article>
		{:else if detail.currentTab === 'tasks'}
			<div class="detail-page__stack">
				{#if detail.recentTasks.length > 0}
					{#each detail.recentTasks as task}
						<TaskTimelineItem task={mapRelatedTask(task, detail.summary.volumeId, 'volume')} compact />
					{/each}
				{:else}
					<StateBanner
						variant="empty"
						title="No related volume tasks"
						description="Volume mutations and lifecycle actions will appear here once the control plane persists them."
						hint="The volume list remains wired to produce task references after mutations."
					/>
				{/if}
			</div>
		{:else}
			<article class="detail-page__panel">
				<div class="detail-page__eyebrow">Configuration</div>
				<h2>Volume configuration</h2>
				<div class="detail-page__kv-list">
					{#each detail.configuration as item}
						<div class="detail-page__kv-row">
							<div>{item.label}</div>
							<div>{item.value}</div>
						</div>
					{/each}
				</div>
			</article>
		{/if}
	{/if}
</PageShell>

<style>
	.detail-page__hero,
	.detail-page__summary-card,
	.detail-page__panel {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.detail-page__hero {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.detail-page__eyebrow {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	h1 {
		margin-top: 0.25rem;
		font-size: 2rem;
		color: var(--shell-text);
	}

	h2 {
		font-size: 1.2rem;
		color: var(--shell-text);
	}

	.detail-page__hero p,
	.detail-page__summary-card p {
		margin-top: 0.35rem;
		color: var(--shell-text-secondary);
		line-height: 1.5;
	}

	.detail-page__hero-badges,
	.detail-page__action-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	.detail-page__hero-side {
		display: grid;
		gap: 0.75rem;
	}

	.detail-page__summary-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1rem;
	}

	.detail-page__summary-value {
		margin-top: 0.8rem;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.detail-page__stack {
		display: grid;
		gap: 0.8rem;
	}

	.detail-page__kv-list {
		display: grid;
		gap: 0.65rem;
		margin-top: 0.9rem;
	}

	.detail-page__kv-row {
		display: grid;
		grid-template-columns: minmax(10rem, 0.75fr) minmax(0, 1fr);
		gap: 0.9rem;
		padding-bottom: 0.65rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
	}

	.detail-page__kv-row div:first-child {
		color: var(--shell-text-muted);
	}

	.detail-page__confirm-bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.85rem 1rem;
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface-muted);
	}

	.detail-page__confirm-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	@media (max-width: 1100px) {
		.detail-page__summary-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
