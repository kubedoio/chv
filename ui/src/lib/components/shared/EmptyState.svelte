<script lang="ts">
	import type { Snippet, Component } from 'svelte';
	let { icon: Icon, title, description, children, role = 'status' }: { icon: Component<{ size?: number }>; title: string; description: string; children?: Snippet; role?: string } = $props();
</script>

<div
	class="empty-state"
	{role}
	aria-live="polite"
	aria-label="{title}: {description}"
>
	<div class="empty-state-icon" aria-hidden="true">
		<Icon size={48} />
	</div>
	<h2 class="empty-state-title">
		{title}
	</h2>
	<p class="empty-state-description">
		{description}
	</p>
	{#if children}
		<div class="empty-state-actions">
			{@render children()}
		</div>
	{/if}
</div>

<style>
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 3rem 1.5rem;
		text-align: center;
	}

	.empty-state-icon {
		color: var(--color-neutral-300);
		margin-bottom: 1rem;
	}

	.empty-state-title {
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-neutral-700);
		margin: 0 0 0.5rem 0;
	}

	.empty-state-description {
		font-size: var(--text-sm);
		color: var(--color-neutral-500);
		max-width: 400px;
		margin: 0 0 1.5rem 0;
		line-height: 1.5;
	}

	.empty-state-actions {
		display: flex;
		gap: 0.75rem;
		flex-wrap: wrap;
		justify-content: center;
	}

	/* Reduced motion */
	@media (prefers-reduced-motion: reduce) {
		.empty-state {
			animation: none;
		}
	}
</style>
