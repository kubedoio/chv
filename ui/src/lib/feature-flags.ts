/**
 * Feature flag helpers.
 *
 * Flags are read from Vite's `import.meta.env`. Anything Claude / users / the
 * browser must read at runtime MUST be prefixed `PUBLIC_` — that is the
 * SvelteKit convention and the only way Vite exposes it to client code.
 *
 * All flags here default to OFF. A flag is enabled only when its env var is
 * the literal string `'1'`. This is intentionally strict: `'true'`,
 * `'on'`, `1` (number), and `true` (boolean) all evaluate to false. Keep the
 * matrix simple — one switch, one accepted truthy value.
 */

interface PublicEnv {
	readonly PUBLIC_ARCHITECTURE_DESIGNER_CANVAS?: string;
}

function readPublicEnv(): PublicEnv {
	// `import.meta.env` is statically rewritten by Vite at build time. Under
	// vitest (jsdom + vite) the same shape is available. We narrow to the
	// fields we actually consume so accidental flag typos surface as
	// compile-time misses rather than silent reads.
	return import.meta.env as unknown as PublicEnv;
}

/**
 * Returns true when the architecture designer Svelte Flow canvas should
 * mount. Default OFF — production builds without `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1`
 * keep showing the Phase-1 placeholder.
 */
export function architectureDesignerCanvasEnabled(): boolean {
	return readPublicEnv().PUBLIC_ARCHITECTURE_DESIGNER_CANVAS === '1';
}
