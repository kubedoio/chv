<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import VisuallyHidden from '../VisuallyHidden.svelte';

	interface Props extends HTMLInputAttributes {
		value?: string;
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
</script>

<div class="input-wrapper">
	{#if label}
		<label for={inputId} class="label">
			{label}
			{#if required}
				<span class="required-indicator" aria-hidden="true">*</span>
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
		class="input"
		class:error
		aria-invalid={error ? 'true' : 'false'}
		aria-describedby={describedBy}
		aria-required={required}
		{...rest}
	/>
	{#if error}
		<span id={errorId} class="error-message" role="alert" aria-live="assertive">
			{error}
		</span>
	{:else if hint}
		<span id={hintId} class="hint-text">
			{hint}
		</span>
	{/if}
</div>

<style>
	.input-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.label {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-neutral-700);
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	.required-indicator {
		color: var(--color-danger);
	}

	.input {
		width: 100%;
		padding: 0.625rem var(--space-3);
		font-size: var(--text-sm);
		border: 1px solid var(--color-neutral-300);
		border-radius: var(--radius-sm);
		background: white;
		color: var(--color-neutral-900);
		transition:
			border-color var(--duration-fast) var(--ease-default),
			box-shadow var(--duration-fast) var(--ease-default);
	}

	.input:hover:not(:disabled):not(.error) {
		border-color: var(--color-neutral-400);
	}

	.input:focus {
		outline: none;
		border-color: var(--color-primary);
		box-shadow: 0 0 0 3px rgba(229, 112, 53, 0.15);
	}

	.input::placeholder {
		color: var(--color-neutral-400);
	}

	.input:disabled {
		background: var(--color-neutral-100);
		color: var(--color-neutral-400);
		cursor: not-allowed;
	}

	.input.error {
		border-color: var(--color-danger);
		box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.15);
	}

	.input.error:focus {
		border-color: var(--color-danger);
		box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.2);
	}

	.error-message {
		font-size: var(--text-xs);
		color: var(--color-danger);
	}

	.hint-text {
		font-size: var(--text-xs);
		color: var(--color-neutral-500);
	}

	/* High contrast mode */
	@media (prefers-contrast: high) {
		.input:focus {
			outline: 3px solid currentColor;
			box-shadow: none;
		}
	}

	/* Reduced motion */
	@media (prefers-reduced-motion: reduce) {
		.input {
			transition: none;
		}
	}
</style>
