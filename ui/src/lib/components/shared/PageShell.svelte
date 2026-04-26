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

<div class="grid gap-[1.2rem]">
	<header class="grid gap-[0.35rem]">
		{#if eyebrow}
			<div class="text-[0.74rem] font-bold tracking-[0.12em] uppercase text-[var(--shell-text-muted)]">{eyebrow}</div>
		{/if}
		<h1 class="text-2xl font-bold text-[var(--shell-text)]">{title}</h1>
		{#if description}
			<p class="text-[0.92rem] text-[var(--shell-text-secondary)] leading-relaxed">{description}</p>
		{/if}
	</header>
	{#if children}
		<div class="grid gap-[1.2rem]">
			{@render children()}
		</div>
	{/if}
</div>
