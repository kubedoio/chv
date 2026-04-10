<script lang="ts">
	import { onMount } from 'svelte';
	import { browser } from '$app/environment';

	type AnnouncementPriority = 'polite' | 'assertive';

	interface Announcement {
		id: string;
		message: string;
		priority: AnnouncementPriority;
	}

	// Store for announcements - using a reactive Set for Svelte 5
	let announcements = $state<Announcement[]>([]);

	/**
	 * Announce a message to screen readers
	 */
	export function announce(message: string, priority: AnnouncementPriority = 'polite') {
		if (!browser) return;
		
		const id = `announce-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
		const announcement: Announcement = { id, message, priority };
		
		announcements = [...announcements, announcement];

		// Remove the announcement after it's been read
		setTimeout(() => {
			announcements = announcements.filter(a => a.id !== id);
		}, 1000);
	}

	/**
	 * Announce a toast notification
	 */
	export function announceToast(type: string, message: string) {
		const prefix = type === 'error' ? 'Error: ' : type === 'success' ? 'Success: ' : '';
		announce(`${prefix}${message}`, type === 'error' ? 'assertive' : 'polite');
	}

	/**
	 * Announce a loading state
	 */
	export function announceLoading(message: string) {
		announce(`Loading: ${message}`, 'polite');
	}

	/**
	 * Announce completion
	 */
	export function announceComplete(message: string) {
		announce(`Complete: ${message}`, 'polite');
	}

	/**
	 * Announce batch progress
	 */
	export function announceBatchProgress(current: number, total: number, operation: string) {
		const percent = Math.round((current / total) * 100);
		announce(`${operation} progress: ${current} of ${total} (${percent}%)`, 'polite');
	}

	// Filter by priority
	let politeAnnouncements = $derived(announcements.filter(a => a.priority === 'polite'));
	let assertiveAnnouncements = $derived(announcements.filter(a => a.priority === 'assertive'));
</script>

<!-- Live region for polite announcements -->
<div
	role="status"
	aria-live="polite"
	aria-atomic="true"
	class="sr-only"
>
	{#each politeAnnouncements as announcement (announcement.id)}
		{announcement.message}
	{/each}
</div>

<!-- Live region for assertive announcements -->
<div
	role="alert"
	aria-live="assertive"
	aria-atomic="true"
	class="sr-only"
>
	{#each assertiveAnnouncements as announcement (announcement.id)}
		{announcement.message}
	{/each}
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
