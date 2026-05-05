/**
 * Auth cookie management. The BFF server sets an HttpOnly cookie on login.
 * Client-side code cannot read or set the auth token directly — it only
 * clears the cookie on logout by setting max-age=0.
 *
 * SECURITY: Tokens must NOT be stored in localStorage (XSS-accessible).
 * The server's Set-Cookie with HttpOnly + Secure + SameSite=Strict is the
 * only mechanism that should create the session cookie.
 */
const COOKIE_NAME = 'chv_session';

export function syncAuthCookieFromLocalStorage(): void {
	// No-op: auth uses localStorage tokens (set by login handler JSON response).
	// HttpOnly cookie auth is not yet implemented in the BFF.
}

export function clearAuthCookie(): void {
	if (typeof document === 'undefined') return;
	const secureFlag = window.location.protocol === 'https:' ? '; Secure' : '';
	document.cookie = `${COOKIE_NAME}=; path=/; max-age=0; SameSite=Strict${secureFlag}`;
}
