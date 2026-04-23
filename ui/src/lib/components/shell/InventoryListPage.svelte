<script lang="ts">
	import type { Snippet } from 'svelte';
	import PageHeaderWithAction from './PageHeaderWithAction.svelte';
	import InventoryTable from './InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import ErrorState from './ErrorState.svelte';
	import EmptyInfrastructureState from './EmptyInfrastructureState.svelte';
	import SectionCard from './SectionCard.svelte';
	import CompactMetricCard from '$lib/components/CompactMetricCard.svelte';
	import type { PageDefinition } from '$lib/shell/app-shell';

	interface FilterDef {
		key: string;
		label: string;
		type: 'text' | 'select' | 'boolean';
		placeholder?: string;
		options?: { value: string; label: string }[];
	}

	interface Props {
		page: PageDefinition;
		headerActions?: Snippet;
		metrics: { label: string; value: number; trend?: number; color?: string }[];
		filters: FilterDef[];
		activeFilters: Record<string, string>;
		onFilterChange: (key: string, value: unknown) => void;
		onClearFilters: () => void;
		state: 'ready' | 'empty' | 'error';
		emptyTitle: string;
		emptyDescription: string;
		emptyHint: string;
		columns: any[];
		rows: any[];
		rowHref: (row: any) => string;
		cell?: Snippet<[{ column: any, row: any }]>
		sidebar: Snippet;
	}

	let {
		page,
		headerActions,
		metrics,
		filters,
		activeFilters,
		onFilterChange,
		onClearFilters,
		state,
		emptyTitle,
		emptyDescription,
		emptyHint,
		columns,
		rows,
		rowHref,
		cell,
		sidebar
	}: Props = $props();
</script>

<div class="inventory-page">
	<PageHeaderWithAction {page}>
		{#snippet actions()}
			{#if headerActions}
				{@render headerActions()}
			{/if}
		{/snippet}
	</PageHeaderWithAction>

	<div class="inventory-metrics">
		{#each metrics as m}
			<CompactMetricCard
				label={m.label}
				value={m.value}
				trend={m.trend ?? 0}
				color={m.color as any}
			/>
		{/each}
	</div>

	<div class="inventory-controls">
		<FilterBar
			{filters}
			{activeFilters}
			onFilterChange={onFilterChange}
			onClearAll={onClearFilters}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if state === 'error'}
				<ErrorState />
			{:else if state === 'empty'}
				<EmptyInfrastructureState
					title={emptyTitle}
					description={emptyDescription}
					hint={emptyHint}
				/>
			{:else}
				<InventoryTable {columns} {rows} {rowHref} {cell} />
			{/if}
		</section>

		<aside class="support-area">
			{@render sidebar()}
		</aside>
	</main>
</div>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.inventory-metrics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.75rem;
	}

	.inventory-controls {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		overflow: hidden;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.empty-hint {
		font-size: 11px;
		color: var(--color-neutral-400);
		padding: 1rem;
		text-align: center;
	}

	.attention-list {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.attention-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
		text-decoration: none;
		color: var(--color-neutral-800);
		transition: background 0.1s ease;
	}

	.attention-card:hover {
		background: var(--bg-surface-hover);
	}

	.attention-card__main {
		display: flex;
		flex-direction: column;
	}

	.res-name {
		font-size: 11px;
		font-weight: 700;
	}

	.res-issue {
		font-size: 9px;
		color: var(--color-danger);
		font-weight: 600;
		text-transform: uppercase;
	}

	.task-list {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.task-item {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		padding: 0.35rem 0.5rem;
		background: var(--bg-surface-muted);
		border-radius: var(--radius-xs);
	}

	.task-label {
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.task-time {
		color: var(--color-success);
		font-weight: 700;
		text-transform: uppercase;
		font-size: 9px;
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
