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

function getHeader(response: Response, name: string): string | null {
	return response.headers?.get?.(name) ?? null;
}

function isJsonResponse(response: Response): boolean {
	const contentType = getHeader(response, 'content-type');
	return contentType?.toLowerCase().includes('application/json') ?? false;
}

async function parseSuccessPayload<T>(response: Response): Promise<T> {
	if (response.status === 204 || getHeader(response, 'content-length') === '0') {
		return undefined as T;
	}

	if (!isJsonResponse(response)) {
		let bodyPrefix = '';
		try {
			bodyPrefix = (await response.text()).trim().slice(0, 64);
		} catch {
			bodyPrefix = '';
		}

		const contentType = getHeader(response, 'content-type') ?? 'unknown content-type';
		const suffix = bodyPrefix ? ` (response starts with "${bodyPrefix}")` : '';
		throw new BFFError(
			`Expected JSON response but received ${contentType}${suffix}`,
			response.status,
			'INVALID_RESPONSE'
		);
	}

	try {
		return (await response.json()) as T;
	} catch {
		throw new BFFError('Failed to parse JSON response', response.status, 'INVALID_RESPONSE');
	}
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
			if (!isJsonResponse(response)) {
				throw new Error('non-json response');
			}
			payload = (await response.json()) as { message?: string; code?: string };
		} catch {
			payload = undefined;
		}

		const message = payload?.message ?? `Request failed with status ${response.status}`;
		const code = payload?.code ?? 'UNKNOWN_ERROR';
		throw new BFFError(message, response.status, code);
	}

	return parseSuccessPayload<T>(response);
}
