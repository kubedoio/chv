<script lang="ts">
	import type { PageDefinition } from '$lib/shell/app-shell';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';

	interface Props {
		page: PageDefinition;
		actions?: import('svelte').Snippet;
	}

	let { page, actions }: Props = $props();
</script>

<header class="section-header">
	<div class="section-header__title-row">
		<div class="section-header__icon" aria-hidden="true">
			<page.icon size={20}></page.icon>
		</div>
		<div>
			<div class="section-header__eyebrow">{page.eyebrow}</div>
			<h1>{page.title}</h1>
		</div>
		{#if actions}
			<div class="section-header__actions">
				{@render actions()}
			</div>
		{/if}
	</div>

	<div class="section-header__meta">
		<p>{page.description}</p>
		<div class="section-header__badges">
			{#each page.badges as badge}
				<StatusBadge {...badge} />
			{/each}
		</div>
	</div>
</header>

<style>
	.section-header {
		display: grid;
		gap: 1rem;
	}

	.section-header__title-row {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.section-header__actions {
		display: flex;
		margin-left: auto;
		gap: 0.5rem;
		align-items: center;
	}

	.section-header__icon {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: 0.5rem;
		border: 1px solid var(--shell-line);
		background: var(--shell-surface);
		color: var(--shell-accent);
	}

	.section-header__eyebrow {
		font-size: var(--text-xs);
		font-weight: 600;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	h1 {
		margin-top: 0.2rem;
		font-size: var(--text-3xl);
		line-height: 1.05;
		color: var(--shell-text);
	}

	.section-header__meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.9rem 1.25rem;
	}

	.section-header__meta p {
		max-width: 48rem;
		font-size: var(--text-sm);
		line-height: 1.6;
		color: var(--shell-text-secondary);
	}

	.section-header__badges {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	@media (max-width: 720px) {
		.section-header__title-row {
			align-items: flex-start;
		}
	}
</style>
