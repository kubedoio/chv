<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { PageShell, StateBanner, Badge } from '$lib/components/system';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import type { TaskTimelineItemModel } from '$lib/webui/tasks';
	import type { RecentTask, HealthTile, CapacityTile } from '$lib/bff/types';
	import { normalizeTone, formatDateTimeLabel, formatDurationLabel } from '$lib/webui/formatters';
	import { mapRecentTask } from '$lib/webui/task-helpers';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/');

	const overview = $derived(data.overview);
	const isEmpty = $derived(
		!!overview &&
		overview.health_tiles.length === 0 &&
		overview.capacity_tiles.length === 0 &&
		overview.recent_tasks.length === 0
	);

	type HealthTileVM = HealthTile & { detail?: string };
	type CapacityTileVM = CapacityTile & { detail?: string; status?: string };
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<div class="overview-page">
		{#if data.meta.error}
			<StateBanner
				variant="error"
				title="Overview data is unavailable"
				description={data.meta.message ??
					'The dashboard could not assemble its browser-safe view model from the current API surface.'}
				hint="The shell stays usable while the page waits for a successful refresh."
			/>
		{:else if !overview}
			<StateBanner
				variant="loading"
				title="Loading fleet overview"
				description="This route waits for the server-side pass before shaping protected dashboard data."
				hint="Overview stays shell-first while the server rehydrates with the latest session state."
			/>
		{:else if isEmpty}
			<StateBanner
				variant="empty"
				title="No fleet data yet"
				description="Health, capacity, alerts, and task summaries will appear here once the control plane has inventory to shape."
				hint="This page is ready for live BFF data, but the environment does not expose any infrastructure records yet."
			/>
		{:else}
			<section class="overview-page__grid" aria-label="Overview health">
				{#each overview.health_tiles as tile}
					<article class="overview-page__metric-card">
						<div class="overview-page__metric-topline">
							<div class="overview-page__metric-label">{tile.label}</div>
							<Badge label={tile.status} tone={normalizeTone(tile.status)} />
						</div>
						<div class="overview-page__metric-value">{tile.value}</div>
						{#if (tile as HealthTileVM).detail}
							<p>{(tile as HealthTileVM).detail}</p>
						{/if}
					</article>
				{/each}
			</section>

			<section class="overview-page__grid overview-page__grid--capacity" aria-label="Overview capacity">
				{#each overview.capacity_tiles as tile}
					<article class="overview-page__capacity-card">
						<div class="overview-page__metric-topline">
							<div class="overview-page__metric-label">{tile.label}</div>
							{#if (tile as CapacityTileVM).status}
								<Badge
									label={(tile as CapacityTileVM).status!}
									tone={normalizeTone((tile as CapacityTileVM).status!)}
								/>
							{/if}
						</div>
						<div class="overview-page__capacity-values">
							<div>
								<div class="overview-page__capacity-number">{tile.used}</div>
								<div class="overview-page__capacity-caption">Used</div>
							</div>
							<div class="overview-page__capacity-divider" aria-hidden="true"></div>
							<div>
								<div class="overview-page__capacity-number">{tile.total}</div>
								<div class="overview-page__capacity-caption">Total</div>
							</div>
						</div>
						{#if (tile as CapacityTileVM).detail}
							<p>{(tile as CapacityTileVM).detail}</p>
						{/if}
					</article>
				{/each}
			</section>

			<section class="overview-page__detail-grid">
				<article class="overview-page__panel">
					<div class="overview-page__panel-label">Active alerts</div>
					<h2>Operator-visible degradations and failures</h2>
					{#if overview.active_alerts.length > 0}
						<div class="overview-page__alert-list">
							{#each overview.active_alerts as alert}
								<div class="overview-page__alert-item">
									<Badge label="attention" tone="failed" />
									<p>{alert}</p>
								</div>
							{/each}
						</div>
					{:else}
						<StateBanner
							variant="empty"
							title="No active alerts"
							description="Recent failures, degraded services, and important warnings will land here."
							hint="Overview keeps alerting separate from the task feed so operators can scan incidents first."
						/>
					{/if}
				</article>

				<article class="overview-page__panel">
					<div class="overview-page__panel-header">
						<div>
							<div class="overview-page__panel-label">Recent tasks</div>
							<h2>Accepted, running, and completed work</h2>
						</div>
						<a class="overview-page__panel-link" href="/tasks">Open task center</a>
					</div>

					{#if overview.recent_tasks.length > 0}
						<div class="overview-page__task-list">
							{#each overview.recent_tasks as task}
								<TaskTimelineItem task={mapRecentTask(task)} compact />
							{/each}
						</div>
					{:else}
						<StateBanner
							variant="empty"
							title="No recent tasks"
							description="Accepted work, active operations, and completed runs will appear here once task records exist."
							hint="The overview task strip is intentionally short so it stays scannable."
						/>
					{/if}
				</article>
			</section>
		{/if}
	</div>
</PageShell>

<style>
	.overview-page {
		display: grid;
		gap: 1.3rem;
	}

	.overview-page__notice,
	.overview-page__metric-card,
	.overview-page__capacity-card,
	.overview-page__panel {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.overview-page__notice {
		padding: 0.95rem 1rem;
		background: linear-gradient(135deg, rgba(252, 249, 244, 0.98), rgba(244, 238, 228, 0.96));
	}

	.overview-page__notice-label,
	.overview-page__metric-label,
	.overview-page__panel-label,
	.overview-page__capacity-caption {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.overview-page__notice p {
		margin-top: 0.35rem;
		font-size: 0.92rem;
		color: var(--shell-text-secondary);
	}

	.overview-page__grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1rem;
	}

	.overview-page__grid--capacity {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}

	.overview-page__metric-card,
	.overview-page__capacity-card,
	.overview-page__panel {
		padding: 1.05rem;
	}

	.overview-page__metric-topline {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.8rem;
	}

	.overview-page__metric-value,
	.overview-page__capacity-number {
		margin-top: 0.95rem;
		font-size: clamp(1.25rem, 2vw, 1.9rem);
		font-weight: 700;
		color: var(--shell-text);
	}

	.overview-page__metric-card p,
	.overview-page__capacity-card p {
		margin-top: 0.45rem;
		font-size: 0.92rem;
		line-height: 1.5;
		color: var(--shell-text-secondary);
	}

	.overview-page__capacity-values {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
		align-items: center;
		gap: 0.9rem;
		margin-top: 0.9rem;
	}

	.overview-page__capacity-divider {
		width: 1px;
		height: 2.6rem;
		background: var(--shell-line);
	}

	.overview-page__detail-grid {
		display: grid;
		grid-template-columns: minmax(0, 0.92fr) minmax(0, 1.08fr);
		gap: 1rem;
	}

	.overview-page__panel {
		display: grid;
		gap: 0.95rem;
	}

	.overview-page__panel-header {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.8rem;
	}

	.overview-page__panel h2 {
		font-size: 1.25rem;
		color: var(--shell-text);
	}

	.overview-page__panel-link {
		color: var(--shell-accent);
		font-size: 0.9rem;
		font-weight: 600;
		text-decoration: none;
	}

	.overview-page__panel-link:hover {
		text-decoration: underline;
	}

	.overview-page__alert-list,
	.overview-page__task-list {
		display: grid;
		gap: 0.8rem;
	}

	.overview-page__alert-item {
		display: grid;
		grid-template-columns: auto 1fr;
		align-items: start;
		gap: 0.75rem;
		padding: 0.9rem;
		border-radius: 0.95rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
	}

	.overview-page__alert-item p {
		font-size: 0.92rem;
		line-height: 1.5;
		color: var(--shell-text-secondary);
	}

	@media (max-width: 1200px) {
		.overview-page__grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.overview-page__grid--capacity,
		.overview-page__detail-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.overview-page__grid {
			grid-template-columns: 1fr;
		}

		.overview-page__capacity-values {
			grid-template-columns: 1fr;
			gap: 0.5rem;
		}

		.overview-page__capacity-divider {
			display: none;
		}
	}
</style>
