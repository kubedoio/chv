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
	<nav class="url-pagination" aria-label="Pagination">
		<a
			href={buildHref(page - 1)}
			class="url-pagination__link"
			class:url-pagination__link--disabled={page <= 1}
			aria-disabled={page <= 1 ? 'true' : undefined}
		>
			Previous
		</a>
		<span class="url-pagination__info">
			Page {page} of {totalPages}
			<span class="url-pagination__detail">
				({totalItems} items)
			</span>
		</span>
		<a
			href={buildHref(page + 1)}
			class="url-pagination__link"
			class:url-pagination__link--disabled={page >= totalPages}
			aria-disabled={page >= totalPages ? 'true' : undefined}
		>
			Next
		</a>
	</nav>
{/if}

<style>
	.url-pagination {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.85rem 1rem;
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.url-pagination__link {
		display: inline-flex;
		align-items: center;
		padding: 0.5rem 0.9rem;
		border-radius: 0.75rem;
		background: var(--shell-accent);
		color: #fff9f2;
		font-size: 0.9rem;
		font-weight: 600;
		text-decoration: none;
	}

	.url-pagination__link:hover:not(.url-pagination__link--disabled) {
		opacity: 0.92;
	}

	.url-pagination__link--disabled {
		background: var(--shell-surface-muted);
		color: var(--shell-text-muted);
		cursor: not-allowed;
		pointer-events: none;
	}

	.url-pagination__info {
		font-size: 0.9rem;
		color: var(--shell-text-secondary);
	}

	.url-pagination__detail {
		color: var(--shell-text-muted);
	}
</style>
