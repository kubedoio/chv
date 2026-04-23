<script lang="ts">
	import type { Snippet } from 'svelte';
	import StatusBadge from './StatusBadge.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';

	interface Column {
		key: string;
		label: string;
		align?: 'left' | 'right' | 'center';
	}

	interface BadgeData {
		label: string;
		tone: ShellTone;
	}

	interface Props<T = Record<string, unknown>> {
		columns: Column[];
		rows: T[];
		rowHref?: (row: T) => string | null;
		emptySnippet?: Snippet;
		cell?: Snippet<[{ column: Column, row: T }]>
	}

	let { columns, rows, rowHref, emptySnippet, cell }: Props = $props();

	function isBadge(val: unknown): val is BadgeData {
		return val !== null && typeof val === 'object' && 'tone' in val && 'label' in val;
	}
</script>

<div class="inventory-table-container">
	{#if rows.length === 0}
		{#if emptySnippet}
			{@render emptySnippet()}
		{:else}
			<div class="empty-placeholder" role="status">No inventory matched current filters.</div>
		{/if}
	{:else}
		<table class="inventory-table">
			<thead>
				<tr>
					{#each columns as col}
						<th class="align-{col.align ?? 'left'}" scope="col">{col.label}</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each rows as row}
					<tr class:has-link={!!rowHref?.(row)}>
						{#each columns as col, i}
							{@const val = (row as Record<string, unknown>)[col.key]}
							<td class="align-{col.align ?? 'left'}">
								{#if cell}
									{@render cell({ column: col, row })}
								{:else if i === 0 && rowHref?.(row)}
									<a href={rowHref(row)} class="row-link">
										{val}
									</a>
								{:else if isBadge(val)}
									<StatusBadge label={val.label} tone={val.tone} />
								{:else}
									<span class="cell-text">{val}</span>
								{/if}
							</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>

<style>
	.inventory-table-container {
		width: 100%;
		overflow-x: auto;
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		background: var(--shell-surface);
	}

	.inventory-table {
		width: 100%;
		border-collapse: collapse;
		text-align: left;
	}

	th {
		background: var(--shell-surface-muted);
		padding: 0.45rem 0.75rem;
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
		border-bottom: 1px solid var(--shell-line);
		white-space: nowrap;
	}

	td {
		padding: 0.4rem 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: var(--text-sm);
		color: var(--shell-text-secondary);
		vertical-align: middle;
		white-space: nowrap;
	}

	tr:last-child td {
		border-bottom: none;
	}

	tr:hover {
		background: rgba(143, 90, 42, 0.02);
	}

	.row-link {
		color: var(--shell-text);
		font-weight: 600;
		text-decoration: none;
	}

	.row-link:hover {
		color: var(--shell-accent);
		text-decoration: underline;
	}

	.cell-text {
		font-variant-numeric: tabular-nums;
	}

	.align-right { text-align: right; }
	.align-center { text-align: center; }

	.empty-placeholder {
		padding: 2rem;
		text-align: center;
		color: var(--shell-text-muted);
		font-size: var(--text-sm);
	}
</style>
