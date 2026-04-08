<script lang="ts">
	import { toast, type ToastType } from '$lib/stores/toast';

	interface Props {
		id: string;
		type: ToastType;
		message: string;
	}

	let { id, type, message }: Props = $props();

	const styles: Record<ToastType, { bg: string; border: string; iconColor: string }> = {
		success: {
			bg: 'bg-[#F0F9F0]',
			border: 'border-l-[#54B435]',
			iconColor: 'text-[#54B435]'
		},
		error: {
			bg: 'bg-[#FFF0F0]',
			border: 'border-l-[#E60000]',
			iconColor: 'text-[#E60000]'
		},
		info: {
			bg: 'bg-[#E8F4FC]',
			border: 'border-l-[#0066CC]',
			iconColor: 'text-[#0066CC]'
		}
	};

	let style = $derived(styles[type]);

	function handleDismiss() {
		toast.dismiss(id);
	}
</script>

<div
	class="w-[320px] rounded shadow-[0_4px_12px_rgba(0,0,0,0.15)] border-l-4 flex items-start gap-3 p-4 {style.bg} {style.border}"
	role="alert"
	aria-live="polite"
>
	<!-- Icon -->
	<div class="flex-shrink-0 {style.iconColor}">
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
				aria-hidden="true"
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
				aria-hidden="true"
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
				aria-hidden="true"
			>
				<circle cx="12" cy="12" r="10" />
				<path d="M12 16v-4" />
				<path d="M12 8h.01" />
			</svg>
		{/if}
	</div>

	<!-- Message -->
	<div class="flex-1 text-sm text-ink leading-5">
		{message}
	</div>

	<!-- Close Button -->
	<button
		onclick={handleDismiss}
		class="flex-shrink-0 p-1 rounded hover:bg-black/5 transition-colors"
		aria-label="Dismiss notification"
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
