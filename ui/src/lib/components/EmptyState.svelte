<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		icon: any;
		title: string;
		description: string;
		children?: Snippet;
	}

	let { icon: Icon, title, description, children }: Props = $props();
</script>

<div
	class="flex flex-col items-center justify-center py-12 text-center"
	role="status"
	aria-live="polite"
>
	<div class="mb-4 text-line">
		<Icon size={48} />
	</div>
	<h3 class="mb-2 text-base font-medium text-muted">
		{title}
	</h3>
	<p class="mb-6 max-w-sm text-sm text-light">
		{description}
	</p>
	{#if children}
		<div>
			{@render children()}
		</div>
	{/if}
</div>
