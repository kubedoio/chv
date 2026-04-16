<script lang="ts">
	interface Chip {
		label: string;
		value: string | number;
		variant?: 'default' | 'healthy' | 'warning' | 'degraded' | 'failed';
	}

	interface Props {
		chips: Chip[];
	}

	let { chips }: Props = $props();
</script>

<div class="posture-strip" role="list" aria-label="Fleet posture">
	{#each chips as chip}
		<div class="posture-strip__chip posture-strip__chip--{chip.variant ?? 'default'}" role="listitem">
			<span class="posture-strip__label">{chip.label}</span>
			<span class="posture-strip__value">{chip.value}</span>
		</div>
	{/each}
</div>

<style>
	.posture-strip {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.posture-strip__chip {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		border: 1px solid var(--shell-line);
		border-radius: 999px;
		background: var(--shell-surface);
		padding: 0.35rem 0.8rem;
		font-size: 0.8rem;
	}

	.posture-strip__label {
		color: var(--shell-text-muted);
	}

	.posture-strip__value {
		font-weight: 700;
		color: var(--shell-text);
	}

	.posture-strip__chip--healthy .posture-strip__value {
		color: var(--status-healthy-text);
	}

	.posture-strip__chip--warning .posture-strip__value {
		color: var(--status-warning-text);
	}

	.posture-strip__chip--degraded .posture-strip__value,
	.posture-strip__chip--failed .posture-strip__value {
		color: var(--status-failed-text);
	}
</style>
