<script lang="ts">
	import StatusBadge from './StatusBadge.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { ExternalLink } from 'lucide-svelte';

	interface Task {
		task_id: string;
		summary: string;
		status: string;
		operation: string;
		started_at?: string;
		tone: ShellTone;
	}

	interface Props {
		tasks: Task[];
	}

	let { tasks }: Props = $props();
</script>

<div class="task-timeline">
	{#if tasks.length === 0}
		<p class="empty-hint">No recent tasks recorded for this resource.</p>
	{:else}
		{#each tasks as task}
			<div class="task-entry">
				<div class="task-entry__main">
					<div class="task-title-row">
						<span class="task-summary">{task.summary}</span>
						<StatusBadge label={task.status} tone={task.tone} />
					</div>
					<div class="task-meta">
						<span class="operation">{task.operation}</span>
						{#if task.started_at}
							<span class="separator">·</span>
							<span class="timestamp">{task.started_at}</span>
						{/if}
					</div>
				</div>
				<a href="/tasks?query={task.task_id}" class="task-link" title="View detail">
					<ExternalLink size={14} />
				</a>
			</div>
		{/each}
	{/if}
</div>

<style>
	.task-timeline {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		padding: 0.5rem 0;
		text-align: center;
	}

	.task-entry {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.25rem;
		gap: 1rem;
	}

	.task-entry__main {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		min-width: 0;
		flex: 1;
	}

	.task-title-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		justify-content: space-between;
	}

	.task-summary {
		font-weight: 600;
		font-size: var(--text-sm);
		color: var(--shell-text);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.task-meta {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.separator {
		opacity: 0.5;
	}

	.task-link {
		color: var(--shell-text-muted);
		transition: color 0.15s ease;
		display: flex;
		padding: 0.25rem;
	}

	.task-link:hover {
		color: var(--shell-accent);
	}
</style>
