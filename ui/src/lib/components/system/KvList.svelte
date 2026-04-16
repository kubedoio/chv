<script lang="ts">
	interface Item {
		label: string;
		value: string;
	}

	interface Props {
		items: Item[];
	}

	let { items }: Props = $props();
</script>

<div class="kv-list">
	{#each items as item}
		<div class="kv-row">
			<div class="kv-row__label">{item.label}</div>
			<div class="kv-row__value">{item.value}</div>
		</div>
	{/each}
</div>

<style>
	.kv-list {
		display: grid;
		gap: 0.65rem;
	}

	.kv-row {
		display: grid;
		grid-template-columns: minmax(10rem, 0.75fr) minmax(0, 1fr);
		gap: 0.9rem;
		padding-bottom: 0.65rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
	}

	.kv-row__label {
		color: var(--shell-text-muted);
	}

	.kv-row__value {
		color: var(--shell-text);
	}
</style>
