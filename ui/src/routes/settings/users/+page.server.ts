import { redirect } from '@sveltejs/kit';
import type { RequestEvent } from '@sveltejs/kit';

/**
 * Server-side guard: only admins may access the user management page.
 * The token stores a role claim that we can read from the session cookie
 * without making an extra network call.
 */
export async function load({ cookies }: RequestEvent): Promise<void> {
	const token = cookies.get('chv_session');
	if (!token) {
		throw redirect(302, '/login');
	}

	// Decode the JWT payload (no signature verification here — the BFF owns
	// that responsibility; we just need the role claim for the redirect guard).
	try {
		const parts = token.split('.');
		if (parts.length !== 3) throw new Error('malformed token');
		const payload = JSON.parse(atob(parts[1])) as Record<string, unknown>;
		const role = typeof payload.role === 'string' ? payload.role : '';
		if (role !== 'admin') {
			throw redirect(302, '/settings');
		}
	} catch (err) {
		// If it's a redirect, re-throw it; otherwise bounce to login
		if (err instanceof Response || (err as { status?: number }).status === 302) {
			throw err;
		}
		throw redirect(302, '/login');
	}
}
