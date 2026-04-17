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

	function getStatusClass(status?: string) {
		switch (status) {
			case 'healthy': return 'stat-value--healthy';
			case 'warning': return 'stat-value--warning';
			case 'critical': return 'stat-value--critical';
			default: return 'stat-value--neutral';
		}
	}
</script>

<div class="compact-stat-strip">
	{#each stats as stat}
		<div class="stat-item">
			<div class="stat-label">{stat.label}</div>
			<div class="stat-value {getStatusClass(stat.status)}">{stat.value}</div>
		</div>
	{/each}
</div>

<style>
	.compact-stat-strip {
		display: flex;
		gap: 1.5rem;
		padding: 0.5rem 1rem;
		background: var(--shell-surface);
		border: 1px solid var(--shell-line);
		border-radius: 0.5rem;
	}

	.stat-item {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.stat-label {
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
	}

	.stat-value {
		font-size: var(--text-lg);
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		color: var(--shell-text);
	}

	.stat-value--healthy { color: var(--color-success); }
	.stat-value--warning { color: var(--color-warning); }
	.stat-value--critical { color: var(--color-danger); }
	.stat-value--neutral { color: var(--shell-text); }
</style>
