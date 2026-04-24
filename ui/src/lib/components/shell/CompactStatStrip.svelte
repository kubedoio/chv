<script lang="ts">
	interface Stat {
		label: string;
		value: number | string;
		status?: 'healthy' | 'warning' | 'critical' | 'neutral';
	}

	interface Props {
		stats: Stat[];
	}

	let { stats }: Props = $props();

	const statusClasses: Record<string, string> = {
		healthy: 'text-[var(--color-success)]',
		warning: 'text-[var(--color-warning)]',
		critical: 'text-[var(--color-danger)]',
		neutral: 'text-[var(--shell-text)]'
	};
</script>

<div class="flex gap-6 px-4 py-2 bg-[var(--shell-surface)] border border-[var(--shell-line)] rounded-[0.5rem]">
	{#each stats as stat}
		<div class="flex flex-col gap-[0.125rem]">
			<div class="text-[length:var(--text-xs)] font-semibold uppercase tracking-[0.05em] text-[var(--shell-text-muted)]">{stat.label}</div>
			<div class="text-[length:var(--text-lg)] font-semibold tabular-nums {statusClasses[stat.status ?? 'neutral']}">{stat.value}</div>
		</div>
	{/each}
</div>
