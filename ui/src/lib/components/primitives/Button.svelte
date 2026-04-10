<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { Loader2 } from 'lucide-svelte';

	interface Props extends HTMLButtonAttributes {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md' | 'lg';
		loading?: boolean;
		disabled?: boolean;
		ariaLabel?: string;
		title?: string;
	}

	let {
		variant = 'primary',
		size = 'md',
		loading = false,
		disabled = false,
		ariaLabel,
		title,
		children,
		...rest
	}: Props = $props();

	const isDisabled = $derived(disabled || loading);

	const variantClasses = {
		primary: 'btn-primary',
		secondary: 'btn-secondary',
		ghost: 'btn-ghost',
		danger: 'btn-danger'
	};

	const sizeClasses = {
		sm: 'btn-sm',
		md: 'btn-md',
		lg: 'btn-lg'
	};

	// Check if button has only icon (no text content)
	let buttonRef = $state<HTMLButtonElement | null>(null);
	let isIconOnly = $state(false);

	$effect(() => {
		if (buttonRef && !children) {
			// If no children slot is provided, it's likely an icon-only button
			isIconOnly = true;
		}
	});
</script>

<button
	bind:this={buttonRef}
	type="button"
	class="btn {variantClasses[variant]} {sizeClasses[size]}"
	class:icon-only={isIconOnly}
	disabled={isDisabled}
	aria-disabled={isDisabled}
	aria-busy={loading}
	aria-label={ariaLabel}
	{title}
	{...rest}
>
	{#if loading}
		<Loader2 size={size === 'lg' ? 20 : 16} aria-hidden="true" class="animate-spin" />
	{/if}
	{@render children?.()}
</button>

<style>
	.btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
		font-weight: 500;
		border-radius: var(--radius-sm);
		border: none;
		cursor: pointer;
		transition:
			background-color var(--duration-fast) var(--ease-default),
			border-color var(--duration-fast) var(--ease-default),
			box-shadow var(--duration-fast) var(--ease-default),
			transform var(--duration-fast) var(--ease-default);
	}

	.btn:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Icon-only button styles */
	.btn.icon-only {
		padding: var(--space-2);
	}

	.btn.icon-only.btn-sm {
		width: 2rem;
		padding: 0;
	}

	.btn.icon-only.btn-md {
		width: 2.5rem;
		padding: 0;
	}

	.btn.icon-only.btn-lg {
		width: 3rem;
		padding: 0;
	}

	/* Button Sizes */
	.btn-sm {
		padding: var(--space-1) var(--space-3);
		height: 2rem;
	}

	.btn-md {
		padding: 0.625rem var(--space-4);
		height: 2.5rem;
	}

	.btn-lg {
		padding: var(--space-3) var(--space-5);
		height: 3rem;
		font-size: var(--text-base);
	}

	/* Button Variants */
	.btn-primary {
		background: linear-gradient(135deg, var(--color-primary) 0%, var(--color-primary-active) 100%);
		color: white;
		box-shadow: 0 2px 8px rgba(229, 112, 53, 0.3);
	}

	.btn-primary:hover:not(:disabled) {
		background: linear-gradient(135deg, var(--color-primary-hover) 0%, #e05a35 100%);
		box-shadow: 0 4px 12px rgba(229, 112, 53, 0.4);
		transform: translateY(-1px);
	}

	.btn-primary:active:not(:disabled) {
		transform: translateY(0);
		box-shadow: 0 1px 4px rgba(229, 112, 53, 0.3);
	}

	.btn-secondary {
		background: white;
		color: var(--color-neutral-700);
		border: 1px solid var(--color-neutral-300);
	}

	.btn-secondary:hover:not(:disabled) {
		background: var(--color-neutral-50);
		border-color: var(--color-neutral-400);
	}

	.btn-ghost {
		background: transparent;
		color: var(--color-neutral-600);
	}

	.btn-ghost:hover:not(:disabled) {
		background: var(--color-neutral-100);
		color: var(--color-neutral-900);
	}

	.btn-danger {
		background: var(--color-danger);
		color: white;
	}

	.btn-danger:hover:not(:disabled) {
		background: #dc2626;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.animate-spin {
		animation: spin 1s linear infinite;
	}

	/* High contrast mode */
	@media (prefers-contrast: high) {
		.btn {
			border: 1px solid currentColor;
		}
		
		.btn:focus-visible {
			outline: 3px solid currentColor;
			outline-offset: 2px;
		}
	}

	/* Reduced motion */
	@media (prefers-reduced-motion: reduce) {
		.btn {
			transition: none;
		}
		
		.btn:hover:not(:disabled) {
			transform: none;
		}
	}
</style>
