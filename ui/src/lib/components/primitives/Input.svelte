<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import VisuallyHidden from '../shared/VisuallyHidden.svelte';

	interface Props extends HTMLInputAttributes {
		value?: string | number;
		label?: string;
		error?: string;
		hint?: string;
		required?: boolean;
	}

	let {
		value = $bindable(''),
		label,
		error,
		hint,
		required = false,
		type = 'text',
		placeholder,
		disabled,
		id,
		...rest
	}: Props = $props();

	let generatedId = $state(`input-${Math.random().toString(36).slice(2, 11)}`);
	let inputId = $derived(id ?? generatedId);
	let errorId = $derived(error ? `${inputId}-error` : undefined);
	let hintId = $derived(hint ? `${inputId}-hint` : undefined);
	let describedBy = $derived([errorId, hintId].filter(Boolean).join(' ') || undefined);

	const baseClasses =
		'w-full py-2.5 px-3.5 text-sm rounded-sm bg-[var(--bg-surface)] text-[var(--color-neutral-900)] transition-all duration-150 ease-in-out focus:outline-none disabled:bg-[var(--color-neutral-100)] disabled:text-[var(--color-neutral-400)] disabled:cursor-not-allowed placeholder:text-[var(--color-neutral-400)]';

	const errorClasses =
		'border-[var(--color-danger)] shadow-[0_0_0_3px_var(--color-danger-glow)] hover:border-[var(--color-danger)] focus:border-[var(--color-danger)] focus:shadow-[0_0_0_3px_var(--color-danger-glow)]';

	const normalClasses =
		'border-[var(--color-neutral-300)] hover:border-[var(--color-neutral-400)] focus:border-[var(--color-primary)] focus:shadow-[0_0_0_3px_var(--color-primary-glow)]';
</script>

<div class="flex flex-col gap-2">
	{#if label}
		<label for={inputId} class="text-sm font-medium text-[var(--color-neutral-700)] flex items-center gap-1">
			{label}
			{#if required}
				<span class="text-[var(--color-danger)]" aria-hidden="true">*</span>
				<VisuallyHidden>(required)</VisuallyHidden>
			{/if}
		</label>
	{/if}
	<input
		bind:value
		{type}
		{placeholder}
		{disabled}
		id={inputId}
		class="{baseClasses} {error ? errorClasses : normalClasses}"
		aria-invalid={error ? 'true' : 'false'}
		aria-describedby={describedBy}
		aria-required={required}
		{...rest}
	/>
	{#if error}
		<span id={errorId} class="text-xs text-[var(--color-danger)]" role="alert" aria-live="assertive">
			{error}
		</span>
	{:else if hint}
		<span id={hintId} class="text-xs text-[var(--color-neutral-500)]">
			{hint}
		</span>
	{/if}
</div>
