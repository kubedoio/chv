<script lang="ts">
	interface Props {
		label: string;
		value: string | number;
		note?: string;
	}

	let { label, value, note }: Props = $props();
</script>

<article class="border border-[var(--shell-line)] rounded-[1rem] bg-[var(--shell-surface)] p-4">
	<div class="text-[0.7rem] font-bold tracking-[0.1em] uppercase text-[var(--shell-text-muted)]">{label}</div>
	<div class="mt-[0.35rem] text-[1.15rem] font-bold text-[var(--shell-text)]">{value}</div>
	{#if note}
		<p class="mt-1 text-[0.85rem] leading-[1.45] text-[var(--shell-text-secondary)]">{note}</p>
	{/if}
</article>
