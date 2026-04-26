<script lang="ts">
	interface Props {
		label: string;
		tone: 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown';
	}

	let { label, tone }: Props = $props();

	const toneClasses: Record<string, string> = {
		healthy: 'bg-[var(--status-healthy-bg)] border-[var(--status-healthy-border)] text-[var(--status-healthy-text)]',
		warning: 'bg-[var(--status-warning-bg)] border-[var(--status-warning-border)] text-[var(--status-warning-text)]',
		degraded: 'bg-[var(--status-degraded-bg)] border-[var(--status-degraded-border)] text-[var(--status-degraded-text)]',
		failed: 'bg-[var(--status-failed-bg)] border-[var(--status-failed-border)] text-[var(--status-failed-text)]',
		unknown: 'bg-[var(--status-unknown-bg)] border-[var(--status-unknown-border)] text-[var(--status-unknown-text)]'
	};
</script>

<span class="inline-flex items-center border rounded-full px-[0.7rem] py-[0.28rem] text-[0.7rem] font-semibold uppercase tracking-wider whitespace-nowrap {toneClasses[tone]}">{label}</span>
