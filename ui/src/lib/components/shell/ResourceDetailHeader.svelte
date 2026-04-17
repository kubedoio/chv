<script lang="ts">
	import StatusBadge from './StatusBadge.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { ChevronRight } from 'lucide-svelte';

	interface Props {
		title: string;
		eyebrow?: string;
		tone?: ShellTone;
		statusLabel?: string;
		description?: string;
		parentHref?: string;
		parentLabel?: string;
		actions?: import('svelte').Snippet;
	}

	let { title, eyebrow, tone = 'unknown', statusLabel, description, parentHref, parentLabel, actions }: Props = $props();
</script>

<header class="resource-header">
	<div class="resource-header__main">
		{#if parentHref && parentLabel}
			<nav class="breadcrumb">
				<a href={parentHref}>{parentLabel}</a>
				<ChevronRight size={12} class="breadcrumb-separator" />
			</nav>
		{/if}
		
		<div class="title-row">
			<div class="title-group">
				{#if eyebrow}<span class="eyebrow">{eyebrow}</span>{/if}
				<h1 class="title">{title}</h1>
			</div>
			
			<div class="status-group">
				<StatusBadge label={statusLabel || 'unknown'} {tone} />
			</div>
		</div>

		{#if description}
			<p class="description">{description}</p>
		{/if}
	</div>

	{#if actions}
		<div class="resource-header__actions">
			{@render actions()}
		</div>
	{/if}
</header>

<style>
	.resource-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		padding: 1rem 0;
		border-bottom: 1px solid var(--shell-line);
		margin-bottom: 1rem;
		gap: 2rem;
	}

	.resource-header__main {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		flex: 1;
	}

	.breadcrumb {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.breadcrumb a {
		color: inherit;
		text-decoration: none;
	}

	.breadcrumb a:hover {
		color: var(--shell-text);
	}

	.breadcrumb-separator {
		opacity: 0.5;
	}

	.title-row {
		display: flex;
		align-items: baseline;
		gap: 0.75rem;
	}

	.title-group {
		display: flex;
		flex-direction: column;
	}

	.eyebrow {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
	}

	.title {
		font-size: var(--text-2xl);
		font-weight: 700;
		color: var(--shell-text);
		margin: 0;
		line-height: 1.1;
	}

	.description {
		font-size: var(--text-sm);
		color: var(--shell-text-muted);
		margin: 0.25rem 0 0 0;
		max-width: 600px;
	}

	.resource-header__actions {
		display: flex;
		gap: 0.5rem;
		padding-top: 0.25rem;
	}

	@media (max-width: 768px) {
		.resource-header {
			flex-direction: column;
			gap: 1rem;
		}
	}
</style>
