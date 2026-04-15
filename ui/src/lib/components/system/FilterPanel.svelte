<script lang="ts">
	import { page } from '$app/stores';

	interface FilterOption {
		value: string;
		label: string;
	}

	interface Filter {
		name: string;
		label: string;
		type: 'search' | 'select';
		options?: FilterOption[];
	}

	interface Props {
		filters: Filter[];
		values: Record<string, string>;
	}

	let { filters, values }: Props = $props();

	const otherParams = $derived.by(() => {
		const params = new URLSearchParams($page.url.searchParams);
		const filterNames = new Set(filters.map((f) => f.name));
		for (const name of filterNames) {
			params.delete(name);
		}
		return params.toString();
	});
</script>

<form class="filter-panel" method="GET">
	{#if otherParams}
		{#each otherParams.split('&') as pair}
			{#if pair.includes('=')}
				{@const [key, value] = pair.split('=')}
				<input type="hidden" name={decodeURIComponent(key)} value={decodeURIComponent(value)} />
			{/if}
		{/each}
	{/if}
	{#each filters as filter}
		<label class="filter-panel__field">
			<span>{filter.label}</span>
			{#if filter.type === 'search'}
				<input type="search" name={filter.name} value={values[filter.name] ?? ''} placeholder="Search…" />
			{:else if filter.type === 'select'}
				<select name={filter.name}>
					{#each filter.options ?? [] as option}
						<option value={option.value} selected={values[filter.name] === option.value}>
							{option.label}
						</option>
					{/each}
				</select>
			{/if}
		</label>
	{/each}
	<div class="filter-panel__actions">
		<button type="submit">Apply</button>
		<a href="?{otherParams}">Reset</a>
	</div>
</form>

<style>
	.filter-panel {
		display: grid;
		grid-template-columns: minmax(0, 1.4fr) repeat(2, minmax(0, 0.9fr)) auto;
		gap: 0.85rem;
		align-items: end;
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.filter-panel__field {
		display: grid;
		gap: 0.35rem;
	}

	.filter-panel__field span {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.filter-panel__field input,
	.filter-panel__field select {
		min-height: 2.75rem;
		border-radius: 0.85rem;
		border: 1px solid var(--shell-line-strong);
		background: var(--shell-surface-muted);
		padding: 0.7rem 0.8rem;
		color: var(--shell-text);
		font-size: 0.92rem;
	}

	.filter-panel__actions {
		display: flex;
		align-items: center;
		gap: 0.7rem;
	}

	.filter-panel__actions button,
	.filter-panel__actions a {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.75rem;
		padding: 0 1rem;
		border-radius: 999px;
		font-size: 0.9rem;
		font-weight: 600;
		text-decoration: none;
	}

	.filter-panel__actions button {
		border: 1px solid transparent;
		background: var(--shell-accent);
		color: #fff9f2;
		cursor: pointer;
	}

	.filter-panel__actions a {
		color: var(--shell-text-secondary);
	}

	@media (max-width: 980px) {
		.filter-panel {
			grid-template-columns: 1fr;
		}

		.filter-panel__actions {
			justify-content: flex-start;
		}
	}
</style>
