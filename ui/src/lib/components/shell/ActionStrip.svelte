<script lang="ts">
	interface Props {
		children: import('svelte').Snippet;
		title?: string;
	}

	let { children, title }: Props = $props();
</script>

<div class="action-strip-container">
	{#if title}<h3 class="strip-title">{title}</h3>{/if}
	<div class="action-strip">
		{@render children()}
	</div>
</div>

<style>
	.action-strip-container {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.strip-title {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
		margin: 0;
	}

	.action-strip {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}
</style>
