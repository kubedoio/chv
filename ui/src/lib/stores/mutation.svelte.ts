import { liveState } from './live-state.svelte';
import { toast } from './toast.svelte';

export interface MutateOpts<T = unknown> {
	patterns?: string[];
	sidebar?: boolean;
	detailId?: string;
	delayMs?: number;
	successMessage?: string | ((result: T) => string);
	errorMessage?: string;
	skipRefresh?: boolean;
}

function getErrorMessage(err: unknown): string {
	if (err instanceof Error) return err.message;
	if (typeof err === 'string') return err;
	return 'Operation failed';
}

export async function mutateWithRefresh<T>(
	mutator: () => Promise<T>,
	opts: MutateOpts<T> = {}
): Promise<T> {
	try {
		const result = await mutator();
		if (opts.successMessage) {
			const message = typeof opts.successMessage === 'function' ? opts.successMessage(result) : opts.successMessage;
			toast.success(message);
		}

		if (!opts.skipRefresh) {
			try {
				await liveState.invalidateAndRefresh({
					patterns: opts.patterns,
					sidebar: opts.sidebar ?? true,
					detailId: opts.detailId,
					delayMs: opts.delayMs,
				});
			} catch (refreshErr) {
				// TODO: integrate structured logger instead of console
				// eslint-disable-next-line no-console
				console.warn('[mutateWithRefresh] refresh failed after successful mutation:', refreshErr);
			}
		}

		return result;
	} catch (err: unknown) {
		const msg = opts.errorMessage || getErrorMessage(err);
		toast.error(msg);
		throw err;
	}
}
