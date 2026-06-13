<script lang="ts">
	import { goto } from '$app/navigation';
	import Button from '$lib/components/primitives/Button.svelte';
	import ArchitectureMetaPanel from '$lib/components/architectures/dashboard/ArchitectureMetaPanel.svelte';
	import StaleVersionBanner from '$lib/components/architectures/dashboard/StaleVersionBanner.svelte';
	import { liveState } from '$lib/stores/live-state.svelte';
	import type { Architecture } from '$lib/bff/architectures';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const detail = $derived(data.detail);

	// Local mirror of the loaded architecture so inline edits can update the UI
	// without a full page reload. The +page.ts loader is the source of truth on
	// initial load and after the user clicks "Reload" on the stale-version banner.
	// We start at null and let the $effect below sync from the loader's data;
	// this avoids capturing the initial prop value at component creation time.
	let current = $state<Architecture | null>(null);
	let staleVersion = $state(false);

	$effect(() => {
		if (detail.state === 'ready') {
			current = detail.architecture;
			staleVersion = false;
		} else {
			current = null;
		}
	});

	const heading = $derived(current ? (current.display_name ?? current.name) : '');

	function handleUpdated(next: Architecture) {
		current = next;
	}

	function handleStaleVersion() {
		staleVersion = true;
	}

	async function handleReload() {
		staleVersion = false;
		await liveState.invalidateAndRefresh({ patterns: ['architectures:'], detailId: current?.id });
	}
</script>

<svelte:head>
	<title>
		{current ? `${heading} · Architecture` : 'Architecture · CellHV'}
	</title>
</svelte:head>

<div class="page" data-testid="architecture-detail-page">
	{#if detail.state === 'error' || !current}
		<div class="error-banner" role="alert">
			<strong>Could not load architecture {detail.state === 'error' ? detail.id : ''}.</strong>
			<span>
				{detail.state === 'error' ? detail.errorMessage : 'No data returned by the server.'}
			</span>
			<div>
				<Button variant="secondary" size="sm" onclick={() => goto('/architectures')}>
					Back to list
				</Button>
			</div>
		</div>
	{:else}
		<header class="page-header">
			<div>
				<button
					type="button"
					class="back-link"
					onclick={() => goto('/architectures')}
					aria-label="Back to architectures"
				>
					← Architectures
				</button>
				<h1 class="page-title" data-testid="architecture-name">{heading}</h1>
				<p class="page-subtitle">
					Phase 0 metadata view. The visual designer arrives in Phase 2.
				</p>
			</div>
		</header>

		{#if staleVersion}
			<StaleVersionBanner onReload={handleReload} />
		{/if}

		<ArchitectureMetaPanel
			architecture={current}
			onUpdated={handleUpdated}
			onStaleVersion={handleStaleVersion}
		/>

		<section class="canvas-placeholder" aria-label="Designer canvas placeholder">
			<div class="placeholder-title">Designer canvas coming in Phase 2</div>
			<p class="placeholder-text">
				This pane will host the Svelte Flow canvas, node palette and inspector
				once the YAML model and validator land. For now, drafts can be created
				and renamed; the canvas, YAML editor and plan view will follow.
			</p>
		</section>
	{/if}
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.page-header {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.back-link {
		background: none;
		border: none;
		padding: 0;
		font-size: var(--text-xs);
		color: var(--color-neutral-500);
		cursor: pointer;
		align-self: flex-start;
	}

	.back-link:hover,
	.back-link:focus-visible {
		color: var(--color-primary);
		outline: none;
	}

	.page-title {
		font-size: var(--text-lg);
		font-weight: 700;
		margin: 0;
	}

	.page-subtitle {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-neutral-600);
	}

	.canvas-placeholder {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 2rem;
		background: var(--color-neutral-50, #f8fafc);
		border: 1px dashed var(--color-neutral-300);
		border-radius: var(--radius-sm);
		text-align: center;
	}

	.placeholder-title {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.placeholder-text {
		margin: 0 auto;
		max-width: 520px;
		font-size: 12px;
		color: var(--color-neutral-500);
		line-height: 1.5;
	}

	.error-banner {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0.75rem 1rem;
		background: rgba(220, 38, 38, 0.08);
		border: 1px solid rgba(220, 38, 38, 0.4);
		border-radius: var(--radius-xs);
		color: rgb(153, 27, 27);
		font-size: var(--text-sm);
	}
</style>
