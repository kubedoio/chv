<script lang="ts">
	interface Props {
		columns?: number;
	}

	let { columns = 4 }: Props = $props();

	// Generate array for columns
	const columnArray = $derived(Array.from({ length: columns }, (_, i) => i));

	// Vary the width of each column for natural look
	function getColumnWidth(index: number): string {
		if (index === 0) return '60%';
		if (index === columns - 1) return '40%';
		if (index % 2 === 0) return '70%';
		return '85%';
	}
</script>

<tr class="skeleton-pulse">
	{#each columnArray as i}
		<td class="border-b border-line px-4 py-3">
			<div
				class="h-4 rounded bg-gradient-to-r from-gray-200 via-gray-100 to-gray-200"
				style="width: {getColumnWidth(i)}"
			></div>
		</td>
	{/each}
</tr>

<style>
	.skeleton-pulse {
		animation: pulse 1.5s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}
</style>
