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
		default: 'bg-[var(--color-neutral-100)] text-[var(--color-neutral-600)] border-[var(--color-neutral-200)]',
		success: 'bg-[var(--color-success)]/15 text-[var(--color-success-dark)] border-[var(--color-success)]/20',
		warning: 'bg-[var(--color-warning)]/15 text-[var(--color-warning-dark)] border-[var(--color-warning)]/20',
		danger: 'bg-[var(--color-danger)]/15 text-[var(--color-danger-dark)] border-[var(--color-danger)]/20',
		info: 'bg-[var(--color-info)]/15 text-[var(--color-info-dark)] border-[var(--color-info)]/20'
	};

	const dotClasses = {
		default: 'bg-[var(--color-neutral-400)]',
		success: 'bg-[var(--color-success)] shadow-[var(--shadow-glow-success)]',
		warning: 'bg-[var(--color-warning)]',
		danger: 'bg-[var(--color-danger)]',
		info: 'bg-[var(--color-info)]'
	};
</script>

<span
	class="inline-flex items-center gap-1 px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide rounded-full border {variantClasses[variant]}"
	role="status"
>
	{#if dot}
		<span class="w-1.5 h-1.5 rounded-full {dotClasses[variant]} {animate ? 'animate-pulse' : ''}" aria-hidden="true"
		></span>
	{/if}
	{@render children?.()}
</span>
