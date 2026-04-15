<script lang="ts">
	interface TabItem {
		id: string;
		label: string;
		count?: number;
	}

	interface Props {
		tabs: TabItem[];
		currentId: string;
	}

	let { tabs, currentId }: Props = $props();
</script>

<nav class="detail-tabs" aria-label="Detail sections">
	{#each tabs as tab}
		<a
			class={`detail-tabs__link ${currentId === tab.id ? 'detail-tabs__link--active' : ''}`}
			href={`?tab=${tab.id}`}
			aria-current={currentId === tab.id ? 'page' : undefined}
		>
			<span>{tab.label}</span>
			{#if tab.count !== undefined}
				<span class="detail-tabs__count">{tab.count}</span>
			{/if}
		</a>
	{/each}
</nav>

<style>
	.detail-tabs {
		display: flex;
		flex-wrap: wrap;
		gap: 0.65rem;
	}

	.detail-tabs__link {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.72rem 0.95rem;
		border-radius: 999px;
		border: 1px solid var(--shell-line);
		background: var(--shell-surface);
		color: var(--shell-text-secondary);
		text-decoration: none;
		font-size: 0.9rem;
		font-weight: 600;
	}

	.detail-tabs__link--active {
		border-color: var(--shell-accent);
		background: var(--shell-accent-soft);
		color: var(--shell-text);
	}

	.detail-tabs__count {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 1.35rem;
		height: 1.35rem;
		padding: 0 0.35rem;
		border-radius: 999px;
		background: rgba(31, 38, 45, 0.08);
		font-size: 0.76rem;
	}
</style>
