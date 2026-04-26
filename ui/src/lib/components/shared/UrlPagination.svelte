<script lang="ts">
	interface Props {
		page: number;
		pageSize: number;
		totalItems: number;
		basePath: string;
		params: Record<string, string>;
	}

	let { page, pageSize, totalItems, basePath, params }: Props = $props();

	const totalPages = $derived(Math.max(1, Math.ceil(totalItems / pageSize)));

	function buildHref(targetPage: number): string {
		const url = new URL(basePath, 'http://localhost');
		Object.entries(params).forEach(([key, value]) => {
			if (value && value !== 'all') url.searchParams.set(key, value);
		});
		if (targetPage > 1) url.searchParams.set('page', String(targetPage));
		return url.pathname + url.search;
	}
</script>

{#if totalItems > 0}
	<nav class="flex items-center justify-between gap-4 px-4 py-[0.85rem] border border-[var(--shell-line)] rounded-[1.15rem] bg-[var(--shell-surface)]" aria-label="Pagination">
		<a
			href={buildHref(page - 1)}
			class="inline-flex items-center px-[0.9rem] py-2 rounded-[0.75rem] text-[0.9rem] font-semibold no-underline {page <= 1 ? 'bg-[var(--shell-surface-muted)] text-[var(--shell-text-muted)] cursor-not-allowed pointer-events-none' : 'bg-[var(--shell-accent)] text-[#fff9f2] hover:opacity-[0.92]'}"
			aria-disabled={page <= 1 ? 'true' : undefined}
		>
			Previous
		</a>
		<span class="text-[0.9rem] text-[var(--shell-text-secondary)]">
			Page {page} of {totalPages}
			<span class="text-[var(--shell-text-muted)]">
				({totalItems} items)
			</span>
		</span>
		<a
			href={buildHref(page + 1)}
			class="inline-flex items-center px-[0.9rem] py-2 rounded-[0.75rem] text-[0.9rem] font-semibold no-underline {page >= totalPages ? 'bg-[var(--shell-surface-muted)] text-[var(--shell-text-muted)] cursor-not-allowed pointer-events-none' : 'bg-[var(--shell-accent)] text-[#fff9f2] hover:opacity-[0.92]'}"
			aria-disabled={page >= totalPages ? 'true' : undefined}
		>
			Next
		</a>
	</nav>
{/if}
