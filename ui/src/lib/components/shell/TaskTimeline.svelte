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
		emptyText?: string;
	}

	let { tasks, emptyText }: Props = $props();
</script>

<div class="flex flex-col gap-2">
	{#if tasks.length === 0}
		<p class="text-[length:var(--text-xs)] text-[var(--shell-text-muted)] py-2 text-center">{emptyText ?? 'No recent tasks recorded for this resource.'}</p>
	{:else}
		{#each tasks as task}
			<div class="flex justify-between items-center p-2 bg-[var(--shell-surface-muted)] border border-[var(--shell-line)] rounded-[0.25rem] gap-4">
				<div class="flex flex-col gap-[0.15rem] min-w-0 flex-1">
					<div class="flex items-center gap-2 justify-between">
						<span class="font-semibold text-[length:var(--text-sm)] text-[var(--shell-text)] whitespace-nowrap overflow-hidden text-ellipsis">{task.summary}</span>
						<StatusBadge label={task.status} tone={task.tone} />
					</div>
					<div class="flex items-center gap-[0.35rem] text-[length:var(--text-xs)] text-[var(--shell-text-muted)]">
						<span>{task.operation}</span>
						{#if task.started_at}
							<span class="opacity-50">·</span>
							<span>{task.started_at}</span>
						{/if}
					</div>
				</div>
				<a href="/tasks?query={task.task_id}" class="text-[var(--shell-text-muted)] transition-colors duration-150 ease-in-out flex p-1 hover:text-[var(--shell-accent)]" title="View detail">
					<ExternalLink size={14} />
				</a>
			</div>
		{/each}
	{/if}
</div>
