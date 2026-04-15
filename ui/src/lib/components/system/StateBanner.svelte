<script lang="ts">
	import { AlertTriangle, CheckCircle, Inbox, LoaderCircle } from 'lucide-svelte';

	interface Props {
		variant: 'loading' | 'empty' | 'error' | 'degraded' | 'success';
		title: string;
		description: string;
		hint?: string;
	}

	let { variant, title, description, hint }: Props = $props();

	const Icon = $derived(
		variant === 'loading'
			? LoaderCircle
			: variant === 'empty'
				? Inbox
				: variant === 'success'
					? CheckCircle
					: AlertTriangle
	);
</script>

<article class="state-banner state-banner--{variant}" aria-live={variant === 'loading' ? 'polite' : 'assertive'}>
	<div class="state-banner__icon" aria-hidden="true">
		<Icon size={18} class={variant === 'loading' ? 'state-banner__spin' : undefined} />
	</div>
	<div class="state-banner__content">
		<div class="state-banner__title">{title}</div>
		<p class="state-banner__description">{description}</p>
		{#if hint}
			<p class="state-banner__hint">{hint}</p>
		{/if}
		{#if variant === 'loading'}
			<div class="state-banner__skeletons" aria-hidden="true">
				<span></span>
				<span></span>
				<span></span>
			</div>
		{/if}
	</div>
</article>

<style>
	.state-banner {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.9rem;
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface-muted);
		padding: 1rem;
	}

	.state-banner__icon {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: 999px;
		background: var(--shell-surface);
		border: 1px solid var(--shell-line);
		color: var(--shell-text-secondary);
	}

	.state-banner--loading .state-banner__icon {
		color: var(--status-unknown-text);
	}

	.state-banner--empty .state-banner__icon,
.state-banner--success .state-banner__icon {
		color: var(--status-healthy-text);
	}

	.state-banner--error .state-banner__icon,
	.state-banner--degraded .state-banner__icon {
		color: var(--status-failed-text);
	}

	.state-banner__spin {
		animation: shell-spin 1s linear infinite;
	}

	.state-banner__content {
		min-width: 0;
	}

	.state-banner__title {
		font-size: 0.95rem;
		font-weight: 600;
		color: var(--shell-text);
	}

	.state-banner__description,
	.state-banner__hint {
		margin-top: 0.3rem;
		font-size: 0.88rem;
		line-height: 1.45;
	}

	.state-banner__description {
		color: var(--shell-text-secondary);
	}

	.state-banner__hint {
		color: var(--shell-text-muted);
	}

	.state-banner__skeletons {
		display: grid;
		gap: 0.4rem;
		margin-top: 0.8rem;
	}

	.state-banner__skeletons span {
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

	.state-banner__skeletons span:nth-child(2) {
		width: 82%;
	}

	.state-banner__skeletons span:nth-child(3) {
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
		.state-banner__spin,
		.state-banner__skeletons span {
			animation: none;
		}
	}
</style>
