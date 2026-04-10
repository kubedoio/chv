<script lang="ts">
	interface Props {
		variant?: 'default' | 'success' | 'warning' | 'danger' | 'info';
		dot?: boolean;
		animate?: boolean;
	}

	let {
		variant = 'default',
		dot = false,
		animate = false,
		children
	}: Props & { children?: import('svelte').Snippet } = $props();

	const variantClasses = {
		default: 'badge-default',
		success: 'badge-success',
		warning: 'badge-warning',
		danger: 'badge-danger',
		info: 'badge-info'
	};

	const dotClasses = {
		default: 'dot-default',
		success: 'dot-success',
		warning: 'dot-warning',
		danger: 'dot-danger',
		info: 'dot-info'
	};
</script>

<span class="badge {variantClasses[variant]}" role="status">
	{#if dot}
		<span class="status-dot {dotClasses[variant]} {animate ? 'animate-pulse' : ''}" aria-hidden="true"></span>
	{/if}
	{@render children?.()}
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		padding: 0.25rem 0.625rem;
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		border-radius: var(--radius-full);
		border: 1px solid transparent;
	}

	.badge-default {
		background: var(--color-neutral-100);
		color: var(--color-neutral-600);
		border-color: var(--color-neutral-200);
	}

	.badge-success {
		background: rgba(34, 197, 94, 0.15);
		color: var(--color-success-dark);
		border-color: rgba(34, 197, 94, 0.2);
	}

	.badge-warning {
		background: rgba(234, 179, 8, 0.15);
		color: var(--color-warning-dark);
		border-color: rgba(234, 179, 8, 0.2);
	}

	.badge-danger {
		background: rgba(239, 68, 68, 0.15);
		color: var(--color-danger-dark);
		border-color: rgba(239, 68, 68, 0.2);
	}

	.badge-info {
		background: rgba(59, 130, 246, 0.15);
		color: var(--color-info-dark);
		border-color: rgba(59, 130, 246, 0.2);
	}

	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}

	.dot-default {
		background: var(--color-neutral-400);
	}

	.dot-success {
		background: var(--color-success);
		box-shadow: var(--shadow-glow-success);
	}

	.dot-warning {
		background: var(--color-warning);
	}

	.dot-danger {
		background: var(--color-danger);
	}

	.dot-info {
		background: var(--color-info);
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}

	.animate-pulse {
		animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
	}
</style>
