<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	interface Props extends HTMLInputAttributes {
		value?: string;
		error?: string;
	}

	let {
		value = $bindable(''),
		error,
		type = 'text',
		placeholder,
		disabled,
		id,
		class: className = '',
		...rest
	}: Props = $props();

	let inputRef = $state<HTMLInputElement | null>(null);
	let isFocused = $state(false);

	export function focus() {
		inputRef?.focus();
	}

	const baseClasses =
		'h-9 w-full rounded border px-3 py-2 text-sm font-sans transition-colors duration-150';

	const stateClasses = $derived(() => {
		if (disabled) {
			return 'border-[#CCCCCC] bg-gray-50 text-muted cursor-not-allowed';
		}
		if (error) {
			return 'border-danger bg-white text-ink focus:border-danger focus:outline-none focus:ring-2 focus:ring-danger/20';
		}
		if (isFocused) {
			return 'border-primary bg-white text-ink focus:outline-none focus:ring-2 focus:ring-primary/20';
		}
		return 'border-[#CCCCCC] bg-white text-ink hover:border-muted focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20';
	});
</script>

<input
	bind:this={inputRef}
	bind:value
	{type}
	{placeholder}
	{disabled}
	{id}
	class="{baseClasses} {stateClasses()} {className}"
	onfocus={() => (isFocused = true)}
	onblur={() => (isFocused = false)}
	{...rest}
/>
