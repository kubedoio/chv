<script lang="ts">
	interface Props {
		cpuUsage?: number;
		memoryUsagePercent?: number;
	}

	let { cpuUsage = 0, memoryUsagePercent = 0 }: Props = $props();
</script>

<div class="metric-list">
	<div class="metric-item">
		<div class="metric-meta">
			<span>CPU_PRESSURE</span>
			<span>{Math.round(cpuUsage)}%</span>
		</div>
		<div class="bar-track">
			<div class="bar-fill" style="width: {cpuUsage}%"></div>
		</div>
	</div>
	<div class="metric-item">
		<div class="metric-meta">
			<span>RAM_RESERVATION</span>
			<span>{Math.round(memoryUsagePercent)}%</span>
		</div>
		<div class="bar-track">
			<div class="bar-fill" style="width: {memoryUsagePercent}%"></div>
		</div>
	</div>
</div>

<style>
	.metric-list {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.metric-item {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.metric-meta {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		font-weight: 800;
		color: var(--color-neutral-500);
	}

	.bar-track {
		height: 4px;
		background: var(--bg-surface-muted);
		border-radius: 2px;
		overflow: hidden;
	}

	.bar-fill {
		height: 100%;
		background: var(--color-primary);
		transition: width 0.3s ease;
	}
</style>
