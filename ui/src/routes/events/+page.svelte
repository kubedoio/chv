<script lang="ts">
	import {
		PageShell,
		FilterPanel,
		ResourceTable,
		StateBanner,
		UrlPagination,
		PostureStrip,
		PostureCard
	} from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/events');
	const model = $derived(data.events);

	const filterConfig = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'severity',
			label: 'Severity',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All severities' },
				{ value: 'critical', label: 'Critical' },
				{ value: 'warning', label: 'Warning' },
				{ value: 'info', label: 'Info' }
			]
		},
		{
			name: 'state',
			label: 'State',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All states' },
				{ value: 'open', label: 'Open' },
				{ value: 'acknowledged', label: 'Acknowledged' },
				{ value: 'resolved', label: 'Resolved' }
			]
		}
	];

	function severityTone(severity: string): ShellTone {
		switch (severity) {
			case 'critical':
				return 'failed';
			case 'warning':
				return 'warning';
			case 'info':
				return 'unknown';
			default:
				return 'unknown';
		}
	}

	function stateTone(state: string): ShellTone {
		switch (state) {
			case 'open':
				return 'failed';
			case 'acknowledged':
				return 'warning';
			case 'resolved':
				return 'healthy';
			default:
				return 'unknown';
		}
	}

	const posture = $derived(() => {
		const items = model.items;
		return {
			total: items.length,
			critical: items.filter((e) => e.severity === 'critical' && e.state !== 'resolved').length,
			warning: items.filter((e) => e.severity === 'warning' && e.state !== 'resolved').length,
			open: items.filter((e) => e.state === 'open').length,
			acknowledged: items.filter((e) => e.state === 'acknowledged').length
		};
	});

	const columns = [
		{ key: 'severity', label: 'Severity' },
		{ key: 'summary', label: 'Summary' },
		{ key: 'resource', label: 'Resource' },
		{ key: 'type', label: 'Type' },
		{ key: 'state', label: 'State' },
		{ key: 'occurred', label: 'Occurred' }
	];

	const rows = $derived(
		model.items.map((item) => ({
			event_id: item.event_id,
			severity: { label: item.severity, tone: severityTone(item.severity) },
			summary: item.summary,
			resource: `${item.resource_kind} / ${item.resource_name}`,
			type: item.type,
			state: { label: item.state, tone: stateTone(item.state) },
			occurred: new Date(item.occurred_at).toLocaleString('en-US', {
				month: 'short',
				day: 'numeric',
				hour: 'numeric',
				minute: '2-digit'
			})
		}))
	);
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<div class="events-header">
		<PostureStrip
			chips={[
				{ label: 'Total', value: posture().total },
				{
					label: 'Critical',
					value: posture().critical,
					variant: posture().critical > 0 ? 'failed' : 'default'
				},
				{
					label: 'Warning',
					value: posture().warning,
					variant: posture().warning > 0 ? 'warning' : 'default'
				},
				{
					label: 'Open',
					value: posture().open,
					variant: posture().open > 0 ? 'failed' : 'default'
				},
				{ label: 'Acknowledged', value: posture().acknowledged }
			]}
		/>
	</div>

	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Event history unavailable"
			description="The event feed could not be loaded from the control plane."
			hint="Navigation remains available. Retry once the event stream is reachable."
		/>
	{:else if model.state === 'empty'}
		<StateBanner
			variant="empty"
			title="No events match the current view"
			description="Try widening the severity or state filters, or check back once new events are generated."
			hint="Filters are URL-backed so a filtered view can be shared between operators."
		/>
	{:else}
		<div class="posture-grid">
			<PostureCard
				label="Unresolved critical"
				value={posture().critical}
				note={posture().critical > 0 ? 'Critical events requiring immediate attention.' : 'No unresolved critical events.'}
			/>
			<PostureCard
				label="Degraded resources"
				value={posture().warning}
				note={posture().warning > 0 ? 'Warnings that may escalate without intervention.' : 'No active warnings.'}
			/>
		</div>

		<section class="inventory-section" aria-labelledby="inventory-title">
			<h2 id="inventory-title" class="inventory-section__title">Event feed</h2>
			<FilterPanel filters={filterConfig} values={model.filters.current} />
			<ResourceTable {columns} {rows} emptyTitle="No events match the current filters" />
			<UrlPagination
				page={model.page.page}
				pageSize={model.page.pageSize}
				totalItems={model.page.totalItems}
				basePath="/events"
				params={model.filters.current}
			/>
		</section>
	{/if}
</PageShell>

<style>
	.events-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.posture-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1rem;
	}

	.inventory-section {
		display: grid;
		gap: 1.2rem;
	}

	.inventory-section__title {
		font-size: 1rem;
		font-weight: 700;
		color: var(--shell-text);
		margin: 0;
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
