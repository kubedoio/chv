<script lang="ts">
	interface Props {
		label: string;
		tone: 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown';
	}

	let { label, tone }: Props = $props();

	const toneClass = $derived(`badge--${tone}`);
</script>

<span class="badge {toneClass}">{label}</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		border: 1px solid var(--shell-line-strong);
		border-radius: 999px;
		padding: 0.28rem 0.7rem;
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		white-space: nowrap;
	}

	.badge--healthy {
		background: var(--status-healthy-bg);
		border-color: var(--status-healthy-border);
		color: var(--status-healthy-text);
	}

	.badge--warning {
		background: var(--status-warning-bg);
		border-color: var(--status-warning-border);
		color: var(--status-warning-text);
	}

	.badge--degraded {
		background: var(--status-degraded-bg);
		border-color: var(--status-degraded-border);
		color: var(--status-degraded-text);
	}

	.badge--failed {
		background: var(--status-failed-bg);
		border-color: var(--status-failed-border);
		color: var(--status-failed-text);
	}

	.badge--unknown {
		background: var(--status-unknown-bg);
		border-color: var(--status-unknown-border);
		color: var(--status-unknown-text);
	}
</style>
