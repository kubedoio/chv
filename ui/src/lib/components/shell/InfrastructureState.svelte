<script lang="ts">
	import { AlertTriangle, Inbox, LoaderCircle } from 'lucide-svelte';

	interface Props {
		variant: 'loading' | 'empty' | 'error';
		title: string;
		description: string;
		hint: string;
	}

	let { variant, title, description, hint }: Props = $props();

	const icon = $derived(
		variant === 'loading' ? LoaderCircle : variant === 'error' ? AlertTriangle : Inbox
	);
</script>

<article class={`state-panel state-panel--${variant}`}>
	<div class="state-panel__icon" aria-hidden="true">
		<icon size={18} class={variant === 'loading' ? 'state-panel__icon--spinning' : undefined}></icon>
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
		gap: 0.75rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.5rem;
		background: var(--shell-surface-muted);
		padding: 0.75rem;
	}

	.state-panel__icon {
		display: grid;
		place-items: center;
		width: 1.5rem;
		height: 1.5rem;
		border-radius: 999px;
		background: var(--shell-surface);
		border: 1px solid var(--shell-line);
		color: var(--shell-text-secondary);
	}

	.state-panel--loading .state-panel__icon {
		color: var(--status-unknown-text);
	}

	.state-panel--empty .state-panel__icon {
		color: var(--status-healthy-text);
	}

	.state-panel--error .state-panel__icon {
		color: var(--status-failed-text);
	}

	.state-panel__icon--spinning {
		animation: shell-spin 1s linear infinite;
	}

	.state-panel__content {
		min-width: 0;
	}

	.state-panel__title {
		font-size: var(--text-base);
		font-weight: 600;
		color: var(--shell-text);
	}

	.state-panel__description,
	.state-panel__hint {
		margin-top: 0.3rem;
		font-size: var(--text-sm);
		line-height: 1.45;
	}

	.state-panel__description {
		color: var(--shell-text-secondary);
	}

	.state-panel__hint {
		color: var(--shell-text-muted);
	}

	.state-panel__skeletons {
		display: grid;
		gap: 0.4rem;
		margin-top: 0.8rem;
	}

	.state-panel__skeletons span {
		display: block;
		height: 0.55rem;
		border-radius: 999px;
		background: linear-gradient(
			90deg,
			rgba(169, 160, 147, 0.16),
			rgba(169, 160, 147, 0.34),
			rgba(169, 160, 147, 0.16)
		);
		background-size: 200% 100%;
		animation: shell-shimmer 1.5s ease-in-out infinite;
	}

	.state-panel__skeletons span:nth-child(2) {
		width: 82%;
	}

	.state-panel__skeletons span:nth-child(3) {
		width: 68%;
	}

	@keyframes shell-spin {
		to {
			transform: rotate(360deg);
		}
	}

	@keyframes shell-shimmer {
		0% {
			background-position: 200% 0;
		}

		100% {
			background-position: -200% 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.state-panel__icon--spinning,
		.state-panel__skeletons span {
			animation: none;
		}
	}
</style>
