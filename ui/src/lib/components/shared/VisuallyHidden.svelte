<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
		as?: 'span' | 'div' | 'p';
	}

	let { children, as = 'span' }: Props = $props();
</script>

{#if as === 'div'}
	<div class="sr-only">
		{@render children()}
	</div>
{:else if as === 'p'}
	<p class="sr-only">
		{@render children()}
	</p>
{:else}
	<span class="sr-only">
		{@render children()}
	</span>
{/if}

<style>
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
