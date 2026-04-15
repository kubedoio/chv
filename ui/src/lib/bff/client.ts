export class BFFError extends Error {
	public readonly status: number;
	public readonly code: string;

	constructor(message: string, status: number, code: string) {
		super(message);
		this.name = 'BFFError';
		this.status = status;
		this.code = code;
	}
}

function getBaseUrl(): string {
	const g = globalThis as typeof globalThis & { process?: { env?: Record<string, string> } };
	return g.process?.env?.BFF_BASE_URL || g.process?.env?.CHV_BFF_BASE_URL || 'http://localhost:8080';
}

export async function bffFetch<T>(
	path: string,
	init?: RequestInit & { token?: string }
): Promise<T> {
	const baseUrl = getBaseUrl();
	const headers = new Headers(init?.headers ?? {});

	headers.set('Content-Type', 'application/json');
	if (init?.token) {
		headers.set('Authorization', `Bearer ${init.token}`);
	}

	let response: Response;
	const controller = new AbortController();
	const timeoutId = setTimeout(() => controller.abort(), 30000);
	try {
		response = await fetch(`${baseUrl}${path}`, {
			...init,
			headers,
			signal: controller.signal
		});
	} catch (networkError) {
		if (networkError instanceof Error && networkError.name === 'AbortError') {
			throw new BFFError('Request timed out', 0, 'TIMEOUT');
		}
		const message = networkError instanceof Error ? networkError.message : 'Network error';
		throw new BFFError(message, 0, 'NETWORK_ERROR');
	} finally {
		clearTimeout(timeoutId);
	}

	if (!response.ok) {
		let payload: { message?: string; code?: string } | undefined;
		try {
			payload = (await response.json()) as { message?: string; code?: string };
		} catch {
			payload = undefined;
		}

		const message = payload?.message ?? `Request failed with status ${response.status}`;
		const code = payload?.code ?? 'UNKNOWN_ERROR';
		throw new BFFError(message, response.status, code);
	}

	return (await response.json()) as T;
}
