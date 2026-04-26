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

<form class="grid gap-[0.85rem] items-end border border-[var(--shell-line)] rounded-[1.15rem] bg-[var(--shell-surface)] p-4 max-[980px]:!grid-cols-1" style="grid-template-columns: minmax(0, 1.4fr) repeat(2, minmax(0, 0.9fr)) auto;" method="GET">
	{#if otherParams}
		{#each otherParams.split('&') as pair}
			{#if pair.includes('=')}
				{@const [key, value] = pair.split('=')}
				<input type="hidden" name={decodeURIComponent(key)} value={decodeURIComponent(value)} />
			{/if}
		{/each}
	{/if}
	{#each filters as filter}
		<label class="grid gap-[0.35rem]">
			<span class="text-[0.74rem] font-bold tracking-[0.12em] uppercase text-[var(--shell-text-muted)]">{filter.label}</span>
			{#if filter.type === 'search'}
				<input type="search" name={filter.name} value={values[filter.name] ?? ''} placeholder="Search…" class="min-h-[2.75rem] rounded-[0.85rem] border border-[var(--shell-line-strong)] bg-[var(--shell-surface-muted)] px-[0.8rem] py-[0.7rem] text-[var(--shell-text)] text-[0.92rem]" />
			{:else if filter.type === 'select'}
				<select name={filter.name} class="min-h-[2.75rem] rounded-[0.85rem] border border-[var(--shell-line-strong)] bg-[var(--shell-surface-muted)] px-[0.8rem] py-[0.7rem] text-[var(--shell-text)] text-[0.92rem]">
					{#each filter.options ?? [] as option}
						<option value={option.value} selected={values[filter.name] === option.value}>
							{option.label}
						</option>
					{/each}
				</select>
			{/if}
		</label>
	{/each}
	<div class="flex items-center gap-[0.7rem] max-[980px]:justify-start">
		<button type="submit" class="inline-flex items-center justify-center min-h-[2.75rem] px-4 rounded-full text-[0.9rem] font-semibold border border-transparent bg-[var(--shell-accent)] text-[#fff9f2] cursor-pointer">Apply</button>
		<a href={otherParams ? `?${otherParams}` : '?'} class="inline-flex items-center justify-center min-h-[2.75rem] px-4 rounded-full text-[0.9rem] font-semibold no-underline text-[var(--shell-text-secondary)]">Reset</a>
	</div>
</form>
