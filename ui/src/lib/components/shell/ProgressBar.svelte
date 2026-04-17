<script lang="ts">
	interface Props {
		progress: number;
		tone?: 'healthy' | 'warning' | 'failed' | 'neutral';
	}

	let { progress, tone = 'healthy' }: Props = $props();

	const color = $derived({
		healthy: 'var(--color-success)',
		warning: 'var(--color-warning-dark)',
		failed: 'var(--color-danger)',
		neutral: 'var(--shell-accent)'
	}[tone]);
</script>

<div class="progress-container">
	<div class="progress-bar" style="width: {progress}%; background: {color}"></div>
</div>

<style>
	.progress-container {
		width: 100%;
		height: 4px;
		background: var(--shell-line);
		border-radius: 999px;
		overflow: hidden;
	}

	.progress-bar {
		height: 100%;
		transition: width 0.3s ease;
	}
</style>
