<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onMount } from 'svelte';
	import SectionHeader from '$lib/components/shell/SectionHeader.svelte';
	import StatePanel from '$lib/components/shell/StatePanel.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import TaskStatusBadge from '$lib/components/webui/TaskStatusBadge.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import { getStoredToken } from '$lib/api/client';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { getTaskStatusMeta } from '$lib/webui/tasks';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/tasks');
	const tasks = $derived(data.tasks);
	const statusLegend = [
		getTaskStatusMeta('queued'),
		getTaskStatusMeta('running'),
		getTaskStatusMeta('succeeded'),
		getTaskStatusMeta('failed'),
		getTaskStatusMeta('cancelled'),
		getTaskStatusMeta('unknown')
	];
	const hasAppliedFilters = $derived(Object.keys(tasks.filters.applied).length > 0);

	onMount(() => {
		if (data.meta.clientRefreshRecommended && getStoredToken()) {
			invalidate('webui:tasks');
		}
	});
</script>

<div class="tasks-page">
	<SectionHeader {page} />

	<div class="tasks-page__legend">
		{#each statusLegend as status}
			<TaskStatusBadge label={status.label} detail={status.detail} tone={status.tone} />
		{/each}
	</div>

	<form class="tasks-page__filters" method="GET">
		<label class="tasks-page__field">
			<span>Search</span>
			<input
				type="search"
				name="query"
				value={tasks.filters.current.query}
				placeholder="Task id, resource, operation, or failure text"
			/>
		</label>

		<label class="tasks-page__field">
			<span>Status</span>
			<select name="status">
				<option value="all" selected={tasks.filters.current.status === 'all'}>All states</option>
				{#each tasks.filters.options.statuses as status}
					<option value={status} selected={tasks.filters.current.status === status}>
						{getTaskStatusMeta(status).label}
					</option>
				{/each}
			</select>
		</label>

		<label class="tasks-page__field">
			<span>Resource</span>
			<select name="resourceKind">
				<option value="all" selected={tasks.filters.current.resourceKind === 'all'}>
					All resources
				</option>
				{#each tasks.filters.options.resourceKinds as resourceKind}
					<option
						value={resourceKind}
						selected={tasks.filters.current.resourceKind === resourceKind}
					>
						{resourceKind}
					</option>
				{/each}
			</select>
		</label>

		<label class="tasks-page__field">
			<span>Window</span>
			<select name="window">
				<option value="active" selected={tasks.filters.current.window === 'active'}>
					Active only
				</option>
				<option value="24h" selected={tasks.filters.current.window === '24h'}>
					Last 24 hours
				</option>
				<option value="7d" selected={tasks.filters.current.window === '7d'}>
					Last 7 days
				</option>
				<option value="30d" selected={tasks.filters.current.window === '30d'}>
					Last 30 days
				</option>
				<option value="all" selected={tasks.filters.current.window === 'all'}>All time</option>
			</select>
		</label>

		<div class="tasks-page__actions">
			<button type="submit">Apply filters</button>
			<a href="/tasks">Reset</a>
		</div>
	</form>

	<div class="tasks-page__summary">
		<div>
			<div class="tasks-page__summary-label">Visible tasks</div>
			<div class="tasks-page__summary-value">{tasks.page.totalItems}</div>
		</div>
		<div class="tasks-page__summary-badges">
			{#if data.meta.partial}
				<StatusBadge label="partial data" tone="warning" />
			{/if}
			{#if hasAppliedFilters}
				<StatusBadge label="filters applied" tone="unknown" />
			{/if}
		</div>
	</div>

	{#if data.meta.deferred}
		<StatePanel
			variant="loading"
			title="Loading task history"
			description="This route waits for the client-authenticated pass before loading protected task data."
			hint="The task center will refresh once the browser rehydrates with the stored session token."
		/>
	{:else if tasks.state === 'error'}
		<StatePanel
			variant="error"
			title="Task center is unavailable"
			description="The task list could not be shaped from the current API responses."
			hint="Accepted, running, and completed work stays distinct once the task feed is reachable again."
		/>
	{:else if tasks.state === 'empty'}
		<StatePanel
			variant="empty"
			title={hasAppliedFilters ? 'No tasks match the current filters' : 'No tasks yet'}
			description={hasAppliedFilters
				? 'Try widening the status, resource, or time window filters to bring more task history back into view.'
				: 'Accepted operations, active runs, and completed work will appear here once the control plane starts producing task records.'}
			hint="The task center keeps filter state even when the result set is empty."
		/>
	{:else}
		<section class="tasks-page__list" aria-label="Task timeline">
			{#each tasks.items as task}
				<TaskTimelineItem {task} />
			{/each}
		</section>
	{/if}
</div>

<style>
	.tasks-page {
		display: grid;
		gap: 1.25rem;
	}

	.tasks-page__legend,
	.tasks-page__filters,
	.tasks-page__summary,
	.tasks-page__list {
		display: grid;
	}

	.tasks-page__legend,
	.tasks-page__filters,
	.tasks-page__summary {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.tasks-page__legend {
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.8rem;
		padding: 1rem;
	}

	.tasks-page__filters {
		grid-template-columns: minmax(0, 1.4fr) repeat(3, minmax(0, 0.8fr)) auto;
		gap: 0.85rem;
		padding: 1rem;
		align-items: end;
	}

	.tasks-page__field {
		display: grid;
		gap: 0.38rem;
	}

	.tasks-page__field span,
	.tasks-page__summary-label {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.tasks-page__field input,
	.tasks-page__field select {
		width: 100%;
		min-height: 2.75rem;
		border-radius: 0.85rem;
		border: 1px solid var(--shell-line-strong);
		background: var(--shell-surface-muted);
		padding: 0.7rem 0.8rem;
		color: var(--shell-text);
	}

	.tasks-page__actions {
		display: flex;
		align-items: center;
		gap: 0.7rem;
		padding-bottom: 0.05rem;
	}

	.tasks-page__actions button,
	.tasks-page__actions a {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.75rem;
		padding: 0 1rem;
		border-radius: 999px;
		font-size: 0.9rem;
		font-weight: 600;
		text-decoration: none;
	}

	.tasks-page__actions button {
		border: 1px solid transparent;
		background: var(--shell-accent);
		color: #fff9f2;
		cursor: pointer;
	}

	.tasks-page__actions a {
		color: var(--shell-text-secondary);
	}

	.tasks-page__summary {
		grid-template-columns: auto 1fr;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.95rem 1rem;
	}

	.tasks-page__summary-value {
		margin-top: 0.25rem;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.tasks-page__summary-badges {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.6rem;
	}

	.tasks-page__list {
		gap: 0.85rem;
	}

	@media (max-width: 1200px) {
		.tasks-page__legend {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.tasks-page__filters {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 720px) {
		.tasks-page__legend,
		.tasks-page__filters,
		.tasks-page__summary {
			grid-template-columns: 1fr;
		}

		.tasks-page__actions,
		.tasks-page__summary-badges {
			justify-content: flex-start;
		}
	}
</style>
