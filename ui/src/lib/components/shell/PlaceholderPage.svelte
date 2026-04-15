<script lang="ts">
	import type { PageDefinition } from '$lib/shell/app-shell';
	import { getPrimaryStateDefinition } from '$lib/shell/app-shell';
	import SectionHeader from '$lib/components/shell/SectionHeader.svelte';
	import StatePanel from '$lib/components/shell/StatePanel.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';

	interface Props {
		page: PageDefinition;
	}

	let { page }: Props = $props();

	const primaryState = $derived(getPrimaryStateDefinition(page));
	const primaryStateVariant = $derived(page.previewState ?? 'empty');
</script>

<div class="placeholder-page">
	<SectionHeader {page} />

	<section class="placeholder-page__summary-grid" aria-label="Planned page surfaces">
		{#each page.summary as item}
			<article class="placeholder-page__summary-card">
				<div class="placeholder-page__summary-label">{item.label}</div>
				<div class="placeholder-page__summary-value">{item.value}</div>
				<p>{item.note}</p>
				{#if item.tone}
					<StatusBadge label={item.tone} tone={item.tone} />
				{/if}
			</article>
		{/each}
	</section>

	<section class="placeholder-page__detail-grid">
		<article class="placeholder-page__panel">
			<div class="placeholder-page__panel-label">Page coverage</div>
			<h2>Starter-bundle placeholder with realistic states</h2>
			<p>
				This route is intentionally shell-first. It reserves the page structure, status semantics,
				and operator language we will keep once real BFF data replaces the placeholders.
			</p>

			<ul>
				{#each page.focusAreas as area}
					<li>{area}</li>
				{/each}
			</ul>
		</article>

		<article class="placeholder-page__panel">
			<div class="placeholder-page__panel-label">Primary placeholder</div>
			<StatePanel variant={primaryStateVariant} {...primaryState} />
			<p>
				This starter route focuses on the most believable first-load condition for the section while
				retaining support for the full loading, empty, and error state set.
			</p>
			<div class="placeholder-page__supported-states" aria-label="Supported states">
				<StatusBadge label="loading supported" tone="unknown" />
				<StatusBadge label="empty supported" tone="healthy" />
				<StatusBadge label="error supported" tone="failed" />
			</div>
		</article>
	</section>
</div>

<style>
	.placeholder-page {
		display: grid;
		gap: 1.35rem;
	}

	.placeholder-page__summary-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1rem;
	}

	.placeholder-page__summary-card,
	.placeholder-page__panel {
		border: 1px solid var(--shell-line);
		border-radius: 1.2rem;
		background: var(--shell-surface);
		padding: 1.15rem;
	}

	.placeholder-page__summary-card {
		display: grid;
		gap: 0.65rem;
		align-content: start;
	}

	.placeholder-page__summary-label,
	.placeholder-page__panel-label {
		font-size: 0.74rem;
		font-weight: 600;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.placeholder-page__summary-value {
		font-size: 1.2rem;
		font-weight: 600;
		color: var(--shell-text);
	}

	.placeholder-page__summary-card p,
	.placeholder-page__panel p {
		font-size: 0.92rem;
		line-height: 1.55;
		color: var(--shell-text-secondary);
	}

	.placeholder-page__detail-grid {
		display: grid;
		grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
		gap: 1rem;
	}

	.placeholder-page__panel {
		display: grid;
		gap: 0.9rem;
	}

	.placeholder-page__panel h2 {
		font-size: 1.25rem;
		color: var(--shell-text);
	}

	.placeholder-page__panel ul {
		display: grid;
		gap: 0.65rem;
		padding: 0;
		margin: 0;
		list-style: none;
	}

	.placeholder-page__panel li {
		position: relative;
		padding-left: 1rem;
		font-size: 0.94rem;
		line-height: 1.45;
		color: var(--shell-text-secondary);
	}

	.placeholder-page__panel li::before {
		content: '';
		position: absolute;
		top: 0.56rem;
		left: 0;
		width: 0.38rem;
		height: 0.38rem;
		border-radius: 999px;
		background: var(--shell-accent-soft);
		border: 1px solid var(--shell-accent);
	}

	.placeholder-page__supported-states {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	@media (max-width: 1080px) {
		.placeholder-page__summary-grid,
		.placeholder-page__detail-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
