export type ToastType = 'success' | 'error' | 'info';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration?: number; // ms, undefined = no auto-dismiss
}

// Generate a simple unique ID (works in all browsers)
function generateId(): string {
	return `${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 9)}`;
}

let toasts = $state<Toast[]>([]);
const timeouts = new Map<string, ReturnType<typeof setTimeout>>();

function showToast(message: string, type: ToastType, duration?: number): void {
	const id = generateId();
	const newToast: Toast = { id, type, message, duration };

	toasts = [...toasts, newToast];

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

	toasts = toasts.filter((t) => t.id !== id);
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

export const toast = {
	get toasts() {
		return toasts;
	},
	showToast,
	success,
	error,
	info,
	dismiss
};
