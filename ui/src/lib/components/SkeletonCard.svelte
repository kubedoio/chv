<script lang="ts">
	interface Props {
		lines?: number;
	}

	let { lines = 3 }: Props = $props();

	// Generate array for lines
	const lineArray = $derived(Array.from({ length: lines }, (_, i) => i));

	// Vary the width of each line for natural look
	function getLineWidth(index: number): string {
		if (index === 0) return '80%';
		if (index === lines - 1) return '40%';
		if (index % 2 === 0) return '65%';
		return '90%';
	}
</script>

<div class="skeleton-pulse rounded border border-line bg-white p-4">
	<div class="flex items-start gap-4">
		<!-- Avatar circle -->
		<div
			class="h-10 w-10 flex-shrink-0 rounded-full bg-gradient-to-br from-gray-200 via-gray-100 to-gray-200"
		></div>
		<!-- Text lines -->
		<div class="flex-1 space-y-3 py-1">
			{#each lineArray as i}
				<div
					class="h-3 rounded bg-gradient-to-r from-gray-200 via-gray-100 to-gray-200"
					style="width: {getLineWidth(i)}"
				></div>
			{/each}
		</div>
	</div>
</div>

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
