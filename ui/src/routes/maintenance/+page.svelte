<script lang="ts">
	import {
		PageShell,
		FilterPanel,
		ResourceTable,
		StateBanner,
		Badge,
		PostureCard
	} from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/maintenance');
	const model = $derived(data.maintenance);

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'state',
			label: 'State',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				{ value: 'in_maintenance', label: 'In maintenance' },
				{ value: 'draining', label: 'Draining' },
				{ value: 'scheduled', label: 'Scheduled' }
			]
		}
	];

	function stateTone(state: string): ShellTone {
		switch (state) {
			case 'in_maintenance':
				return 'warning';
			case 'draining':
				return 'degraded';
			case 'scheduled':
				return 'unknown';
			default:
				return 'unknown';
		}
	}

	function windowTone(status: string): ShellTone {
		switch (status) {
			case 'active':
				return 'warning';
			case 'scheduled':
				return 'unknown';
			case 'completed':
				return 'healthy';
			default:
				return 'unknown';
		}
	}

	const nodeColumns = [
		{ key: 'name', label: 'Node' },
		{ key: 'cluster', label: 'Cluster' },
		{ key: 'state', label: 'State' },
		{ key: 'window', label: 'Window' },
		{ key: 'task', label: 'Task' }
	];

	const nodeRows = $derived(
		model.nodes.map((n) => ({
			node_id: n.node_id,
			name: n.name,
			cluster: n.cluster,
			state: { label: n.state.replace('_', ' '), tone: stateTone(n.state) },
			window: n.window_start && n.window_end ? `${n.window_start.slice(0, 10)} → ${n.window_end.slice(11, 16)}` : '—',
			task: n.task_id ?? '—'
		}))
	);

	function rowHref(row: Record<string, unknown>): string | null {
		const id = row.node_id;
		return typeof id === 'string' ? `/nodes/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Maintenance status unavailable"
			description="The maintenance schedule and node state could not be loaded."
			hint="Navigation remains available while maintenance data recovers."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No maintenance windows scheduled"
			description="There are no active or planned maintenance windows in the fleet."
			hint="Scheduled maintenance and draining operations will appear here."
		/>
	{:else}
		<div class="posture-grid">
			<PostureCard
				label="Active windows"
				value={model.windows.filter((w) => w.status === 'active').length}
				note="Maintenance windows currently in effect."
			/>
			<PostureCard
				label="Nodes in maintenance"
				value={model.nodes.filter((n) => n.state === 'in_maintenance').length}
				note="Nodes with scheduling paused."
			/>
			<PostureCard
				label="Draining"
				value={model.nodes.filter((n) => n.state === 'draining').length}
				note="Nodes actively evacuating workloads."
			/>
			<PostureCard
				label="Pending actions"
				value={model.pending_actions}
				note="Operator actions required to proceed."
			/>
		</div>

		<section class="windows-section" aria-labelledby="windows-title">
			<h2 id="windows-title" class="section-title">Maintenance windows</h2>
			{#if model.windows.length > 0}
				<div class="window-list">
					{#each model.windows as w}
						<article class="window-card">
							<div class="window-card__header">
								<div>
									<div class="window-card__name">{w.name}</div>
									<div class="window-card__cluster">{w.cluster}</div>
								</div>
								<Badge label={w.status} tone={windowTone(w.status)} />
							</div>
							<div class="window-card__meta">
								<span>Start: {new Date(w.start_time).toLocaleString('en-US', { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })}</span>
								<span>End: {new Date(w.end_time).toLocaleString('en-US', { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })}</span>
								<span>{w.affected_nodes} affected node{w.affected_nodes === 1 ? '' : 's'}</span>
							</div>
						</article>
					{/each}
				</div>
			{:else}
				<StateBanner
					variant="empty"
					title="No maintenance windows"
					description="There are no scheduled or active maintenance windows."
				/>
			{/if}
		</section>

		<section class="nodes-section" aria-labelledby="nodes-title">
			<h2 id="nodes-title" class="section-title">Nodes in maintenance or drain</h2>
			<FilterPanel filters={filterConfig} values={model.filters.current} />
			{#if model.nodes.length > 0}
				<ResourceTable columns={nodeColumns} rows={nodeRows} {rowHref} emptyTitle="No nodes match" />
			{:else}
				<StateBanner
					variant="empty"
					title="No nodes in maintenance"
					description="No nodes are currently under maintenance, draining, or scheduled for change."
				/>
			{/if}
		</section>
	{/if}
</PageShell>

<style>
	.posture-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1rem;
	}

	.section-title {
		font-size: 1rem;
		font-weight: 700;
		color: var(--shell-text);
		margin: 0;
	}

	.windows-section,
	.nodes-section {
		display: grid;
		gap: 1.2rem;
	}

	.window-list {
		display: grid;
		gap: 0.8rem;
	}

	.window-card {
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.window-card__header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.window-card__name {
		font-weight: 700;
		color: var(--shell-text);
	}

	.window-card__cluster {
		font-size: 0.85rem;
		color: var(--shell-text-muted);
	}

	.window-card__meta {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		margin-top: 0.75rem;
		font-size: 0.85rem;
		color: var(--shell-text-secondary);
	}

	@media (max-width: 1080px) {
		.posture-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 640px) {
		.posture-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
