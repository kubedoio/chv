<script lang="ts">
	import TaskStatusBadge from '$lib/components/webui/TaskStatusBadge.svelte';
	import type { VmLifecycleActionResult } from '$lib/webui/vm-actions';

	interface Props {
		result: VmLifecycleActionResult;
	}

	let { result }: Props = $props();
</script>

<article class="task-reference-callout">
	<div class="task-reference-callout__topline">
		<div class="task-reference-callout__eyebrow">Lifecycle task</div>
		<TaskStatusBadge label={result.taskLabel} tone={result.taskTone} compact />
	</div>
	<h3>{result.summary}</h3>
	<div class="task-reference-callout__meta">
		<span>Action: {result.action}</span>
		{#if result.taskId}
			<span>Task: {result.taskId}</span>
		{/if}
		{#if result.taskHref}
			<a href={result.taskHref}>Open in task center</a>
		{/if}
	</div>
</article>

<style>
	.task-reference-callout {
		display: grid;
		gap: 0.75rem;
		padding: 1rem;
		border-radius: 1rem;
		border: 1px solid var(--shell-line);
		background: linear-gradient(135deg, rgba(252, 249, 244, 0.98), rgba(244, 238, 228, 0.96));
	}

	.task-reference-callout__topline {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.task-reference-callout__eyebrow {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	h3 {
		font-size: 1rem;
		line-height: 1.45;
		color: var(--shell-text);
	}

	.task-reference-callout__meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem 0.9rem;
		font-size: 0.85rem;
		color: var(--shell-text-secondary);
	}

	.task-reference-callout__meta a {
		color: var(--shell-accent);
		text-decoration: none;
		font-weight: 600;
	}

	.task-reference-callout__meta a:hover {
		text-decoration: underline;
	}
</style>
