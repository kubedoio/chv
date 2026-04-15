<script lang="ts">
	import type { Snippet } from 'svelte';
	import Badge from './Badge.svelte';

	interface Column {
		key: string;
		label: string;
		sortable?: boolean;
	}

	interface ToneBadge {
		label: string;
		tone: 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown';
	}

	interface Props {
		columns: Column[];
		rows: Record<string, unknown>[];
		rowHref?: (row: Record<string, unknown>) => string | null;
		emptyTitle?: string;
		emptyDescription?: string;
		actionCell?: Snippet<[Record<string, unknown>]>;
	}

	let { columns, rows, rowHref, emptyTitle, emptyDescription, actionCell }: Props = $props();

	function renderCell(row: Record<string, unknown>, key: string): string {
		const value = row[key];
		if (value === null || value === undefined) return '';
		return String(value);
	}

	function getToneBadge(value: unknown): ToneBadge | null {
		if (
			typeof value === 'object' &&
			value !== null &&
			'label' in value &&
			'tone' in value &&
			typeof (value as Record<string, unknown>).label === 'string' &&
			typeof (value as Record<string, unknown>).tone === 'string'
		) {
			return value as ToneBadge;
		}
		return null;
	}
</script>

<div class="resource-table__shell">
	{#if rows.length === 0}
		<div class="resource-table__empty">
			<div class="resource-table__empty-title">{emptyTitle ?? 'No data'}</div>
			{#if emptyDescription}
				<p class="resource-table__empty-description">{emptyDescription}</p>
			{/if}
		</div>
	{:else}
		<table class="resource-table">
			<thead>
				<tr>
					{#each columns as column}
						<th>
							{column.label}
						</th>
					{/each}
					{#if actionCell}
						<th>Actions</th>
					{/if}
				</tr>
			</thead>
			<tbody>
				{#each rows as row}
					<tr>
						{#each columns as column, i}
							{@const cellValue = row[column.key]}
							{@const badge = getToneBadge(cellValue)}
							<td>
								{#if i === 0 && rowHref}
									{@const href = rowHref(row)}
									{#if href}
										<a href={href} class="resource-table__link">{renderCell(row, column.key)}</a>
									{:else if badge}
										<Badge label={badge.label} tone={badge.tone} />
									{:else}
										{renderCell(row, column.key)}
									{/if}
								{:else if badge}
									<Badge label={badge.label} tone={badge.tone} />
								{:else}
									{renderCell(row, column.key)}
								{/if}
							</td>
						{/each}
						{#if actionCell}
							<td>{@render actionCell(row)}</td>
						{/if}
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>

<style>
	.resource-table__shell {
		overflow-x: auto;
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.resource-table {
		width: 100%;
		border-collapse: collapse;
		min-width: 600px;
	}

	.resource-table th,
	.resource-table td {
		padding: 0.95rem 1rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
		text-align: left;
		color: var(--shell-text-secondary);
		vertical-align: middle;
	}

	.resource-table th {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
		background: rgba(247, 242, 234, 0.75);
	}

	.resource-table tbody tr:hover {
		background: rgba(247, 242, 234, 0.35);
	}

	.resource-table__link {
		color: var(--shell-text);
		font-weight: 700;
		text-decoration: none;
	}

	.resource-table__link:hover {
		color: var(--shell-accent);
	}

	.resource-table__empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 3rem 1.5rem;
		text-align: center;
	}

	.resource-table__empty-title {
		font-size: 1rem;
		font-weight: 600;
		color: var(--shell-text);
	}

	.resource-table__empty-description {
		margin-top: 0.35rem;
		font-size: 0.88rem;
		color: var(--shell-text-secondary);
		max-width: 400px;
	}
</style>
