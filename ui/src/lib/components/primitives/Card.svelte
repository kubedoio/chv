<script lang="ts">
	interface Props {
		interactive?: boolean;
		padding?: 'none' | 'sm' | 'md' | 'lg';
	}

	let {
		interactive = false,
		padding = 'md',
		children,
		header,
		footer
	}: Props & { children?: import('svelte').Snippet; header?: import('svelte').Snippet; footer?: import('svelte').Snippet } = $props();

	const paddingClasses = {
		none: '',
		sm: 'padding-sm',
		md: 'padding-md',
		lg: 'padding-lg'
	};
</script>

<div class="card {interactive ? 'card-interactive' : ''}" role="article">
	{#if header}
		<div class="card-header {paddingClasses[padding]}">
			{@render header()}
		</div>
	{/if}
	<div class="card-body {paddingClasses[padding]}">
		{@render children?.()}
	</div>
	{#if footer}
		<div class="card-footer {paddingClasses[padding]}">
			{@render footer()}
		</div>
	{/if}
</div>

<style>
	.card {
		background: white;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-sm);
		overflow: hidden;
		transition:
			box-shadow var(--duration-normal) var(--ease-default),
			border-color var(--duration-normal) var(--ease-default),
			transform var(--duration-normal) var(--ease-default);
	}

	.card-interactive {
		cursor: pointer;
	}

	.card-interactive:hover {
		box-shadow: var(--shadow-lg);
		border-color: rgba(229, 112, 53, 0.3);
		transform: translateY(-2px);
	}

	.card-header {
		border-bottom: 1px solid var(--color-neutral-100);
		background: var(--color-neutral-50);
	}

	.card-footer {
		border-top: 1px solid var(--color-neutral-100);
		background: var(--color-neutral-50);
	}

	.padding-sm {
		padding: var(--space-3);
	}

	.padding-md {
		padding: var(--space-4);
	}

	.padding-lg {
		padding: var(--space-6);
	}
</style>
