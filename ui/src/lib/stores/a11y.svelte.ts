import { browser } from '$app/environment';

/**
 * Accessibility Store
 * 
 * Manages accessibility-related state and announcements for screen readers.
 */

// Announcement types
type AnnouncementPriority = 'polite' | 'assertive';

interface Announcement {
	id: string;
	message: string;
	priority: AnnouncementPriority;
	timestamp: number;
}

// Reactive state using Svelte 5 runes
let announcements = $state<Announcement[]>([]);
let reducedMotion = $state(false);
let highContrast = $state(false);

/**
 * Initialize accessibility preferences from system settings
 */
export function initA11yPreferences(): void {
	if (!browser) return;

	// Check reduced motion preference
	const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
	reducedMotion = motionQuery.matches;
	
	motionQuery.addEventListener('change', (e) => {
		reducedMotion = e.matches;
	});

	// Check high contrast preference (Windows)
	const contrastQuery = window.matchMedia('(prefers-contrast: high)');
	highContrast = contrastQuery.matches;
	
	contrastQuery.addEventListener('change', (e) => {
		highContrast = e.matches;
	});
}

/**
 * Announce a message to screen readers
 */
export function announce(message: string, priority: AnnouncementPriority = 'polite'): void {
	if (!browser) return;
	
	const id = `announce-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
	const announcement: Announcement = { 
		id, 
		message, 
		priority,
		timestamp: Date.now()
	};
	
	announcements = [...announcements, announcement];

	// Remove the announcement after it's been read
	setTimeout(() => {
		announcements = announcements.filter(a => a.id !== id);
	}, 1000);
}

/**
 * Announce a toast notification
 */
export function announceToast(type: 'success' | 'error' | 'info', message: string): void {
	const prefix = type === 'error' ? 'Error: ' : type === 'success' ? 'Success: ' : '';
	announce(`${prefix}${message}`, type === 'error' ? 'assertive' : 'polite');
}

/**
 * Announce a loading state
 */
export function announceLoading(message: string): void {
	announce(`Loading: ${message}`, 'polite');
}

/**
 * Announce completion
 */
export function announceComplete(message: string): void {
	announce(`Complete: ${message}`, 'polite');
}

/**
 * Announce batch progress
 */
export function announceBatchProgress(current: number, total: number, operation: string): void {
	const percent = Math.round((current / total) * 100);
	announce(`${operation} progress: ${current} of ${total} (${percent}%)`, 'polite');
}

/**
 * Announce page changes
 */
export function announcePageChange(pageName: string): void {
	announce(`Navigated to ${pageName}`, 'polite');
}

/**
 * Get current announcements
 */
export function getAnnouncements(): Announcement[] {
	return announcements;
}

/**
 * Check if reduced motion is preferred
 */
export function prefersReducedMotion(): boolean {
	return reducedMotion;
}

/**
 * Check if high contrast is preferred
 */
export function prefersHighContrast(): boolean {
	return highContrast;
}

/**
 * Get animation duration accounting for reduced motion
 */
export function getAnimationDuration(normalDuration: number): number {
	return reducedMotion ? 0 : normalDuration;
}

/**
 * Get transition config respecting reduced motion
 */
export function getTransitionConfig() {
	return {
		duration: reducedMotion ? 0 : 200,
		easing: 'ease-out'
	};
}

// Export reactive state for components to subscribe to
export const a11yStore = {
	get announcements() { return announcements; },
	get reducedMotion() { return reducedMotion; },
	get highContrast() { return highContrast; }
};
