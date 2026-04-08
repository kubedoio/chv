<script lang="ts">
	import { toast } from '$lib/stores/toast';
	import Toast from './Toast.svelte';
</script>

{#if $toast.toasts.length > 0}
	<div
		class="fixed top-4 right-4 z-50 flex flex-col gap-2"
		role="region"
		aria-label="Notifications"
	>
		{#each $toast.toasts as toastItem (toastItem.id)}
			<Toast
				id={toastItem.id}
				type={toastItem.type}
				message={toastItem.message}
			/>
		{/each}
	</div>
{/if}
