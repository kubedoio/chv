/**
 * Syncs the localStorage auth token to a session cookie so SvelteKit server
 * loads can read it. This is a bridge during the auth migration.
 */
const COOKIE_NAME = 'chv_session';
const COOKIE_MAX_AGE = 60 * 60 * 24 * 7; // 7 days

export function syncAuthCookieFromLocalStorage(): void {
	if (typeof document === 'undefined') return;
	try {
		const token = localStorage.getItem('chv-api-token');
		const secureFlag = window.location.protocol === 'https:' ? '; Secure' : '';
		if (token) {
			document.cookie = `${COOKIE_NAME}=${encodeURIComponent(token)}; path=/; max-age=${COOKIE_MAX_AGE}; SameSite=Lax; HttpOnly${secureFlag}`;
		} else {
			// Clear cookie if token removed
			document.cookie = `${COOKIE_NAME}=; path=/; max-age=0; SameSite=Lax; HttpOnly${secureFlag}`;
		}
	} catch {
		// ignore
	}
}

export function clearAuthCookie(): void {
	if (typeof document === 'undefined') return;
	document.cookie = `${COOKIE_NAME}=; path=/; max-age=0; SameSite=Lax`;
}
