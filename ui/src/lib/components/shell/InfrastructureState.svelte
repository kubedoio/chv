<script lang="ts">
	import { AlertTriangle, Inbox, LoaderCircle } from 'lucide-svelte';

	interface Props {
		variant: 'loading' | 'empty' | 'error';
		title: string;
		description: string;
		hint: string;
	}

	let { variant, title, description, hint }: Props = $props();

	const Icon = $derived(
		variant === 'loading' ? LoaderCircle : variant === 'error' ? AlertTriangle : Inbox
	);
</script>

<article class={`state-panel state-panel--${variant}`}>
	<div class="state-panel__icon" aria-hidden="true">
		<Icon size={18} class={variant === 'loading' ? 'state-panel__icon--spinning' : undefined} />
	</div>
	<div class="state-panel__content">
		<div class="state-panel__title">{title}</div>
		<p class="state-panel__description">{description}</p>
		<p class="state-panel__hint">{hint}</p>
		{#if variant === 'loading'}
			<div class="state-panel__skeletons" aria-hidden="true">
				<span></span>
				<span></span>
				<span></span>
			</div>
		{/if}
	</div>
</article>

<style>
	.state-panel {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 1rem;
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		background: var(--bg-surface);
		padding: 2rem;
		position: relative;
		overflow: hidden;
		box-shadow: var(--shadow-sm);
	}

	.state-panel::before {
		content: '';
		position: absolute;
		inset: 0;
		background-image: radial-gradient(var(--dot-grid) 1px, transparent 0);
		background-size: 20px 20px;
		opacity: 0.5;
		pointer-events: none;
	}

	.state-panel__icon {
		display: grid;
		place-items: center;
		width: 3rem;
		height: 3rem;
		border-radius: var(--radius-sm);
		background: var(--bg-surface-muted);
		border: 1px solid var(--border-subtle);
		color: var(--color-neutral-400);
		position: relative;
		z-index: 1;
	}

	.state-panel--loading .state-panel__icon { color: var(--color-primary); }
	.state-panel--empty .state-panel__icon { color: var(--color-neutral-300); }
	.state-panel--error .state-panel__icon { color: var(--color-danger); border-color: var(--color-danger); }

	.state-panel__icon--spinning {
		animation: shell-spin 1s linear infinite;
	}

	.state-panel__content {
		position: relative;
		z-index: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
	}

	.state-panel__title {
		font-size: var(--text-base);
		font-weight: 700;
		color: var(--color-neutral-900);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.state-panel__description {
		margin-top: 0.5rem;
		font-size: var(--text-sm);
		color: var(--color-neutral-600);
		max-width: 400px;
	}

	.state-panel__hint {
		margin-top: 0.75rem;
		font-size: var(--text-xs);
		color: var(--color-neutral-400);
		font-style: italic;
	}

	.state-panel__skeletons {
		display: grid;
		gap: 0.5rem;
		margin-top: 1rem;
		width: 100%;
		max-width: 300px;
	}

	.state-panel__skeletons span {
		display: block;
		height: 4px;
		border-radius: 2px;
		background: var(--color-neutral-100);
		position: relative;
		overflow: hidden;
	}

	.state-panel__skeletons span::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(90deg, transparent, var(--color-neutral-200), transparent);
		animation: shell-shimmer 1.5s infinite;
	}

	@keyframes shell-spin {
		to { transform: rotate(360deg); }
	}

	@keyframes shell-shimmer {
		0% { transform: translateX(-100%); }
		100% { transform: translateX(100%); }
	}

	@media (prefers-reduced-motion: reduce) {
		.state-panel__icon--spinning,
		.state-panel__skeletons span {
			animation: none;
		}
	}
</style>
