<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		eyebrow?: string;
		description?: string;
		children?: Snippet;
	}

	let { title, eyebrow, description, children }: Props = $props();
</script>

<div class="page-shell">
	<header class="page-shell__header">
		{#if eyebrow}
			<div class="page-shell__eyebrow">{eyebrow}</div>
		{/if}
		<h1 class="page-shell__title">{title}</h1>
		{#if description}
			<p class="page-shell__description">{description}</p>
		{/if}
	</header>
	{#if children}
		<div class="page-shell__content">
			{@render children()}
		</div>
	{/if}
</div>

<style>
	.page-shell {
		display: grid;
		gap: 1.2rem;
	}

	.page-shell__header {
		display: grid;
		gap: 0.35rem;
	}

	.page-shell__eyebrow {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.page-shell__title {
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.page-shell__description {
		font-size: 0.92rem;
		color: var(--shell-text-secondary);
		line-height: 1.5;
	}

	.page-shell__content {
		display: grid;
		gap: 1.2rem;
	}
</style>
