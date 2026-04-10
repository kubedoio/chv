<script lang="ts">
	import { flip } from 'svelte/animate';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { toast } from '$lib/stores/toast';
	import Toast from './Toast.svelte';
	import { announceToast } from '$lib/stores/a11y.svelte';

	// Announce toast changes for screen readers
	$effect(() => {
		const toasts = $toast.toasts;
		if (toasts.length > 0) {
			const latest = toasts[toasts.length - 1];
			announceToast(latest.type, latest.message);
		}
	});
</script>

<!-- Toast Container with role="region" and aria-label for landmark -->
<div 
	class="toast-container"
	role="region"
	aria-label="Notifications"
	aria-live="polite"
	aria-atomic="false"
>
	{#each $toast.toasts as t (t.id)}
		<div 
			animate:flip={{ duration: 200 }}
			in:fly={{ y: 20, duration: 300, easing: cubicOut }}
			out:fly={{ y: -20, duration: 200 }}
			class="toast-wrapper"
		>
			<Toast id={t.id} type={t.type} message={t.message} />
		</div>
	{/each}
</div>

<style>
	.toast-container {
		position: fixed;
		top: 1rem;
		right: 1rem;
		z-index: 100;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		pointer-events: none;
		max-width: calc(100vw - 2rem);
	}

	.toast-wrapper {
		pointer-events: auto;
	}

	/* Mobile adjustments */
	@media (max-width: 640px) {
		.toast-container {
			left: 1rem;
			right: 1rem;
		}
	}
</style>
