<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import ResourceLink from '$lib/components/shell/ResourceLink.svelte';
	import SeverityShield from '$lib/components/shell/SeverityShield.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import { Bell, AlertTriangle, Info, ShieldAlert, ChevronRight, Activity } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page as appPage } from '$app/stores';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/events');
	const model = $derived(data.events);
	const items = $derived(model.items);

	const stats = $derived([
		{ label: 'Total Events', value: items.length },
		{ label: 'Critical', value: items.filter(e => e.severity === 'critical' && e.state !== 'resolved').length, status: 'critical' as const },
		{ label: 'Warning', value: items.filter(e => e.severity === 'warning' && e.state !== 'resolved').length, status: 'warning' as const },
		{ label: 'Unresolved', value: items.filter(e => e.state !== 'resolved').length, status: 'neutral' as const }
	]);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Summary or resource...' },
		{
			key: 'severity',
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
			key: 'state',
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

	function handleFilterChange(key: string, value: any) {
		const newParams = new URLSearchParams($appPage.url.searchParams);
		if (value === '' || value === 'all') {
			newParams.delete(key);
		} else {
			newParams.set(key, String(value));
		}
		goto(`?${newParams.toString()}`, { keepFocus: true, noScroll: true });
	}

	const columns = [
		{ key: 'severity', label: 'Sev', align: 'center' as const },
		{ key: 'summary', label: 'Event Summary' },
		{ key: 'resource', label: 'Affected Resource' },
		{ key: 'type', label: 'Category' },
		{ key: 'state', label: 'State' },
		{ key: 'occurred', label: 'Time', align: 'right' as const }
	];

	const rows = $derived(
		items.map((item) => ({
			...item,
			occurred: new Date(item.occurred_at).toLocaleString('en-US', {
				month: 'short',
				day: 'numeric',
				hour: 'numeric',
				minute: '2-digit'
			})
		}))
	);

	const criticalEvents = $derived(items.filter(e => e.severity === 'critical' && e.state !== 'resolved').slice(0, 3));
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={page} />

	<div class="posture-strip-wrapper">
		<CompactStatStrip {stats} />
	</div>

	<div class="inventory-controls">
		<FilterBar 
			{filters} 
			activeFilters={model.filters.current} 
			onFilterChange={handleFilterChange}
			onClearAll={() => goto($appPage.url.pathname)}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if model.state === 'error'}
				<ErrorState />
			{:else if model.state === 'empty'}
				<EmptyInfrastructureState 
					title="No events match your criteria" 
					description="Adjust filters to view diagnostic history." 
					hint="System-level events are kept until resolved or rotated."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={rows}
				>
					{#snippet cell({ column, row })}
						{#if column.key === 'severity'}
							<SeverityShield severity={row.severity} />
						{:else if column.key === 'resource'}
							<ResourceLink kind={row.resource_kind} id={row.resource_id} name={row.resource_name} compact />
						{:else if column.key === 'state'}
							<span class="state-label state-{row.state}">{row.state}</span>
						{:else}
							{row[column.key]}
						{/if}
					{/snippet}
				</InventoryTable>
			{/if}
		</section>

		<aside class="support-area">
			<SectionCard title="Priority Inspection" icon={ShieldAlert} badgeTone={criticalEvents.length > 0 ? 'failed' : 'neutral'}>
				{#if criticalEvents.length === 0}
					<p class="empty-hint">No unresolved critical alerts.</p>
				{:else}
					<ul class="priority-list">
						{#each criticalEvents as event}
							<li>
								<div class="priority-item">
									<div class="priority-main">
										<span class="p-summary">{event.summary}</span>
										<div class="p-meta">
											<span class="p-time">{new Date(event.occurred_at).toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}</span>
											<span class="dot">·</span>
											<span class="p-res">{event.resource_name}</span>
										</div>
									</div>
									<a href="/events?query={event.event_id}" class="p-link">
										<ChevronRight size={14} />
									</a>
								</div>
							</li>
						{/each}
					</ul>
				{/if}
			</SectionCard>

			<SectionCard title="Resource Posture" icon={Activity}>
				<div class="resource-summary">
					<div class="res-stat">
						<span class="val">{items.filter(e => e.severity === 'warning').length}</span>
						<span class="lbl">Warnings</span>
					</div>
					<div class="res-stat">
						<span class="val">{items.filter(e => e.state === 'resolved').length}</span>
						<span class="lbl">Resolved</span>
					</div>
				</div>
			</SectionCard>
		</aside>
	</main>
</div>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.posture-strip-wrapper {
		margin-top: -0.25rem;
	}

	.inventory-controls {
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		overflow: hidden;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.state-label {
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.state-open { color: var(--color-danger); }
	.state-acknowledged { color: var(--color-warning-dark); }
	.state-resolved { color: var(--color-success); }

	.priority-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.priority-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.25rem;
	}

	.priority-main {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.p-summary {
		font-weight: 600;
		font-size: var(--text-sm);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.p-meta {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: 10px;
		color: var(--shell-text-muted);
	}

	.p-link {
		color: var(--shell-text-muted);
	}

	.resource-summary {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}

	.res-stat {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border-radius: 0.25rem;
	}

	.res-stat .val {
		font-size: var(--text-lg);
		font-weight: 700;
	}

	.res-stat .lbl {
		font-size: 10px;
		color: var(--shell-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 0.5rem 0;
	}

	@media (max-width: 1200px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
