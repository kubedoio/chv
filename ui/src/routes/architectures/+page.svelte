<script lang="ts">
	import { goto } from '$app/navigation';
	import Button from '$lib/components/primitives/Button.svelte';
	import ArchitectureCard from '$lib/components/architectures/dashboard/ArchitectureCard.svelte';
	import EmptyState from '$lib/components/architectures/dashboard/EmptyState.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const model = $derived(data.architectures);
</script>

<svelte:head>
	<title>Architectures · CellHV</title>
</svelte:head>

<div class="page" data-testid="architectures-page">
	<header class="page-header">
		<div>
			<h1 class="page-title">Saved Topologies</h1>
			<p class="page-subtitle">Architecture drafts and applied topologies for this cloud.</p>
		</div>
		<Button variant="primary" onclick={() => goto('/architectures/new')}>
			New architecture
		</Button>
	</header>

	{#if model.state === 'error'}
		<div class="error-banner" role="alert" data-testid="architectures-error">
			<strong>Could not load architectures.</strong>
			<span>{model.errorMessage}</span>
		</div>
	{:else if model.state === 'empty'}
		<EmptyState />
	{:else}
		<div class="grid" role="list" data-testid="architectures-list">
			{#each model.items as architecture (architecture.id)}
				<div role="listitem">
					<ArchitectureCard {architecture} />
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
	}

	.page-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 1rem;
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

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: 0.75rem;
	}

	.error-banner {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0.75rem 1rem;
		background: rgba(220, 38, 38, 0.08);
		border: 1px solid rgba(220, 38, 38, 0.4);
		border-radius: var(--radius-xs);
		color: rgb(153, 27, 27);
		font-size: var(--text-sm);
	}
</style>
