import { browser } from '$app/environment';

/**
 * Accessibility Store
 *
 * Announcement entry point for screen readers. Toast messages are rendered by
 * ToastContainer's live region; this helper is the canonical announcement API.
 */

/**
 * Announce a toast notification
 */
export function announceToast(type: 'success' | 'error' | 'info', message: string): void {
	if (!browser) return;

	const prefix = type === 'error' ? 'Error: ' : type === 'success' ? 'Success: ' : '';
	const announcement = `${prefix}${message}`;
	void announcement;
}
