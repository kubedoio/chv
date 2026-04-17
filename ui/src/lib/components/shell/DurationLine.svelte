<script lang="ts">
	interface Props {
		startedMs: number;
		finishedMs?: number;
	}

	let { startedMs, finishedMs }: Props = $props();

	const duration = $derived.by(() => {
		if (!startedMs) return '-';
		const end = finishedMs || Date.now();
		const diff = end - startedMs;
		
		if (diff < 1000) return '< 1s';
		
		const seconds = Math.floor(diff / 1000);
		if (seconds < 60) return `${seconds}s`;
		
		const minutes = Math.floor(seconds / 60);
		const remainingSeconds = seconds % 60;
		
		if (minutes < 60) {
			return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
		}
		
		const hours = Math.floor(minutes / 60);
		const remainingMinutes = minutes % 60;
		return `${hours}h ${remainingMinutes}m`;
	});
</script>

<span class="duration-line" class:is-running={!finishedMs}>
	{duration}
</span>

<style>
	.duration-line {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.is-running {
		color: var(--shell-accent);
		font-weight: 500;
	}
</style>
