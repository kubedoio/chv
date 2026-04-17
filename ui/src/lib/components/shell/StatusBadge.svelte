<script lang="ts">
	import type { ShellTone } from '$lib/shell/app-shell';

	interface Props {
		label: string;
		tone: ShellTone;
	}

	let { label, tone }: Props = $props();

	const toneClass = $derived(`status-badge--${tone}`);
</script>

<span class={`status-badge ${toneClass}`}>{label}</span>

<style>
	.status-badge {
		display: inline-flex;
		align-items: center;
		border: 1px solid var(--shell-line-strong);
		border-radius: 999px;
		padding: 0.15rem 0.5rem;
		font-size: var(--text-xs);
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		white-space: nowrap;
	}

	.status-badge--healthy {
		background: var(--status-healthy-bg);
		border-color: var(--status-healthy-border);
		color: var(--status-healthy-text);
	}

	.status-badge--warning {
		background: var(--status-warning-bg);
		border-color: var(--status-warning-border);
		color: var(--status-warning-text);
	}

	.status-badge--degraded {
		background: var(--status-degraded-bg);
		border-color: var(--status-degraded-border);
		color: var(--status-degraded-text);
	}

	.status-badge--failed {
		background: var(--status-failed-bg);
		border-color: var(--status-failed-border);
		color: var(--status-failed-text);
	}

	.status-badge--unknown {
		background: var(--status-unknown-bg);
		border-color: var(--status-unknown-border);
		color: var(--status-unknown-text);
	}
</style>
