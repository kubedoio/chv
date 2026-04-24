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

<div class="grid gap-[0.65rem]">
	{#each items as item}
		<div class="grid gap-[0.9rem] pb-[0.65rem] border-b border-[var(--shell-line)] text-[0.92rem]" style="grid-template-columns: minmax(10rem, 0.75fr) minmax(0, 1fr);">
			<div class="text-[var(--shell-text-muted)]">{item.label}</div>
			<div class="text-[var(--shell-text)]">{item.value}</div>
		</div>
	{/each}
</div>
