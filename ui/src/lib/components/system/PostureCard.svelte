<script lang="ts">
	interface Props {
		label: string;
		value: string | number;
		note?: string;
	}

	let { label, value, note }: Props = $props();
</script>

<article class="posture-card">
	<div class="posture-card__label">{label}</div>
	<div class="posture-card__value">{value}</div>
	{#if note}
		<p class="posture-card__note">{note}</p>
	{/if}
</article>

<style>
	.posture-card {
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.posture-card__label {
		font-size: 0.7rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.posture-card__value {
		margin-top: 0.35rem;
		font-size: 1.15rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.posture-card__note {
		margin: 0.25rem 0 0 0;
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--shell-text-secondary);
	}
</style>
