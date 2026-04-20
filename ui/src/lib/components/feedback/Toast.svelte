<script lang="ts">
	import { toast, type ToastType } from '$lib/stores/toast';

	interface Props {
		id: string;
		type: ToastType;
		message: string;
	}

	let { id, type, message }: Props = $props();

	const styles: Record<ToastType, { bg: string; border: string; iconColor: string; label: string }> = {
		success: {
			bg: 'bg-[var(--color-success-light)]',
			border: 'border-l-[var(--color-success)]',
			iconColor: 'text-[var(--color-success)]',
			label: 'Success'
		},
		error: {
			bg: 'bg-[var(--color-danger-light)]',
			border: 'border-l-[var(--color-danger)]',
			iconColor: 'text-[var(--color-danger)]',
			label: 'Error'
		},
		info: {
			bg: 'bg-[var(--color-info-light)]',
			border: 'border-l-[var(--color-info)]',
			iconColor: 'text-[var(--color-info)]',
			label: 'Information'
		}
	};

	let style = $derived(styles[type]);

	function handleDismiss() {
		toast.dismiss(id);
	}
</script>

<div
	class="w-[320px] max-w-full rounded shadow-[0_4px_12px_rgba(0,0,0,0.15)] border-l-4 flex items-start gap-3 p-4 {style.bg} {style.border}"
	role="alert"
	aria-live={type === 'error' ? 'assertive' : 'polite'}
	aria-atomic="true"
>
	<!-- Icon with aria-label for screen readers -->
	<div class="flex-shrink-0 {style.iconColor}" aria-hidden="true">
		{#if type === 'success'}
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="20"
				height="20"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M20 6 9 17l-5-5" />
			</svg>
		{:else if type === 'error'}
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="20"
				height="20"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M12 16h.01" />
				<path d="M12 8v4" />
				<path
					d="M15.312 2a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586l-4.688-4.688A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2z"
				/>
			</svg>
		{:else}
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="20"
				height="20"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<circle cx="12" cy="12" r="10" />
				<path d="M12 16v-4" />
				<path d="M12 8h.01" />
			</svg>
		{/if}
	</div>

	<!-- Message with visually hidden type label -->
	<div class="flex-1 text-sm text-ink leading-5">
		<span class="sr-only">{style.label}:</span>
		{message}
	</div>

	<!-- Close Button -->
	<button
		onclick={handleDismiss}
		class="flex-shrink-0 p-1 rounded hover:bg-black/5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1"
		aria-label="Dismiss {style.label.toLowerCase()} notification"
		type="button"
	>
		<svg
			xmlns="http://www.w3.org/2000/svg"
			width="16"
			height="16"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			class="text-muted"
			aria-hidden="true"
		>
			<path d="M18 6 6 18" />
			<path d="m6 6 12 12" />
		</svg>
	</button>
</div>

<style>
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
