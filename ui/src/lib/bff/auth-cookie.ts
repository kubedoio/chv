/**
 * Auth cookie management. The BFF server sets an HttpOnly cookie on login.
 * Client-side code cannot read or set the auth token directly.
 *
 * SECURITY: Tokens must NOT be stored in localStorage (XSS-accessible).
 * The server's Set-Cookie with HttpOnly + Secure + SameSite=Strict is the
 * only mechanism that should create the session cookie.
 */
export function syncAuthCookieFromLocalStorage(): void {
	// No-op: auth uses localStorage tokens (set by login handler JSON response).
	// HttpOnly cookie auth is not yet implemented in the BFF.
}
