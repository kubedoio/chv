import { writable } from 'svelte/store';

export type ToastType = 'success' | 'error' | 'info';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration?: number; // ms, undefined = no auto-dismiss
}

interface ToastState {
	toasts: Toast[];
}

// Generate a simple unique ID (works in all browsers)
function generateId(): string {
	return `${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 9)}`;
}

function createToastStore() {
	const { subscribe, update } = writable<ToastState>({ toasts: [] });

	const timeouts = new Map<string, ReturnType<typeof setTimeout>>();

	function showToast(message: string, type: ToastType, duration?: number): void {
		const id = generateId();
		const toast: Toast = { id, type, message, duration };

		update((state) => ({
			toasts: [...state.toasts, toast]
		}));

		// Set up auto-dismiss if duration is provided
		if (duration !== undefined && duration > 0) {
			const timeout = setTimeout(() => {
				dismiss(id);
			}, duration);
			timeouts.set(id, timeout);
		}
	}

	function dismiss(id: string): void {
		// Clear any pending timeout for this toast
		const timeout = timeouts.get(id);
		if (timeout) {
			clearTimeout(timeout);
			timeouts.delete(id);
		}

		update((state) => ({
			toasts: state.toasts.filter((t) => t.id !== id)
		}));
	}

	function success(message: string): void {
		showToast(message, 'success', 5000);
	}

	function error(message: string): void {
		showToast(message, 'error'); // No duration = manual close only
	}

	function info(message: string): void {
		showToast(message, 'info', 5000);
	}

	return {
		subscribe,
		showToast,
		success,
		error,
		info,
		dismiss
	};
}

export const toast = createToastStore();
