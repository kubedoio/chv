<script lang="ts">
	import type { ShellTone } from '$lib/shell/app-shell';

	interface Props {
		label: string;
		detail?: string;
		tone: ShellTone;
		compact?: boolean;
	}

	let { label, detail, tone, compact = false }: Props = $props();
	const toneClass = $derived(`task-status-badge--${tone}`);
</script>

<span class={`task-status-badge ${toneClass} ${compact ? 'task-status-badge--compact' : ''}`}>
	<span class="task-status-badge__dot" aria-hidden="true"></span>
	<span class="task-status-badge__content">
		<span class="task-status-badge__label">{label}</span>
		{#if detail && !compact}
			<span class="task-status-badge__detail">{detail}</span>
		{/if}
	</span>
</span>

<style>
	.task-status-badge {
		display: inline-flex;
		align-items: center;
		gap: 0.55rem;
		padding: 0.45rem 0.7rem;
		border-radius: 999px;
		border: 1px solid var(--shell-line-strong);
		font-size: 0.75rem;
		line-height: 1;
		white-space: nowrap;
	}

	.task-status-badge--compact {
		padding-right: 0.6rem;
	}

	.task-status-badge__dot {
		width: 0.45rem;
		height: 0.45rem;
		border-radius: 999px;
		background: currentColor;
		flex: 0 0 auto;
	}

	.task-status-badge__content {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
	}

	.task-status-badge__label {
		font-weight: 700;
		letter-spacing: 0.03em;
	}

	.task-status-badge__detail {
		color: var(--shell-text-secondary);
		font-size: 0.72rem;
	}

	.task-status-badge--healthy {
		background: var(--status-healthy-bg);
		border-color: var(--status-healthy-border);
		color: var(--status-healthy-text);
	}

	.task-status-badge--warning {
		background: var(--status-warning-bg);
		border-color: var(--status-warning-border);
		color: var(--status-warning-text);
	}

	.task-status-badge--degraded {
		background: var(--status-degraded-bg);
		border-color: var(--status-degraded-border);
		color: var(--status-degraded-text);
	}

	.task-status-badge--failed {
		background: var(--status-failed-bg);
		border-color: var(--status-failed-border);
		color: var(--status-failed-text);
	}

	.task-status-badge--unknown {
		background: var(--status-unknown-bg);
		border-color: var(--status-unknown-border);
		color: var(--status-unknown-text);
	}
</style>
