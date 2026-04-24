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

<article class="grid gap-4 relative overflow-hidden border border-[var(--border-subtle)] rounded-[var(--radius-sm)] bg-[var(--bg-surface)] p-8 shadow-[var(--shadow-sm)] state-panel state-panel--{variant}" style="grid-template-columns: auto 1fr;">
	<div class="grid place-items-center w-12 h-12 rounded-[var(--radius-sm)] bg-[var(--bg-surface-muted)] border border-[var(--border-subtle)] text-[var(--color-neutral-400)] relative z-[1] {variant === 'loading' ? 'text-[var(--color-primary)]' : variant === 'empty' ? 'text-[var(--color-neutral-300)]' : 'text-[var(--color-danger)] border-[var(--color-danger)]'}" aria-hidden="true">
		<Icon size={18} class={variant === 'loading' ? 'animate-spin' : undefined} />
	</div>
	<div class="relative z-[1] flex flex-col justify-center">
		<div class="text-[length:var(--text-base)] font-bold text-[var(--color-neutral-900)] uppercase tracking-[0.05em]">{title}</div>
		<p class="mt-2 text-[length:var(--text-sm)] text-[var(--color-neutral-600)] max-w-[400px]">{description}</p>
		<p class="mt-3 text-[length:var(--text-xs)] text-[var(--color-neutral-400)] italic">{hint}</p>
		{#if variant === 'loading'}
			<div class="grid gap-2 mt-4 w-full max-w-[300px]" aria-hidden="true">
				<span class="block h-1 rounded-sm bg-[var(--color-neutral-100)] relative overflow-hidden skeleton-bar"></span>
				<span class="block h-1 rounded-sm bg-[var(--color-neutral-100)] relative overflow-hidden skeleton-bar"></span>
				<span class="block h-1 rounded-sm bg-[var(--color-neutral-100)] relative overflow-hidden skeleton-bar"></span>
			</div>
		{/if}
	</div>
</article>

<style>
	.state-panel::before {
		content: '';
		position: absolute;
		inset: 0;
		background-image: radial-gradient(var(--dot-grid) 1px, transparent 0);
		background-size: 20px 20px;
		opacity: 0.5;
		pointer-events: none;
	}

	.skeleton-bar::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(90deg, transparent, var(--color-neutral-200), transparent);
		animation: shell-shimmer 1.5s infinite;
	}

	@keyframes shell-shimmer {
		0% { transform: translateX(-100%); }
		100% { transform: translateX(100%); }
	}

	@media (prefers-reduced-motion: reduce) {
		.skeleton-bar::after {
			animation: none;
		}
	}
</style>
