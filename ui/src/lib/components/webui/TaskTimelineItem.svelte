<script lang="ts">
	import {
		AlertTriangle,
		CheckCircle2,
		CircleHelp,
		Clock3,
		LoaderCircle,
		XCircle
	} from 'lucide-svelte';
	import TaskStatusBadge from '$lib/components/webui/TaskStatusBadge.svelte';
	import type { TaskTimelineItemModel } from '$lib/webui/tasks';

	interface Props {
		task: TaskTimelineItemModel;
		compact?: boolean;
	}

	let { task, compact = false }: Props = $props();

	const icon = $derived(
		task.tone === 'healthy'
			? CheckCircle2
			: task.tone === 'failed'
				? XCircle
				: task.tone === 'warning'
					? Clock3
					: task.tone === 'degraded'
						? LoaderCircle
						: CircleHelp
	);
</script>

<article class={`task-timeline-item ${compact ? 'task-timeline-item--compact' : ''}`}>
	<div class={`task-timeline-item__icon task-timeline-item__icon--${task.tone}`} aria-hidden="true">
		<icon
			size={16}
			class={task.tone === 'degraded' ? 'task-timeline-item__icon--spinning' : undefined}
		></icon>
	</div>

	<div class="task-timeline-item__content">
		<div class="task-timeline-item__topline">
			<TaskStatusBadge label={task.label} detail={task.detail} tone={task.tone} compact={compact} />
			<span class="task-timeline-item__duration">{task.durationLabel}</span>
		</div>

		<h3>{task.summary}</h3>
		<p class="task-timeline-item__subtitle">{task.timelineDetail}</p>

		<div class="task-timeline-item__meta">
			<span>{task.operation}</span>
			<span>{task.actor}</span>
			<span>Started {task.startedAtLabel}</span>
			{#if task.finishedAtLabel}
				<span>Finished {task.finishedAtLabel}</span>
			{/if}
		</div>

		{#if task.failureReason}
			<p class="task-timeline-item__failure">
				<AlertTriangle size={14} />
				<span>{task.failureReason}</span>
			</p>
		{/if}
	</div>
</article>

<style>
	.task-timeline-item {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.9rem;
		padding: 1rem 1.05rem;
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface);
	}

	.task-timeline-item--compact {
		padding: 0.85rem 0.9rem;
	}

	.task-timeline-item__icon {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: 999px;
		border: 1px solid var(--shell-line);
	}

	.task-timeline-item__icon--healthy {
		background: var(--status-healthy-bg);
		color: var(--status-healthy-text);
	}

	.task-timeline-item__icon--warning {
		background: var(--status-warning-bg);
		color: var(--status-warning-text);
	}

	.task-timeline-item__icon--degraded {
		background: var(--status-degraded-bg);
		color: var(--status-degraded-text);
	}

	.task-timeline-item__icon--failed {
		background: var(--status-failed-bg);
		color: var(--status-failed-text);
	}

	.task-timeline-item__icon--unknown {
		background: var(--status-unknown-bg);
		color: var(--status-unknown-text);
	}

	.task-timeline-item__icon--spinning {
		animation: task-spin 1.2s linear infinite;
	}

	.task-timeline-item__content {
		display: grid;
		gap: 0.55rem;
		min-width: 0;
	}

	.task-timeline-item__topline {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.65rem;
	}

	.task-timeline-item__duration {
		font-size: 0.8rem;
		font-weight: 600;
		color: var(--shell-text-muted);
	}

	h3 {
		font-size: 1rem;
		line-height: 1.35;
		color: var(--shell-text);
	}

	.task-timeline-item__subtitle {
		font-size: 0.9rem;
		color: var(--shell-text-secondary);
	}

	.task-timeline-item__meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.45rem 0.8rem;
		font-size: 0.8rem;
		color: var(--shell-text-muted);
	}

	.task-timeline-item__failure {
		display: inline-flex;
		align-items: flex-start;
		gap: 0.45rem;
		padding: 0.7rem 0.8rem;
		border-radius: 0.85rem;
		background: var(--status-failed-bg);
		color: var(--status-failed-text);
		font-size: 0.85rem;
		line-height: 1.45;
	}

	@keyframes task-spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.task-timeline-item__icon--spinning {
			animation: none;
		}
	}
</style>
