<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		error?: string;
		helper?: string;
		required?: boolean;
		labelFor?: string;
		children: Snippet;
	}

	let {
		label,
		error,
		helper,
		required = false,
		labelFor,
		children
	}: Props = $props();
</script>

<div class="flex flex-col gap-1">
	<!-- Label -->
	<label
		for={labelFor}
		class="text-xs font-medium text-muted"
	>
		{label}
		{#if required}
			<span class="text-danger">*</span>
		{/if}
	</label>

	<!-- Input/Select -->
	{@render children()}

	<!-- Helper Text -->
	{#if helper && !error}
		<p class="text-xs text-muted mt-1">
			{helper}
		</p>
	{/if}

	<!-- Error Message -->
	{#if error}
		<div class="flex items-center gap-1.5 mt-1" role="alert">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="14"
				height="14"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				class="text-danger flex-shrink-0"
				aria-hidden="true"
			>
				<circle cx="12" cy="12" r="10" />
				<line x1="12" x2="12" y1="8" y2="12" />
				<line x1="12" x2="12.01" y1="16" y2="16" />
			</svg>
			<p class="text-xs text-danger">{error}</p>
		</div>
	{/if}
</div>
